use crate::platform::{Autostart, PlatformError};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Windows Autostart implementation managing registry entries in HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
pub struct WindowsAutostart {
    subkey: String,
}

impl WindowsAutostart {
    pub fn new() -> Self {
        Self {
            subkey: RUN_KEY.to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_subkey(subkey: &str) -> Self {
        Self {
            subkey: subkey.to_string(),
        }
    }
}

impl Default for WindowsAutostart {
    fn default() -> Self {
        Self::new()
    }
}

impl Autostart for WindowsAutostart {
    fn is_enabled(&self, app_name: &str) -> Result<bool, PlatformError> {
        let subkey = HSTRING::from(&self.subkey);
        let value_name = HSTRING::from(app_name);
        let mut hkey = HKEY::default();

        unsafe {
            let res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                KEY_READ,
                &mut hkey,
            );
            if res.is_err() {
                return Ok(false);
            }

            let mut data_type = REG_SZ;
            let mut data_size = 0u32;
            let query_res = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );
            let _ = RegCloseKey(hkey);

            Ok(query_res.is_ok() && data_size > 0)
        }
    }

    fn enable(&self, app_name: &str, app_path: &str) -> Result<(), PlatformError> {
        let subkey = HSTRING::from(&self.subkey);
        let value_name = HSTRING::from(app_name);
        let mut hkey = HKEY::default();

        unsafe {
            RegCreateKeyW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                &mut hkey,
            )
            .ok()
            .map_err(|e| {
                PlatformError::AutostartError(format!("Failed to open/create Run registry key: {e}"))
            })?;

            let wide_path: Vec<u16> = app_path.encode_utf16().chain(std::iter::once(0)).collect();
            let byte_slice = std::slice::from_raw_parts(
                wide_path.as_ptr() as *const u8,
                wide_path.len() * std::mem::size_of::<u16>(),
            );

            let set_res = RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                REG_SZ,
                Some(byte_slice),
            );
            let _ = RegCloseKey(hkey);

            set_res.ok().map_err(|e| {
                PlatformError::AutostartError(format!("Failed to write registry value: {e}"))
            })?;
        }

        Ok(())
    }

    fn disable(&self, app_name: &str) -> Result<(), PlatformError> {
        let subkey = HSTRING::from(&self.subkey);
        let value_name = HSTRING::from(app_name);
        let mut hkey = HKEY::default();

        unsafe {
            let open_res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                KEY_WRITE,
                &mut hkey,
            );

            if open_res == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }

            open_res.ok().map_err(|e| {
                PlatformError::AutostartError(format!("Failed to open Run registry key: {e}"))
            })?;

            let del_res = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));
            let _ = RegCloseKey(hkey);

            if del_res.is_err() && del_res != ERROR_FILE_NOT_FOUND {
                return Err(PlatformError::AutostartError(format!(
                    "Failed to delete registry value: {:?}",
                    del_res.to_hresult()
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::RegDeleteKeyW;

    #[test]
    fn test_autostart_enable_query_disable() {
        let test_key = r"Software\MurmurTest_Scaffold";
        let autostart = WindowsAutostart::with_subkey(test_key);
        let app_name = "MurmurTestApp";
        let fake_exe = r"C:\Program Files\Murmur\murmur.exe";

        // 1. Initial query should be false
        assert_eq!(autostart.is_enabled(app_name).unwrap(), false);

        // 2. Enable autostart
        assert!(autostart.enable(app_name, fake_exe).is_ok());

        // 3. Query should be true
        assert_eq!(autostart.is_enabled(app_name).unwrap(), true);

        // 4. Disable autostart
        assert!(autostart.disable(app_name).is_ok());

        // 5. Query should be false again
        assert_eq!(autostart.is_enabled(app_name).unwrap(), false);

        // 6. Idempotent disable
        assert!(autostart.disable(app_name).is_ok());

        // Clean up test registry key
        unsafe {
            let subkey = HSTRING::from(test_key);
            let _ = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()));
        }
    }
}
