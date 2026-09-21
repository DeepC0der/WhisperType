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
