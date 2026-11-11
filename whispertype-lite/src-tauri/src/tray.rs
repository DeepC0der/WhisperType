use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use crate::storage::Database;

pub const TRAY_ID: &str = "murmur-tray";

pub fn get_default_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/32x32.png")).expect("Failed to load 32x32.png")
}

pub fn get_recording_icon() -> Image<'static> {
    let base = get_default_icon();
    let mut red_rgba = base.rgba().to_vec();
    for pixel in red_rgba.chunks_exact_mut(4) {
        if pixel[3] > 10 {
            pixel[0] = 239;
            pixel[1] = 68;
            pixel[2] = 68;
        }
    }
    Image::new_owned(red_rgba, base.width(), base.height())
}

pub fn build_tray_menu(app: &AppHandle, is_recording: bool) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let menu = Menu::new(app)?;

    let dictation_label = if is_recording {
        "Stop Dictation"
    } else {
        "Start Dictation"
    };
    let start_stop = MenuItem::with_id(app, "toggle_dictation", dictation_label, true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Open WhisperType", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    menu.append(&start_stop)?;
    menu.append(&open_item)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    // Recent 5 entries submenu
    let recent_menu = Submenu::with_id(app, "recent", "Recent (last 5)", true)?;
    if let Some(db) = app.try_state::<Database>() {
        if let Ok(entries) = db.get_history(Some(5), None) {
            if entries.is_empty() {
                let empty_item = MenuItem::with_id(app, "recent_empty", "(No recent dictations)", false, None::<&str>)?;
                recent_menu.append(&empty_item)?;
            } else {
                for (idx, entry) in entries.iter().enumerate() {
                    let mut snippet = entry.final_text.replace(['\r', '\n'], " ");
                    if snippet.chars().count() > 32 {
                        let truncated: String = snippet.chars().take(30).collect();
                        snippet = format!("{}...", truncated.trim());
                    }
                    let item_label = format!("{}. {}", idx + 1, snippet);
                    let item_id = format!("copy_recent:{}", entry.id);
                    let item = MenuItem::with_id(app, item_id, item_label, true, None::<&str>)?;
                    recent_menu.append(&item)?;
                }
            }
        }
    }
    menu.append(&recent_menu)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit_item)?;

    Ok(menu)
}

pub fn init_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let default_icon = get_default_icon();
    let menu = build_tray_menu(app, false)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(default_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app_handle, event| {
            let event_id = event.id.as_ref();
            match event_id {
                "toggle_dictation" => {
                    let _ = crate::toggle_dictation_pipeline(app_handle);
                }
                "open" => {
                    if let Some(w) = app_handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.unminimize();
                        let _ = w.set_focus();
                    }
                }
                "quit" => {
                    app_handle.exit(0);
                }
                id if id.starts_with("copy_recent:") => {
                    let entry_id = &id["copy_recent:".len()..];
                    if let Some(db) = app_handle.try_state::<Database>() {
                        if let Ok(entries) = db.get_history(Some(10), None) {
                            if let Some(entry) = entries.iter().find(|e| e.id == entry_id) {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(&entry.final_text);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

pub fn set_tray_recording_state(app: &AppHandle, is_recording: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let icon = if is_recording {
            get_recording_icon()
        } else {
            get_default_icon()
        };
        let _ = tray.set_icon(Some(icon));

        if let Ok(menu) = build_tray_menu(app, is_recording) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

pub fn update_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let is_recording = {
            if let Some(sm) = app.try_state::<crate::state::AppStateManager>() {
                sm.get_state() == crate::state::AppState::RECORDING
            } else {
                false
            }
        };
        if let Ok(menu) = build_tray_menu(app, is_recording) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}
