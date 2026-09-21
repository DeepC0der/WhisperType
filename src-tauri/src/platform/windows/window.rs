use crate::platform::{OverlayWindowExt, PlatformError};
use windows::Win32::Foundation::{HWND, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT,
};

impl OverlayWindowExt for tauri::WebviewWindow {
    fn apply_overlay_styles(&self) -> Result<(), PlatformError> {
        let raw_hwnd = self
            .hwnd()
            .map_err(|e| PlatformError::WindowError(format!("Failed to retrieve HWND: {e}")))?;

        // Ensure HWND type compatibility
        let hwnd = HWND(raw_hwnd.0);

        unsafe {
            let current_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let target_style = current_style
                | (WS_EX_NOACTIVATE.0 as i32)
                | (WS_EX_TOOLWINDOW.0 as i32)
                | (WS_EX_LAYERED.0 as i32)
                | (WS_EX_TRANSPARENT.0 as i32);

            // Win32 convention: clear last error before SetWindowLongW
            windows::Win32::Foundation::SetLastError(WIN32_ERROR(0));
            let res = SetWindowLongW(hwnd, GWL_EXSTYLE, target_style);
            if res == 0 && windows::Win32::Foundation::GetLastError().is_err() {
                return Err(PlatformError::WindowError(
                    "SetWindowLongW failed when applying overlay extended styles".into(),
                ));
            }

            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            )
            .map_err(|e| {
                PlatformError::WindowError(format!(
                    "SetWindowPos failed to update window frame for overlay styles: {e}"
                ))
            })?;
        }

        Ok(())
    }
}
