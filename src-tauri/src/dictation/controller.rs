use crate::asr::{AsrEngine, PostProcessConfig};
use crate::audio::{resample_and_normalize, AudioCapture, CaptureConfig, CapturedAudio};
use crate::llm::LmStudioProvider;
use crate::platform::OverlayWindowExt;
use crate::state::{AppState, AppStateManager};
use crate::storage::{Database, InsertionMode, NewHistoryRecord, SettingsManager};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "windows")]
use crate::platform::windows::WindowsTextInjector;
#[cfg(not(target_os = "windows"))]
use crate::platform::linux::LinuxTextInjector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationMode {
    Normal,
    Professional,
}

#[derive(Debug, Clone)]
pub struct DictationSessionSnapshot {
    pub session_id: u64,
    pub mode: DictationMode,
    pub is_cancelled: bool,
}

struct ActiveSession {
    id: u64,
    mode: DictationMode,
    cancel_token: Arc<AtomicBool>,
    _started_at: Instant,
}

/// Helper to verify whether an HTTP endpoint is purely loopback (localhost / 127.0.0.1 / ::1).
pub fn is_loopback_url(url_str: &str) -> bool {
    let clean = url_str.trim().to_lowercase();
    let without_proto = clean
        .strip_prefix("http://")
        .or_else(|| clean.strip_prefix("https://"))
        .unwrap_or(&clean);
    let host_port = without_proto.split('/').next().unwrap_or("");
    let host = if host_port.starts_with('[') {
        if let Some(end_bracket) = host_port.find(']') {
            &host_port[1..end_bracket]
        } else {
            host_port
        }
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    host == "localhost" || host == "127.0.0.1" || host == "::1" || host.starts_with("127.")
}

fn schedule_error_auto_reset(app: &AppHandle) {
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(3000));
        let sm = app_clone.state::<AppStateManager>();
        if sm.get_state() == AppState::ERROR {
            log::info!("Self-healing error reset: returning to IDLE after 3 seconds");
            sm.reset_to_idle();
            let _ = app_clone.emit("state-changed", sm.get_snapshot());
            if let Some(w) = app_clone.get_webview_window("overlay") {
                let _ = w.hide_overlay();
            }
        }
    });
}

/// Dedicated coordinator orchestrating audio capture, ASR transcription, LLM refinement, text injection, and state transitions.
pub struct DictationController {
    next_session_id: AtomicU64,
    active_session: Mutex<Option<ActiveSession>>,
}

impl Default for DictationController {
    fn default() -> Self {
        Self::new()
    }
}

impl DictationController {
    pub fn new() -> Self {
        Self {
            next_session_id: AtomicU64::new(1),
            active_session: Mutex::new(None),
        }
    }

    /// Retrieve snapshot of active session for diagnostics/testing.
    pub fn get_session_snapshot(&self) -> Option<DictationSessionSnapshot> {
        let guard = self.active_session.lock().unwrap();
        guard.as_ref().map(|s| DictationSessionSnapshot {
            session_id: s.id,
            mode: s.mode,
            is_cancelled: s.cancel_token.load(Ordering::SeqCst),
        })
    }

    /// Start audio recording pipeline in either Normal or Professional mode.
    pub fn start(&self, app: &AppHandle, mode: DictationMode) -> Result<(), String> {
        // Release any active live mic test probe before acquiring recording hardware
        if let Some(probe) = app.try_state::<Arc<Mutex<crate::audio::AudioMonitorProbe>>>() {
            if let Ok(mut p) = probe.lock() {
                let _ = p.stop(app);
            }
        }

        let sm = app.state::<AppStateManager>();
        if sm.transition_to(AppState::RECORDING).is_err() {
            return Ok(());
        }

        let session_id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let cancel_token = Arc::new(AtomicBool::new(false));

        {
            let mut guard = self.active_session.lock().unwrap();
            // If previous session was still around, cancel it
            if let Some(prev) = guard.take() {
                prev.cancel_token.store(true, Ordering::SeqCst);
            }
            *guard = Some(ActiveSession {
                id: session_id,
                mode,
                cancel_token: cancel_token.clone(),
                _started_at: Instant::now(),
            });
        }

        let mode_str = match mode {
            DictationMode::Professional => "professional",
            DictationMode::Normal => "normal",
        };
        let _ = app.emit("dictation-mode-changed", mode_str);
        let _ = app.emit("state-changed", sm.get_snapshot());
        crate::tray::set_tray_recording_state(app, true);

        let settings = app.state::<SettingsManager>().get();

        // 1. Overlay positioning and fullscreen behavior (<50ms target)
        if let Some(w) = app.get_webview_window("overlay") {
            let is_fullscreen = w.is_fullscreen_active();
            let _ = app.emit("fullscreen-active", is_fullscreen);

            if is_fullscreen && settings.fullscreen_behavior == "hide" {
                let _ = w.hide_overlay();
            } else {
                let _ = w.position_overlay_with_size(
                    &settings.overlay_position,
                    &settings.overlay_size,
                );
                let _ = w.show_overlay();
            }
        }

        // 2. Start bounded audio capture with gain and memory limits
        let capture_arc = app.state::<Arc<Mutex<AudioCapture>>>();
        let mut cap = capture_arc
            .lock()
            .map_err(|_| "Audio capture lock poisoned".to_string())?;

        let max_recording_seconds = match mode {
            DictationMode::Professional => settings.professional_max_recording_seconds.clamp(10, 600),
            DictationMode::Normal => settings.max_recording_seconds.clamp(5, 600),
        };

        let capture_config = CaptureConfig {
            device_name: settings.audio_device.clone(),
            max_recording_seconds,
            audio_gain_percent: settings.audio_gain_percent,
        };

        if let Err(e) = cap.start_recording_with_config(capture_config, Some(app.clone())) {
            log::error!("Microphone start error: {e}");
            cancel_token.store(true, Ordering::SeqCst);
            sm.set_error(e.to_string());
            schedule_error_auto_reset(app);
            let _ = app.emit("state-changed", sm.get_snapshot());
            crate::tray::set_tray_recording_state(app, false);
            if let Some(w) = app.get_webview_window("overlay") {
                let _ = w.hide_overlay();
            }
            return Err(e.to_string());
        }

        // 3. Audio cue
        if settings.sound_effects_enabled {
            if let Err(e) = crate::sound::play_audio_cue(app, "start") {
                log::warn!("Failed to play start audio cue: {e}");
            }
        }

        Ok(())
    }

    /// Stop audio capture and trigger transcription / text injection background thread.
    pub fn stop(&self, app: &AppHandle) -> Result<(), String> {
        let sm = app.state::<AppStateManager>();
        if sm.transition_to(AppState::TRANSCRIBING).is_err() {
            return Ok(());
        }
        let _ = app.emit("state-changed", sm.get_snapshot());
        crate::tray::set_tray_recording_state(app, false);

        let (session_id, mode, cancel_token) = {
            let guard = self.active_session.lock().unwrap();
            match guard.as_ref() {
                Some(s) => (s.id, s.mode, s.cancel_token.clone()),
                None => return Ok(()),
            }
        };

        let capture_arc = app.state::<Arc<Mutex<AudioCapture>>>();
        let mut cap = capture_arc
            .lock()
            .map_err(|_| "Audio capture lock poisoned".to_string())?;

        match cap.stop_recording() {
            Ok(captured) => {
                let settings = app.state::<SettingsManager>().get();
                if settings.sound_effects_enabled {
                    if let Err(e) = crate::sound::play_audio_cue(app, "stop") {
                        log::warn!("Failed to play stop audio cue: {e}");
                    }
                }

                let app_clone = app.clone();
                std::thread::Builder::new()
                    .name("murmur-pipeline-worker".to_string())
                    .spawn(move || {
                        run_worker_pipeline(session_id, cancel_token, mode, captured, app_clone);
                    })
                    .expect("Failed to spawn pipeline worker thread");
            }
            Err(e) => {
                log::error!("Audio capture stop error: {e}");
                cancel_token.store(true, Ordering::SeqCst);
                sm.set_error(e.to_string());
                schedule_error_auto_reset(app);
                let _ = app.emit("state-changed", sm.get_snapshot());
                return Err(e.to_string());
            }
        }

        Ok(())
    }

    /// Cancel the active dictation session immediately without injecting text.
    pub fn cancel(&self, app: &AppHandle) -> Result<(), String> {
        if let Some(session) = self.active_session.lock().unwrap().take() {
            session.cancel_token.store(true, Ordering::SeqCst);
        }

        let sm = app.state::<AppStateManager>();
        sm.reset_to_idle();
        let _ = app.emit("state-changed", sm.get_snapshot());
        crate::tray::set_tray_recording_state(app, false);

        crate::sound::stop_audio_cues();

        let capture_arc = app.state::<Arc<Mutex<AudioCapture>>>();
        if let Ok(mut cap) = capture_arc.lock() {
            cap.cancel_recording();
        }

        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.hide_overlay();
        }

        Ok(())
    }

    /// Toggle dictation on hotkey press.
    pub fn toggle(&self, app: &AppHandle) -> Result<(), String> {
        let sm = app.state::<AppStateManager>();
        match sm.get_state() {
            AppState::RECORDING => self.stop(app),
            AppState::IDLE | AppState::ERROR => self.start(app, DictationMode::Normal),
            _ => Ok(()),
        }
    }
}

/// The core background worker running ASR, optional local LLM refinement, injection, and persistence.
fn run_worker_pipeline(
    session_id: u64,
    cancel_token: Arc<AtomicBool>,
    mode: DictationMode,
    captured: CapturedAudio,
    app: AppHandle,
) {
    let is_cancelled = || -> bool {
        cancel_token.load(Ordering::SeqCst)
    };

    let sm = app.state::<AppStateManager>();
    let engine = app.state::<AsrEngine>();
    let settings = app.state::<SettingsManager>().get();
    let db = app.state::<Database>();

    if is_cancelled() {
        log::info!("Session {session_id} was cancelled before processing. Terminating.");
        return;
    }

    // 1. Resample to 16kHz mono PCM
    let pcm = match resample_and_normalize(
        &captured.samples,
        captured.sample_rate,
        captured.channels,
    ) {
        Ok(p) => p,
        Err(e) => {
            if is_cancelled() {
                return;
            }
            log::error!("Resampling error in session {session_id}: {e}");
            sm.set_error(e.to_string());
            schedule_error_auto_reset(&app);
            let _ = app.emit("state-changed", sm.get_snapshot());
            if let Some(w) = app.get_webview_window("overlay") {
                let _ = w.hide_overlay();
            }
            return;
        }
    };

    if is_cancelled() {
        return;
    }

    // 2. Transcribe with Parakeet TDT INT8 model + dictionary
    let post_cfg = PostProcessConfig::from(&settings);
    let dict = db.get_dictionary().unwrap_or_default();

    let asr_result = match engine.transcribe_samples(&pcm, &post_cfg, &dict, None) {
        Ok(res) => res,
        Err(e) => {
            if is_cancelled() {
                return;
            }
            log::error!("Transcription error in session {session_id}: {e}");
            sm.set_error(e.to_string());
            schedule_error_auto_reset(&app);
            let _ = app.emit("state-changed", sm.get_snapshot());
            if let Some(w) = app.get_webview_window("overlay") {
                let _ = w.hide_overlay();
            }
            return;
        }
    };

    if is_cancelled() {
        return;
    }

    // Discard silent audio (<300ms speech or empty)
    if asr_result.discarded_silent || asr_result.final_text.is_empty() {
        log::info!("VAD discarded audio (silent or <300ms speech) in session {session_id}. Resetting to IDLE.");
        sm.reset_to_idle();
        let _ = app.emit("state-changed", sm.get_snapshot());
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.hide_overlay();
        }
        return;
    }

    let raw_transcript = asr_result.final_text.clone();

    // 3. Optional Professional Mode (Local LLM Refinement)
    let (final_text_to_inject, refined_text, record_mode) = if mode == DictationMode::Professional {
        if sm.transition_to(AppState::REFINING).is_ok() {
            let _ = app.emit("state-changed", sm.get_snapshot());
        }

        if is_cancelled() {
            return;
        }

        // Loopback / local endpoint privacy validation
        if !is_loopback_url(&settings.lm_studio_url) && !settings.lm_studio_allow_remote {
            log::warn!("LM Studio URL {} is not loopback. Refusing to send transcript over network without confirmation.", settings.lm_studio_url);
            let _ = app.emit(
                "professional-error",
                "Non-loopback LM Studio URL rejected: Murmur offline privacy guarantee restricts LLM calls to local machine (localhost/127.0.0.1). Enable 'Allow Remote Endpoint' in Settings to override.".to_string(),
            );
            (raw_transcript.clone(), None, Some("professional".to_string()))
        } else {
            let llm_provider = LmStudioProvider::new(settings.lm_studio_url.clone());
            let req = crate::llm::LlmRequest {
                model_id: settings.lm_studio_model_id.clone(),
                system_prompt: crate::llm::prompt::PROFESSIONAL_SYSTEM_PROMPT.to_string(),
                user_content: format!(
                    "Transform this spoken recall transcript into structured Markdown notes (`##` headings, `###` subheadings, `-` bullet points). CRITICAL: Apply the Speech Disfluency & Backtracking Rule—if the speaker corrects themselves either immediately or later in the transcript, completely discard the initial mistaken/retracted statement and only record the final corrected assertion as ground truth:\n\n{}",
                    raw_transcript
                ),
                temperature: settings.lm_studio_temperature,
                timeout_secs: settings.lm_studio_timeout_seconds as u64,
            };

            let rt_result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            match rt_result {
                Ok(rt) => match rt.block_on(llm_provider.generate(req)) {
                    Ok(resp) => {
                        log::info!("Professional Mode refined: \"{raw_transcript}\" -> \"{}\"", resp.text);
                        (resp.text.clone(), Some(resp.text), Some("professional".to_string()))
                    }
                    Err(e) => {
                        log::warn!("Professional refinement failed ({e}). Falling back to raw transcript.");
                        let _ = app.emit("professional-error", e.to_string());
                        (raw_transcript.clone(), None, Some("professional".to_string()))
                    }
                },
                Err(e) => {
                    log::warn!("Failed to initialize tokio runtime for LLM ({e}). Falling back to raw transcript.");
                    (raw_transcript.clone(), None, Some("professional".to_string()))
                }
            }
        }
    } else {
        (raw_transcript.clone(), None, Some("normal".to_string()))
    };

    if is_cancelled() {
        return;
    }

    // 4. Safe Text Injection
    if settings.auto_insert {
        if sm.transition_to(AppState::INSERTING).is_ok() {
            let _ = app.emit("state-changed", sm.get_snapshot());
        }

        if is_cancelled() {
            return;
        }

        #[cfg(target_os = "windows")]
        let injector = WindowsTextInjector::new();
        #[cfg(not(target_os = "windows"))]
        let injector = LinuxTextInjector::new();

        let inject_mode_str = match settings.insertion_mode {
            InsertionMode::UnicodeTyping => Some("unicode"),
            InsertionMode::PasteWithRestore => None,
        };

        #[cfg(target_os = "windows")]
        let inj_res = injector.inject_text_with_diagnostics(&final_text_to_inject, inject_mode_str);
        #[cfg(not(target_os = "windows"))]
        let inj_res = match inject_mode_str {
            Some("unicode") => injector.inject_unicode(&final_text_to_inject),
            _ => injector.inject_paste(&final_text_to_inject),
        };

        if let Err(e) = inj_res {
            log::warn!("Text injection failed ({e}). Falling back to clipboard copy.");
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(&final_text_to_inject);
            }
            let _ = app.emit(
                "injection-fallback",
                serde_json::json!({
                    "text": &final_text_to_inject,
                    "error": e.to_string(),
                }),
            );
        }
    } else {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(&final_text_to_inject);
        }
    }

    if is_cancelled() {
        return;
    }

    // 5. Database History Persistence
    let mut history_id = None;
    if settings.persistent_history {
        let new_rec = NewHistoryRecord {
            raw_text: asr_result.raw_text.clone(),
            final_text: final_text_to_inject.clone(),
            duration_ms: asr_result.duration_ms,
            language: Some("en".to_string()),
            model_variant: Some(settings.model_variant.clone()),
            provider: Some(asr_result.provider.clone()),
            inference_ms: Some(asr_result.inference_ms),
            refined_text,
            mode: record_mode,
        };

        if let Ok(saved) = db.insert_history(new_rec, settings.history_retention_max) {
            history_id = Some(saved.id);
            crate::tray::update_tray_menu(&app);
        }
    }

    // Emit transcript event once after DB persistence so frontend history & scratchpad update cleanly
    if mode == DictationMode::Professional {
        let _ = app.emit("professional-transcript-ready", &final_text_to_inject);
    } else {
        let _ = app.emit("transcript-ready", &final_text_to_inject);
    }

    // 6. Optional Audio Archival
    if settings.store_audio {
        let rec_id = history_id.unwrap_or_else(|| format!("audio_{}", uuid::Uuid::new_v4()));
        let rec_dir = crate::storage::get_default_recordings_dir();
        let _ = crate::storage::save_recording_file(&rec_dir, &rec_id, &pcm);

        if let Ok(recent) = db.get_history(Some(settings.history_retention_max), None) {
            let active_ids: Vec<String> = recent.into_iter().map(|r| r.id).collect();
            let _ = crate::storage::prune_unreferenced_recordings(&rec_dir, &active_ids);
        }
    }

    if is_cancelled() {
        return;
    }

    // 7. Transition to DONE (checkmark display on overlay)
    if sm.transition_to(AppState::DONE).is_ok() {
        let _ = app.emit("state-changed", sm.get_snapshot());
    }

    std::thread::sleep(std::time::Duration::from_millis(600));

    if is_cancelled() {
        return;
    }

    // 8. Hide overlay and return to IDLE
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide_overlay();
    }

    sm.reset_to_idle();
    let _ = app.emit("state-changed", sm.get_snapshot());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_loopback_url() {
        assert!(is_loopback_url("http://localhost:1234/v1"));
        assert!(is_loopback_url("http://127.0.0.1:1234/v1"));
        assert!(is_loopback_url("http://127.0.0.5:8080"));
        assert!(is_loopback_url("http://[::1]:1234/v1"));
        assert!(is_loopback_url("localhost:1234"));

        assert!(!is_loopback_url("http://192.168.1.50:1234/v1"));
        assert!(!is_loopback_url("https://api.openai.com/v1"));
        assert!(!is_loopback_url("http://example.com/api"));
    }

    #[test]
    fn test_controller_session_monotonic_and_cancellation() {
        let controller = DictationController::new();
        assert!(controller.get_session_snapshot().is_none());

        let id1 = controller.next_session_id.fetch_add(1, Ordering::SeqCst);
        let id2 = controller.next_session_id.fetch_add(1, Ordering::SeqCst);
        assert!(id2 > id1);

        let cancel_token = Arc::new(AtomicBool::new(false));
        {
            let mut guard = controller.active_session.lock().unwrap();
            *guard = Some(ActiveSession {
                id: id1,
                mode: DictationMode::Normal,
                cancel_token: cancel_token.clone(),
                _started_at: Instant::now(),
            });
        }

        let snap = controller.get_session_snapshot().unwrap();
        assert_eq!(snap.session_id, id1);
        assert!(!snap.is_cancelled);

        // Cancel
        cancel_token.store(true, Ordering::SeqCst);
        let snap2 = controller.get_session_snapshot().unwrap();
        assert!(snap2.is_cancelled);
    }
}
