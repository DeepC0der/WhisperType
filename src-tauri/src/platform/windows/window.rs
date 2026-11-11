use crate::platform::{OverlayWindowExt, PlatformError};
use windows::Win32::Foundation::{HWND, POINT, RECT, WIN32_ERROR};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetWindowLongW, SetWindowLongW, SetWindowPos,
    ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE,
    WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

/// Physical dimensions in logical pixels for overlay size presets (+15-20% spacious).
pub fn get_overlay_dimensions(size_preset: &str) -> (i32, i32) {
    match size_preset.to_uppercase().as_str() {
        "S" => (240, 56),
        "L" => (330, 76),
        _ => (280, 66),
    }
}

/// Calculate physical screen coordinates for positioning the overlay window within a monitor work area.
pub fn calculate_overlay_position(
    work_rect: &RECT,
    window_width: i32,
    window_height: i32,
    anchor: &str,
) -> (i32, i32) {
    let work_w = work_rect.right - work_rect.left;
    let work_h = work_rect.bottom - work_rect.top;
    let margin = 48; // 48px from screen / taskbar edge (Decision D7)

    match anchor.to_lowercase().as_str() {
        "top-center" => {
            let x = work_rect.left + (work_w - window_width) / 2;
            let y = work_rect.top + margin;
            (x, y)
        }
        "top-left" => {
            let x = work_rect.left + margin;
            let y = work_rect.top + margin;
            (x, y)
        }
        "top-right" => {
            let x = work_rect.right - window_width - margin;
            let y = work_rect.top + margin;
            (x, y)
        }
        "bottom-left" => {
            let x = work_rect.left + margin;
            let y = work_rect.bottom - window_height - margin;
            (x, y)
        }
        "bottom-right" => {
            let x = work_rect.right - window_width - margin;
            let y = work_rect.bottom - window_height - margin;
            (x, y)
        }
        "center" => {
            let x = work_rect.left + (work_w - window_width) / 2;
            let y = work_rect.top + (work_h - window_height) / 2;
            (x, y)
        }
        _ => {
            // "bottom-center" (default)
            let x = work_rect.left + (work_w - window_width) / 2;
            let y = work_rect.bottom - window_height - margin;
            (x, y)
        }
    }
}

/// Detect target monitor: foreground focused window first, then cursor location, then overlay window itself.
unsafe fn get_target_monitor(hwnd: HWND) -> HMONITOR {
    let fg_hwnd = GetForegroundWindow();
    if !fg_hwnd.0.is_null() && fg_hwnd != hwnd {
        let monitor = MonitorFromWindow(fg_hwnd, MONITOR_DEFAULTTONEAREST);
        if !monitor.0.is_null() {
            return monitor;
        }
    }

    let mut pt = POINT::default();
    if GetCursorPos(&mut pt).is_ok() {
        let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if !monitor.0.is_null() {
            return monitor;
        }
    }

    MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
}

impl OverlayWindowExt for tauri::WebviewWindow {
    fn apply_overlay_styles(&self) -> Result<(), PlatformError> {
        let raw_hwnd = self
            .hwnd()
            .map_err(|e| PlatformError::WindowError(format!("Failed to retrieve HWND: {e}")))?;

        let hwnd = HWND(raw_hwnd.0);

        unsafe {
            let current_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            // Apply WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT
            // Also strip WS_EX_APPWINDOW to ensure it never appears in taskbar or Alt-Tab
            let target_style = (current_style & !(WS_EX_APPWINDOW.0 as i32))
                | (WS_EX_NOACTIVATE.0 as i32)
                | (WS_EX_TOOLWINDOW.0 as i32)
                | (WS_EX_LAYERED.0 as i32)
                | (WS_EX_TRANSPARENT.0 as i32);

            windows::Win32::Foundation::SetLastError(WIN32_ERROR(0));
            let res = SetWindowLongW(hwnd, GWL_EXSTYLE, target_style);
            if res == 0 && windows::Win32::Foundation::GetLastError().is_err() {
                return Err(PlatformError::WindowError(
                    "SetWindowLongW failed when applying overlay extended styles".into(),
                ));
            }

            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOACTIVATE,
            )
            .map_err(|e| {
                PlatformError::WindowError(format!(
                    "SetWindowPos failed to update window frame for overlay styles: {e}"
                ))
            })?;
        }

        Ok(())
    }

    fn set_click_through(&self, click_through: bool) -> Result<(), PlatformError> {
        let raw_hwnd = self
            .hwnd()
            .map_err(|e| PlatformError::WindowError(format!("Failed to retrieve HWND: {e}")))?;

        let hwnd = HWND(raw_hwnd.0);

        unsafe {
            let current_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let target_style = if click_through {
                current_style | (WS_EX_TRANSPARENT.0 as i32)
            } else {
                current_style & !(WS_EX_TRANSPARENT.0 as i32)
            };

            windows::Win32::Foundation::SetLastError(WIN32_ERROR(0));
            let res = SetWindowLongW(hwnd, GWL_EXSTYLE, target_style);
            if res == 0 && windows::Win32::Foundation::GetLastError().is_err() {
                return Err(PlatformError::WindowError(
                    "SetWindowLongW failed while updating click-through style".into(),
                ));
            }

            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
            )
            .map_err(|e| {
                PlatformError::WindowError(format!(
                    "SetWindowPos failed to update window frame for click-through: {e}"
                ))
            })?;
        }

        Ok(())
    }

    fn position_overlay(&self, anchor: &str) -> Result<(), PlatformError> {
        self.position_overlay_with_size(anchor, "M")
    }

    fn position_overlay_with_size(&self, anchor: &str, size_preset: &str) -> Result<(), PlatformError> {
        let raw_hwnd = self
            .hwnd()
            .map_err(|e| PlatformError::WindowError(format!("Failed to retrieve HWND: {e}")))?;

        let hwnd = HWND(raw_hwnd.0);
        let (win_w, win_h) = get_overlay_dimensions(size_preset);

        unsafe {
            let monitor = get_target_monitor(hwnd);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };

            if !GetMonitorInfoW(monitor, &mut mi as *mut _).as_bool() {
                return Err(PlatformError::WindowError("GetMonitorInfoW failed".into()));
            }

            let (x, y) = calculate_overlay_position(&mi.rcWork, win_w, win_h, anchor);

            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                win_w,
                win_h,
                SWP_NOACTIVATE,
            )
            .map_err(|e| PlatformError::WindowError(format!("SetWindowPos position failed: {e}")))?;
        }

        let _ = self.set_size(tauri::LogicalSize::new(win_w as f64, win_h as f64));
        Ok(())
    }

    fn is_fullscreen_active(&self) -> bool {
        unsafe {
            if let Ok(state) = SHQueryUserNotificationState() {
                return state == QUNS_RUNNING_D3D_FULL_SCREEN
                    || state == QUNS_PRESENTATION_MODE
                    || state == QUNS_BUSY;
            }
        }
        false
    }

    fn show_overlay(&self) -> Result<(), PlatformError> {
        let raw_hwnd = self
            .hwnd()
            .map_err(|e| PlatformError::WindowError(format!("Failed to retrieve HWND: {e}")))?;

        let hwnd = HWND(raw_hwnd.0);

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
            )
            .map_err(|e| PlatformError::WindowError(format!("SetWindowPos show failed: {e}")))?;
        }

        Ok(())
    }

    fn hide_overlay(&self) -> Result<(), PlatformError> {
        let raw_hwnd = self
            .hwnd()
            .map_err(|e| PlatformError::WindowError(format!("Failed to retrieve HWND: {e}")))?;

        let hwnd = HWND(raw_hwnd.0);

        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_HIDEWINDOW | SWP_NOACTIVATE,
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_overlay_position_default_bottom_center() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let (x, y) = calculate_overlay_position(&work, 360, 90, "bottom-center");
        assert_eq!(x, (1920 - 360) / 2); // 780
        assert_eq!(y, 1040 - 90 - 48); // 902
    }

    #[test]
    fn test_calculate_overlay_position_top_center() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let (x, y) = calculate_overlay_position(&work, 360, 90, "top-center");
        assert_eq!(x, 780);
        assert_eq!(y, 48);
    }

    #[test]
    fn test_calculate_overlay_position_center() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let (x, y) = calculate_overlay_position(&work, 360, 90, "center");
        assert_eq!(x, 780);
        assert_eq!(y, (1080 - 90) / 2);
    }

    #[test]
    fn test_calculate_overlay_position_corners() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1400,
        };
        // Top-left
        let (tl_x, tl_y) = calculate_overlay_position(&work, 360, 90, "top-left");
        assert_eq!(tl_x, 48);
        assert_eq!(tl_y, 48);

        // Top-right
        let (tr_x, tr_y) = calculate_overlay_position(&work, 360, 90, "top-right");
        assert_eq!(tr_x, 2560 - 360 - 48);
        assert_eq!(tr_y, 48);

        // Bottom-left
        let (bl_x, bl_y) = calculate_overlay_position(&work, 360, 90, "bottom-left");
        assert_eq!(bl_x, 48);
        assert_eq!(bl_y, 1400 - 90 - 48);

        // Bottom-right
        let (br_x, br_y) = calculate_overlay_position(&work, 360, 90, "bottom-right");
        assert_eq!(br_x, 2560 - 360 - 48);
        assert_eq!(br_y, 1400 - 90 - 48);
    }

    #[test]
    fn test_calculate_overlay_position_secondary_monitor_offset() {
        // Multi-monitor: second monitor placed to the right of primary monitor
        let work = RECT {
            left: 1920,
            top: 0,
            right: 3840,
            bottom: 1080,
        };
        let (x, y) = calculate_overlay_position(&work, 360, 90, "bottom-center");
        assert_eq!(x, 1920 + (1920 - 360) / 2);
        assert_eq!(y, 1080 - 90 - 48);
    }

    #[test]
    fn test_calculate_overlay_position_unknown_fallback() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let (x, y) = calculate_overlay_position(&work, 360, 90, "some-random-invalid-anchor");
        assert_eq!(x, 780);
        assert_eq!(y, 902);
    }

    #[test]
    fn test_calculate_overlay_position_compact_dimensions() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let (x, y) = calculate_overlay_position(&work, 240, 54, "bottom-center");
        assert_eq!(x, (1920 - 240) / 2); // 840
        assert_eq!(y, 1040 - 54 - 48); // 938
    }
}
