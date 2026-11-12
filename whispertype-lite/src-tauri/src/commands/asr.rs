use crate::asr::{
    get_managed_model_directory, import_model_from_folder, inspect_model_directory, AsrEngine,
    BenchmarkMetrics, ModelStatus, ModelValidationResult, PostProcessConfig, TranscriptionResult,
};
use crate::audio::resample_and_normalize;
use crate::storage::{Database, NewHistoryRecord, SettingsManager};
use sherpa_onnx::Wave;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub fn get_model_status(engine: State<'_, AsrEngine>) -> Result<ModelStatus, String> {
    Ok(engine.get_status())
}

#[tauri::command]
pub fn load_model(engine: State<'_, AsrEngine>, app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    let _ = app.emit("model-status-changed", engine.get_status());
    let res = engine.load_model().map_err(|e| e.to_string());
    let _ = app.emit("model-status-changed", engine.get_status());
    res
}

#[tauri::command]
pub fn validate_model_path(path: String) -> Result<ModelValidationResult, String> {
    Ok(inspect_model_directory(Path::new(&path)))
}

#[tauri::command]
pub fn import_local_model(
    source_folder: String,
    engine: State<'_, AsrEngine>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let target = import_model_from_folder(Path::new(&source_folder))?;
    engine.spawn_background_load_with_app(app);
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_managed_model_path() -> Result<String, String> {
    Ok(get_managed_model_directory().to_string_lossy().to_string())
}

#[tauri::command]
pub fn transcribe_audio_buffer(
    samples: Option<Vec<f32>>,
    file_path: Option<String>,
    engine: State<'_, AsrEngine>,
    settings: State<'_, SettingsManager>,
    db: State<'_, Database>,
) -> Result<TranscriptionResult, String> {
    // 1. Resolve 16kHz mono PCM samples
    let pcm_samples: Vec<f32> = if let Some(buf) = samples {
        buf
    } else if let Some(ref path) = file_path {
        let wave = Wave::read(path)
            .ok_or_else(|| format!("Failed to read audio file at: {path}"))?;
        let original_samples = wave.samples();
        let sample_rate = wave.sample_rate();

        if sample_rate == 16_000 {
            original_samples.to_vec()
        } else {
            // Resample to 16,000 Hz mono
            resample_and_normalize(original_samples, sample_rate as u32, 1)
                .map_err(|e| format!("Resampling failed: {e}"))?
        }
    } else {
        return Err("Neither audio samples nor file_path provided".to_string());
    };

    // 2. Load post-processing config and dictionary rules
    let current_settings = settings.get();
    let post_cfg = PostProcessConfig::from(&current_settings);
    let dictionary_entries = db.get_dictionary().unwrap_or_default();

    // 3. Transcribe with VAD and post-processing
    let result = engine
        .transcribe_samples(&pcm_samples, &post_cfg, &dictionary_entries, None)
        .map_err(|e| e.to_string())?;

    // 4. Save to history if non-empty usable transcript was generated
    if !result.discarded_silent && !result.final_text.is_empty() {
        let new_record = NewHistoryRecord {
            raw_text: result.raw_text.clone(),
            final_text: result.final_text.clone(),
            duration_ms: result.duration_ms,
            language: Some("en".to_string()),
            model_variant: Some(current_settings.model_variant),
            provider: Some(result.provider.clone()),
            inference_ms: Some(result.inference_ms),
            refined_text: None,
            mode: Some("normal".to_string()),
        };

        if let Err(e) = db.insert_history(new_record, current_settings.history_retention_max) {
            log::warn!("Failed to persist transcription to history database: {e}");
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn benchmark_model(
    custom_wav_path: Option<String>,
    engine: State<'_, AsrEngine>,
) -> Result<BenchmarkMetrics, String> {
    engine
        .benchmark_model(custom_wav_path.as_deref())
        .map_err(|e| e.to_string())
}
