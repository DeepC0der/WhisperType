pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlatformError {
    #[error("Platform feature not implemented on this OS: {0}")]
    NotImplemented(String),

    #[error("Operating system API error: {0}")]
    OsError(String),

    #[error("Clipboard operation failed: {0}")]
    ClipboardError(String),

    #[error("Hotkey error: {0}")]
    HotkeyError(String),

    #[error("Window manipulation error: {0}")]
    WindowError(String),

    #[error("Autostart registry/desktop error: {0}")]
    AutostartError(String),
}

/// Thread-safe callback invoked when a registered hotkey combination is triggered.
pub type HotkeyCallback = Arc<dyn Fn() + Send + Sync + 'static>;

/// Trait abstracting system-wide global hotkey listening.
pub trait HotkeyBackend: Send + Sync {
    /// Register a hotkey combination (e.g. "Ctrl+Shift+Space") with a callback.
    fn register_hotkey(&mut self, combo: &str, callback: HotkeyCallback) -> Result<(), PlatformError>;

    /// Unregister a previously registered hotkey combination.
    fn unregister_hotkey(&mut self, combo: &str) -> Result<(), PlatformError>;

    /// Start the background OS hook and event dispatch loop.
    fn start_listening(&mut self) -> Result<(), PlatformError>;

    /// Stop listening and detach OS hooks cleanly.
    fn stop_listening(&mut self) -> Result<(), PlatformError>;

    /// Check whether the listener hook thread is currently active.
    fn is_listening(&self) -> bool;
}

/// Trait abstracting simulated keystroke and text injection into target applications.
pub trait TextInjector: Send + Sync {
    /// Inject text using clipboard copy/paste emulation with automatic clipboard restoration.
    fn inject_paste(&self, text: &str) -> Result<(), PlatformError>;

    /// Inject text via synthesized Unicode keystrokes directly.
    fn inject_unicode(&self, text: &str) -> Result<(), PlatformError>;
}

/// Trait extending Tauri windows with platform-specific overlay window extended styles.
pub trait OverlayWindowExt {
    /// Apply extended overlay styles (e.g. non-activating, click-through, transparent, toolwindow).
    fn apply_overlay_styles(&self) -> Result<(), PlatformError>;
}

/// Trait abstracting launch-on-login / autostart configuration.
pub trait Autostart: Send + Sync {
    /// Check if autostart is currently enabled for the given application name.
    fn is_enabled(&self, app_name: &str) -> Result<bool, PlatformError>;

    /// Enable autostart with the specified executable path.
    fn enable(&self, app_name: &str, app_path: &str) -> Result<(), PlatformError>;

    /// Disable autostart.
    fn disable(&self, app_name: &str) -> Result<(), PlatformError>;
}
