use serde::{Deserialize, Serialize};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, Wave};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use thiserror::Error;

use super::postprocess::{post_process, PostProcessConfig};
use super::vad::{VadConfig, VadDecision, VadProcessor};
use crate::storage::DictionaryRecord;

#[derive(Debug, Error)]
pub enum AsrError {
    #[error("Model files not found: {0}")]
    ModelNotFound(String),

    #[error("Model load error: {0}")]
    ModelLoadFailed(String),

    #[error("Audio decode error: {0}")]
    AudioDecodeFailed(String),

    #[error("Inference error: {0}")]
    InferenceFailed(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "message", rename_all = "snake_case")]
pub enum ModelLoadStatus {
    NotLoaded,
    Loading,
    Ready,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelStatus {
    pub status: ModelLoadStatus,
    pub active_provider: String,
    pub model_variant: String,
    pub model_dir: Option<String>,
    pub idle_unload_minutes: u32,
    pub seconds_since_last_use: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub raw_text: String,
    pub final_text: String,
    pub duration_ms: u32,
    pub inference_ms: u32,
    pub rtf: f64,
    pub provider: String,
    pub discarded_silent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub provider: String,
    pub load_time_ms: u64,
    pub inference_time_ms: u64,
    pub audio_duration_ms: u64,
    pub rtf: f64,
    pub transcript: String,
    pub success: bool,
    pub error: Option<String>,
}

struct EngineInner {
    recognizer: Option<OfflineRecognizer>,
    status: ModelLoadStatus,
    active_provider: String,
    requested_provider: String,
    model_variant: String,
    model_dir: Option<PathBuf>,
    last_used: Option<Instant>,
    idle_unload_minutes: u32,
}

#[derive(Clone)]
pub struct AsrEngine {
    inner: Arc<RwLock<EngineInner>>,
    vad: Arc<VadProcessor>,
}

impl AsrEngine {
    pub fn new(requested_provider: &str, idle_unload_minutes: u32) -> Self {
        let vad = Arc::new(VadProcessor::new(VadConfig::default()));
        let inner = EngineInner {
            recognizer: None,
            status: ModelLoadStatus::NotLoaded,
            active_provider: "none".to_string(),
            requested_provider: requested_provider.to_string(),
            model_variant: "parakeet-tdt-0.6b-v3-int8".to_string(),
            model_dir: None,
            last_used: None,
            idle_unload_minutes,
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
            vad,
        }
    }

    /// Locate model directory containing required Parakeet int8 artifacts:
    /// - encoder.int8.onnx
    /// - decoder.int8.onnx
    /// - joiner.int8.onnx
    /// - tokens.txt
    pub fn find_model_directory() -> Result<PathBuf, AsrError> {
        let mut candidate_dirs: Vec<PathBuf> = Vec::new();

        // 1. Standard %APPDATA%\Murmur\models\parakeet-tdt-int8\
        if let Ok(appdata) = std::env::var("APPDATA") {
            candidate_dirs.push(PathBuf::from(appdata).join("Murmur").join("models").join("parakeet-tdt-int8"));
        }
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
            candidate_dirs.push(proj_dirs.data_dir().join("models").join("parakeet-tdt-int8"));
        }

        // 2. Cargo manifest directory (test / dev environment)
        if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            let manifest_path = PathBuf::from(manifest);
            candidate_dirs.push(manifest_path.join("models").join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));
            candidate_dirs.push(manifest_path.join("../models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));
            candidate_dirs.push(manifest_path.join("../models/parakeet-tdt-int8"));
        }

        // 3. Workspace / repository paths
        candidate_dirs.push(PathBuf::from("models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));
        candidate_dirs.push(PathBuf::from("../models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));
        candidate_dirs.push(PathBuf::from("models/parakeet-tdt-int8"));
        candidate_dirs.push(PathBuf::from("../models/parakeet-tdt-int8"));
        candidate_dirs.push(PathBuf::from(r"d:\Projects!@#\Murmur\models\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));

        // 4. Executable relative paths
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(exe_dir) = current_exe.parent() {
                candidate_dirs.push(exe_dir.join("models").join("parakeet-tdt-int8"));
                candidate_dirs.push(exe_dir.join("models").join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));
                candidate_dirs.push(exe_dir.join("../../../models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"));
            }
        }

        for dir in candidate_dirs.iter() {
            if Self::validate_model_dir(dir) {
                log::info!("Discovered ASR model artifacts in: {}", dir.display());
                return Ok(dir.clone());
            }
        }

        let tried = candidate_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        Err(AsrError::ModelNotFound(format!(
            "Parakeet TDT int8 model files (encoder.int8.onnx, decoder.int8.onnx, joiner.int8.onnx, tokens.txt) not found. Checked: [{tried}]"
        )))
    }

    /// Check if directory contains all required Parakeet TDT ONNX model files.
    pub fn validate_model_dir(dir: &Path) -> bool {
        if !dir.exists() {
            return false;
        }
        dir.join("encoder.int8.onnx").exists()
            && dir.join("decoder.int8.onnx").exists()
            && dir.join("joiner.int8.onnx").exists()
            && dir.join("tokens.txt").exists()
    }

    /// Background loading: spawn model loading on a dedicated thread so UI opens immediately.
    pub fn spawn_background_load(&self) {
        let engine = self.clone();
        std::thread::Builder::new()
            .name("murmur-asr-loader".to_string())
            .spawn(move || {
                log::info!("Starting background ASR model load...");
                if let Err(e) = engine.load_model() {
                    log::error!("Background ASR model load failed: {e}");
                }
            })
            .expect("Failed to spawn ASR model loader thread");
    }

    /// Load or reload model with provider fallback chain: CUDA -> DirectML -> CPU int8.
    pub fn load_model(&self) -> Result<(), AsrError> {
        {
            let mut guard = self.inner.write().map_err(|_| AsrError::LockPoisoned)?;
            guard.status = ModelLoadStatus::Loading;
        }

        let model_dir = match Self::find_model_directory() {
            Ok(dir) => dir,
            Err(e) => {
                let err_msg = e.to_string();
                let mut guard = self.inner.write().map_err(|_| AsrError::LockPoisoned)?;
                guard.status = ModelLoadStatus::Error(err_msg.clone());
                return Err(e);
            }
        };

        let encoder = model_dir.join("encoder.int8.onnx");
        let decoder = model_dir.join("decoder.int8.onnx");
        let joiner = model_dir.join("joiner.int8.onnx");
        let tokens = model_dir.join("tokens.txt");

        let requested_provider = {
            let guard = self.inner.read().map_err(|_| AsrError::LockPoisoned)?;
            guard.requested_provider.to_lowercase()
        };

        // Determine fallback order based on requested provider
        let providers_to_try: Vec<&str> = match requested_provider.as_str() {
            "cuda" => vec!["cuda", "directml", "cpu"],
            "directml" => vec!["directml", "cpu"],
            _ => vec!["cpu"],
        };

        let mut loaded_recognizer = None;
        let mut active_provider = "cpu".to_string();

        for provider in providers_to_try {
            log::info!("Attempting to initialize sherpa-onnx with provider [{provider}]...");
            let mut config = OfflineRecognizerConfig::default();
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: Some(encoder.to_string_lossy().to_string()),
                decoder: Some(decoder.to_string_lossy().to_string()),
                joiner: Some(joiner.to_string_lossy().to_string()),
            };
            config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
            config.model_config.model_type = Some("nemo_transducer".to_string());
            config.model_config.provider = Some(provider.to_string());
            config.model_config.num_threads = 4;
            config.model_config.debug = false;

            let t0 = Instant::now();
            if let Some(recognizer) = OfflineRecognizer::create(&config) {
                let elapsed = t0.elapsed().as_millis();
                log::info!("OfflineRecognizer initialized successfully in {elapsed} ms using [{provider}].");
                loaded_recognizer = Some(recognizer);
                active_provider = provider.to_string();
                break;
            } else {
                log::warn!("Provider [{provider}] failed to initialize. Falling back to next available provider.");
            }
        }

        let recognizer = match loaded_recognizer {
            Some(r) => r,
            None => {
                let err_msg = "Failed to initialize OfflineRecognizer with any execution provider (CUDA/DirectML/CPU)".to_string();
                let mut guard = self.inner.write().map_err(|_| AsrError::LockPoisoned)?;
                guard.status = ModelLoadStatus::Error(err_msg.clone());
                return Err(AsrError::ModelLoadFailed(err_msg));
            }
        };

        let mut guard = self.inner.write().map_err(|_| AsrError::LockPoisoned)?;
        guard.recognizer = Some(recognizer);
        guard.status = ModelLoadStatus::Ready;
        guard.active_provider = active_provider;
        guard.model_dir = Some(model_dir);
        guard.last_used = Some(Instant::now());

        Ok(())
    }

    /// Check if idle unload timer expired and release model memory if needed.
    pub fn check_idle_unload(&self) {
        let mut guard = match self.inner.write() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.idle_unload_minutes == 0 || guard.status != ModelLoadStatus::Ready {
            return;
        }

        if let Some(last_used) = guard.last_used {
            let elapsed_secs = last_used.elapsed().as_secs();
            let threshold_secs = guard.idle_unload_minutes as u64 * 60;
            if elapsed_secs >= threshold_secs {
                log::info!(
                    "ASR model idle for {}s (threshold {}s). Unloading from memory.",
                    elapsed_secs,
                    threshold_secs
                );
                guard.recognizer = None;
                guard.status = ModelLoadStatus::NotLoaded;
                guard.active_provider = "none".to_string();
            }
        }
    }

    /// Return current model status snapshot.
    pub fn get_status(&self) -> ModelStatus {
        self.check_idle_unload();
        let guard = self.inner.read().expect("Lock poisoned");
        let seconds_since_last_use = guard.last_used.map(|t| t.elapsed().as_secs());

        ModelStatus {
            status: guard.status.clone(),
            active_provider: guard.active_provider.clone(),
            model_variant: guard.model_variant.clone(),
            model_dir: guard.model_dir.as_ref().map(|p| p.to_string_lossy().to_string()),
            idle_unload_minutes: guard.idle_unload_minutes,
            seconds_since_last_use,
        }
    }

    /// Set idle unload timer in minutes (0 = never unload).
    pub fn set_idle_unload_minutes(&self, minutes: u32) {
        let mut guard = self.inner.write().expect("Lock poisoned");
        guard.idle_unload_minutes = minutes;
    }

    /// Update requested execution provider (cuda, directml, cpu).
    /// If the model is already loaded and the provider changed, reloads the model.
    pub fn set_requested_provider(&self, provider: &str) -> Result<(), AsrError> {
        let (needs_reload, changed) = {
            let mut guard = self.inner.write().map_err(|_| AsrError::LockPoisoned)?;
            if guard.requested_provider.eq_ignore_ascii_case(provider) {
                (false, false)
            } else {
                guard.requested_provider = provider.to_string();
                (guard.status == ModelLoadStatus::Ready, true)
            }
        };

        if needs_reload {
            log::info!("ASR provider changed to [{provider}]. Reloading model...");
            self.load_model()?;
        } else if changed {
            log::info!("ASR requested provider updated to [{provider}].");
        }
        Ok(())
    }

    /// Run full ASR and VAD transcription pipeline on 16,000 Hz mono PCM samples.
    pub fn transcribe_samples(
        &self,
        samples: &[f32],
        postprocess_config: &PostProcessConfig,
        dictionary: &[DictionaryRecord],
    ) -> Result<TranscriptionResult, AsrError> {
        let total_audio_duration_ms = (samples.len() as f64 / 16000.0 * 1000.0) as u32;

        // 1. Voice Activity Detection & Segmentation
        let vad_decision = self.vad.process(samples);
        let (chunks, speech_duration_ms) = match vad_decision {
            VadDecision::DiscardTooShort { duration_ms } => {
                log::info!("VAD discarded audio as silence / too short ({duration_ms} ms speech). Returning to IDLE.");
                return Ok(TranscriptionResult {
                    raw_text: String::new(),
                    final_text: String::new(),
                    duration_ms: total_audio_duration_ms,
                    inference_ms: 0,
                    rtf: 0.0,
                    provider: self.get_status().active_provider,
                    discarded_silent: true,
                });
            }
            VadDecision::SpeechChunks {
                chunks,
                total_speech_duration_ms,
            } => (chunks, total_speech_duration_ms),
        };

        // 2. Ensure model is loaded
        {
            let status = {
                let guard = self.inner.read().map_err(|_| AsrError::LockPoisoned)?;
                guard.status.clone()
            };
            if status != ModelLoadStatus::Ready {
                self.load_model()?;
            }
        }

        // 3. Perform sequential transcription of speech chunks
        let t_start = Instant::now();
        let mut raw_transcripts: Vec<String> = Vec::with_capacity(chunks.len());

        let guard = self.inner.read().map_err(|_| AsrError::LockPoisoned)?;
        let recognizer = guard.recognizer.as_ref().ok_or_else(|| {
            AsrError::ModelLoadFailed("Recognizer not present despite Ready status".to_string())
        })?;

        for chunk in &chunks {
            let stream = recognizer.create_stream();
            stream.accept_waveform(16_000, chunk);
            recognizer.decode(&stream);

            if let Some(res) = stream.get_result() {
                let text = res.text.trim().to_string();
                if !text.is_empty() {
                    raw_transcripts.push(text);
                }
            }
        }

        let inference_ms = t_start.elapsed().as_millis() as u32;

        // Update last used timestamp
        drop(guard);
        if let Ok(mut write_guard) = self.inner.write() {
            write_guard.last_used = Some(Instant::now());
        }

        let raw_text = raw_transcripts.join(" ");
        let final_text = post_process(&raw_text, postprocess_config, dictionary);

        let audio_duration_s = (speech_duration_ms as f64) / 1000.0;
        let rtf = if audio_duration_s > 0.001 {
            (inference_ms as f64 / 1000.0) / audio_duration_s
        } else {
            0.0
        };

        let provider = self.get_status().active_provider;

        log::info!(
            "ASR Transcription completed in {inference_ms} ms (Speech: {speech_duration_ms} ms, RTF: {rtf:.4}): \"{final_text}\""
        );

        Ok(TranscriptionResult {
            raw_text,
            final_text,
            duration_ms: total_audio_duration_ms,
            inference_ms,
            rtf,
            provider,
            discarded_silent: false,
        })
    }

    /// Self-benchmark engine on a test audio file and return performance metrics.
    pub fn benchmark_model(&self, custom_wav_path: Option<&str>) -> Result<BenchmarkMetrics, AsrError> {
        let load_t0 = Instant::now();
        let mut load_time_ms = 0;

        let needs_load = {
            let guard = self.inner.read().map_err(|_| AsrError::LockPoisoned)?;
            guard.status != ModelLoadStatus::Ready
        };

        if needs_load {
            self.load_model()?;
            load_time_ms = load_t0.elapsed().as_millis() as u64;
        }

        // Resolve WAV test file
        let mut wav_candidates: Vec<PathBuf> = Vec::new();
        if let Some(custom) = custom_wav_path {
            wav_candidates.push(PathBuf::from(custom));
        }
        if let Ok(guard) = self.inner.read() {
            if let Some(ref mdir) = guard.model_dir {
                wav_candidates.push(mdir.join("test_wavs").join("en.wav"));
            }
        }
        if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = PathBuf::from(manifest);
            wav_candidates.push(p.join("models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/test_wavs/en.wav"));
            wav_candidates.push(p.join("../models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/test_wavs/en.wav"));
            wav_candidates.push(p.join("../test.wav"));
        }
        wav_candidates.push(PathBuf::from("models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/test_wavs/en.wav"));
        wav_candidates.push(PathBuf::from("../models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/test_wavs/en.wav"));
        wav_candidates.push(PathBuf::from("test.wav"));
        wav_candidates.push(PathBuf::from(r"d:\Projects!@#\Murmur\models\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8\test_wavs\en.wav"));

        let test_wav = wav_candidates
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| AsrError::AudioDecodeFailed("No benchmark WAV audio file found".to_string()))?;

        log::info!("Running ASR benchmark using test file: {}", test_wav.display());

        let wave = Wave::read(&test_wav.to_string_lossy())
            .ok_or_else(|| AsrError::AudioDecodeFailed(format!("Failed to read WAV file: {}", test_wav.display())))?;

        let sample_rate = wave.sample_rate();
        let samples = wave.samples();
        let audio_duration_ms = (samples.len() as f64 / sample_rate as f64 * 1000.0) as u64;

        let guard = self.inner.read().map_err(|_| AsrError::LockPoisoned)?;
        let recognizer = guard.recognizer.as_ref().ok_or_else(|| {
            AsrError::ModelLoadFailed("Recognizer not available".to_string())
        })?;

        let stream = recognizer.create_stream();
        stream.accept_waveform(sample_rate, samples);

        let t_infer = Instant::now();
        recognizer.decode(&stream);
        let inference_time_ms = t_infer.elapsed().as_millis() as u64;

        let transcript = stream.get_result().map(|r| r.text).unwrap_or_default();
        let rtf = (inference_time_ms as f64 / 1000.0) / (audio_duration_ms as f64 / 1000.0);
        let provider = guard.active_provider.clone();

        drop(guard);
        if let Ok(mut write_guard) = self.inner.write() {
            write_guard.last_used = Some(Instant::now());
        }

        Ok(BenchmarkMetrics {
            provider,
            load_time_ms,
            inference_time_ms,
            audio_duration_ms,
            rtf,
            transcript,
            success: true,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_model_directory() {
        let res = AsrEngine::find_model_directory();
        assert!(
            res.is_ok(),
            "Model files should be discoverable in workspace or AppData: {:?}",
            res.err()
        );
        let dir = res.unwrap();
        assert!(AsrEngine::validate_model_dir(&dir));
    }

    #[test]
    fn test_engine_initial_status() {
        let engine = AsrEngine::new("cpu", 0);
        let status = engine.get_status();
        assert_eq!(status.status, ModelLoadStatus::NotLoaded);
        assert_eq!(status.active_provider, "none");
        assert_eq!(status.model_variant, "parakeet-tdt-0.6b-v3-int8");
    }

    #[test]
    fn test_engine_load_and_transcribe_silence() {
        let engine = AsrEngine::new("cpu", 0);
        let silence = vec![0.0f32; 16000 * 2];
        let cfg = PostProcessConfig::default();
        let res = engine.transcribe_samples(&silence, &cfg, &[]);
        assert!(res.is_ok());
        let tr = res.unwrap();
        assert!(tr.discarded_silent);
        assert_eq!(tr.final_text, "");
    }

    #[test]
    fn test_engine_benchmark_and_transcribe_speech() {
        let engine = AsrEngine::new("cpu", 0);
        let bench_res = engine.benchmark_model(None);
        assert!(bench_res.is_ok(), "Benchmark should succeed: {:?}", bench_res.err());
        let metrics = bench_res.unwrap();
        assert!(metrics.success);
        assert!(metrics.rtf < 0.5, "RTF should be fast: {}", metrics.rtf);
        assert!(!metrics.transcript.is_empty(), "Transcript should not be empty");
    }

    #[test]
    fn test_engine_idle_unload() {
        let engine = AsrEngine::new("cpu", 1); // 1 minute
        assert_eq!(engine.get_status().status, ModelLoadStatus::NotLoaded);
        engine.load_model().unwrap();
        assert_eq!(engine.get_status().status, ModelLoadStatus::Ready);
        // Simulate elapsed time by setting last_used in the past
        {
            let mut guard = engine.inner.write().unwrap();
            guard.last_used = Some(Instant::now() - std::time::Duration::from_secs(70));
        }
        engine.check_idle_unload();
        assert_eq!(engine.get_status().status, ModelLoadStatus::NotLoaded);
    }

    #[test]
    fn test_engine_provider_update() {
        let engine = AsrEngine::new("cpu", 0);
        assert_eq!(engine.get_status().active_provider, "none");
        engine.load_model().unwrap();
        assert_eq!(engine.get_status().active_provider, "cpu");

        // Update provider
        let res = engine.set_requested_provider("cpu");
        assert!(res.is_ok());
    }

    #[test]
    fn test_engine_transcribe_long_audio() {
        let engine = AsrEngine::new("cpu", 0);
        let mdir = AsrEngine::find_model_directory().unwrap();
        let wav_path = mdir.join("test_wavs").join("en.wav");
        if !wav_path.exists() {
            return;
        }

        let wave = Wave::read(&wav_path.to_string_lossy()).expect("Failed to read en.wav");
        let speech_samples = wave.samples();

        // Build a 34-second audio track:
        // Speech (3.8s) + 18s silence + Speech (3.8s) + 9s silence
        let mut long_audio = speech_samples.to_vec();
        long_audio.extend(vec![0.0f32; 16000 * 18]); // 18s silence
        long_audio.extend_from_slice(speech_samples);
        long_audio.extend(vec![0.0f32; 16000 * 9]); // 9s silence

        let cfg = PostProcessConfig::default();
        let res = engine.transcribe_samples(&long_audio, &cfg, &[]);
        assert!(res.is_ok(), "Long audio transcription failed: {:?}", res.err());
        let tr = res.unwrap();
        assert!(!tr.discarded_silent);
        assert!(!tr.final_text.is_empty(), "Transcript should not be empty");
    }
}
