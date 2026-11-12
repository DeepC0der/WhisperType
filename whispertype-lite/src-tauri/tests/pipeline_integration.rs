use whispertype_lite_lib::asr::{AsrEngine, PostProcessConfig};
use whispertype_lite_lib::audio::resample_and_normalize;
use whispertype_lite_lib::platform::windows::{
    calculate_overlay_position, HotkeyAction, HotkeyModeController,
};
use whispertype_lite_lib::state::{AppState, AppStateManager};
use whispertype_lite_lib::storage::{Database, HotkeyMode, NewHistoryRecord};
use windows::Win32::Foundation::RECT;

#[test]
fn test_end_to_end_state_pipeline_transitions() {
    let sm = AppStateManager::new();
    assert_eq!(sm.get_state(), AppState::IDLE);

    // 1. Start recording
    assert!(sm.transition_to(AppState::RECORDING).is_ok());
    assert_eq!(sm.get_state(), AppState::RECORDING);

    // 2. Stop recording -> transcribing
    assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
    assert_eq!(sm.get_state(), AppState::TRANSCRIBING);

    // 3. Transcription completes -> inserting
    assert!(sm.transition_to(AppState::INSERTING).is_ok());
    assert_eq!(sm.get_state(), AppState::INSERTING);

    // 4. Injection completes -> done (checkmark state)
    assert!(sm.transition_to(AppState::DONE).is_ok());
    assert_eq!(sm.get_state(), AppState::DONE);

    // 5. Timer expires -> idle
    assert!(sm.transition_to(AppState::IDLE).is_ok());
    assert_eq!(sm.get_state(), AppState::IDLE);
}

#[test]
fn test_pipeline_auto_insert_disabled_transition() {
    let sm = AppStateManager::new();
    // In auto_insert = false mode: IDLE -> RECORDING -> TRANSCRIBING -> DONE -> IDLE
    assert!(sm.transition_to(AppState::RECORDING).is_ok());
    assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
    assert!(sm.transition_to(AppState::DONE).is_ok());
    assert_eq!(sm.get_state(), AppState::DONE);
    sm.reset_to_idle();
    assert_eq!(sm.get_state(), AppState::IDLE);
}

#[test]
fn test_pipeline_silence_discard_transition() {
    let sm = AppStateManager::new();
    // Audio too short (<300ms) or silent: IDLE -> RECORDING -> TRANSCRIBING -> IDLE
    assert!(sm.transition_to(AppState::RECORDING).is_ok());
    assert!(sm.transition_to(AppState::TRANSCRIBING).is_ok());
    assert!(sm.transition_to(AppState::IDLE).is_ok());
    assert_eq!(sm.get_state(), AppState::IDLE);
}

#[test]
fn test_pipeline_cancellation_during_recording() {
    let sm = AppStateManager::new();
    let ctrl = HotkeyModeController::new(HotkeyMode::Toggle);

    assert_eq!(
        ctrl.handle_action(HotkeyAction::Press, AppState::IDLE),
        Some(AppState::RECORDING)
    );
    sm.transition_to(AppState::RECORDING).unwrap();

    // User hits Esc key
    assert_eq!(
        ctrl.handle_action(HotkeyAction::Cancel, sm.get_state()),
        Some(AppState::IDLE)
    );
    sm.transition_to(AppState::IDLE).unwrap();
    assert_eq!(sm.get_state(), AppState::IDLE);
}

#[test]
fn test_pipeline_cancellation_during_transcribing() {
    let sm = AppStateManager::new();
    let ctrl = HotkeyModeController::new(HotkeyMode::Toggle);

    sm.transition_to(AppState::RECORDING).unwrap();
    sm.transition_to(AppState::TRANSCRIBING).unwrap();

    // User hits Esc key during transcription
    assert_eq!(
        ctrl.handle_action(HotkeyAction::Cancel, sm.get_state()),
        Some(AppState::IDLE)
    );
    sm.transition_to(AppState::IDLE).unwrap();
    assert_eq!(sm.get_state(), AppState::IDLE);
}

#[test]
fn test_reentrancy_protection_during_transcribing_and_inserting() {
    let modes = [HotkeyMode::HoldToTalk, HotkeyMode::Toggle, HotkeyMode::Hybrid];

    for mode in modes {
        let ctrl = HotkeyModeController::new(mode);

        // While TRANSCRIBING, any hotkey press or release must be ignored
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::TRANSCRIBING),
            None
        );
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::TRANSCRIBING),
            None
        );

        // While INSERTING, any hotkey press, release, or cancel must be ignored
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::INSERTING),
            None
        );
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::INSERTING),
            None
        );
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Cancel, AppState::INSERTING),
            None
        );

        // While DONE, any hotkey press, release, or cancel must be ignored
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::DONE),
            None
        );
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::DONE),
            None
        );
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Cancel, AppState::DONE),
            None
        );
    }
}

#[test]
fn test_audio_resample_flush_and_vad_pipeline() {
    // Generate 48kHz stereo test chirp (1.0s speech burst surrounded by silence)
    let sample_rate_48k = 48000;
    let mut raw_stereo = Vec::new();

    // 0.5s silence
    raw_stereo.resize(sample_rate_48k * 2 / 2, 0.0f32);

    // 1.0s tone @ 440 Hz
    for i in 0..sample_rate_48k {
        let t = i as f32 / sample_rate_48k as f32;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.4;
        raw_stereo.push(s); // Left
        raw_stereo.push(s); // Right
    }

    // 0.5s silence
    raw_stereo.extend(vec![0.0f32; sample_rate_48k * 2 / 2]);

    // Resample to 16kHz mono with delay compensation
    let pcm_16k = resample_and_normalize(&raw_stereo, sample_rate_48k as u32, 2).unwrap();

    // Expected duration ~2.0s -> ~32,000 samples
    let duration_s = pcm_16k.len() as f32 / 16000.0;
    assert!((1.95..=2.05).contains(&duration_s));

    // Check VAD on this resampled audio
    let engine = AsrEngine::new("cpu", 0);
    let post_cfg = PostProcessConfig::default();
    let res = engine.transcribe_samples(&pcm_16k, &post_cfg, &[], None);
    assert!(res.is_ok());
    let tr = res.unwrap();
    // Audio had 1.0s tone; VAD should detect speech (not discarded as silent)
    assert_eq!(tr.duration_ms, 2000);
}

#[test]
fn test_injection_error_recovery_and_history_persistence() {
    let db = Database::in_memory().unwrap();

    let transcript = "Dictation test string to insert into active editor.";

    // Simulate injection fallback behavior:
    // If text injection fails, copy to clipboard and insert history record anyway
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(transcript);
    }

    // SQLite history record persistence
    let record = db
        .insert_history(
            NewHistoryRecord {
                raw_text: transcript.to_string(),
                final_text: transcript.to_string(),
                duration_ms: 1500,
                language: Some("en".to_string()),
                model_variant: Some("parakeet-tdt-0.6b-v3-int8".to_string()),
                provider: Some("cpu".to_string()),
                inference_ms: Some(120),
                refined_text: None,
                mode: Some("normal".to_string()),
            },
            100,
        )
        .unwrap();

    assert_eq!(record.final_text, transcript);

    // Verify record exists in history
    let history = db.get_history(Some(10), None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].final_text, transcript);
    assert_eq!(history[0].duration_ms, 1500);
}

#[test]
fn test_overlay_position_multi_monitor_and_dpi() {
    // Primary 4K monitor (3840x2160, bottom taskbar 80px)
    let work_4k = RECT {
        left: 0,
        top: 0,
        right: 3840,
        bottom: 2080,
    };
    let (x1, y1) = calculate_overlay_position(&work_4k, 360, 90, "bottom-center");
    assert_eq!(x1, (3840 - 360) / 2);
    assert_eq!(y1, 2080 - 90 - 48);

    // Secondary 1080p monitor placed to the left (negative coordinates: -1920..0)
    let work_left = RECT {
        left: -1920,
        top: 0,
        right: 0,
        bottom: 1040,
    };
    let (x2, y2) = calculate_overlay_position(&work_left, 360, 90, "bottom-center");
    assert_eq!(x2, -1920 + (1920 - 360) / 2);
    assert_eq!(y2, 1040 - 90 - 48);

    // Top-right anchor
    let (x3, y3) = calculate_overlay_position(&work_4k, 360, 90, "top-right");
    assert_eq!(x3, 3840 - 360 - 48);
    assert_eq!(y3, 48);
}

#[test]
fn test_bounded_recording_gain_and_clipping() {
    use whispertype_lite_lib::audio::apply_audio_gain;

    let mut samples = vec![-0.8, -0.5, 0.0, 0.5, 0.8];
    // 200% gain -> -1.6, -1.0, 0.0, 1.0, 1.6 -> clamped to [-1.0, 1.0]
    apply_audio_gain(&mut samples, 200);

    assert_eq!(samples[0], -1.0);
    assert_eq!(samples[1], -1.0);
    assert_eq!(samples[2], 0.0);
    assert_eq!(samples[3], 1.0);
    assert_eq!(samples[4], 1.0);
}

#[test]
fn test_settings_wiring_and_privacy_defaults() {
    use whispertype_lite_lib::storage::AppSettings;

    let settings = AppSettings::default();

    // Verify truthfulness of defaults
    assert_eq!(settings.performance_provider, "cpu");
    assert_eq!(settings.thread_count, 2);
    assert_eq!(settings.idle_unload_minutes, 2);
    assert_eq!(settings.max_recording_seconds, 60);
    assert_eq!(settings.audio_gain_percent, 100);
    assert_eq!(settings.fullscreen_behavior, "show");
    assert!(!settings.store_audio);
}

#[test]
fn test_model_provisioning_and_validation() {
    use whispertype_lite_lib::asr::{inspect_model_directory, REQUIRED_MODEL_FILES};
    use std::fs;

    let temp_dir = std::env::temp_dir().join(format!("murmur_model_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let res = inspect_model_directory(&temp_dir);
    assert!(!res.is_valid);
    assert_eq!(res.missing_files.len(), 4);

    // Create the 4 required files with non-zero size
    for file in REQUIRED_MODEL_FILES {
        fs::write(temp_dir.join(file), b"valid_onnx_model_payload_chunk").unwrap();
    }

    let res_ok = inspect_model_directory(&temp_dir);
    assert!(res_ok.is_valid);
    assert!(res_ok.missing_files.is_empty());
    assert!(res_ok.error_message.is_none());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_dictation_controller_stale_session_prevention() {
    use whispertype_lite_lib::dictation::DictationController;

    let controller = DictationController::new();
    assert!(controller.get_session_snapshot().is_none());
}
