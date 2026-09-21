use crate::platform::{
    Autostart, HotkeyBackend, HotkeyCallback, PlatformError, TextInjector,
};
#[cfg(not(target_os = "windows"))]
use crate::platform::OverlayWindowExt;

/// Stub implementation of HotkeyBackend for Linux.
pub struct LinuxHotkeyBackend;

impl LinuxHotkeyBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxHotkeyBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyBackend for LinuxHotkeyBackend {
    fn register_hotkey(
        &mut self,
        _combo: &str,
        _callback: HotkeyCallback,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux hotkey listener not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn unregister_hotkey(&mut self, _combo: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux hotkey unregistration not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn start_listening(&mut self) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux global hotkey hook not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn stop_listening(&mut self) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux global hotkey stop not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn is_listening(&self) -> bool {
        false
    }
}

/// Stub implementation of TextInjector for Linux.
pub struct LinuxTextInjector;

impl LinuxTextInjector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxTextInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInjector for LinuxTextInjector {
    fn inject_paste(&self, _text: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux text injection (paste) not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn inject_unicode(&self, _text: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux text injection (unicode) not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }
}

/// Stub implementation of Autostart for Linux.
pub struct LinuxAutostart;

impl LinuxAutostart {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxAutostart {
    fn default() -> Self {
        Self::new()
    }
}

impl Autostart for LinuxAutostart {
    fn is_enabled(&self, _app_name: &str) -> Result<bool, PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux autostart query not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn enable(&self, _app_name: &str, _app_path: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux autostart enable not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn disable(&self, _app_name: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux autostart disable not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }
}

#[cfg(not(target_os = "windows"))]
impl OverlayWindowExt for tauri::WebviewWindow {
    fn apply_overlay_styles(&self) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux overlay window styling not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn set_click_through(&self, _click_through: bool) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux overlay click-through not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn position_overlay(&self, _anchor: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented(
            "Linux overlay positioning not yet implemented. See LINUX_TODO.md.".into(),
        ))
    }

    fn is_fullscreen_active(&self) -> bool {
        false
    }

    fn show_overlay(&self) -> Result<(), PlatformError> {
        let _ = self.show();
        Ok(())
    }

    fn hide_overlay(&self) -> Result<(), PlatformError> {
        let _ = self.hide();
        Ok(())
    }
}
