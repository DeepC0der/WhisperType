use std::path::Path;

pub const DEFAULT_SAMPLE_RATE: u32 = 16_000;
pub const DEFAULT_FRAME_SIZE_MS: u32 = 20; // 20 ms per frame (320 samples @ 16kHz)
pub const DEFAULT_ENERGY_THRESHOLD: f32 = 0.008; // -42 dBFS RMS threshold
pub const DEFAULT_MIN_SPEECH_DURATION_MS: u32 = 300; // 300 ms
pub const DEFAULT_MAX_CHUNK_DURATION_SEC: f32 = 30.0; // 30 seconds
pub const SILENCE_HANGOVER_FRAMES: usize = 15; // 300 ms hangover
pub const SPEECH_ONSET_FRAMES: usize = 2; // 40 ms onset

#[derive(Debug, Clone)]
pub struct VadConfig {
    pub sample_rate: u32,
    pub frame_size_ms: u32,
    pub energy_threshold: f32,
    pub min_speech_duration_ms: u32,
    pub max_chunk_duration_sec: f32,
    pub silero_model_path: Option<String>,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            frame_size_ms: DEFAULT_FRAME_SIZE_MS,
            energy_threshold: DEFAULT_ENERGY_THRESHOLD,
            min_speech_duration_ms: DEFAULT_MIN_SPEECH_DURATION_MS,
            max_chunk_duration_sec: DEFAULT_MAX_CHUNK_DURATION_SEC,
            silero_model_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VadDecision {
    /// Discard silently without running inference (pure silence or usable speech < min duration)
    DiscardTooShort { duration_ms: u32 },
    /// Usable speech detected and segmented into one or more <= 30s chunks
    SpeechChunks {
        chunks: Vec<Vec<f32>>,
        total_speech_duration_ms: u32,
    },
}

pub struct VadProcessor {
    config: VadConfig,
}

impl VadProcessor {
    pub fn new(config: VadConfig) -> Self {
        Self { config }
    }

    /// Calculate RMS energy of a frame of samples.
    pub fn compute_rms(frame: &[f32]) -> f32 {
        if frame.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = frame.iter().map(|&s| s * s).sum();
        (sum_sq / frame.len() as f32).sqrt()
    }

    /// Process audio buffer (16 kHz mono PCM) using VAD:
    /// 1. If Silero VAD model exists and configured, optionally use sherpa-onnx VAD.
    /// 2. Otherwise, use high-performance energy VAD.
    /// 3. Trim leading and trailing silence.
    /// 4. Discard if speech < 300 ms.
    /// 5. Segment long recordings (> 30s) at silence boundaries into chunks of <= 30s.
    pub fn process(&self, samples: &[f32]) -> VadDecision {
        if samples.is_empty() {
            return VadDecision::DiscardTooShort { duration_ms: 0 };
        }

        // Check if Silero VAD is configured and exists on disk
        if let Some(ref silero_path) = self.config.silero_model_path {
            if Path::new(silero_path).exists() {
                if let Some(decision) = self.process_silero_vad(samples, silero_path) {
                    return decision;
                }
            }
        }

        // Standard robust energy-based VAD
        self.process_energy_vad(samples)
    }

    /// Process via sherpa-onnx VoiceActivityDetector when Silero VAD ONNX model is available.
    fn process_silero_vad(&self, samples: &[f32], model_path: &str) -> Option<VadDecision> {
        let mut vad_config = sherpa_onnx::VadModelConfig::default();
        vad_config.silero_vad.model = Some(model_path.to_string());
        vad_config.silero_vad.threshold = 0.5;
        vad_config.silero_vad.min_silence_duration = 0.3;
        vad_config.silero_vad.min_speech_duration = self.config.min_speech_duration_ms as f32 / 1000.0;
        vad_config.silero_vad.window_size = 512;
        vad_config.sample_rate = self.config.sample_rate as i32;
        vad_config.num_threads = 1;

        let detector = sherpa_onnx::VoiceActivityDetector::create(&vad_config, 60.0)?;
        detector.accept_waveform(samples);
        detector.flush();

        let mut speech_samples: Vec<f32> = Vec::new();
        while !detector.is_empty() {
            if let Some(segment) = detector.front() {
                speech_samples.extend_from_slice(segment.samples());
            }
            detector.pop();
        }

        let total_samples = speech_samples.len();
        let duration_ms = (total_samples as f64 / self.config.sample_rate as f64 * 1000.0) as u32;

        if duration_ms < self.config.min_speech_duration_ms {
            return Some(VadDecision::DiscardTooShort { duration_ms });
        }

        // Split speech segments if longer than max chunk duration
        let chunks = self.split_long_audio(&speech_samples);
        Some(VadDecision::SpeechChunks {
            chunks,
            total_speech_duration_ms: duration_ms,
        })
    }

    /// Energy-based VAD with frame analysis, hangover smoothing, and silence boundary splitting.
    fn process_energy_vad(&self, samples: &[f32]) -> VadDecision {
        let frame_samples = (self.config.sample_rate * self.config.frame_size_ms / 1000) as usize;
        if frame_samples == 0 {
            return VadDecision::DiscardTooShort { duration_ms: 0 };
        }

        let num_frames = samples.len() / frame_samples;
        if num_frames == 0 {
            return VadDecision::DiscardTooShort { duration_ms: 0 };
        }

        // 1. Frame-level speech detection
        let mut is_speech_frame = vec![false; num_frames];
        let mut frame_rms = vec![0.0f32; num_frames];

        for i in 0..num_frames {
            let start = i * frame_samples;
            let end = (start + frame_samples).min(samples.len());
            let rms = Self::compute_rms(&samples[start..end]);
            frame_rms[i] = rms;
            if rms >= self.config.energy_threshold {
                is_speech_frame[i] = true;
            }
        }

        // 2. Onset and Hangover smoothing
        // Require at least SPEECH_ONSET_FRAMES to trigger speech,
        // and keep active for SILENCE_HANGOVER_FRAMES before turning off.
        let mut smoothed_speech = vec![false; num_frames];
        let mut speech_active = false;
        let mut silence_count = 0;

        for i in 0..num_frames {
            if is_speech_frame[i] {
                // Check onset
                let has_onset = (0..SPEECH_ONSET_FRAMES).all(|k| {
                    let idx = i + k;
                    idx < num_frames && is_speech_frame[idx]
                });
                if has_onset || speech_active {
                    speech_active = true;
                    silence_count = 0;
                    smoothed_speech[i] = true;
                }
            } else if speech_active {
                silence_count += 1;
                if silence_count <= SILENCE_HANGOVER_FRAMES {
                    smoothed_speech[i] = true; // Hangover maintains speech state
                } else {
                    speech_active = false;
                }
            }
        }

        // 3. Trim leading and trailing silence based on smoothed speech activity
        let first_speech_frame = smoothed_speech.iter().position(|&s| s);
        let last_speech_frame = smoothed_speech.iter().rposition(|&s| s);

        let (start_frame, end_frame) = match (first_speech_frame, last_speech_frame) {
            (Some(first), Some(last)) => (first, last),
            _ => {
                // Entire audio is silence or isolated clicks under onset threshold
                return VadDecision::DiscardTooShort { duration_ms: 0 };
            }
        };

        // Measure actual detected speech duration from frames marked as speech
        let speech_frame_count = is_speech_frame[start_frame..=end_frame].iter().filter(|&&s| s).count();
        let actual_speech_ms = (speech_frame_count * self.config.frame_size_ms as usize) as u32;

        // 4. Minimum speech duration check: if usable speech < 300 ms, discard silently
        if actual_speech_ms < self.config.min_speech_duration_ms {
            return VadDecision::DiscardTooShort { duration_ms: actual_speech_ms };
        }

        // Pre-pad 80 ms (4 frames) before first speech frame; end_frame already includes hangover
        let pad_frames = (80 / self.config.frame_size_ms) as usize;
        let padded_start_frame = start_frame.saturating_sub(pad_frames);
        let padded_end_frame = (end_frame + 1).min(num_frames);

        let start_sample = padded_start_frame * frame_samples;
        let end_sample = (padded_end_frame * frame_samples).min(samples.len());

        let trimmed_samples = &samples[start_sample..end_sample];
        let duration_ms = (trimmed_samples.len() as f64 / self.config.sample_rate as f64 * 1000.0) as u32;

        // 5. Long recording handling: split recordings longer than max_chunk_duration_sec
        let chunks = self.split_at_silence_boundaries(
            trimmed_samples,
            &frame_rms[padded_start_frame..padded_end_frame],
            frame_samples,
        );

        VadDecision::SpeechChunks {
            chunks,
            total_speech_duration_ms: duration_ms,
        }
    }

    /// Split audio longer than max_chunk_duration_sec at silence boundaries into chunks of <= 30 seconds.
    fn split_at_silence_boundaries(
        &self,
        samples: &[f32],
        frame_rms: &[f32],
        frame_samples: usize,
    ) -> Vec<Vec<f32>> {
        let max_chunk_samples = (self.config.sample_rate as f32 * self.config.max_chunk_duration_sec) as usize;

        // If audio <= 30 seconds, return single chunk immediately
        if samples.len() <= max_chunk_samples {
            return vec![samples.to_vec()];
        }

        let mut chunks = Vec::new();
        let mut sample_offset = 0;

        while sample_offset < samples.len() {
            let remaining_samples = samples.len() - sample_offset;
            if remaining_samples <= max_chunk_samples {
                chunks.push(samples[sample_offset..].to_vec());
                break;
            }

            // Target window for split: between 10 seconds and 30 seconds from current offset
            let min_chunk_samples = (self.config.sample_rate as f32 * 10.0) as usize;
            let target_chunk_samples = (self.config.sample_rate as f32 * 20.0) as usize;
            let window_start_sample = sample_offset + min_chunk_samples.min(remaining_samples);
            let window_end_sample = sample_offset + max_chunk_samples.min(remaining_samples);

            let window_start_frame = window_start_sample / frame_samples;
            let window_end_frame = (window_end_sample / frame_samples).min(frame_rms.len());

            // First search for silence frames (RMS < threshold) within 10s-30s window to split at natural silence
            let mut best_split_frame = window_end_frame;
            let mut found_silence = false;
            let mut min_energy = f32::MAX;

            for f in window_start_frame..window_end_frame {
                if f < frame_rms.len() {
                    let rms = frame_rms[f];
                    if rms < self.config.energy_threshold {
                        // Found a true silence frame
                        if !found_silence || rms < min_energy {
                            found_silence = true;
                            min_energy = rms;
                            best_split_frame = f;
                        }
                    } else if !found_silence && rms < min_energy {
                        // Keep track of lowest energy frame in case no silence is found
                        let f_sample = f * frame_samples;
                        if f_sample >= sample_offset + target_chunk_samples {
                            min_energy = rms;
                            best_split_frame = f;
                        }
                    }
                }
            }

            let split_sample = (best_split_frame * frame_samples).clamp(window_start_sample, window_end_sample);
            chunks.push(samples[sample_offset..split_sample].to_vec());
            sample_offset = split_sample;
        }

        chunks
    }

    /// Fallback splitter for Silero segments
    fn split_long_audio(&self, samples: &[f32]) -> Vec<Vec<f32>> {
        let max_chunk_samples = (self.config.sample_rate as f32 * self.config.max_chunk_duration_sec) as usize;
        if samples.len() <= max_chunk_samples {
            return vec![samples.to_vec()];
        }

        let mut chunks = Vec::new();
        let mut offset = 0;
        while offset < samples.len() {
            let end = (offset + max_chunk_samples).min(samples.len());
            chunks.push(samples[offset..end].to_vec());
            offset = end;
        }
        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_sine_wave(freq: f32, duration_sec: f32, sample_rate: u32, amp: f32) -> Vec<f32> {
        let num_samples = (duration_sec * sample_rate as f32) as usize;
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq * t).sin() * amp
            })
            .collect()
    }

    #[test]
    fn test_vad_pure_silence_discarded() {
        let vad = VadProcessor::new(VadConfig::default());
        let silence = vec![0.0f32; 16_000 * 2]; // 2 seconds of zeros
        let res = vad.process(&silence);
        match res {
            VadDecision::DiscardTooShort { duration_ms } => {
                assert_eq!(duration_ms, 0);
            }
            _ => panic!("Expected silence to be discarded"),
        }
    }

    #[test]
    fn test_vad_too_short_discarded() {
        let vad = VadProcessor::new(VadConfig::default());
        // 100 ms speech burst + silence
        let mut audio = vec![0.0f32; 1600]; // 100 ms silence
        audio.extend(generate_sine_wave(440.0, 0.15, 16_000, 0.5)); // 150 ms speech
        audio.extend(vec![0.0f32; 1600]); // 100 ms silence

        let res = vad.process(&audio);
        match res {
            VadDecision::DiscardTooShort { duration_ms } => {
                assert!(duration_ms < 300, "Speech duration should be < 300 ms: {duration_ms}");
            }
            _ => panic!("Expected audio under 300ms to be discarded"),
        }
    }

    #[test]
    fn test_vad_valid_speech_trimmed() {
        let vad = VadProcessor::new(VadConfig::default());
        // 1.0s silence + 1.0s speech + 1.0s silence
        let mut audio = vec![0.0f32; 16_000];
        let speech = generate_sine_wave(440.0, 1.0, 16_000, 0.4);
        audio.extend(speech);
        audio.extend(vec![0.0f32; 16_000]);

        let res = vad.process(&audio);
        match res {
            VadDecision::SpeechChunks {
                chunks,
                total_speech_duration_ms,
            } => {
                assert_eq!(chunks.len(), 1);
                // Speech is 1000 ms + up to 200 ms padding
                assert!(
                    (900..=1400).contains(&total_speech_duration_ms),
                    "Expected ~1000-1200 ms, got {total_speech_duration_ms}"
                );
            }
            _ => panic!("Expected valid speech chunks"),
        }
    }

    #[test]
    fn test_vad_long_recording_split_at_silence() {
        let vad = VadProcessor::new(VadConfig::default());
        // 65 seconds total audio:
        // Speech (24s) -> Silence (2s) -> Speech (24s) -> Silence (2s) -> Speech (13s)
        let mut audio = Vec::new();
        audio.extend(generate_sine_wave(300.0, 24.0, 16_000, 0.3));
        audio.extend(vec![0.0f32; 16_000 * 2]); // 2s silence
        audio.extend(generate_sine_wave(300.0, 24.0, 16_000, 0.3));
        audio.extend(vec![0.0f32; 16_000 * 2]); // 2s silence
        audio.extend(generate_sine_wave(300.0, 13.0, 16_000, 0.3));

        let res = vad.process(&audio);
        match res {
            VadDecision::SpeechChunks { chunks, .. } => {
                assert!(chunks.len() >= 2, "Should have split into multiple chunks");
                for (i, chunk) in chunks.iter().enumerate() {
                    let duration_s = chunk.len() as f32 / 16_000.0;
                    assert!(
                        duration_s <= 30.01,
                        "Chunk {i} exceeded 30 seconds limit: {duration_s}s"
                    );
                }
            }
            _ => panic!("Expected speech chunks"),
        }
    }

    #[test]
    fn test_vad_ignores_isolated_click_before_speech() {
        let vad = VadProcessor::new(VadConfig::default());
        // 1 frame (20ms) click at t=0, then 2.0s silence, then 1.0s speech, then 1.0s silence
        let mut audio = generate_sine_wave(1000.0, 0.02, 16_000, 0.8); // 20 ms click
        audio.extend(vec![0.0f32; 16_000 * 2]); // 2s silence
        audio.extend(generate_sine_wave(440.0, 1.0, 16_000, 0.4)); // 1.0s speech
        audio.extend(vec![0.0f32; 16_000]); // 1.0s silence

        let res = vad.process(&audio);
        match res {
            VadDecision::SpeechChunks {
                chunks,
                total_speech_duration_ms,
            } => {
                assert_eq!(chunks.len(), 1);
                // The isolated 20ms click and the 2.0s silence should have been trimmed off!
                // Resulting audio duration should be ~1000-1400 ms, NOT ~3200 ms.
                assert!(
                    total_speech_duration_ms < 1600,
                    "Click at start was not rejected: duration={total_speech_duration_ms}ms"
                );
            }
            _ => panic!("Expected valid speech chunks after trimming click"),
        }
    }
}
