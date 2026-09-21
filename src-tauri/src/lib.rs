pub mod platform;
pub mod state;

use platform::{Autostart, OverlayWindowExt, TextInjector};
use state::{AppState, AppStateManager, StateSnapshot};
use tauri::{AppHandle, Manager, State};

#[cfg(target_os = "windows")]
use platform::windows::{WindowsAutostart, WindowsTextInjector};

#[cfg(not(target_os = "windows"))]
use platform::linux::{LinuxAutostart, LinuxTextInjector};

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
) -> Result<AppState, String> {
    state.transition_to(next_state)
}

#[tauri::command]
fn reset_state(state: State<'_, AppStateManager>) {
    state.reset_to_idle();
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppStateManager::new())
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_state_snapshot,
            transition_state,
            reset_state,
            apply_overlay_styles,
            inject_text,
            is_autostart_enabled,
            set_autostart_enabled
        ])
        .setup(|app| {
            // Apply overlay styles to overlay window if it exists at startup
            if let Some(overlay_window) = app.get_webview_window("overlay") {
                if let Err(e) = overlay_window.apply_overlay_styles() {
                    log::warn!("Could not apply overlay styles on startup: {e}");
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running murmur tauri application");
}
