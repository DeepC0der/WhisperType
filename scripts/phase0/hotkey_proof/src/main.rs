use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VK_CONTROL, VK_ESCAPE, VK_LCONTROL,
    VK_LSHIFT, VK_MENU, VK_SHIFT, VK_SPACE, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SetThreadDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
    HDESK,
};

static EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);
static COMBO_DETECTED: AtomicBool = AtomicBool::new(false);
static MAX_LATENCY_MICROS: AtomicUsize = AtomicUsize::new(0);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

unsafe fn attach_thread_to_input_desktop() -> Option<HDESK> {
    // DESKTOP_ALL_ACCESS = 0x01FF
    if let Ok(hdesk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x01FF)) {
        if SetThreadDesktop(hdesk).is_ok() {
            return Some(hdesk);
        }
        let _ = CloseDesktop(hdesk);
    }
    None
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let start = Instant::now();

    if code >= 0 {
        let msg = wparam.0 as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

        if is_down || is_up {
            let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kbd.vkCode as u16;

            let ctrl_pressed = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
            let shift_pressed = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
            let alt_pressed = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;

            let action = if is_down { "KEY_DOWN" } else { "KEY_UP" };
            let vk_name = match VIRTUAL_KEY(vk) {
                VK_SPACE => "SPACE",
                VK_CONTROL | VK_LCONTROL => "CTRL",
                VK_SHIFT | VK_LSHIFT => "SHIFT",
                VK_ESCAPE => "ESCAPE",
                _ => "OTHER",
            };

            EVENT_COUNT.fetch_add(1, Ordering::SeqCst);

            // Check combo Ctrl+Shift+Space
            if is_down && VIRTUAL_KEY(vk) == VK_SPACE && ctrl_pressed && shift_pressed {
                COMBO_DETECTED.store(true, Ordering::SeqCst);
                println!(
                    "  [HOTKEY MATCH] >>> Ctrl + Shift + Space DETECTED! (Alt={alt_pressed}) <<<"
                );
            }

            let elapsed = start.elapsed();
            let elapsed_micros = elapsed.as_micros() as usize;
            let _ = MAX_LATENCY_MICROS.fetch_max(elapsed_micros, Ordering::SeqCst);

            println!(
                "  [{action}] VK=0x{vk:02X} ({vk_name}) | Ctrl={ctrl_pressed} Shift={shift_pressed} Alt={alt_pressed} | Hook latency: {elapsed_micros} µs"
            );
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

fn simulate_combo() -> bool {
    println!("\n[SIMULATOR] Simulating Ctrl + Shift + Space combo via SendInput...");
    let keys = [
        (VK_LCONTROL, false),
        (VK_LSHIFT, false),
        (VK_SPACE, false),
        (VK_SPACE, true),
        (VK_LSHIFT, true),
        (VK_LCONTROL, true),
    ];

    let mut all_sent = true;
    for (vk, is_up) in keys {
        let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) };
        let flags = if is_up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan as u16,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let sz = std::mem::size_of::<INPUT>() as i32;
            let sent = SendInput(&[input], sz);
            if sent == 0 {
                all_sent = false;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    all_sent
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Murmur Phase 0: Win32 WH_KEYBOARD_LL Hotkey Proof ===");

    let fg = unsafe { GetForegroundWindow() };
    println!("Current Foreground HWND: {:?}", fg);

    let (tx_ready, rx_ready) = mpsc::channel();

    // Spawn dedicated OS thread for the hook and its message loop
    let hook_thread = std::thread::spawn(move || -> Result<(), String> {
        let _hdesk = unsafe { attach_thread_to_input_desktop() };
        let thread_id = unsafe { GetCurrentThreadId() };
        HOOK_THREAD_ID.store(thread_id, Ordering::SeqCst);

        // Ensure thread has a message queue
        unsafe {
            let mut msg = MSG::default();
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        }

        let h_module = match unsafe { GetModuleHandleW(None) } {
            Ok(h) => HINSTANCE(h.0),
            Err(e) => return Err(format!("GetModuleHandleW failed: {e}")),
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
            Err(e) => return Err(format!("SetWindowsHookExW failed: {e}")),
        };

        println!("Hook successfully installed on thread {thread_id} via SetWindowsHookExW.");
        tx_ready.send(()).unwrap();

        // Run message pump
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            UnhookWindowsHookEx(hook).map_err(|e| format!("Unhook failed: {e}"))?;
            if let Some(hd) = _hdesk {
                let _ = CloseDesktop(hd);
            }
        }

        println!("Hook uninstalled cleanly.");
        Ok(())
    });

    // Wait for hook to be fully active
    rx_ready.recv().expect("Hook thread failed to start");
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Attempt simulation on a dedicated thread attached to input desktop
    let sim_thread = std::thread::spawn(|| {
        let _hdesk = unsafe { attach_thread_to_input_desktop() };
        let res = simulate_combo();
        if let Some(hd) = _hdesk {
            unsafe { let _ = CloseDesktop(hd); }
        }
        res
    });
    let sim_success = sim_thread.join().unwrap_or(false);

    // Allow events to be processed
    std::thread::sleep(std::time::Duration::from_millis(200));

    let count = EVENT_COUNT.load(Ordering::SeqCst);
    let combo = COMBO_DETECTED.load(Ordering::SeqCst);
    let max_lat = MAX_LATENCY_MICROS.load(Ordering::SeqCst);

    println!("\n--- Hotkey Proof Verification Summary ---");
    println!("WH_KEYBOARD_LL Hook Installed:  YES (Win32 thread message loop active)");
    println!("Hook Callback Lag Measurement:  {max_lat} µs (budget: < 1000 µs)");
    println!("Total Hook Events Captured:     {count}");
    println!("Ctrl+Shift+Space Detected:      {combo}");

    if !sim_success || count == 0 {
        println!("\n[DIAGNOSTIC NOTICE]");
        println!("Note: If running in headless/unfocused background sandbox, events may require interactive desktop focus.");
    }

    // Tell hook thread to quit
    let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
    let _ = hook_thread.join();

    assert!(count > 0, "No hook events captured!");
    assert!(combo, "Expected Ctrl+Shift+Space combo to be detected!");

    println!("\n>>> RESULT: PASS - Win32 WH_KEYBOARD_LL hook successfully proved <<<\n");
    Ok(())
}
