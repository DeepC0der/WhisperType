use crate::platform::{Autostart, PlatformError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Formats and quotes an executable path so Windows Run key properly launches paths containing spaces.
pub fn quote_executable_path(path: &str) -> String {
    let trimmed = path.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed.to_string()
    } else {
        format!("\"{trimmed}\"")
    }
}

/// Abstract storage interface for registry key operations, enabling hermetic unit testing without touching the Windows Registry.
pub trait RegistryStore: Send + Sync {
    fn is_value_set(&self, subkey: &str, value_name: &str) -> Result<bool, PlatformError>;
    fn set_string_value(
        &self,
        subkey: &str,
        value_name: &str,
        value: &str,
    ) -> Result<(), PlatformError>;
    fn delete_value(&self, subkey: &str, value_name: &str) -> Result<(), PlatformError>;
}

/// Real Win32 Registry implementation reading/writing to HKCU.
pub struct Win32RegistryStore;

impl RegistryStore for Win32RegistryStore {
    fn is_value_set(&self, subkey: &str, value_name: &str) -> Result<bool, PlatformError> {
        let subkey_h = HSTRING::from(subkey);
        let value_name_h = HSTRING::from(value_name);
        let mut hkey = HKEY::default();

        unsafe {
            let res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_h.as_ptr()),
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
                PCWSTR(value_name_h.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );
            let _ = RegCloseKey(hkey);

            Ok(query_res.is_ok() && data_size > 0)
        }
    }

    fn set_string_value(
        &self,
        subkey: &str,
        value_name: &str,
        value: &str,
    ) -> Result<(), PlatformError> {
        let subkey_h = HSTRING::from(subkey);
        let value_name_h = HSTRING::from(value_name);
        let mut hkey = HKEY::default();

        unsafe {
            RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(subkey_h.as_ptr()), &mut hkey)
                .ok()
                .map_err(|e| {
                    PlatformError::AutostartError(format!(
                        "Failed to open/create Run registry key: {e}"
                    ))
                })?;

            let wide_path: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
            let byte_slice = std::slice::from_raw_parts(
                wide_path.as_ptr() as *const u8,
                wide_path.len() * std::mem::size_of::<u16>(),
            );

            let set_res = RegSetValueExW(
                hkey,
                PCWSTR(value_name_h.as_ptr()),
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

    fn delete_value(&self, subkey: &str, value_name: &str) -> Result<(), PlatformError> {
        let subkey_h = HSTRING::from(subkey);
        let value_name_h = HSTRING::from(value_name);
        let mut hkey = HKEY::default();

        unsafe {
            let open_res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_h.as_ptr()),
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

            let del_res = RegDeleteValueW(hkey, PCWSTR(value_name_h.as_ptr()));
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

/// In-memory mock registry store for hermetic unit testing.
#[derive(Default)]
pub struct MockRegistryStore {
    entries: Mutex<HashMap<(String, String), String>>,
}

impl MockRegistryStore {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, subkey: &str, value_name: &str) -> Option<String> {
        self.entries
            .lock()
            .unwrap()
            .get(&(subkey.to_string(), value_name.to_string()))
            .cloned()
    }
}

impl RegistryStore for MockRegistryStore {
    fn is_value_set(&self, subkey: &str, value_name: &str) -> Result<bool, PlatformError> {
        let map = self.entries.lock().unwrap();
        Ok(map.contains_key(&(subkey.to_string(), value_name.to_string())))
    }

    fn set_string_value(
        &self,
        subkey: &str,
        value_name: &str,
        value: &str,
    ) -> Result<(), PlatformError> {
        let mut map = self.entries.lock().unwrap();
        map.insert(
            (subkey.to_string(), value_name.to_string()),
            value.to_string(),
        );
        Ok(())
    }

    fn delete_value(&self, subkey: &str, value_name: &str) -> Result<(), PlatformError> {
        let mut map = self.entries.lock().unwrap();
        map.remove(&(subkey.to_string(), value_name.to_string()));
        Ok(())
    }
}

/// Windows Autostart implementation managing registry entries in HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
pub struct WindowsAutostart {
    subkey: String,
    store: Arc<dyn RegistryStore>,
}

impl WindowsAutostart {
    pub fn new() -> Self {
        Self {
            subkey: RUN_KEY.to_string(),
            store: Arc::new(Win32RegistryStore),
        }
    }

    pub fn with_store(subkey: &str, store: Arc<dyn RegistryStore>) -> Self {
        Self {
            subkey: subkey.to_string(),
            store,
        }
    }

    #[cfg(test)]
    pub fn with_subkey(subkey: &str) -> Self {
        Self {
            subkey: subkey.to_string(),
            store: Arc::new(Win32RegistryStore),
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
        self.store.is_value_set(&self.subkey, app_name)
    }

    fn enable(&self, app_name: &str, app_path: &str) -> Result<(), PlatformError> {
        let quoted = quote_executable_path(app_path);
        self.store.set_string_value(&self.subkey, app_name, &quoted)
    }

    fn disable(&self, app_name: &str) -> Result<(), PlatformError> {
        self.store.delete_value(&self.subkey, app_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_executable_path() {
        // Path with spaces
        let unquoted = r"C:\Program Files\Murmur\murmur.exe";
        assert_eq!(
            quote_executable_path(unquoted),
            r#""C:\Program Files\Murmur\murmur.exe""#
        );

        // Path already quoted
        let already_quoted = r#""C:\Program Files\Murmur\murmur.exe""#;
        assert_eq!(quote_executable_path(already_quoted), already_quoted);

        // Path without spaces
        let simple = r"C:\Murmur\murmur.exe";
        assert_eq!(quote_executable_path(simple), r#""C:\Murmur\murmur.exe""#);
    }

    #[test]
    fn test_hermetic_autostart_enable_query_disable() {
        let mock_store = Arc::new(MockRegistryStore::new());
        let autostart = WindowsAutostart::with_store(RUN_KEY, mock_store.clone());
        let app_name = "MurmurTestApp";
        let exe_path = r"C:\Program Files\Murmur App\murmur.exe";

        // 1. Initial query is false
        assert!(!autostart.is_enabled(app_name).unwrap());

        // 2. Enable
        assert!(autostart.enable(app_name, exe_path).is_ok());

        // 3. Query is true
        assert!(autostart.is_enabled(app_name).unwrap());

        // 4. Verify quoting in store
        let stored = mock_store.get(RUN_KEY, app_name).unwrap();
        assert_eq!(stored, r#""C:\Program Files\Murmur App\murmur.exe""#);

        // 5. Disable
        assert!(autostart.disable(app_name).is_ok());
        assert!(!autostart.is_enabled(app_name).unwrap());

        // 6. Idempotent disable
        assert!(autostart.disable(app_name).is_ok());
    }
}
