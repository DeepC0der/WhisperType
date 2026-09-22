pub mod asr;
pub mod audio;
pub mod commands;
pub mod logging;
pub mod platform;
pub mod sound;
pub mod state;
pub mod storage;
pub mod tray;

use asr::{AsrEngine, PostProcessConfig};
use audio::{enumerate_input_devices, resample_and_normalize, AudioCapture, AudioDeviceInfo};
use platform::{Autostart, HotkeyBackend, OverlayWindowExt, TextInjector};
use state::{AppState, AppStateManager, StateSnapshot};
use storage::{
    get_default_db_path, get_default_settings_path, AppSettings, Database, DictionaryRecord,
    HistoryRecord, HotkeyMode, InsertionMode, NewDictionaryEntry, NewHistoryRecord, SettingsManager,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(target_os = "windows")]
use platform::windows::{
    HotkeyActionCallback, HotkeyModeController, WindowsAutostart,
    WindowsHotkeyBackend, WindowsTextInjector,
};

#[cfg(not(target_os = "windows"))]
use platform::linux::{LinuxAutostart, LinuxTextInjector};

// ==========================================
// STATE IPC COMMANDS
// ==========================================

#[tauri::command]
fn get_state(state: State<'_, AppStateManager>) -> AppState {
    state.get_state()
}

#[tauri::command]
fn get_state_snapshot(state: State<'_, AppStateManager>) -> StateSnapshot {
    state.get_snapshot()
}

#[tauri::command]
fn transition_state(
    next_state: AppState,
    state: State<'_, AppStateManager>,
    app: AppHandle,
) -> Result<AppState, String> {
    let result = state.transition_to(next_state)?;
    let _ = app.emit("state-changed", state.get_snapshot());
    Ok(result)
}

#[tauri::command]
fn reset_state(state: State<'_, AppStateManager>, app: AppHandle) {
    state.reset_to_idle();
    let _ = app.emit("state-changed", state.get_snapshot());
}

// ==========================================
// WINDOW & INJECTION IPC COMMANDS
// ==========================================

#[tauri::command]
fn apply_overlay_styles(app: AppHandle) -> Result<(), String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        overlay_win
            .apply_overlay_styles()
            .map_err(|e| e.to_string())?;
        log::info!("Applied overlay styles to overlay window successfully.");
    }
    Ok(())
}

#[tauri::command]
fn set_overlay_click_through(enabled: bool, app: AppHandle) -> Result<(), String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        overlay_win
            .set_click_through(enabled)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn position_overlay(anchor: Option<String>, app: AppHandle) -> Result<(), String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        let anchor_str = anchor.unwrap_or_else(|| "bottom-center".to_string());
        overlay_win
            .position_overlay(&anchor_str)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn show_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        overlay_win
            .show_overlay()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn hide_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        overlay_win
            .hide_overlay()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn is_fullscreen_active(app: AppHandle) -> Result<bool, String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        Ok(overlay_win.is_fullscreen_active())
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn inject_text(text: String, mode: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let injector = WindowsTextInjector::new();

    #[cfg(not(target_os = "windows"))]
    let injector = LinuxTextInjector::new();

    match mode.as_deref() {
        Some("unicode") => injector.inject_unicode(&text).map_err(|e| e.to_string()),
        _ => injector.inject_paste(&text).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
fn is_autostart_enabled() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    let autostart = WindowsAutostart::new();

    #[cfg(not(target_os = "windows"))]
    let autostart = LinuxAutostart::new();

    autostart.is_enabled("Murmur").map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let autostart = WindowsAutostart::new();

    #[cfg(not(target_os = "windows"))]
    let autostart = LinuxAutostart::new();

    if enabled {
        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        autostart
            .enable("Murmur", &current_exe.to_string_lossy())
            .map_err(|e| e.to_string())
    } else {
        autostart.disable("Murmur").map_err(|e| e.to_string())
    }
}

// ==========================================
// AUDIO IPC COMMANDS
// ==========================================

#[tauri::command]
fn get_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    enumerate_input_devices().map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AudioCaptureSummary {
    pub sample_count: usize,
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub channels: u16,
}

#[tauri::command]
fn start_audio_capture(
    device_name: Option<String>,
    capture: State<'_, Arc<Mutex<AudioCapture>>>,
    app: AppHandle,
) -> Result<(), String> {
    let mut cap = capture.lock().map_err(|_| "Lock poisoned".to_string())?;
    cap.start_recording(device_name.as_deref(), Some(app))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_audio_capture(
    capture: State<'_, Arc<Mutex<AudioCapture>>>,
) -> Result<AudioCaptureSummary, String> {
    let mut cap = capture.lock().map_err(|_| "Lock poisoned".to_string())?;
    let captured = cap.stop_recording().map_err(|e| e.to_string())?;
    let duration_seconds = (captured.samples.len() as f32)
        / (captured.sample_rate as f32 * captured.channels.max(1) as f32);

    // Resample and normalize to 16,000 Hz mono PCM
    let resampled = resample_and_normalize(
        &captured.samples,
        captured.sample_rate,
        captured.channels,
    )
    .map_err(|e| e.to_string())?;

    Ok(AudioCaptureSummary {
        sample_count: resampled.len(),
        duration_seconds,
        sample_rate: 16000,
        channels: 1,
    })
}

#[tauri::command]
fn cancel_audio_capture(
    capture: State<'_, Arc<Mutex<AudioCapture>>>,
) -> Result<(), String> {
    let mut cap = capture.lock().map_err(|_| "Lock poisoned".to_string())?;
    cap.cancel_recording();
    Ok(())
}

#[tauri::command]
fn start_recording(app: AppHandle) -> Result<(), String> {
    start_dictation_pipeline(&app)
}

#[tauri::command]
fn stop_recording(app: AppHandle) -> Result<(), String> {
    stop_dictation_pipeline(&app)
}

#[tauri::command]
fn cancel_recording(app: AppHandle) -> Result<(), String> {
    cancel_dictation_pipeline(&app)
}

#[tauri::command]
fn toggle_recording(app: AppHandle) -> Result<(), String> {
    toggle_dictation_pipeline(&app)
}

#[tauri::command]
fn play_audio_cue(cue: String, app: AppHandle) -> Result<(), String> {
    crate::sound::play_audio_cue(&app, &cue)
}

// ==========================================
// SETTINGS IPC COMMANDS
// ==========================================

use commands::asr::{benchmark_model, get_model_status, load_model, transcribe_audio_buffer};

#[tauri::command]
fn get_settings(settings: State<'_, SettingsManager>) -> Result<AppSettings, String> {
    Ok(settings.get())
}

#[tauri::command]
fn update_settings(
    new_settings: AppSettings,
    settings: State<'_, SettingsManager>,
    hotkey_service: State<'_, HotkeyService>,
    asr_engine: State<'_, AsrEngine>,
    app: AppHandle,
) -> Result<(), String> {
    let old_settings = settings.get();
    let settings_path = get_default_settings_path();

    settings
        .update_and_save(&settings_path, new_settings.clone())
        .map_err(|e| e.to_string())?;

    let _ = asr_engine.set_requested_provider(&new_settings.performance_provider);
    asr_engine.set_idle_unload_minutes(new_settings.idle_unload_minutes);

    // Update autostart if changed
    if old_settings.launch_on_startup != new_settings.launch_on_startup {
        let _ = set_autostart_enabled(new_settings.launch_on_startup);
    }

    // Rebind hotkey if combo or mode changed
    if old_settings.hotkey != new_settings.hotkey
        || old_settings.hotkey_mode != new_settings.hotkey_mode
    {
        hotkey_service.rebind(
            &app,
            &old_settings.hotkey,
            &new_settings.hotkey,
            new_settings.hotkey_mode,
        )?;
    }

    let _ = app.emit("settings-changed", &new_settings);

    Ok(())
}

// ==========================================
// HISTORY IPC COMMANDS
// ==========================================

#[tauri::command]
fn get_history(
    limit: Option<usize>,
    offset: Option<usize>,
    db: State<'_, Database>,
) -> Result<Vec<HistoryRecord>, String> {
    db.get_history(limit, offset).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_history_entry(id: String, text: String, db: State<'_, Database>, app: AppHandle) -> Result<bool, String> {
    let res = db.update_history(&id, &text).map_err(|e| e.to_string())?;
    crate::tray::update_tray_menu(&app);
    Ok(res)
}

#[tauri::command]
fn delete_history(id: String, db: State<'_, Database>, app: AppHandle) -> Result<bool, String> {
    let res = db.delete_history(&id).map_err(|e| e.to_string())?;
    crate::tray::update_tray_menu(&app);
    Ok(res)
}

#[tauri::command]
fn clear_history(db: State<'_, Database>, app: AppHandle) -> Result<usize, String> {
    let res = db.clear_history().map_err(|e| e.to_string())?;
    crate::tray::update_tray_menu(&app);
    Ok(res)
}

#[tauri::command]
fn export_history(db: State<'_, Database>) -> Result<String, String> {
    db.export_history_json().map_err(|e| e.to_string())
}

#[tauri::command]
fn reinsert_text(text: String, settings: State<'_, SettingsManager>, app: AppHandle) -> Result<(), String> {
    let sm = app.state::<AppStateManager>();
    let _ = sm.transition_to(AppState::INSERTING);
    let _ = app.emit("state-changed", sm.get_snapshot());

    let s = settings.get();
    let inject_mode = match s.insertion_mode {
        InsertionMode::UnicodeTyping => Some("unicode".to_string()),
        InsertionMode::PasteWithRestore => None,
    };

    let res = inject_text(text.clone(), inject_mode);
    if let Err(e) = &res {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(&text);
        }
        log::warn!("Reinsert text failed ({e}), copied to clipboard instead");
    }

    sm.reset_to_idle();
    let _ = app.emit("state-changed", sm.get_snapshot());
    res
}

// ==========================================
// DICTIONARY IPC COMMANDS
// ==========================================

#[tauri::command]
fn get_dictionary(db: State<'_, Database>) -> Result<Vec<DictionaryRecord>, String> {
    db.get_dictionary().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_dictionary_entries(db: State<'_, Database>) -> Result<Vec<DictionaryRecord>, String> {
    db.get_dictionary().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_dictionary_entry(
    entry: NewDictionaryEntry,
    db: State<'_, Database>,
) -> Result<DictionaryRecord, String> {
    db.insert_dictionary(entry).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_dictionary_entry(
    id: Option<String>,
    find_text: String,
    replace_text: String,
    is_case_sensitive: Option<bool>,
    db: State<'_, Database>,
) -> Result<DictionaryRecord, String> {
    if let Some(existing_id) = id {
        let rec = DictionaryRecord {
            id: existing_id,
            find_text,
            replace_text,
            is_case_sensitive: is_case_sensitive.unwrap_or(false),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.update_dictionary(rec.clone()).map_err(|e| e.to_string())?;
        Ok(rec)
    } else {
        db.insert_dictionary(NewDictionaryEntry {
            find_text,
            replace_text,
            is_case_sensitive,
        }).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn update_dictionary_entry(
    entry: DictionaryRecord,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.update_dictionary(entry).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_dictionary_entry(id: String, db: State<'_, Database>) -> Result<bool, String> {
    db.delete_dictionary(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_dictionary(
    entries: Vec<NewDictionaryEntry>,
    db: State<'_, Database>,
) -> Result<usize, String> {
    let mut count = 0;
    for entry in entries {
        if !entry.find_text.trim().is_empty() && db.insert_dictionary(entry).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

// ==========================================
// DIAGNOSTICS & SYSTEM IPC COMMANDS
// ==========================================

#[tauri::command]
fn copy_diagnostics(settings: State<'_, SettingsManager>) -> Result<String, String> {
    let report = logging::generate_diagnostics_report(&settings.get(), &logging::get_default_log_dir());
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(&report);
    }
    Ok(report)
}

#[tauri::command]
fn open_system_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
fn open_log_dir() -> Result<(), String> {
    let dir = logging::get_default_log_dir();
    let dir_str = dir.to_string_lossy().to_string();
    open_system_url(dir_str)
}

// ==========================================
// HOTKEY SERVICE
// ==========================================

pub struct HotkeyService {
    #[cfg(target_os = "windows")]
    backend: Arc<Mutex<WindowsHotkeyBackend>>,
    #[cfg(target_os = "windows")]
    mode_controller: Arc<Mutex<HotkeyModeController>>,
}

pub fn start_dictation_pipeline(app: &AppHandle) -> Result<(), String> {
    let sm = app.state::<AppStateManager>();
    if sm.transition_to(AppState::RECORDING).is_err() {
        return Ok(());
    }
    let _ = app.emit("state-changed", sm.get_snapshot());
    crate::tray::set_tray_recording_state(app, true);

    let settings = app.state::<SettingsManager>().get();

    // 1. Immediately position and show overlay (<50ms target)
    if let Some(w) = app.get_webview_window("overlay") {
        let is_fullscreen = w.is_fullscreen_active();
        let _ = app.emit("fullscreen-active", is_fullscreen);
        let _ = w.position_overlay(&settings.overlay_position);
        let _ = w.show_overlay();
    }

    // 2. Start audio capture
    let capture_arc = app.state::<Arc<Mutex<AudioCapture>>>();
    let mut cap = capture_arc.lock().map_err(|_| "Audio capture lock poisoned".to_string())?;
    if let Err(e) = cap.start_recording(
        settings.audio_device.as_deref(),
        Some(app.clone()),
    ) {
        log::error!("Microphone start error: {e}");
        sm.set_error(e.to_string());
        let _ = app.emit("state-changed", sm.get_snapshot());
        crate::tray::set_tray_recording_state(app, false);
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.hide_overlay();
        }
        return Err(e.to_string());
    }

    if settings.sound_effects_enabled {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = crate::sound::play_audio_cue(&app_clone, "start") {
                log::warn!("Failed to play start audio cue: {e}");
            }
        });
    }

    Ok(())
}

pub fn stop_dictation_pipeline(app: &AppHandle) -> Result<(), String> {
    let sm = app.state::<AppStateManager>();
    if sm.transition_to(AppState::TRANSCRIBING).is_err() {
        return Ok(());
    }
    let _ = app.emit("state-changed", sm.get_snapshot());
    crate::tray::set_tray_recording_state(app, false);

    let capture_arc = app.state::<Arc<Mutex<AudioCapture>>>();
    let mut cap = capture_arc.lock().map_err(|_| "Audio capture lock poisoned".to_string())?;
    match cap.stop_recording() {
        Ok(captured) => {
            let settings = app.state::<SettingsManager>().get();
            if settings.sound_effects_enabled {
                let app_clone = app.clone();
                std::thread::spawn(move || {
                    if let Err(e) = crate::sound::play_audio_cue(&app_clone, "stop") {
                        log::warn!("Failed to play stop audio cue: {e}");
                    }
                });
            }

            let app_clone = app.clone();
            std::thread::Builder::new()
                .name("murmur-transcriber".to_string())
                .spawn(move || {
                    let sm = app_clone.state::<AppStateManager>();
                    let engine = app_clone.state::<AsrEngine>();
                    let settings = app_clone.state::<SettingsManager>().get();
                    let db = app_clone.state::<Database>();

                    // Check cancellation
                    if sm.get_state() != AppState::TRANSCRIBING {
                        return;
                    }

                    // 1. Resample to 16kHz mono PCM with delay compensation
                    let pcm = match resample_and_normalize(
                        &captured.samples,
                        captured.sample_rate,
                        captured.channels,
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            log::error!("Resampling error: {e}");
                            sm.set_error(e.to_string());
                            let _ = app_clone.emit("state-changed", sm.get_snapshot());
                            if let Some(w) = app_clone.get_webview_window("overlay") {
                                let _ = w.hide_overlay();
                            }
                            return;
                        }
                    };

                    if sm.get_state() != AppState::TRANSCRIBING {
                        return;
                    }

                    let post_cfg = PostProcessConfig::from(&settings);
                    let dict = db.get_dictionary().unwrap_or_default();

                    // 2. Transcribe samples (VAD trimming, Parakeet inference, chunking, postprocessing)
                    match engine.transcribe_samples(&pcm, &post_cfg, &dict) {
                        Ok(result) => {
                            if sm.get_state() != AppState::TRANSCRIBING {
                                return;
                            }

                            // Discard silently if speech < 300ms or silent
                            if result.discarded_silent || result.final_text.is_empty() {
                                log::info!("VAD discarded audio (silent or <300ms speech). Resetting to IDLE.");
                                sm.reset_to_idle();
                                let _ = app_clone.emit("state-changed", sm.get_snapshot());
                                if let Some(w) = app_clone.get_webview_window("overlay") {
                                    let _ = w.hide_overlay();
                                }
                                return;
                            }

                            // 3. Emit transcript ready event
                            let _ = app_clone.emit("transcript-ready", &result.final_text);

                            // 4. Perform text injection if auto-insert enabled
                            if settings.auto_insert {
                                if sm.transition_to(AppState::INSERTING).is_ok() {
                                    let _ = app_clone.emit("state-changed", sm.get_snapshot());
                                }

                                let inject_mode = match settings.insertion_mode {
                                    InsertionMode::UnicodeTyping => Some("unicode".to_string()),
                                    InsertionMode::PasteWithRestore => None,
                                };

                                if let Err(e) = inject_text(result.final_text.clone(), inject_mode) {
                                    log::warn!("Text injection failed ({e}). Falling back to clipboard copy.");
                                    if let Ok(mut cb) = arboard::Clipboard::new() {
                                        let _ = cb.set_text(&result.final_text);
                                    }
                                    let _ = app_clone.emit("injection-fallback", serde_json::json!({
                                        "text": &result.final_text,
                                        "error": e,
                                    }));
                                }
                            } else {
                                // Copy to clipboard even if auto_insert is false
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(&result.final_text);
                                }
                            }

                            // 5. Save entry to SQLite history database
                            if settings.persistent_history {
                                let _ = db.insert_history(
                                    NewHistoryRecord {
                                        raw_text: result.raw_text.clone(),
                                        final_text: result.final_text.clone(),
                                        duration_ms: result.duration_ms,
                                        language: Some("en".to_string()),
                                        model_variant: Some(settings.model_variant),
                                        provider: Some(result.provider.clone()),
                                        inference_ms: Some(result.inference_ms),
                                    },
                                    settings.history_retention_max,
                                );
                                crate::tray::update_tray_menu(&app_clone);
                            }

                            // 6. Transition to AppState::DONE and show checkmark on overlay for ~600ms
                            if sm.transition_to(AppState::DONE).is_ok() {
                                let _ = app_clone.emit("state-changed", sm.get_snapshot());
                            }

                            std::thread::sleep(std::time::Duration::from_millis(600));

                            // 7. Hide overlay window completely
                            if let Some(w) = app_clone.get_webview_window("overlay") {
                                let _ = w.hide_overlay();
                            }

                            // 8. Return to AppState::IDLE
                            sm.reset_to_idle();
                            let _ = app_clone.emit("state-changed", sm.get_snapshot());
                        }
                        Err(e) => {
                            log::error!("Transcription error: {e}");
                            sm.set_error(e.to_string());
                            let _ = app_clone.emit("state-changed", sm.get_snapshot());
                            if let Some(w) = app_clone.get_webview_window("overlay") {
                                let _ = w.hide_overlay();
                            }
                        }
                    }
                })
                .expect("Failed to spawn transcriber thread");
        }
        Err(e) => {
            log::error!("Microphone stop error: {e}");
            sm.set_error(e.to_string());
            let _ = app.emit("state-changed", sm.get_snapshot());
            return Err(e.to_string());
        }
    }
    Ok(())
}

pub fn cancel_dictation_pipeline(app: &AppHandle) -> Result<(), String> {
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

pub fn toggle_dictation_pipeline(app: &AppHandle) -> Result<(), String> {
    let sm = app.state::<AppStateManager>();
    match sm.get_state() {
        AppState::RECORDING => stop_dictation_pipeline(app),
        AppState::IDLE | AppState::ERROR => start_dictation_pipeline(app),
        _ => Ok(()),
    }
}

#[cfg(target_os = "windows")]
fn create_hotkey_action_callback(
    app: AppHandle,
    mode_controller: Arc<Mutex<HotkeyModeController>>,
) -> HotkeyActionCallback {
    Arc::new(move |action| {
        let sm = app.state::<AppStateManager>();
        let current = sm.get_state();
        let next_opt = {
            let mc = mode_controller.lock().unwrap();
            mc.handle_action(action, current)
        };

        if let Some(next) = next_opt {
            match next {
                AppState::RECORDING => {
                    let _ = start_dictation_pipeline(&app);
                }
                AppState::TRANSCRIBING => {
                    let _ = stop_dictation_pipeline(&app);
                }
                AppState::IDLE => {
                    let _ = cancel_dictation_pipeline(&app);
                }
                _ => {}
            }
        }
    })
}

impl HotkeyService {
    pub fn new(app_handle: &AppHandle, initial_hotkey: &str, initial_mode: HotkeyMode) -> Self {
        #[cfg(target_os = "windows")]
        {
            let mut backend = WindowsHotkeyBackend::new();
            let mode_controller = Arc::new(Mutex::new(HotkeyModeController::new(initial_mode)));
            let action_cb = create_hotkey_action_callback(app_handle.clone(), Arc::clone(&mode_controller));

            if let Err(e) = backend.register_hotkey_action(initial_hotkey, action_cb) {
                log::error!("Failed to register initial hotkey '{initial_hotkey}': {e}");
            }
            if let Err(e) = backend.start_listening() {
                log::error!("Failed to start hotkey listener thread: {e}");
            }

            Self {
                backend: Arc::new(Mutex::new(backend)),
                mode_controller,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (app_handle, initial_hotkey, initial_mode);
            Self {}
        }
    }

    pub fn rebind(
        &self,
        app_handle: &AppHandle,
        old_hotkey: &str,
        new_hotkey: &str,
        new_mode: HotkeyMode,
    ) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let mut backend = self.backend.lock().map_err(|_| "Backend lock poisoned")?;
            self.mode_controller
                .lock()
                .map_err(|_| "Controller lock poisoned")?
                .set_mode(new_mode);

            let _ = backend.unregister_hotkey(old_hotkey);
            let action_cb = create_hotkey_action_callback(app_handle.clone(), Arc::clone(&self.mode_controller));

            backend
                .register_hotkey_action(new_hotkey, action_cb)
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (app_handle, old_hotkey, new_hotkey, new_mode);
        }
        Ok(())
    }
}

// ==========================================
// APPLICATION LIFECYCLE & RUN ENTRYPOINT
// ==========================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. Initialize logging
    let log_dir = logging::get_default_log_dir();
    let _log_guard = logging::init_logging(&log_dir).ok();

    // 2. Initialize settings
    let settings_path = get_default_settings_path();
    let settings_manager = SettingsManager::load_from_file(&settings_path).unwrap_or_else(|e| {
        log::error!("Failed to load settings from file: {e}. Using defaults.");
        SettingsManager::default()
    });

    // 3. Initialize SQLite database
    let db_path = get_default_db_path();
    let database = Database::open(&db_path).unwrap_or_else(|e| {
        log::error!("Failed to open history.db: {e}. Falling back to in-memory db.");
        Database::in_memory().expect("In-memory DB fallback failed")
    });

    // 4. Audio capture buffer (allocated only during active recording)
    let audio_capture = Arc::new(Mutex::new(AudioCapture::new()));

    // 5. Initialize ASR engine and spawn background loading
    let initial_settings = settings_manager.get();
    let asr_engine = AsrEngine::new(
        &initial_settings.performance_provider,
        initial_settings.idle_unload_minutes,
    );
    asr_engine.spawn_background_load();

    tauri::Builder::default()
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .manage(AppStateManager::new())
        .manage(database)
        .manage(settings_manager.clone())
        .manage(audio_capture)
        .manage(asr_engine)
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_state_snapshot,
            transition_state,
            reset_state,
            apply_overlay_styles,
            set_overlay_click_through,
            position_overlay,
            show_overlay,
            hide_overlay,
            is_fullscreen_active,
            inject_text,
            is_autostart_enabled,
            set_autostart_enabled,
            get_audio_devices,
            start_audio_capture,
            stop_audio_capture,
            cancel_audio_capture,
            start_recording,
            stop_recording,
            cancel_recording,
            toggle_recording,
            play_audio_cue,
            reinsert_text,
            update_history_entry,
            get_dictionary_entries,
            set_dictionary_entry,
            import_dictionary,
            open_system_url,
            open_log_dir,
            get_settings,
            update_settings,
            get_history,
            delete_history,
            clear_history,
            export_history,
            get_dictionary,
            add_dictionary_entry,
            update_dictionary_entry,
            delete_dictionary_entry,
            copy_diagnostics,
            get_model_status,
            load_model,
            transcribe_audio_buffer,
            benchmark_model,
        ])
        .setup(move |app| {
            // Initialize System Tray
            if let Err(e) = tray::init_tray(&app.handle().clone()) {
                log::warn!("Could not initialize system tray: {e}");
            }

            // Apply overlay styles and initial positioning to overlay window if it exists at startup
            if let Some(overlay_window) = app.get_webview_window("overlay") {
                if let Err(e) = overlay_window.apply_overlay_styles() {
                    log::warn!("Could not apply overlay styles on startup: {e}");
                }
                let current_settings = settings_manager.get();
                let _ = overlay_window.position_overlay(&current_settings.overlay_position);
            }

            // Initialize HotkeyService wired to AppState and AudioCapture
            let current_settings = settings_manager.get();
            let hotkey_service = HotkeyService::new(
                &app.handle().clone(),
                &current_settings.hotkey,
                current_settings.hotkey_mode,
            );
            app.manage(hotkey_service);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running murmur tauri application");
}
