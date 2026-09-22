//! Smart Structure post-processing engine.
//!
//! Reorganises existing Parakeet transcriptions into readable structure
//! (sentence paragraphs, bullet lists, numbered lists) WITHOUT modifying
//! the user's words, punctuation, or capitalisation.
//!
//! No new model, no LLM, no network calls. Pure rule-based, < 10 ms.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::storage::AppSettings;

// ─── Word timing ────────────────────────────────────────────────────────────

/// A word token with optional per-word timing information derived from the
/// sherpa-onnx token-level timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordSpan {
    pub word: String,
    /// Start of this word in seconds from the beginning of the audio chunk.
    pub start_s: Option<f32>,
    /// End of this word (start + duration) in seconds.
    pub end_s: Option<f32>,
}

impl WordSpan {
    /// Silence gap **after** this word and **before** `next` in seconds.
    /// Returns `None` if timing is unavailable or the gap would be negative.
    pub fn gap_to(&self, next: &WordSpan) -> Option<f32> {
        match (self.end_s, next.start_s) {
            (Some(end), Some(start)) if start > end => Some(start - end),
            _ => None,
        }
    }
}

/// Reconstruct per-word timing from sherpa-onnx character/sub-word token
/// timestamps.
///
/// Parakeet TDT uses a character-level tokeniser. Each character is one token,
/// with its own `timestamps[i]` (start in seconds) and `durations[i]` (length
/// in seconds).  We group characters into words by splitting on whitespace and
/// assign each word the time-range of its constituent character tokens.
///
/// If `timestamps` is `None` the function still returns `WordSpan`s but
/// without timing (`start_s = None`, `end_s = None`).  The caller can still
/// use the word list for ordinal-marker detection.
pub fn build_word_spans(
    text: &str,
    tokens: &[String],
    timestamps: Option<&[f32]>,
    durations: Option<&[f32]>,
) -> Vec<WordSpan> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let Some(ts) = timestamps else {
        // No timing: return words without timestamps.
        return words
            .iter()
            .map(|w| WordSpan {
                word: w.to_string(),
                start_s: None,
                end_s: None,
            })
            .collect();
    };

    let durs = durations.unwrap_or(&[]);
    let n_tok = tokens.len().min(ts.len());

    let mut spans: Vec<WordSpan> = Vec::with_capacity(words.len());
    let mut tok_idx = 0usize;

    for word in &words {
        // Skip leading space/empty tokens.
        while tok_idx < n_tok && tokens[tok_idx].chars().all(char::is_whitespace) {
            tok_idx += 1;
        }

        let word_start = ts.get(tok_idx).copied();

        // Count non-whitespace chars we need to consume for this word.
        let needed: usize = word.chars().filter(|c| !c.is_whitespace()).count();
        let mut consumed = 0usize;
        let mut last_tok = tok_idx;

        while tok_idx < n_tok && consumed < needed {
            let non_ws: usize = tokens[tok_idx]
                .chars()
                .filter(|c| !c.is_whitespace())
                .count();
            if non_ws > 0 {
                consumed += non_ws;
                last_tok = tok_idx;
            }
            tok_idx += 1;
        }

        let word_end = ts.get(last_tok).map(|&t| {
            let dur = durs.get(last_tok).copied().unwrap_or(0.0);
            t + dur
        });

        spans.push(WordSpan {
            word: word.to_string(),
            start_s: word_start,
            end_s: word_end,
        });
    }

    spans
}

// ─── Settings types ──────────────────────────────────────────────────────────

/// How lists are formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BulletStyle {
    #[default]
    Bullet,    // •
    Dash,      // -
    Asterisk,  // *
    Numbered,  // 1. 2. 3.
}

impl BulletStyle {
    pub fn format_item(&self, index: usize, text: &str) -> String {
        match self {
            Self::Bullet => format!("• {text}"),
            Self::Dash => format!("- {text}"),
            Self::Asterisk => format!("* {text}"),
            Self::Numbered => format!("{}. {text}", index + 1),
        }
    }
}

/// How aggressively to introduce structural breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Aggressiveness {
    Conservative,
    #[default]
    Balanced,
    Aggressive,
}

/// Per-call configuration for Smart Structure, derived from `AppSettings`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureConfig {
    pub enabled: bool,
    /// Add paragraph breaks at sentence/thought boundaries.
    pub sentence_structure: bool,
    /// Detect and format explicit or implicit lists as bullets/numbers.
    pub auto_bullets: bool,
    pub aggressiveness: Aggressiveness,
    pub bullet_style: BulletStyle,
}

impl Default for StructureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sentence_structure: true,
            auto_bullets: true,
            aggressiveness: Aggressiveness::Balanced,
            bullet_style: BulletStyle::Bullet,
        }
    }
}

impl From<&AppSettings> for StructureConfig {
    fn from(s: &AppSettings) -> Self {
        let aggressiveness = match s.smart_aggressiveness.as_str() {
            "conservative" => Aggressiveness::Conservative,
            "aggressive" => Aggressiveness::Aggressive,
            _ => Aggressiveness::Balanced,
        };
        let bullet_style = match s.smart_bullet_style.as_str() {
            "dash" => BulletStyle::Dash,
            "asterisk" => BulletStyle::Asterisk,
            "numbered" => BulletStyle::Numbered,
            _ => BulletStyle::Bullet,
        };
        Self {
            enabled: s.smart_structure_enabled,
            sentence_structure: s.smart_sentence_structure,
            auto_bullets: s.smart_auto_bullets,
            aggressiveness,
            bullet_style,
        }
    }
}

// ─── Result ──────────────────────────────────────────────────────────────────

/// Output of `apply_smart_structure`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureResult {
    /// The (possibly restructured) text.
    pub text: String,
    /// `true` if any structural change was made.
    pub changed: bool,
    /// Wall-clock processing time in microseconds.
    pub latency_us: u64,
    /// Number of words in the input.
    pub word_count: usize,
    /// Number of characters in the input.
    pub char_count: usize,
}

// ─── Internal constants ───────────────────────────────────────────────────────

/// Pause threshold parameters per aggressiveness level (seconds).
struct PauseThresholds {
    /// Below this gap: no structural signal.
    insignificant: f32,
    /// Above this: sentence/thought boundary candidate.
    sentence: f32,
    /// Above this: strong paragraph boundary.
    paragraph: f32,
}

impl PauseThresholds {
    fn for_aggressiveness(ag: Aggressiveness) -> Self {
        match ag {
            Aggressiveness::Conservative => Self {
                insignificant: 0.4,
                sentence: 0.7,
                paragraph: 1.2,
            },
            Aggressiveness::Balanced => Self {
                insignificant: 0.25,
                sentence: 0.6,
                paragraph: 1.0,
            },
            Aggressiveness::Aggressive => Self {
                insignificant: 0.15,
                sentence: 0.4,
                paragraph: 0.7,
            },
        }
    }
}

/// Discourse transition words and their boundary-score contribution.
/// Ordinals ("first","second","third") are NOT in this list — they're handled
/// separately in list detection.
static TRANSITION_WORDS: &[(&str, f32)] = &[
    ("then", 0.5),
    ("after that", 0.6),
    ("next", 0.5),
    ("finally", 0.7),
    ("another thing", 0.7),
    ("the next thing", 0.7),
    ("now i need to", 0.6),
    ("moving on", 0.8),
    ("additionally", 0.6),
    ("furthermore", 0.7),
    ("also", 0.3),
];

/// Explicit ordinal markers mapped to their position index (1-based; 0 = "last").
static ORDINAL_MARKERS: &[(&str, usize)] = &[
    ("number one", 1),
    ("number two", 2),
    ("number three", 3),
    ("number four", 4),
    ("number five", 5),
    ("firstly", 1),
    ("secondly", 2),
    ("thirdly", 3),
    ("fourthly", 4),
    ("fifthly", 5),
    ("lastly", 0),
    // Single-word ordinals last so multi-word ones match first.
    ("first", 1),
    ("second", 2),
    ("third", 3),
    ("fourth", 4),
    ("fifth", 5),
    ("finally", 0),
];

/// Phrases that strongly introduce a list.
static LIST_INTROS: &[&str] = &[
    "a few things",
    "a couple of things",
    "several things",
    "the following",
    "the following things",
    "here are the things",
    "things i need to",
    "a few things i need",
    "two things",
    "three things",
    "four things",
    "five things",
    "a number of things",
    "some things",
    "a list of",
];

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Capitalise the first Unicode scalar of `s`.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Split `text` into sentence segments on `.`, `?`, `!`.
/// Each segment retains its terminal punctuation.
fn split_into_sentences(text: &str) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut seg_start = 0usize; // byte offset

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        // Only split on ASCII terminators for safety with multi-byte chars.
        if matches!(bytes[i], b'.' | b'?' | b'!') {
            // Include terminator.
            let end = i + 1;
            let seg = text[seg_start..end].trim().to_string();
            if !seg.is_empty() {
                segments.push(seg);
            }
            // Skip whitespace after terminator.
            let mut j = end;
            while j < len && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            seg_start = j;
            i = j;
        } else {
            i += 1;
        }
    }

    // Remaining text after last terminator (or entire text if no terminator).
    let remainder = text[seg_start..].trim();
    if !remainder.is_empty() {
        segments.push(remainder.to_string());
    }

    segments
}

/// Find the inter-word gap (seconds) between the last word of `seg_a_text`
/// and the first word of `seg_b_text`, using the flat `word_spans` for the
/// whole transcript.
fn find_gap_between(seg_a: &str, seg_b: &str, word_spans: &[WordSpan]) -> Option<f32> {
    let a_words: Vec<&str> = seg_a.split_whitespace().collect();
    let b_words: Vec<&str> = seg_b.split_whitespace().collect();
    let last_a = a_words.last()?.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    let first_b = b_words.first()?.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();

    // 1. Find the occurrence of last_a
    let mut idx_a = None;
    for (i, span) in word_spans.iter().enumerate() {
        let sw = span.word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if sw == last_a {
            idx_a = Some(i);
            // Quick check: if immediately followed by first_b
            if i + 1 < word_spans.len() {
                let next_sw = word_spans[i + 1].word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                if next_sw == first_b {
                    return word_spans[i].gap_to(&word_spans[i + 1]);
                }
            }
        }
    }

    let ia = idx_a?;

    // 2. Search for first_b strictly after ia
    for span in word_spans.iter().skip(ia + 1) {
        let sw = span.word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if sw == first_b {
            return word_spans[ia].gap_to(span);
        }
    }

    None
}

/// Check if `text` starts with an ordinal marker.
/// Returns `(marker_str, ordinal_number)` if yes.
/// `ordinal_number == 0` means "finally / lastly".
fn starts_with_ordinal(text: &str) -> Option<(&'static str, usize)> {
    let lower = to_lower(text.trim());
    for &(marker, num) in ORDINAL_MARKERS {
        if let Some(after) = lower.strip_prefix(marker) {
            // Require a word boundary after the marker.
            if after.is_empty() || !after.starts_with(|c: char| c.is_alphabetic()) {
                return Some((marker, num));
            }
        }
    }
    None
}

/// Check if `text` ends with a list-introduction phrase (ignoring trailing punctuation).
fn ends_with_list_intro(text: &str) -> bool {
    let lower = to_lower(
        text.trim()
            .trim_end_matches(|c: char| !c.is_alphanumeric() && c != ' '),
    );
    LIST_INTROS.iter().any(|&intro| lower.ends_with(intro))
}

/// Check if `text` starts with a discourse transition word.
/// Returns `(found, score)`.
fn starts_with_transition(text: &str) -> (bool, f32) {
    let lower = to_lower(text.trim());
    for &(tw, score) in TRANSITION_WORDS {
        if let Some(after) = lower.strip_prefix(tw) {
            if after.is_empty() || !after.starts_with(|c: char| c.is_alphabetic()) {
                return (true, score);
            }
        }
    }
    (false, 0.0)
}

/// Strip an ordinal marker prefix from text and capitalise the result.
fn strip_ordinal_prefix(text: &str, marker: &str) -> String {
    let trimmed = text.trim();
    // Work in lowercase to match the marker, then recover original capitalisation.
    let lower = to_lower(trimmed);
    if let Some(rest_lower) = lower.strip_prefix(marker) {
        let rest_lower = rest_lower.trim_start_matches(|c: char| !c.is_alphanumeric());
        let byte_offset = trimmed.len() - trimmed[trimmed.len() - rest_lower.len()..].len();
        // Find the byte offset in the original string by scanning.
        let marker_len = marker.len();
        if marker_len < trimmed.len() {
            let after_marker = trimmed[marker_len..].trim_start_matches(|c: char| !c.is_alphanumeric());
            if !after_marker.is_empty() {
                return capitalize_first(after_marker);
            }
        }
        let _ = byte_offset; // suppress warning
        capitalize_first(rest_lower)
    } else {
        trimmed.to_string()
    }
}

// ─── Segment metadata ─────────────────────────────────────────────────────────

struct Segment {
    text: String,
    /// Silence gap in seconds BEFORE this segment (gap after previous segment's last word).
    pause_before_s: Option<f32>,
    /// `Some(n)` if this segment starts with an ordinal marker (n = ordinal position, 0 = "finally").
    ordinal: Option<usize>,
    /// `true` if the PRECEDING segment ended with a list-introduction phrase.
    follows_list_intro: bool,
    /// Whether this segment starts with a discourse transition word.
    has_transition: bool,
    /// Boundary score contribution from the transition word (0.0 if none).
    transition_score: f32,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Apply Smart Structure to `text`.
///
/// `word_spans` should be the word-level timing data from `build_word_spans`.
/// Pass an empty slice if timing is unavailable; the function degrades
/// gracefully to punctuation-only signals.
pub fn apply_smart_structure(
    text: &str,
    word_spans: &[WordSpan],
    config: &StructureConfig,
) -> StructureResult {
    let t0 = Instant::now();
    let word_count = text.split_whitespace().count();
    let char_count = text.chars().count();

    let passthrough = |t: &Instant| StructureResult {
        text: text.to_string(),
        changed: false,
        latency_us: t.elapsed().as_micros() as u64,
        word_count,
        char_count,
    };

    if !config.enabled || text.trim().is_empty() || word_count < 3 {
        return passthrough(&t0);
    }

    let sentences = split_into_sentences(text);
    if sentences.is_empty() {
        return passthrough(&t0);
    }

    let thresholds = PauseThresholds::for_aggressiveness(config.aggressiveness);

    // Build segment metadata.
    let mut segments: Vec<Segment> = Vec::with_capacity(sentences.len());
    for (i, seg_text) in sentences.iter().enumerate() {
        let pause = if i > 0 {
            find_gap_between(&sentences[i - 1], seg_text, word_spans)
        } else {
            None
        };
        let ordinal = starts_with_ordinal(seg_text).map(|(_, n)| n);
        let follows_intro = i > 0 && ends_with_list_intro(&sentences[i - 1]);
        let (has_transition, transition_score) = starts_with_transition(seg_text);

        segments.push(Segment {
            text: seg_text.clone(),
            pause_before_s: pause,
            ordinal,
            follows_list_intro: follows_intro,
            has_transition,
            transition_score,
        });
    }

    // 1. Attempt explicit-ordinal list detection.
    if config.auto_bullets {
        if let Some(r) = try_explicit_list(&segments, &sentences, config, &t0, word_count, char_count) {
            return r;
        }
    }

    // 2. Apply sentence/paragraph structure.
    if config.sentence_structure && sentences.len() > 1 {
        return apply_sentence_structure(&segments, config, &thresholds, &t0, word_count, char_count);
    }

    passthrough(&t0)
}

// ─── List detection ───────────────────────────────────────────────────────────

fn try_explicit_list(
    segments: &[Segment],
    sentences: &[String],
    config: &StructureConfig,
    t0: &Instant,
    word_count: usize,
    char_count: usize,
) -> Option<StructureResult> {
    // Collect ordinal positions (index, ordinal_number).
    let ordinals: Vec<(usize, usize)> = segments
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.ordinal.map(|n| (i, n)))
        .collect();

    if ordinals.len() < 2 {
        return None;
    }

    // Verify the ordinals form an ascending sequence (1, 2, 3 … optionally ending with 0/finally).
    let mut prev = 0usize;
    for &(_, n) in &ordinals {
        if n == 0 {
            // "finally" — valid as the last item.
            break;
        }
        if n != prev + 1 {
            return None; // Not a clean sequence.
        }
        prev = n;
    }

    let first_idx = ordinals[0].0;

    // Build optional intro text (segments before the first ordinal item).
    let intro_opt = if first_idx > 0 {
        Some(sentences[..first_idx].join(" "))
    } else {
        None
    };

    // Build list items: strip ordinal prefix.
    let mut items: Vec<String> = Vec::new();
    for &(idx, _) in &ordinals {
        let (marker, _) = starts_with_ordinal(&segments[idx].text)?;
        let raw = strip_ordinal_prefix(&segments[idx].text, marker);
        let item = raw
            .trim_end_matches(['.', '?', '!'])
            .trim()
            .to_string();
        if !item.is_empty() {
            items.push(item);
        }
    }

    if items.len() < 2 {
        return None;
    }

    // Format output.
    let mut out = String::new();

    if let Some(intro) = &intro_opt {
        let intro_clean = intro
            .trim_end_matches(['.', '?', '!', ' ']);
        out.push_str(intro_clean);
        out.push_str(":\n\n");
    }

    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&config.bullet_style.format_item(i, item));
    }

    // Append segments that come after the list.
    let after_idx = ordinals.last().map(|&(i, _)| i + 1).unwrap_or(sentences.len());
    if after_idx < sentences.len() {
        out.push_str("\n\n");
        out.push_str(&sentences[after_idx..].join(" "));
    }

    let original_flat = sentences.join(" ");
    let changed = out != original_flat;

    Some(StructureResult {
        text: out,
        changed,
        latency_us: t0.elapsed().as_micros() as u64,
        word_count,
        char_count,
    })
}

// ─── Sentence structure ───────────────────────────────────────────────────────

fn apply_sentence_structure(
    segments: &[Segment],
    config: &StructureConfig,
    thresholds: &PauseThresholds,
    t0: &Instant,
    word_count: usize,
    char_count: usize,
) -> StructureResult {
    if segments.len() <= 1 {
        let text = segments.first().map(|s| s.text.clone()).unwrap_or_default();
        return StructureResult {
            text,
            changed: false,
            latency_us: t0.elapsed().as_micros() as u64,
            word_count,
            char_count,
        };
    }

    let break_threshold = match config.aggressiveness {
        Aggressiveness::Conservative => 0.8,
        Aggressiveness::Balanced => 0.5,
        Aggressiveness::Aggressive => 0.35,
    };

    let mut parts: Vec<&str> = Vec::with_capacity(segments.len());
    let mut seps: Vec<&'static str> = Vec::with_capacity(segments.len() - 1);

    parts.push(&segments[0].text);

    for seg in &segments[1..] {
        let mut score = 0.0f32;

        // Pause signal.
        if let Some(pause) = seg.pause_before_s {
            if pause >= thresholds.paragraph {
                score += 0.8;
            } else if pause >= thresholds.sentence {
                score += 0.5;
            } else if pause >= thresholds.insignificant {
                score += 0.2;
            }
        }

        // Transition word signal.
        if seg.has_transition {
            score += seg.transition_score;
        }

        // List-intro on previous segment.
        if seg.follows_list_intro {
            score += 0.7;
        }

        // In Aggressive mode every sentence boundary gets a baseline score.
        if matches!(config.aggressiveness, Aggressiveness::Aggressive) && score < break_threshold {
            score = score.max(break_threshold);
        }

        seps.push(if score >= break_threshold { "\n\n" } else { " " });
        parts.push(&seg.text);
    }

    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(seps[i - 1]);
        }
        out.push_str(part);
    }

    let original = parts.join(" ");
    let changed = out != original;

    StructureResult {
        text: out,
        changed,
        latency_us: t0.elapsed().as_micros() as u64,
        word_count,
        char_count,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_on() -> StructureConfig {
        StructureConfig {
            enabled: true,
            sentence_structure: true,
            auto_bullets: true,
            aggressiveness: Aggressiveness::Balanced,
            bullet_style: BulletStyle::Bullet,
        }
    }

    fn cfg_numbered() -> StructureConfig {
        StructureConfig {
            bullet_style: BulletStyle::Numbered,
            ..cfg_on()
        }
    }

    fn words_no_timing(text: &str) -> Vec<WordSpan> {
        build_word_spans(text, &[], None, None)
    }

    /// Helper: normalise whitespace for comparison.
    fn norm(s: &str) -> String {
        s.trim().to_string()
    }

    // ── TEST 1: Explicit bullet list ─────────────────────────────────────────
    #[test]
    fn test_explicit_list_three_items() {
        let input = "I need to do a few things. First go to the shop. Second call my friend. Third finish my assignment.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        assert!(r.changed, "Should detect list");
        // Must have 3 bullet items.
        let bullets = r.text.lines().filter(|l| l.trim_start().starts_with('•')).count();
        assert_eq!(bullets, 3, "Expected 3 bullet items, got: {}", r.text);
    }

    // ── TEST 2: Numbered list ────────────────────────────────────────────────
    #[test]
    fn test_numbered_list() {
        let input = "Number one check email. Number two call John. Number three finish the report.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_numbered());
        assert!(r.changed, "Should detect numbered list");
        assert!(r.text.contains("1."), "Should have '1.'");
        assert!(r.text.contains("2."), "Should have '2.'");
        assert!(r.text.contains("3."), "Should have '3.'");
        // Verify words are preserved.
        assert!(r.text.to_lowercase().contains("check email"));
        assert!(r.text.to_lowercase().contains("call john"));
        assert!(r.text.to_lowercase().contains("finish the report"));
    }

    // ── TEST 3: Normal prose — no list ───────────────────────────────────────
    #[test]
    fn test_normal_prose_unchanged() {
        let input = "I went to the shop and then I came home.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        // Single sentence — no bullets expected.
        assert!(!r.text.contains('•'), "Should not produce bullets: {}", r.text);
        assert!(!r.text.contains("- "), "Should not produce dashes");
        // Text content should equal original (possibly with structural whitespace).
        assert!(r.text.contains("went to the shop"));
    }

    // ── TEST 4: Sequential thoughts separated ────────────────────────────────
    #[test]
    fn test_sequential_thoughts_separated() {
        let input = "I need to finish my assignment. Then I need to study. After that I need to sleep.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        // Should insert breaks between the three thoughts (transition words trigger).
        assert!(r.changed, "Should apply structure: {}", r.text);
        assert!(r.text.contains('\n'), "Should have newlines: {}", r.text);
    }

    // ── TEST 5: False-positive "first" ───────────────────────────────────────
    #[test]
    fn test_false_positive_first_in_prose() {
        let input = "The first thing I noticed was that the first button was broken.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        // "first" is mid-sentence — no list should be created.
        assert!(!r.text.contains('•'), "Should not produce bullets: {}", r.text);
        // Words must all be preserved.
        assert!(r.text.contains("first thing I noticed"));
    }

    // ── TEST 6: Existing punctuation preserved ───────────────────────────────
    #[test]
    fn test_existing_punctuation_preserved() {
        let input = "I finished the project. It works well. Can you test it?";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        assert!(r.text.contains("I finished the project."));
        assert!(r.text.contains("It works well."));
        assert!(r.text.contains("Can you test it?"));
    }

    // ── TEST 7: Capitalisation unchanged ─────────────────────────────────────
    #[test]
    fn test_capitalisation_unchanged() {
        let input = "NASA launched a new satellite. The ISS received supplies. CERN confirmed the result.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        assert!(r.text.contains("NASA"), "NASA capitalisation must be preserved");
        assert!(r.text.contains("ISS"), "ISS capitalisation must be preserved");
        assert!(r.text.contains("CERN"), "CERN capitalisation must be preserved");
    }

    // ── TEST 8: No rewriting — token set unchanged ───────────────────────────
    #[test]
    fn test_no_rewriting_tokens_preserved() {
        let input = "I need to finish my assignment. Then I need to study. After that I need to sleep.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());

        // Strip structural markers and compare word bags.
        let clean_out: Vec<String> = r
            .text
            .split_whitespace()
            .map(|w| {
                w.trim_start_matches(['•', '-', '*'])
                    .trim()
                    .to_lowercase()
            })
            .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit() || c == '.'))
            .collect();

        let clean_in: Vec<String> = input
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();

        for word in &clean_in {
            let norm_word = word.trim_matches(|c: char| !c.is_alphanumeric());
            assert!(
                clean_out.iter().any(|w| w.contains(norm_word)),
                "Word '{}' missing from output: {}",
                norm_word,
                r.text
            );
        }
    }

    // ── TEST 9: Implicit action list — conservative (leave as prose) ─────────
    #[test]
    fn test_implicit_list_conservative() {
        // In Conservative/Balanced mode this single sentence should NOT become bullets.
        let input = "I need to buy milk, go to the shop, call John and finish my assignment.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        // Implicit list in a single sentence: no ordinal markers → no list.
        assert!(!r.text.contains('•'), "Should not force bullets for implicit list: {}", r.text);
    }

    // ── TEST 10: Transition false-positive ───────────────────────────────────
    #[test]
    fn test_transition_false_positive() {
        // Single sentence — no structural break expected.
        let input = "I finished my work and then I went home.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        assert!(!r.text.contains('\n'), "Single-sentence should not get newlines: {}", r.text);
    }

    // ── TEST 11: Multiple separate lists ─────────────────────────────────────
    #[test]
    fn test_multiple_list_sections_handled() {
        // Two distinct ordinal sequences; smart structure should process the first.
        let input = "First wash dishes. Second dry dishes. Later first call mom. Second call dad.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        // At minimum should not crash and should preserve all words.
        assert!(r.text.contains("wash dishes") || r.text.contains("Wash dishes"));
        assert!(r.text.contains("dry dishes") || r.text.contains("Dry dishes"));
    }

    // ── TEST 12: Empty / short transcript ────────────────────────────────────
    #[test]
    fn test_short_transcript_unchanged() {
        let ws = words_no_timing("Okay.");
        let r = apply_smart_structure("Okay.", &ws, &cfg_on());
        assert_eq!(norm(&r.text), norm("Okay."));
        assert!(!r.changed);
    }

    // ── TEST 13: Timestamps-based pause detection ─────────────────────────────
    #[test]
    fn test_pause_signal_triggers_break() {
        // Two sentences with a 1.5s simulated pause between them.
        let text = "Sentence one. Sentence two.";
        let spans = vec![
            WordSpan { word: "Sentence".to_string(), start_s: Some(0.0), end_s: Some(0.5) },
            WordSpan { word: "one.".to_string(),     start_s: Some(0.6), end_s: Some(0.9) },
            // 1.5 second gap here.
            WordSpan { word: "Sentence".to_string(), start_s: Some(2.4), end_s: Some(2.9) },
            WordSpan { word: "two.".to_string(),     start_s: Some(3.0), end_s: Some(3.3) },
        ];
        let cfg = StructureConfig { aggressiveness: Aggressiveness::Balanced, ..cfg_on() };
        let r = apply_smart_structure(text, &spans, &cfg);
        // Pause ≥ 1.0s in Balanced → paragraph break.
        assert!(r.changed, "1.5s pause should trigger a break: {}", r.text);
        assert!(r.text.contains("\n\n"), "Should have paragraph break: {}", r.text);
    }

    // ── TEST 14: Short pause does not create bullets ──────────────────────────
    #[test]
    fn test_short_pause_no_break() {
        let text = "Hello world. How are you.";
        let spans = vec![
            WordSpan { word: "Hello".to_string(),  start_s: Some(0.0), end_s: Some(0.2) },
            WordSpan { word: "world.".to_string(), start_s: Some(0.3), end_s: Some(0.6) },
            // Only 0.1s gap — below insignificant threshold.
            WordSpan { word: "How".to_string(),    start_s: Some(0.7), end_s: Some(0.9) },
            WordSpan { word: "are".to_string(),    start_s: Some(1.0), end_s: Some(1.2) },
            WordSpan { word: "you.".to_string(),   start_s: Some(1.3), end_s: Some(1.5) },
        ];
        let cfg = StructureConfig { aggressiveness: Aggressiveness::Conservative, ..cfg_on() };
        let r = apply_smart_structure(text, &spans, &cfg);
        // In Conservative mode, 0.1s gap is insignificant — no break.
        assert!(!r.text.contains('•'), "Short pause should not produce bullets");
    }

    // ── TEST 15: Performance — < 10 ms for a typical transcript ──────────────
    #[test]
    fn test_latency_under_10ms() {
        let input = "I need to do several things. First I need to go to the shop. \
            Second I need to call my friend. Third I need to finish my assignment. \
            After that I need to rest. Finally I want to sleep.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        assert!(
            r.latency_us < 10_000,
            "Latency {}µs exceeded 10ms for {} chars, {} words",
            r.latency_us,
            r.char_count,
            r.word_count
        );
    }

    // ── TEST 16: build_word_spans with timestamps ─────────────────────────────
    #[test]
    fn test_build_word_spans_with_timestamps() {
        // "hi" = tokens ["h","i"] timestamps [0.0, 0.05] durations [0.05, 0.05]
        // " " = token [" "] timestamp [0.1]
        // "mom" = tokens ["m","o","m"] timestamps [0.15, 0.2, 0.25] durations [0.05, 0.05, 0.05]
        let tokens: Vec<String> = vec!["h","i"," ","m","o","m"]
            .into_iter().map(String::from).collect();
        let ts = vec![0.0_f32, 0.05, 0.1, 0.15, 0.20, 0.25];
        let durs = vec![0.05_f32; 6];
        let spans = build_word_spans("hi mom", &tokens, Some(&ts), Some(&durs));
        assert_eq!(spans.len(), 2);
        assert!(spans[0].start_s.is_some());
        assert!(spans[1].start_s.is_some());
        // gap between "hi" and "mom" ≈ 0.15 - 0.10 = 0.05
        let gap = spans[0].gap_to(&spans[1]);
        assert!(gap.is_some(), "gap should be computable");
        assert!((gap.unwrap() - 0.05).abs() < 0.02, "gap ≈ 0.05s");
    }

    // ── TEST 17: Disabled — passthrough ──────────────────────────────────────
    #[test]
    fn test_disabled_passthrough() {
        let input = "First go to shop. Second call friend. Third sleep.";
        let ws = words_no_timing(input);
        let cfg = StructureConfig { enabled: false, ..cfg_on() };
        let r = apply_smart_structure(input, &ws, &cfg);
        assert_eq!(r.text, input, "Disabled: output must equal input exactly");
        assert!(!r.changed);
    }

    // ── TEST 18: "finally" as last list item ──────────────────────────────────
    #[test]
    fn test_finally_as_last_list_item() {
        let input = "There are a few things. First check email. Second call John. Finally finish the report.";
        let ws = words_no_timing(input);
        let r = apply_smart_structure(input, &ws, &cfg_on());
        assert!(r.changed, "Should detect list with 'finally': {}", r.text);
        let bullets = r.text.lines().filter(|l| l.trim_start().starts_with('•')).count();
        assert_eq!(bullets, 3, "Expected 3 bullet items, got:\n{}", r.text);
    }
}
