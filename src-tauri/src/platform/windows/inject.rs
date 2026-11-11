use crate::platform::{PlatformError, TextInjector};
use arboard::Clipboard;
use std::borrow::Cow;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SetThreadDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
    HDESK,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VK_CONTROL, VIRTUAL_KEY,
};

const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);
const CLIPBOARD_RETRY_COUNT: usize = 5;
const CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(20);

unsafe fn attach_thread_to_input_desktop() -> Option<HDESK> {
    if let Ok(hdesk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x01FF)) {
        if SetThreadDesktop(hdesk).is_ok() {
            return Some(hdesk);
        }
        let _ = CloseDesktop(hdesk);
    }
    None
}

/// Attempt to open system clipboard with retries in case another process currently holds the lock.
fn open_clipboard_with_retry(max_retries: usize, delay: Duration) -> Result<Clipboard, PlatformError> {
    let mut last_err = None;
    for attempt in 0..max_retries {
        if attempt > 0 {
            sleep(delay);
        }
        match Clipboard::new() {
            Ok(cb) => return Ok(cb),
            Err(e) => {
                last_err = Some(e);
            }
        }
    }
    Err(PlatformError::ClipboardError(format!(
        "Failed to open clipboard after {max_retries} attempts (locked by another process): {:?}",
        last_err
    )))
}

/// Attempt to set clipboard text with retries.
fn set_clipboard_text_with_retry(
    clipboard: &mut Clipboard,
    text: &str,
    max_retries: usize,
    delay: Duration,
) -> Result<(), PlatformError> {
    let mut last_err = None;
    for attempt in 0..max_retries {
        if attempt > 0 {
            sleep(delay);
        }
        match clipboard.set_text(text) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
            }
        }
    }
    Err(PlatformError::ClipboardError(format!(
        "Failed to write text to clipboard after {max_retries} attempts: {:?}",
        last_err
    )))
}

/// Snapshot of clipboard content before injection for complete restoration.
enum SavedClipboard {
    Text(String),
    Image(arboard::ImageData<'static>),
    Empty,
}

/// Check if foreground target window belongs to an elevated (administrator) process,
/// which triggers Windows User Interface Privilege Isolation (UIPI) blocking standard SendInput.
pub fn is_foreground_window_elevated() -> bool {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        let mut pid = 0u32;
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return false;
        }

        let proc_res = windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        );
        let Ok(proc) = proc_res else {
            // Access denied opening process info almost always indicates higher integrity/elevated process
            return true;
        };

        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        let mut token = windows::Win32::Foundation::HANDLE::default();
        let elevated = if windows::Win32::System::Threading::OpenProcessToken(proc, TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut ret_len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret_len,
            );
            let _ = windows::Win32::Foundation::CloseHandle(token);
            ok.is_ok() && elevation.TokenIsElevated != 0
        } else {
            false
        };

        let _ = windows::Win32::Foundation::CloseHandle(proc);
        elevated
    }
}

/// Windows implementation of TextInjector utilizing Win32 SendInput and arboard.
pub struct WindowsTextInjector;

impl WindowsTextInjector {
    pub fn new() -> Self {
        Self
    }

    /// Primary paste injection with automatic fallback to Unicode typing, plus UIPI elevation diagnostics.
    pub fn inject_text_with_diagnostics(
        &self,
        text: &str,
        preferred_mode: Option<&str>,
    ) -> Result<(), PlatformError> {
        let is_unicode = preferred_mode == Some("unicode");

        let first_res = if is_unicode {
            self.inject_unicode(text)
        } else {
            self.inject_paste(text)
        };

        if first_res.is_ok() {
            return Ok(());
        }

        log::warn!(
            "First injection strategy ({}) failed: {:?}. Attempting fallback...",
            if is_unicode { "Unicode" } else { "Paste" },
            first_res
        );

        // Fallback to alternate strategy
        let fallback_res = if is_unicode {
            self.inject_paste(text)
        } else {
            self.inject_unicode(text)
        };

        if fallback_res.is_ok() {
            return Ok(());
        }

        // Both strategies failed: diagnose if UIPI elevation is blocking SendInput
        if is_foreground_window_elevated() {
            Err(PlatformError::OsError(
                "Windows UIPI blocked text injection: the active foreground window is running with Administrator privileges. Standard applications cannot send input to elevated windows. Run Murmur as Administrator to dictate into elevated apps.".to_string(),
            ))
        } else {
            Err(PlatformError::OsError(
                "Text injection failed: SendInput could not deliver keystrokes to the active target application.".to_string(),
            ))
        }
    }
}

impl Default for WindowsTextInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInjector for WindowsTextInjector {
    fn inject_paste(&self, text: &str) -> Result<(), PlatformError> {
        let mut clipboard = open_clipboard_with_retry(CLIPBOARD_RETRY_COUNT, CLIPBOARD_RETRY_DELAY)?;

        // 1. Save original clipboard contents (text or image)
        let saved_content = if let Ok(orig_text) = clipboard.get_text() {
            SavedClipboard::Text(orig_text)
        } else if let Ok(orig_img) = clipboard.get_image() {
            SavedClipboard::Image(arboard::ImageData {
                width: orig_img.width,
                height: orig_img.height,
                bytes: Cow::Owned(orig_img.bytes.into_owned()),
            })
        } else {
            SavedClipboard::Empty
        };

        // 2. Set injection text to clipboard with retries
        set_clipboard_text_with_retry(&mut clipboard, text, CLIPBOARD_RETRY_COUNT, CLIPBOARD_RETRY_DELAY)?;

        // 3. Emulate Ctrl+V via SendInput attached to input desktop
        let send_result = std::thread::spawn(|| {
            let _hdesk = unsafe { attach_thread_to_input_desktop() };
            let inputs = [
                // Ctrl Down
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_CONTROL,
                            wScan: 0,
                            dwFlags: KEYBD_EVENT_FLAGS(0),
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // V Down
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_V,
                            wScan: 0,
                            dwFlags: KEYBD_EVENT_FLAGS(0),
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // V Up
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_V,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // Ctrl Up
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_CONTROL,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];

            let sz = std::mem::size_of::<INPUT>() as i32;
            let sent = unsafe { SendInput(&inputs, sz) };
            if let Some(hd) = _hdesk {
                let _ = unsafe { CloseDesktop(hd) };
            }
            sent
        })
        .join()
        .map_err(|_| PlatformError::OsError("SendInput worker thread panicked".into()))?;

        if send_result == 0 {
            return Err(PlatformError::OsError(
                "SendInput for Ctrl+V keystroke returned 0 (failed)".into(),
            ));
        }

        // 4. Delay ~120ms to allow target window to process paste event (Decision D10)
        sleep(Duration::from_millis(120));

        // 5. Restore original clipboard contents
        if let Ok(mut restore_cb) = open_clipboard_with_retry(CLIPBOARD_RETRY_COUNT, CLIPBOARD_RETRY_DELAY) {
            match saved_content {
                SavedClipboard::Text(orig_text) => {
                    let _ = set_clipboard_text_with_retry(
                        &mut restore_cb,
                        &orig_text,
                        CLIPBOARD_RETRY_COUNT,
                        CLIPBOARD_RETRY_DELAY,
                    );
                }
                SavedClipboard::Image(orig_img) => {
                    let _ = restore_cb.set_image(orig_img);
                }
                SavedClipboard::Empty => {
                    let _ = restore_cb.clear();
                }
            }
        }

        Ok(())
    }

    fn inject_unicode(&self, text: &str) -> Result<(), PlatformError> {
        let text_units: Vec<u16> = text.encode_utf16().collect();
        std::thread::spawn(move || -> Result<(), PlatformError> {
            let _hdesk = unsafe { attach_thread_to_input_desktop() };
            for ch in text_units {
                let key_down = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch,
                            dwFlags: KEYEVENTF_UNICODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                let key_up = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch,
                            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };

                let sz = std::mem::size_of::<INPUT>() as i32;
                let sent = unsafe { SendInput(&[key_down, key_up], sz) };
                if sent == 0 {
                    if let Some(hd) = _hdesk {
                        let _ = unsafe { CloseDesktop(hd) };
                    }
                    return Err(PlatformError::OsError(format!(
                        "SendInput failed for Unicode code unit 0x{ch:04X}"
                    )));
                }
                sleep(Duration::from_millis(5));
            }

            if let Some(hd) = _hdesk {
                let _ = unsafe { CloseDesktop(hd) };
            }
            Ok(())
        })
        .join()
        .map_err(|_| PlatformError::OsError("Unicode injection thread panicked".into()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_injector_creation() {
        let injector = WindowsTextInjector::new();
        let _default = WindowsTextInjector;
        let _ = injector;
    }

    #[test]
    fn test_clipboard_save_and_restore_cycle() {
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(_) => return, // In headless CI without desktop window station, skip gracefully
        };

        let original = "Murmur Unit Test Original String";
        let _ = cb.set_text(original);

        let injector = WindowsTextInjector::new();
        // Injecting text pastes to active window; original clipboard should be restored
        let _ = injector.inject_paste("Injected string");

        // Verify restored text
        if let Ok(after) = cb.get_text() {
            assert_eq!(after, original);
        }
    }
}
