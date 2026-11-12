pub mod db;
pub mod settings;

pub use db::{
    apply_dictionary, get_default_db_path, Database, DbError, DictionaryRecord, HistoryRecord,
    NewDictionaryEntry, NewHistoryRecord,
};
pub use settings::{
    atomic_write_json, get_default_settings_path, AppSettings, HotkeyMode, InsertionMode,
    SettingsError, SettingsManager,
};
