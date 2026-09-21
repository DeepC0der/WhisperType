use crate::platform::{HotkeyBackend, HotkeyCallback, PlatformError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SetThreadDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
    HDESK,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_BACK, VK_CONTROL, VK_ESCAPE, VK_F1, VK_F12, VK_LCONTROL, VK_LMENU,
    VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RETURN, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    VK_SPACE, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// Modifier key bitmask flags for hotkey detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HotkeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// Parsed representation of a key combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParsedHotkey {
    pub modifiers: HotkeyModifiers,
    pub vk: u16,
}

impl ParsedHotkey {
    /// Check if this hotkey matches the active keypress and modifier state.
    pub fn matches(&self, vk: u16, current_mods: HotkeyModifiers) -> bool {
        self.vk == vk && self.modifiers == current_mods
    }
}

/// Parse a hotkey combo string (e.g. "Ctrl+Shift+Space", "Alt+Space", "F8", "Ctrl+Alt+S").
pub fn parse_hotkey_string(combo: &str) -> Result<ParsedHotkey, PlatformError> {
    let trimmed = combo.trim();
    if trimmed.is_empty() {
        return Err(PlatformError::HotkeyError(
            "Hotkey string cannot be empty".into(),
        ));
    }

    let parts: Vec<&str> = trimmed.split('+').map(|s| s.trim()).collect();
    let mut modifiers = HotkeyModifiers::default();
    let mut primary_vk: Option<u16> = None;

    for part in parts {
        if part.is_empty() {
            return Err(PlatformError::HotkeyError(format!(
                "Invalid empty token in hotkey string: '{combo}'"
            )));
        }

        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => {
                if modifiers.ctrl {
                    return Err(PlatformError::HotkeyError(format!(
                        "Duplicate Ctrl modifier in: '{combo}'"
                    )));
                }
                modifiers.ctrl = true;
            }
            "alt" | "menu" => {
                if modifiers.alt {
                    return Err(PlatformError::HotkeyError(format!(
                        "Duplicate Alt modifier in: '{combo}'"
                    )));
                }
                modifiers.alt = true;
            }
            "shift" => {
                if modifiers.shift {
                    return Err(PlatformError::HotkeyError(format!(
                        "Duplicate Shift modifier in: '{combo}'"
                    )));
                }
                modifiers.shift = true;
            }
            "win" | "windows" | "super" | "meta" => {
                if modifiers.win {
                    return Err(PlatformError::HotkeyError(format!(
                        "Duplicate Win modifier in: '{combo}'"
                    )));
                }
                modifiers.win = true;
            }
            // Primary trigger keys
            "space" => {
                set_primary_vk(&mut primary_vk, VK_SPACE.0, combo)?;
            }
            "enter" | "return" => {
                set_primary_vk(&mut primary_vk, VK_RETURN.0, combo)?;
            }
            "tab" => {
                set_primary_vk(&mut primary_vk, VK_TAB.0, combo)?;
            }
            "escape" | "esc" => {
                set_primary_vk(&mut primary_vk, VK_ESCAPE.0, combo)?;
            }
            "backspace" => {
                set_primary_vk(&mut primary_vk, VK_BACK.0, combo)?;
            }
            // Function keys F1..F24
            s if s.starts_with('f') && s.len() >= 2 => {
                if let Ok(num) = s[1..].parse::<u16>() {
                    if (1..=12).contains(&num) {
                        set_primary_vk(&mut primary_vk, VK_F1.0 + (num - 1), combo)?;
                    } else if (13..=24).contains(&num) {
                        set_primary_vk(&mut primary_vk, VK_F12.0 + (num - 12), combo)?;
                    } else {
                        return Err(PlatformError::HotkeyError(format!(
                            "Unsupported function key '{part}' in '{combo}'"
                        )));
                    }
                } else {
                    return Err(PlatformError::HotkeyError(format!(
                        "Unknown hotkey key token '{part}' in '{combo}'"
                    )));
                }
            }
            // Single alphanumeric characters A-Z, 0-9
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    let vk = ch.to_ascii_uppercase() as u16;
                    set_primary_vk(&mut primary_vk, vk, combo)?;
                } else if ch.is_ascii_digit() {
                    let vk = ch as u16;
                    set_primary_vk(&mut primary_vk, vk, combo)?;
                } else {
                    return Err(PlatformError::HotkeyError(format!(
                        "Unsupported character key '{part}' in '{combo}'"
                    )));
                }
            }
            _ => {
                return Err(PlatformError::HotkeyError(format!(
                    "Unrecognized hotkey token '{part}' in '{combo}'"
                )));
            }
        }
    }

    match primary_vk {
        Some(vk) => Ok(ParsedHotkey { modifiers, vk }),
        None => Err(PlatformError::HotkeyError(format!(
            "Hotkey string missing primary trigger key: '{combo}'"
        ))),
    }
}

fn set_primary_vk(slot: &mut Option<u16>, vk: u16, combo: &str) -> Result<(), PlatformError> {
    if slot.is_some() {
        return Err(PlatformError::HotkeyError(format!(
            "Multiple primary keys specified in hotkey string: '{combo}'"
        )));
    }
    *slot = Some(vk);
    Ok(())
}

use crate::state::AppState;
use crate::storage::HotkeyMode;
use std::time::{Duration, Instant};

/// Action event dispatched from keyboard hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Press,
    Release,
    Cancel,
}

pub type HotkeyActionCallback = Arc<dyn Fn(HotkeyAction) + Send + Sync + 'static>;

#[derive(Clone)]
struct HotkeyEntry {
    parsed: ParsedHotkey,
    callback: Option<HotkeyCallback>,
    action_callback: Option<HotkeyActionCallback>,
    is_pressed: Arc<AtomicBool>,
}

static GLOBAL_REGISTRY: Mutex<Option<HashMap<String, HotkeyEntry>>> = Mutex::new(None);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static IS_RUNNING: AtomicBool = AtomicBool::new(false);

unsafe fn attach_thread_to_input_desktop() -> Option<HDESK> {
    if let Ok(hdesk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x01FF)) {
        if SetThreadDesktop(hdesk).is_ok() {
            return Some(hdesk);
        }
        let _ = CloseDesktop(hdesk);
    }
    None
}

/// Check if a released virtual key corresponds to the hotkey's primary trigger key or any of its active modifiers.
pub fn is_hotkey_release(parsed: &ParsedHotkey, vk: u16) -> bool {
    if parsed.vk == vk {
        return true;
    }
    if parsed.modifiers.ctrl
        && (vk == VK_CONTROL.0 || vk == VK_LCONTROL.0 || vk == VK_RCONTROL.0)
    {
        return true;
    }
    if parsed.modifiers.shift
        && (vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0)
    {
        return true;
    }
    if parsed.modifiers.alt
        && (vk == VK_MENU.0 || vk == VK_LMENU.0 || vk == VK_RMENU.0)
    {
        return true;
    }
    if parsed.modifiers.win && (vk == VK_LWIN.0 || vk == VK_RWIN.0) {
        return true;
    }
    false
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kbd.vkCode as u16;

        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            if vk == VK_ESCAPE.0 {
                // Global Esc key pressed -> cancel active recording if any and reset pressed states
                if let Ok(guard) = GLOBAL_REGISTRY.lock() {
                    if let Some(map) = guard.as_ref() {
                        for entry in map.values() {
                            entry.is_pressed.store(false, Ordering::SeqCst);
                            if let Some(ref acb) = entry.action_callback {
                                let cb_clone = Arc::clone(acb);
                                std::thread::spawn(move || {
                                    cb_clone(HotkeyAction::Cancel);
                                });
                            }
                        }
                    }
                }
            } else {
                let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
                let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
                let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
                let win = ((GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0)
                    || ((GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0);

                let current_mods = HotkeyModifiers {
                    ctrl,
                    alt,
                    shift,
                    win,
                };

                if let Ok(guard) = GLOBAL_REGISTRY.lock() {
                    if let Some(map) = guard.as_ref() {
                        for entry in map.values() {
                            if entry.parsed.matches(vk, current_mods) {
                                // Prevent typematic auto-repeat multiple firings
                                if !entry.is_pressed.swap(true, Ordering::SeqCst) {
                                    let legacy_cb = entry.callback.clone();
                                    let action_cb = entry.action_callback.clone();
                                    std::thread::spawn(move || {
                                        if let Some(lcb) = legacy_cb {
                                            lcb();
                                        }
                                        if let Some(acb) = action_cb {
                                            acb(HotkeyAction::Press);
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
            }
        } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
            if let Ok(guard) = GLOBAL_REGISTRY.lock() {
                if let Some(map) = guard.as_ref() {
                    for entry in map.values() {
                        // Release when either the primary key or any of the combo's modifier keys is released
                        if is_hotkey_release(&entry.parsed, vk)
                            && entry.is_pressed.swap(false, Ordering::SeqCst)
                        {
                            if let Some(ref acb) = entry.action_callback {
                                let cb_clone = Arc::clone(acb);
                                std::thread::spawn(move || {
                                    cb_clone(HotkeyAction::Release);
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Windows HotkeyBackend implementation using low-level keyboard hook (WH_KEYBOARD_LL) on a dedicated thread.
pub struct WindowsHotkeyBackend {
    registered_combos: HashMap<String, HotkeyEntry>,
    thread_handle: Option<JoinHandle<()>>,
}

impl WindowsHotkeyBackend {
    pub fn new() -> Self {
        Self {
            registered_combos: HashMap::new(),
            thread_handle: None,
        }
    }

    fn sync_registry(&self) {
        if let Ok(mut guard) = GLOBAL_REGISTRY.lock() {
            *guard = Some(self.registered_combos.clone());
        }
    }

    /// Register a hotkey with an action callback handling Press, Release, and Cancel events.
    pub fn register_hotkey_action(
        &mut self,
        combo: &str,
        callback: HotkeyActionCallback,
    ) -> Result<(), PlatformError> {
        let parsed = parse_hotkey_string(combo)?;
        self.registered_combos.insert(
            combo.to_string(),
            HotkeyEntry {
                parsed,
                callback: None,
                action_callback: Some(callback),
                is_pressed: Arc::new(AtomicBool::new(false)),
            },
        );
        self.sync_registry();
        Ok(())
    }
}

impl Default for WindowsHotkeyBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyBackend for WindowsHotkeyBackend {
    fn register_hotkey(
        &mut self,
        combo: &str,
        callback: HotkeyCallback,
    ) -> Result<(), PlatformError> {
        let parsed = parse_hotkey_string(combo)?;
        self.registered_combos.insert(
            combo.to_string(),
            HotkeyEntry {
                parsed,
                callback: Some(callback),
                action_callback: None,
                is_pressed: Arc::new(AtomicBool::new(false)),
            },
        );
        self.sync_registry();
        Ok(())
    }

    fn unregister_hotkey(&mut self, combo: &str) -> Result<(), PlatformError> {
        self.registered_combos.remove(combo);
        self.sync_registry();
        Ok(())
    }

    fn start_listening(&mut self) -> Result<(), PlatformError> {
        if IS_RUNNING.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.sync_registry();

        let (tx_ready, rx_ready) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let _hdesk = unsafe { attach_thread_to_input_desktop() };
            let tid = unsafe { GetCurrentThreadId() };
            HOOK_THREAD_ID.store(tid, Ordering::SeqCst);

            unsafe {
                let mut msg = MSG::default();
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }

            let h_module = match unsafe { GetModuleHandleW(None) } {
                Ok(h) => HINSTANCE(h.0),
                Err(e) => {
                    let _ = tx_ready.send(Err(format!("GetModuleHandleW failed: {e}")));
                    return;
                }
            };

            let hook = match unsafe {
                SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(low_level_keyboard_proc),
                    Some(h_module),
                    0,
                )
            } {
                Ok(h) => h,
                Err(e) => {
                    let _ = tx_ready.send(Err(format!("SetWindowsHookExW failed: {e}")));
                    return;
                }
            };

            IS_RUNNING.store(true, Ordering::SeqCst);
            let _ = tx_ready.send(Ok(()));

            // Win32 message pump
            unsafe {
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                let _ = UnhookWindowsHookEx(hook);
                if let Some(hd) = _hdesk {
                    let _ = CloseDesktop(hd);
                }
            }

            IS_RUNNING.store(false, Ordering::SeqCst);
            HOOK_THREAD_ID.store(0, Ordering::SeqCst);
        });

        match rx_ready.recv() {
            Ok(Ok(())) => {
                self.thread_handle = Some(handle);
                Ok(())
            }
            Ok(Err(e)) => Err(PlatformError::HotkeyError(e)),
            Err(e) => Err(PlatformError::HotkeyError(format!(
                "Hook thread startup communication failed: {e}"
            ))),
        }
    }

    fn stop_listening(&mut self) -> Result<(), PlatformError> {
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        IS_RUNNING.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_listening(&self) -> bool {
        IS_RUNNING.load(Ordering::SeqCst)
    }
}

impl Drop for WindowsHotkeyBackend {
    fn drop(&mut self) {
        let _ = self.stop_listening();
        if let Ok(mut guard) = GLOBAL_REGISTRY.lock() {
            *guard = None;
        }
    }
}

/// Controller mapping hotkey actions (Press, Release, Cancel) to AppState transitions
/// according to the configured HotkeyMode (HoldToTalk, Toggle, Hybrid).
#[derive(Debug)]
pub struct HotkeyModeController {
    mode: HotkeyMode,
    press_time: Mutex<Option<Instant>>,
}

impl HotkeyModeController {
    pub fn new(mode: HotkeyMode) -> Self {
        Self {
            mode,
            press_time: Mutex::new(None),
        }
    }

    pub fn set_mode(&mut self, mode: HotkeyMode) {
        self.mode = mode;
        *self.press_time.lock().unwrap() = None;
    }

    pub fn get_mode(&self) -> HotkeyMode {
        self.mode
    }

    /// Process a hotkey action against the current application state.
    /// Returns Some(next_state) if a valid state transition should occur, or None if no transition is needed.
    pub fn handle_action(&self, action: HotkeyAction, current_state: AppState) -> Option<AppState> {
        match action {
            HotkeyAction::Cancel => {
                // Esc key pressed: cancel recording cleanly and reset to IDLE
                *self.press_time.lock().unwrap() = None;
                if current_state == AppState::RECORDING {
                    Some(AppState::IDLE)
                } else {
                    None
                }
            }
            HotkeyAction::Press => match self.mode {
                HotkeyMode::HoldToTalk => {
                    if current_state == AppState::IDLE {
                        Some(AppState::RECORDING)
                    } else {
                        None
                    }
                }
                HotkeyMode::Toggle => {
                    if current_state == AppState::IDLE {
                        Some(AppState::RECORDING)
                    } else if current_state == AppState::RECORDING {
                        Some(AppState::TRANSCRIBING)
                    } else {
                        None
                    }
                }
                HotkeyMode::Hybrid => {
                    if current_state == AppState::IDLE {
                        *self.press_time.lock().unwrap() = Some(Instant::now());
                        Some(AppState::RECORDING)
                    } else if current_state == AppState::RECORDING {
                        // User pressed again while recording -> toggle off
                        *self.press_time.lock().unwrap() = None;
                        Some(AppState::TRANSCRIBING)
                    } else {
                        None
                    }
                }
            },
            HotkeyAction::Release => match self.mode {
                HotkeyMode::HoldToTalk => {
                    if current_state == AppState::RECORDING {
                        Some(AppState::TRANSCRIBING)
                    } else {
                        None
                    }
                }
                HotkeyMode::Toggle => {
                    // Key release is ignored in Toggle mode
                    None
                }
                HotkeyMode::Hybrid => {
                    if let Some(press_start) = self.press_time.lock().unwrap().take() {
                        if press_start.elapsed() >= Duration::from_millis(300) {
                            // Key was held for >= 300ms -> treat as hold-to-talk release!
                            if current_state == AppState::RECORDING {
                                Some(AppState::TRANSCRIBING)
                            } else {
                                None
                            }
                        } else {
                            // Key was tapped quickly (< 300ms) -> keep recording running in toggle mode
                            None
                        }
                    } else {
                        None
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_parse_valid_hotkeys() {
        let p1 = parse_hotkey_string("Ctrl+Shift+Space").unwrap();
        assert!(p1.modifiers.ctrl);
        assert!(p1.modifiers.shift);
        assert!(!p1.modifiers.alt);
        assert!(!p1.modifiers.win);
        assert_eq!(p1.vk, VK_SPACE.0);

        let p2 = parse_hotkey_string("Alt+Space").unwrap();
        assert!(p2.modifiers.alt);
        assert!(!p2.modifiers.ctrl);
        assert_eq!(p2.vk, VK_SPACE.0);

        let p3 = parse_hotkey_string("F8").unwrap();
        assert_eq!(p3.modifiers, HotkeyModifiers::default());
        assert_eq!(p3.vk, VK_F1.0 + 7);

        let p4 = parse_hotkey_string("Win+Ctrl+A").unwrap();
        assert!(p4.modifiers.win);
        assert!(p4.modifiers.ctrl);
        assert_eq!(p4.vk, 0x41);
    }

    #[test]
    fn test_parse_invalid_hotkeys() {
        assert!(parse_hotkey_string("").is_err());
        assert!(parse_hotkey_string("Ctrl+").is_err());
        assert!(parse_hotkey_string("Ctrl+Alt").is_err());
        assert!(parse_hotkey_string("InvalidToken").is_err());
        assert!(parse_hotkey_string("Ctrl+Ctrl+Space").is_err());
        assert!(parse_hotkey_string("A+B").is_err());
    }

    #[test]
    fn test_modifier_matching_exactness() {
        let hotkey = parse_hotkey_string("Ctrl+Shift+Space").unwrap();

        // Exact match
        assert!(hotkey.matches(
            VK_SPACE.0,
            HotkeyModifiers {
                ctrl: true,
                shift: true,
                alt: false,
                win: false,
            }
        ));

        // Alt is pressed too -> should NOT match (prevent accidental false positives)
        assert!(!hotkey.matches(
            VK_SPACE.0,
            HotkeyModifiers {
                ctrl: true,
                shift: true,
                alt: true,
                win: false,
            }
        ));

        // Wrong VK
        assert!(!hotkey.matches(
            VK_RETURN.0,
            HotkeyModifiers {
                ctrl: true,
                shift: true,
                alt: false,
                win: false,
            }
        ));
    }

    #[test]
    fn test_backend_registration_and_unregistration() {
        let mut backend = WindowsHotkeyBackend::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let cb = {
            let c = Arc::clone(&counter);
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            })
        };

        assert!(backend.register_hotkey("Ctrl+Shift+Space", cb).is_ok());
        assert_eq!(backend.registered_combos.len(), 1);

        // Invalid registration rejected
        assert!(backend
            .register_hotkey("NotAKey", Arc::new(|| {}))
            .is_err());
        assert_eq!(backend.registered_combos.len(), 1);

        // Unregister
        assert!(backend.unregister_hotkey("Ctrl+Shift+Space").is_ok());
        assert_eq!(backend.registered_combos.len(), 0);
    }

    #[test]
    fn test_backend_action_registration() {
        let mut backend = WindowsHotkeyBackend::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let action_cb: HotkeyActionCallback = {
            let c = Arc::clone(&counter);
            Arc::new(move |_act| {
                c.fetch_add(1, Ordering::SeqCst);
            })
        };

        assert!(backend
            .register_hotkey_action("Ctrl+Shift+Space", action_cb)
            .is_ok());
        assert_eq!(backend.registered_combos.len(), 1);

        assert!(backend.unregister_hotkey("Ctrl+Shift+Space").is_ok());
        assert_eq!(backend.registered_combos.len(), 0);
    }

    #[test]
    fn test_hotkey_mode_hold_to_talk() {
        let ctrl = HotkeyModeController::new(HotkeyMode::HoldToTalk);

        // Press from IDLE -> RECORDING
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::IDLE),
            Some(AppState::RECORDING)
        );

        // Repeated press (auto-repeat) while RECORDING -> None
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::RECORDING),
            None
        );

        // Release from RECORDING -> TRANSCRIBING
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::RECORDING),
            Some(AppState::TRANSCRIBING)
        );

        // Release from IDLE -> None
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::IDLE),
            None
        );
    }

    #[test]
    fn test_hotkey_mode_toggle() {
        let ctrl = HotkeyModeController::new(HotkeyMode::Toggle);

        // First press -> RECORDING
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::IDLE),
            Some(AppState::RECORDING)
        );

        // Release in toggle mode is ignored
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::RECORDING),
            None
        );

        // Second press -> TRANSCRIBING
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::RECORDING),
            Some(AppState::TRANSCRIBING)
        );
    }

    #[test]
    fn test_hotkey_mode_hybrid_tap_and_hold() {
        let ctrl = HotkeyModeController::new(HotkeyMode::Hybrid);

        // Case A: Short Tap (< 300ms)
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::IDLE),
            Some(AppState::RECORDING)
        );
        // Instant release -> should remain in RECORDING (toggle mode activated)
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::RECORDING),
            None
        );
        // Second press stops it
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::RECORDING),
            Some(AppState::TRANSCRIBING)
        );

        // Case B: Long Hold (>= 300ms)
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Press, AppState::IDLE),
            Some(AppState::RECORDING)
        );
        // Simulate holding for 350ms by modifying the stored instant
        {
            let mut guard = ctrl.press_time.lock().unwrap();
            *guard = Some(Instant::now() - Duration::from_millis(350));
        }
        // Release after hold -> should stop recording and transition to TRANSCRIBING
        assert_eq!(
            ctrl.handle_action(HotkeyAction::Release, AppState::RECORDING),
            Some(AppState::TRANSCRIBING)
        );
    }

    #[test]
    fn test_hotkey_esc_cancel() {
        let modes = [HotkeyMode::HoldToTalk, HotkeyMode::Toggle, HotkeyMode::Hybrid];
        for mode in modes {
            let ctrl = HotkeyModeController::new(mode);

            // Cancel during RECORDING resets to IDLE
            assert_eq!(
                ctrl.handle_action(HotkeyAction::Cancel, AppState::RECORDING),
                Some(AppState::IDLE)
            );

            // Cancel when not RECORDING does nothing
            assert_eq!(
                ctrl.handle_action(HotkeyAction::Cancel, AppState::IDLE),
                None
            );
            assert_eq!(
                ctrl.handle_action(HotkeyAction::Cancel, AppState::TRANSCRIBING),
                None
            );
        }
    }

    #[test]
    fn test_is_hotkey_release() {
        let hotkey = parse_hotkey_string("Ctrl+Shift+Space").unwrap();

        // Primary key release triggers release
        assert!(is_hotkey_release(&hotkey, VK_SPACE.0));

        // Modifier release triggers release (either generic or left/right)
        assert!(is_hotkey_release(&hotkey, VK_CONTROL.0));
        assert!(is_hotkey_release(&hotkey, VK_LCONTROL.0));
        assert!(is_hotkey_release(&hotkey, VK_RCONTROL.0));
        assert!(is_hotkey_release(&hotkey, VK_SHIFT.0));
        assert!(is_hotkey_release(&hotkey, VK_LSHIFT.0));
        assert!(is_hotkey_release(&hotkey, VK_RSHIFT.0));

        // Unrelated modifier or key does NOT trigger release
        assert!(!is_hotkey_release(&hotkey, VK_MENU.0));
        assert!(!is_hotkey_release(&hotkey, VK_LMENU.0));
        assert!(!is_hotkey_release(&hotkey, VK_LWIN.0));
        assert!(!is_hotkey_release(&hotkey, VK_RETURN.0));
    }
}
