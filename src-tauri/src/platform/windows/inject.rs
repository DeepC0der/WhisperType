use crate::platform::{PlatformError, TextInjector};
use arboard::Clipboard;
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

unsafe fn attach_thread_to_input_desktop() -> Option<HDESK> {
    if let Ok(hdesk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x01FF)) {
        if SetThreadDesktop(hdesk).is_ok() {
            return Some(hdesk);
        }
        let _ = CloseDesktop(hdesk);
    }
    None
}

/// Windows implementation of TextInjector utilizing Win32 SendInput and arboard.
pub struct WindowsTextInjector;

impl WindowsTextInjector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsTextInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInjector for WindowsTextInjector {
    fn inject_paste(&self, text: &str) -> Result<(), PlatformError> {
        let mut clipboard = Clipboard::new().map_err(|e| {
            PlatformError::ClipboardError(format!("Failed to open clipboard: {e}"))
        })?;

        // 1. Save original clipboard text
        let original_text = clipboard.get_text().ok();

        // 2. Set injection text to clipboard
        clipboard.set_text(text).map_err(|e| {
            PlatformError::ClipboardError(format!("Failed to write to clipboard: {e}"))
        })?;

        // 3. Emulate Ctrl+V via SendInput
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

        // 4. Delay 120ms to allow target window to process paste event (Decision D10)
        sleep(Duration::from_millis(120));

        // 5. Restore clipboard contents
        if let Some(orig) = original_text {
            let _ = clipboard.set_text(orig);
        } else {
            let _ = clipboard.clear();
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
