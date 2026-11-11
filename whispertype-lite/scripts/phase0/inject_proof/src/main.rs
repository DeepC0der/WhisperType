use arboard::Clipboard;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VK_CONTROL, VIRTUAL_KEY,
};

use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SetThreadDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
    HDESK,
};

const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56); // 'V' key

unsafe fn attach_thread_to_input_desktop() -> Option<HDESK> {
    if let Ok(hdesk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x01FF)) {
        if SetThreadDesktop(hdesk).is_ok() {
            return Some(hdesk);
        }
        let _ = CloseDesktop(hdesk);
    }
    None
}

fn send_ctrl_v() -> u32 {
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

    unsafe {
        let sz = std::mem::size_of::<INPUT>() as i32;
        let sent = SendInput(&inputs, sz);
        println!("  [SendInput] Sent {sent}/4 keyboard events for Ctrl+V.");
        if sent == 0 {
            let err = windows::Win32::Foundation::GetLastError();
            println!("  [SendInput] GetLastError: {err:?} ({:?})", err.to_hresult());
        }
        sent
    }
}

fn send_unicode_text(text: &str) -> u32 {
    println!("  [SendInput] Sending Unicode keystrokes for: \"{text}\"");
    let mut total_sent = 0;
    for ch in text.encode_utf16() {
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

        unsafe {
            let sent = SendInput(&[key_down, key_up], std::mem::size_of::<INPUT>() as i32);
            total_sent += sent;
        }
        sleep(Duration::from_millis(10));
    }
    println!("  [SendInput] Total Unicode events sent: {total_sent}");
    total_sent
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Murmur Phase 0: Win32 Text Injection Proof ===");

    let mut clipboard = Clipboard::new()?;

    // 1. Save original clipboard
    let original_text = clipboard.get_text().ok();
    println!(
        "1. Saved original clipboard content: {:?}",
        original_text.as_deref().unwrap_or("<empty / non-text>")
    );

    // 2. Set injection test text
    let test_inject = "Murmur injection test ✓";
    println!("2. Setting clipboard to: \"{test_inject}\"");
    clipboard.set_text(test_inject)?;

    // Verify it was set
    let current = clipboard.get_text()?;
    assert_eq!(current, test_inject, "Clipboard should hold test text");
    println!("   Clipboard verified: \"{current}\"");

    // 3. Send Ctrl+V
    println!("3. Emulating Ctrl+V via SendInput...");
    let ctrl_v_sent = std::thread::spawn(|| {
        let _hdesk = unsafe { attach_thread_to_input_desktop() };
        let sent = send_ctrl_v();
        if let Some(hd) = _hdesk {
            unsafe { let _ = CloseDesktop(hd); }
        }
        sent
    }).join().unwrap_or(0);

    assert_eq!(ctrl_v_sent, 4, "SendInput failed to send all 4 events for Ctrl+V!");

    // 4. Wait 120ms
    println!("4. Waiting 120ms injection delay...");
    sleep(Duration::from_millis(120));

    // 5. Restore original clipboard
    println!("5. Restoring original clipboard content...");
    match &original_text {
        Some(orig) => clipboard.set_text(orig)?,
        None => clipboard.clear()?,
    }

    // 6. Verify restored clipboard
    let restored = clipboard.get_text().ok();
    println!(
        "   Restored clipboard verified: {:?}",
        restored.as_deref().unwrap_or("<empty / non-text>")
    );
    assert_eq!(
        restored, original_text,
        "Clipboard must match original content!"
    );
    println!("   [PASS] Clipboard save, injection, 120ms delay, and restore cycle verified!");

    // 7. Unicode fallback typing test
    println!("\n6. Testing Unicode fallback injection (SendInput KEYEVENTF_UNICODE)...");
    let fallback_text = "Murmur unicode fallback: 你好 World ✓\n";
    let expected_unicode_events = (fallback_text.encode_utf16().count() * 2) as u32;
    let unicode_sent = std::thread::spawn(move || {
        let _hdesk = unsafe { attach_thread_to_input_desktop() };
        let sent = send_unicode_text(fallback_text);
        if let Some(hd) = _hdesk {
            unsafe { let _ = CloseDesktop(hd); }
        }
        sent
    }).join().unwrap_or(0);

    assert_eq!(
        unicode_sent, expected_unicode_events,
        "Unicode SendInput failed to send expected events!"
    );
    println!("   [PASS] Unicode SendInput fallback executed successfully ({unicode_sent}/{expected_unicode_events} events)!");

    println!("\n>>> RESULT: PASS - All text injection mechanisms functional <<<\n");
    Ok(())
}
