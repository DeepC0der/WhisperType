pub mod autostart;
pub mod hotkey;
pub mod inject;
pub mod window;

pub use autostart::WindowsAutostart;
pub use hotkey::{
    parse_hotkey_string, HotkeyAction, HotkeyActionCallback, HotkeyModeController,
    HotkeyModifiers, ParsedHotkey, WindowsHotkeyBackend,
};
pub use inject::WindowsTextInjector;
pub use window::calculate_overlay_position;

pub mod mouse;
pub use mouse::{reset_mouse_hook_state, MouseAction, MouseButton, MouseActionCallback, WindowsMouseHook};
pub use hotkey::{reset_ctrl_win_state, set_ctrl_win_callback, set_ctrl_win_enabled};
