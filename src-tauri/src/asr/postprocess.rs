use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::storage::{apply_dictionary, AppSettings, DictionaryRecord};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PostProcessConfig {
    pub trim_whitespace: bool,
    pub preserve_punctuation: bool,
    pub remove_fillers: bool,
    pub apply_spoken_commands: bool,
    pub apply_dictionary: bool,
    pub add_trailing_space: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            trim_whitespace: true,
            preserve_punctuation: true,
            remove_fillers: true,
            apply_spoken_commands: true,
            apply_dictionary: true,
            add_trailing_space: false,
        }
    }
}

impl From<&AppSettings> for PostProcessConfig {
    fn from(s: &AppSettings) -> Self {
        Self {
            trim_whitespace: s.trim_whitespace,
            preserve_punctuation: s.preserve_punctuation,
            remove_fillers: s.remove_fillers,
            apply_spoken_commands: s.apply_spoken_commands,
            apply_dictionary: s.apply_dictionary,
            add_trailing_space: s.add_trailing_space,
        }
    }
}

static FILLER_COMMA_SURROUND_RE: OnceLock<Regex> = OnceLock::new();
static FILLER_TRAILING_RE: OnceLock<Regex> = OnceLock::new();
static FILLER_COMMA_PREFIX_RE: OnceLock<Regex> = OnceLock::new();
static FILLER_START_RE: OnceLock<Regex> = OnceLock::new();
static FILLER_WORD_RE: OnceLock<Regex> = OnceLock::new();
static MULTI_SPACE_RE: OnceLock<Regex> = OnceLock::new();
static SPACE_BEFORE_PUNCT_RE: OnceLock<Regex> = OnceLock::new();
static SPOKEN_NEWLINE_RE: OnceLock<Regex> = OnceLock::new();
static SPOKEN_PUNCT_RE: OnceLock<Regex> = OnceLock::new();
static PUNCT_SPACE_RE: OnceLock<Regex> = OnceLock::new();
static DANGLING_COMMA_RE: OnceLock<Regex> = OnceLock::new();
static MULTI_COMMA_RE: OnceLock<Regex> = OnceLock::new();
static TERMINATOR_SPACE_RE: OnceLock<Regex> = OnceLock::new();

fn get_filler_comma_surround_re() -> &'static Regex {
    FILLER_COMMA_SURROUND_RE.get_or_init(|| {
        Regex::new(r"(?i),\s*(?:um|uh|erm)\s*,").expect("Invalid regex")
    })
}

fn get_filler_trailing_re() -> &'static Regex {
    FILLER_TRAILING_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:,\s*|\s+)(?:um|uh|erm)\s*([.?!;:]|$)").expect("Invalid regex")
    })
}

fn get_filler_comma_prefix_re() -> &'static Regex {
    FILLER_COMMA_PREFIX_RE.get_or_init(|| {
        Regex::new(r"(?i),\s*(?:um|uh|erm)\b\s*").expect("Invalid regex")
    })
}

fn get_filler_start_re() -> &'static Regex {
    FILLER_START_RE.get_or_init(|| {
        Regex::new(r"(?i)^(?:\s*(?:um|uh|erm)[,\s]*)+").expect("Invalid regex")
    })
}

fn get_filler_word_re() -> &'static Regex {
    FILLER_WORD_RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:um|uh|erm)\b\s*").expect("Invalid regex")
    })
}

fn get_multi_space_re() -> &'static Regex {
    MULTI_SPACE_RE.get_or_init(|| {
        Regex::new(r"[ \t]{2,}").expect("Invalid regex")
    })
}

fn get_space_before_punct_re() -> &'static Regex {
    SPACE_BEFORE_PUNCT_RE.get_or_init(|| {
        Regex::new(r"\s+([,.;?!:])").expect("Invalid regex")
    })
}

fn get_spoken_newline_re() -> &'static Regex {
    SPOKEN_NEWLINE_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:,\s*|\s+)?\b(new\s+paragraph)\b(?:\s*,|\s*)?|(?:,\s*|\s+)?\b(new\s+line|newline)\b(?:\s*,|\s*)?").expect("Invalid regex")
    })
}

fn get_spoken_punct_re() -> &'static Regex {
    SPOKEN_PUNCT_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:,\s*|\s*)?\b(question\s+mark|exclamation\s+mark|exclamation\s+point|full\s+stop|period|comma|colon|semicolon)\b(?:\s*,)?").expect("Invalid regex")
    })
}

fn get_punct_space_re() -> &'static Regex {
    PUNCT_SPACE_RE.get_or_init(|| {
        Regex::new(r"([,;:])([^\s\d\n])").expect("Invalid regex")
    })
}

fn get_dangling_comma_re() -> &'static Regex {
    DANGLING_COMMA_RE.get_or_init(|| {
        Regex::new(r",\s*([.?!;:])").expect("Invalid regex")
    })
}

fn get_multi_comma_re() -> &'static Regex {
    MULTI_COMMA_RE.get_or_init(|| {
        Regex::new(r",(?:\s*,)+").expect("Invalid regex")
    })
}

fn get_terminator_space_re() -> &'static Regex {
    TERMINATOR_SPACE_RE.get_or_init(|| {
        Regex::new(r"([.?!])([a-zA-Z])").expect("Invalid regex")
    })
}

/// Remove speech fillers ("um", "uh", "erm") at word boundaries.
pub fn remove_fillers(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }

    // 1. Remove leading fillers at start of string
    let after_start = get_filler_start_re().replace(input, "").to_string();

    // 2. Handle fillers surrounded by commas: ", um," -> ", " (preserves comma and prevents word fusion)
    let after_comma_surround = get_filler_comma_surround_re().replace_all(&after_start, ", ").to_string();

    // 3. Handle trailing fillers before punctuation or at end of string
    let after_trailing = get_filler_trailing_re().replace_all(&after_comma_surround, "$1").to_string();

    // 4. Handle fillers with comma prefix before a word: ", um word" -> " word"
    let after_comma_prefix = get_filler_comma_prefix_re().replace_all(&after_trailing, " ").to_string();

    // 5. Remove any remaining standalone filler words
    let without_fillers = get_filler_word_re().replace_all(&after_comma_prefix, "").to_string();

    // 6. Clean up dangling commas before sentence terminators (e.g. ", .")
    let cleaned_dangling = get_dangling_comma_re().replace_all(&without_fillers, "$1").to_string();

    // 7. Clean up multiple consecutive commas
    let cleaned_commas = get_multi_comma_re().replace_all(&cleaned_dangling, ", ").to_string();

    // 8. Clean up spaces before punctuation (e.g. "word ." -> "word.")
    let cleaned_punct_spaces = get_space_before_punct_re().replace_all(&cleaned_commas, "$1").to_string();

    // 9. Normalize internal consecutive spaces
    let collapsed = get_multi_space_re().replace_all(&cleaned_punct_spaces, " ").to_string();
    let trimmed = collapsed.trim();

    // If input only contained fillers and punctuation (no actual words left), return empty
    if !trimmed.chars().any(|c| c.is_alphanumeric()) {
        return String::new();
    }

    // 10. Capitalize first letter if leading filler was removed
    capitalize_first_grapheme(trimmed)
}

/// Convert spoken punctuation commands into punctuation symbols and formatting.
pub fn apply_spoken_commands(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    // 1. Replace newlines, consuming surrounding whitespace and optional leading comma
    let after_newlines = get_spoken_newline_re().replace_all(input, |caps: &regex::Captures| {
        if caps.get(1).is_some() {
            "\n\n"
        } else {
            "\n"
        }
    });

    // 2. Replace punctuation marks, consuming optional leading comma and spaces
    let replaced = get_spoken_punct_re().replace_all(&after_newlines, |caps: &regex::Captures| {
        let matched = caps[1].to_lowercase();
        match matched.as_str() {
            "question mark" => "?",
            "exclamation mark" | "exclamation point" => "!",
            "period" | "full stop" => ".",
            "comma" => ",",
            "colon" => ":",
            "semicolon" => ";",
            _ => "",
        }
    });

    // 3. Clean up dangling comma before sentence terminators (e.g. ", ." -> ".")
    let cleaned_dangling = get_dangling_comma_re().replace_all(&replaced, "$1").to_string();

    // 4. Clean up spaces before punctuation (e.g. "word ." -> "word.")
    let cleaned_punct_spaces = get_space_before_punct_re().replace_all(&cleaned_dangling, "$1").to_string();

    // 5. Ensure a space after comma, colon, semicolon if directly followed by a word character
    let spaced = get_punct_space_re().replace_all(&cleaned_punct_spaces, "$1 $2");

    // 6. Ensure a space after sentence terminators if followed by a word character without space
    let terminator_spaced = get_terminator_space_re()
        .replace_all(&spaced, "$1 $2")
        .to_string();

    // 7. Capitalize letter following sentence terminators (. ? ! \n \n\n), skipping quotes and parens
    capitalize_after_sentence_boundaries(&terminator_spaced)
}

/// Strip punctuation marks if preserve_punctuation is disabled.
pub fn strip_punctuation(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() || c.is_whitespace() {
            out.push(c);
        } else {
            // If punctuation is between two alphanumeric characters, insert space to prevent word fusing
            let prev_alnum = i > 0 && chars[i - 1].is_alphanumeric();
            let next_alnum = i + 1 < chars.len() && chars[i + 1].is_alphanumeric();
            if prev_alnum && next_alnum {
                out.push(' ');
            }
        }
    }
    // Clean up duplicate spaces that might be left by removed punctuation
    get_multi_space_re().replace_all(&out, " ").trim().to_string()
}

/// Capitalize the first non-whitespace character in the string.
fn capitalize_first_grapheme(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            if first.is_lowercase() {
                first.to_uppercase().collect::<String>() + chars.as_str()
            } else {
                s.to_string()
            }
        }
    }
}

/// Capitalize characters immediately following sentence boundaries (., ?, !, \n).
/// Handles skipping opening quotes and parentheses.
fn capitalize_after_sentence_boundaries(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;

    for c in s.chars() {
        if c == '.' || c == '?' || c == '!' || c == '\n' {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next && (c.is_whitespace() || c == '"' || c == '\'' || c == '“' || c == '”' || c == '(' || c == '[') {
            result.push(c);
        } else if capitalize_next && c.is_alphabetic() {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            capitalize_next = false;
            result.push(c);
        }
    }

    result
}

/// Complete post-processing pipeline.
pub fn post_process(
    raw_text: &str,
    config: &PostProcessConfig,
    dictionary: &[DictionaryRecord],
) -> String {
    let mut text = raw_text.to_string();

    // 1. Remove fillers
    if config.remove_fillers {
        text = remove_fillers(&text);
    }

    // 2. Apply spoken punctuation commands
    if config.apply_spoken_commands {
        text = apply_spoken_commands(&text);
    }

    // 3. Apply custom SQLite dictionary replacements
    if config.apply_dictionary && !dictionary.is_empty() {
        text = apply_dictionary(&text, dictionary);
    }

    // 4. Preserve or strip punctuation
    if !config.preserve_punctuation {
        text = strip_punctuation(&text);
    }

    // 5. Trim whitespace
    if config.trim_whitespace {
        text = text.trim().to_string();
    }

    // 6. Add trailing space for continuous dictation
    if config.add_trailing_space && !text.is_empty() && !text.ends_with(' ') && !text.ends_with('\n') {
        text.push(' ');
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_fillers_basic() {
        let input = "Hello um how are you uh doing today erm friend";
        let out = remove_fillers(input);
        assert_eq!(out, "Hello how are you doing today friend");
    }

    #[test]
    fn test_remove_fillers_at_start_and_end() {
        let input = "Um, hello world, uh.";
        let out = remove_fillers(input);
        assert_eq!(out, "Hello world.");
    }

    #[test]
    fn test_remove_fillers_only_fillers() {
        let input = "Um, uh, erm...";
        let out = remove_fillers(input);
        assert_eq!(out, "");
    }

    #[test]
    fn test_remove_fillers_with_surrounding_commas() {
        let input = "Wait, um, actually";
        let out = remove_fillers(input);
        assert_eq!(out, "Wait, actually");

        let input2 = "I think, uh, we should go.";
        let out2 = remove_fillers(input2);
        assert_eq!(out2, "I think, we should go.");
    }

    #[test]
    fn test_apply_spoken_commands_with_native_commas() {
        let input = "Hello, period";
        let out = apply_spoken_commands(input);
        assert_eq!(out, "Hello.");

        let input2 = "Hello, question mark";
        let out2 = apply_spoken_commands(input2);
        assert_eq!(out2, "Hello?");

        let input3 = "Hello, comma world";
        let out3 = apply_spoken_commands(input3);
        assert_eq!(out3, "Hello, world");
    }

    #[test]
    fn test_remove_fillers_does_not_corrupt_similar_words() {
        let input = "The summary of the umbrella is humorous.";
        let out = remove_fillers(input);
        assert_eq!(out, "The summary of the umbrella is humorous.");
    }

    #[test]
    fn test_apply_spoken_commands_punctuation() {
        let input = "Hello comma world period How are you question mark Awesome exclamation mark";
        let out = apply_spoken_commands(input);
        assert_eq!(out, "Hello, world. How are you? Awesome!");
    }

    #[test]
    fn test_apply_spoken_commands_newlines() {
        let input = "Title new line first paragraph text new paragraph second paragraph text";
        let out = apply_spoken_commands(input);
        assert_eq!(out, "Title\nFirst paragraph text\n\nSecond paragraph text");
    }

    #[test]
    fn test_strip_punctuation() {
        let input = "Hello, world! How are you? Fine: thanks.";
        let out = strip_punctuation(input);
        assert_eq!(out, "Hello world How are you Fine thanks");
    }

    #[test]
    fn test_pipeline_full_integration() {
        let config = PostProcessConfig::default();
        let dict = vec![DictionaryRecord {
            id: "1".to_string(),
            find_text: "Murmer".to_string(),
            replace_text: "Murmur".to_string(),
            is_case_sensitive: false,
            created_at: "".to_string(),
        }];

        let raw = "  Um, this is Murmer comma a local voice tool period  ";
        let processed = post_process(raw, &config, &dict);
        assert_eq!(processed, "This is Murmur, a local voice tool.");
    }

    #[test]
    fn test_pipeline_trailing_space() {
        let config = PostProcessConfig {
            add_trailing_space: true,
            ..Default::default()
        };
        let raw = "Hello world";
        let processed = post_process(raw, &config, &[]);
        assert_eq!(processed, "Hello world ");
    }

    #[test]
    fn test_pipeline_no_preserve_punctuation() {
        let config = PostProcessConfig {
            preserve_punctuation: false,
            ..Default::default()
        };
        let raw = "Hello, world!";
        let processed = post_process(raw, &config, &[]);
        assert_eq!(processed, "Hello world");
    }

    #[test]
    fn test_pipeline_empty_input() {
        let config = PostProcessConfig::default();
        let processed = post_process("", &config, &[]);
        assert_eq!(processed, "");
    }

    #[test]
    fn test_capitalize_quotes_after_boundary() {
        let input = "Hello. \"how are you?\"";
        let out = apply_spoken_commands(input);
        assert_eq!(out, "Hello. \"How are you?\"");
    }

    #[test]
    fn test_strip_punctuation_no_fuse() {
        let input = "Hello,world;how:are?you!";
        let out = strip_punctuation(input);
        assert_eq!(out, "Hello world how are you");
    }
}
