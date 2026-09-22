use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Path error: {0}")]
    PathError(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HotkeyMode {
    #[default]
    HoldToTalk,
    Toggle,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InsertionMode {
    #[default]
    PasteWithRestore,
    UnicodeTyping,
}

fn default_hotkey() -> String {
    "Ctrl+Shift+Space".to_string()
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_history_retention() -> usize {
    100
}

fn default_performance_provider() -> String {
    "cpu".to_string()
}

fn default_model_variant() -> String {
    "parakeet-tdt-0.6b-v3-int8".to_string()
}

fn default_overlay_position() -> String {
    "bottom-center".to_string()
}

fn default_gain_percent() -> u32 {
    100
}

fn default_max_recording_seconds() -> u32 {
    60
}

fn default_thread_count() -> u32 {
    4
}

fn default_overlay_size() -> String {
    "M".to_string()
}

fn default_dim_delay_seconds() -> u32 {
    2
}

fn default_dim_opacity_percent() -> u32 {
    40
}

fn default_fullscreen_behavior() -> String {
    "show".to_string()
}

fn default_smart_aggressiveness() -> String {
    "balanced".to_string()
}

fn default_smart_bullet_style() -> String {
    "bullet".to_string()
}

/// Strongly-typed application configuration with serde defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    #[serde(default)]
    pub hotkey_mode: HotkeyMode,

    #[serde(default)]
    pub audio_device: Option<String>,

    #[serde(default)]
    pub insertion_mode: InsertionMode,

    #[serde(default = "default_true")]
    pub auto_insert: bool,

    #[serde(default)]
    pub launch_on_startup: bool,

    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_history_retention")]
    pub history_retention_max: usize,

    #[serde(default = "default_performance_provider")]
    pub performance_provider: String,

    #[serde(default = "default_model_variant")]
    pub model_variant: String,

    #[serde(default)]
    pub sound_effects_enabled: bool,

    #[serde(default = "default_overlay_position")]
    pub overlay_position: String,

    #[serde(default = "default_true")]
    pub trim_whitespace: bool,

    #[serde(default = "default_true")]
    pub preserve_punctuation: bool,

    #[serde(default = "default_true")]
    pub remove_fillers: bool,

    #[serde(default = "default_true")]
    pub apply_spoken_commands: bool,

    #[serde(default = "default_true")]
    pub apply_dictionary: bool,

    #[serde(default)]
    pub add_trailing_space: bool,

    #[serde(default)]
    pub idle_unload_minutes: u32,

    #[serde(default = "default_gain_percent")]
    pub audio_gain_percent: u32,

    #[serde(default = "default_max_recording_seconds")]
    pub max_recording_seconds: u32,

    #[serde(default = "default_true")]
    pub persistent_history: bool,

    #[serde(default)]
    pub store_audio: bool,

    #[serde(default = "default_thread_count")]
    pub thread_count: u32,

    #[serde(default = "default_overlay_size")]
    pub overlay_size: String,

    #[serde(default = "default_dim_delay_seconds")]
    pub dim_delay_seconds: u32,

    #[serde(default = "default_dim_opacity_percent")]
    pub dim_opacity_percent: u32,

    #[serde(default)]
    pub ultra_minimal_mode: bool,

    #[serde(default = "default_true")]
    pub brighten_on_speech: bool,

    #[serde(default = "default_fullscreen_behavior")]
    pub fullscreen_behavior: String,

    #[serde(default)]
    pub first_run_completed: bool,

    // ── Smart Structure ──────────────────────────────────────────────────────
    /// Master switch: when false the structure engine is bypassed entirely.
    #[serde(default)]
    pub smart_structure_enabled: bool,

    /// Add paragraph breaks at sentence/thought boundaries.
    #[serde(default = "default_true")]
    pub smart_sentence_structure: bool,

    /// Detect and format explicit ordinal lists as bullets/numbers.
    #[serde(default = "default_true")]
    pub smart_auto_bullets: bool,

    /// How aggressively to introduce structural breaks.
    /// Values: "conservative" | "balanced" | "aggressive".
    #[serde(default = "default_smart_aggressiveness")]
    pub smart_aggressiveness: String,

    /// List bullet style. Values: "bullet" | "dash" | "asterisk" | "numbered".
    #[serde(default = "default_smart_bullet_style")]
    pub smart_bullet_style: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            hotkey_mode: HotkeyMode::default(),
            audio_device: None,
            insertion_mode: InsertionMode::default(),
            auto_insert: true,
            launch_on_startup: false,
            theme: default_theme(),
            history_retention_max: default_history_retention(),
            performance_provider: default_performance_provider(),
            model_variant: default_model_variant(),
            sound_effects_enabled: false,
            overlay_position: default_overlay_position(),
            trim_whitespace: true,
            preserve_punctuation: true,
            remove_fillers: true,
            apply_spoken_commands: true,
            apply_dictionary: true,
            add_trailing_space: false,
            idle_unload_minutes: 0,
            audio_gain_percent: default_gain_percent(),
            max_recording_seconds: default_max_recording_seconds(),
            persistent_history: true,
            store_audio: false,
            thread_count: default_thread_count(),
            overlay_size: default_overlay_size(),
            dim_delay_seconds: default_dim_delay_seconds(),
            dim_opacity_percent: default_dim_opacity_percent(),
            ultra_minimal_mode: false,
            brighten_on_speech: true,
            fullscreen_behavior: default_fullscreen_behavior(),
            first_run_completed: false,
            smart_structure_enabled: false,
            smart_sentence_structure: true,
            smart_auto_bullets: true,
            smart_aggressiveness: default_smart_aggressiveness(),
            smart_bullet_style: default_smart_bullet_style(),
        }
    }
}

/// Thread-safe settings manager providing atomic read and write operations.
#[derive(Clone)]
pub struct SettingsManager {
    inner: Arc<RwLock<AppSettings>>,
}

impl Default for SettingsManager {
    fn default() -> Self {
        Self::new(AppSettings::default())
    }
}

impl SettingsManager {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            inner: Arc::new(RwLock::new(settings)),
        }
    }

    /// Retrieve the current application settings snapshot.
    pub fn get(&self) -> AppSettings {
        self.inner.read().expect("Lock poisoned").clone()
    }

    /// Set in-memory settings.
    pub fn set(&self, new_settings: AppSettings) {
        let mut guard = self.inner.write().expect("Lock poisoned");
        *guard = new_settings;
    }

    /// Load settings from disk. If the file does not exist, writes defaults atomically.
    /// If corrupted, backs up corrupted file and falls back to default.
    pub fn load_from_file(path: &Path) -> Result<Self, SettingsError> {
        if !path.exists() {
            let defaults = AppSettings::default();
            atomic_write_json(path, &defaults)?;
            return Ok(Self::new(defaults));
        }

        let content = fs::read_to_string(path)?;
        match serde_json::from_str::<AppSettings>(&content) {
            Ok(settings) => Ok(Self::new(settings)),
            Err(e) => {
                log::warn!("Corrupted settings.json at {}: {e}. Backing up and resetting to default.", path.display());
                let backup_path = path.with_extension(format!("corrupted.{}.json", chrono::Utc::now().timestamp()));
                let _ = fs::rename(path, backup_path);
                let defaults = AppSettings::default();
                atomic_write_json(path, &defaults)?;
                Ok(Self::new(defaults))
            }
        }
    }

    /// Update settings in memory and persist atomically to disk.
    pub fn update_and_save(&self, path: &Path, new_settings: AppSettings) -> Result<(), SettingsError> {
        atomic_write_json(path, &new_settings)?;
        self.set(new_settings);
        Ok(())
    }
}

/// Atomically write JSON data to path using write-to-temp-then-rename pattern.
pub fn atomic_write_json<T: Serialize>(path: &Path, data: &T) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let json_bytes = serde_json::to_vec_pretty(data)?;

    let temp_path = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
    {
        let mut file = File::create(&temp_path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
    }

    // Atomic replacement (MoveFileExW with MOVEFILE_REPLACE_EXISTING on Windows)
    let mut last_err = None;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        match fs::rename(&temp_path, path) {
            Ok(()) => {
                last_err = None;
                break;
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }

    if let Some(e) = last_err {
        let _ = fs::remove_file(&temp_path);
        return Err(SettingsError::Io(e));
    }

    Ok(())
}

/// Locate standard settings path under %APPDATA%\Murmur\settings.json.
pub fn get_default_settings_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Murmur").join("settings.json")
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
        proj_dirs.config_dir().join("settings.json")
    } else {
        PathBuf::from("settings.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = AppSettings::default();
        assert_eq!(s.hotkey, "Ctrl+Shift+Space");
        assert_eq!(s.hotkey_mode, HotkeyMode::HoldToTalk);
        assert_eq!(s.insertion_mode, InsertionMode::PasteWithRestore);
        assert!(s.auto_insert);
        assert!(!s.launch_on_startup);
        assert_eq!(s.history_retention_max, 100);
        assert_eq!(s.theme, "dark");
    }

    #[test]
    fn test_atomic_save_and_load() {
        let temp_dir = std::env::temp_dir().join(format!("murmur_test_{}", uuid::Uuid::new_v4()));
        let settings_path = temp_dir.join("settings.json");

        // First load creates defaults
        let manager = SettingsManager::load_from_file(&settings_path).unwrap();
        assert_eq!(manager.get(), AppSettings::default());
        assert!(settings_path.exists());

        // Update settings
        let custom = AppSettings {
            hotkey: "Alt+Space".to_string(),
            hotkey_mode: HotkeyMode::Hybrid,
            history_retention_max: 250,
            ..Default::default()
        };

        manager.update_and_save(&settings_path, custom.clone()).unwrap();

        // Reload from disk
        let reloaded = SettingsManager::load_from_file(&settings_path).unwrap();
        assert_eq!(reloaded.get(), custom);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_corrupted_file_recovery() {
        let temp_dir = std::env::temp_dir().join(format!("murmur_test_{}", uuid::Uuid::new_v4()));
        let settings_path = temp_dir.join("settings.json");
        fs::create_dir_all(&temp_dir).unwrap();

        // Write corrupt JSON
        fs::write(&settings_path, "NOT VALID JSON {{{{").unwrap();

        // Loading should recover without error and return default settings
        let manager = SettingsManager::load_from_file(&settings_path).unwrap();
        assert_eq!(manager.get(), AppSettings::default());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
