use crate::platform::PlatformError;
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
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, MSLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW,
    WH_MOUSE_LL, WM_QUIT, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    XButton1,
    XButton2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
}

pub type MouseActionCallback = Arc<dyn Fn(MouseButton, MouseAction) + Send + Sync + 'static>;

struct MouseHookState {
    callback: MouseActionCallback,
    button: MouseButton,
    pressed: AtomicBool,
}

static HOOK_STATE: Mutex<Option<Arc<MouseHookState>>> = Mutex::new(None);
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

pub fn reset_mouse_hook_state() {
    if let Ok(guard) = HOOK_STATE.try_lock() {
        if let Some(state) = guard.as_ref() {
            if state.pressed.swap(false, Ordering::SeqCst) {
                let cb = state.callback.clone();
                let b = state.button;
                std::thread::spawn(move || cb(b, MouseAction::Release));
            }
        }
    }
}

unsafe extern "system" fn low_level_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        if msg == WM_XBUTTONDOWN || msg == WM_XBUTTONUP {
            let hook_struct = *(lparam.0 as *const MSLLHOOKSTRUCT);
            let mouse_data = hook_struct.mouseData;
            let xbutton = (mouse_data >> 16) as u16; // HIWORD

            let button = if xbutton == 1 {
                MouseButton::XButton1
            } else if xbutton == 2 {
                MouseButton::XButton2
            } else {
                return CallNextHookEx(None, code, wparam, lparam);
            };

            let state_opt = match HOOK_STATE.try_lock() {
                Ok(guard) => guard.clone(),
                Err(_) => None,
            };

            if let Some(state) = state_opt {
                if state.button == button {
                    if msg == WM_XBUTTONDOWN {
                        if !state.pressed.swap(true, Ordering::SeqCst) {
                            let cb = state.callback.clone();
                            let b = state.button;
                            std::thread::spawn(move || cb(b, MouseAction::Press));
                        }
                        return LRESULT(1);
                    } else if msg == WM_XBUTTONUP {
                        if state.pressed.swap(false, Ordering::SeqCst) {
                            let cb = state.callback.clone();
                            let b = state.button;
                            std::thread::spawn(move || cb(b, MouseAction::Release));
                        }
                        return LRESULT(1);
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

pub struct WindowsMouseHook {
    thread_handle: Option<JoinHandle<()>>,
}

impl WindowsMouseHook {
    pub fn start(button: MouseButton, callback: MouseActionCallback) -> Result<Self, PlatformError> {
        if IS_RUNNING.load(Ordering::SeqCst) {
            return Ok(Self { thread_handle: None });
        }

        {
            let mut guard = HOOK_STATE.lock().unwrap();
            *guard = Some(Arc::new(MouseHookState {
                callback,
                button,
                pressed: AtomicBool::new(false),
            }));
        }

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
                SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), Some(h_module), 0)
            } {
                Ok(h) => h,
                Err(e) => {
                    let _ = tx_ready.send(Err(format!("SetWindowsHookExW failed: {e}")));
                    return;
                }
            };

            IS_RUNNING.store(true, Ordering::SeqCst);
            let _ = tx_ready.send(Ok(()));

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
            Ok(Ok(())) => Ok(Self {
                thread_handle: Some(handle),
            }),
            Ok(Err(e)) => Err(PlatformError::HotkeyError(e)),
            Err(e) => Err(PlatformError::HotkeyError(format!("Mouse hook thread startup failed: {e}"))),
        }
    }

    pub fn stop(&mut self) {
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
        if let Ok(mut guard) = HOOK_STATE.lock() {
            *guard = None;
        }
    }
}

impl Drop for WindowsMouseHook {
    fn drop(&mut self) {
        self.stop();
    }
}
