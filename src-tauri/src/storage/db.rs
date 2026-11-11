use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Database lock poisoned")]
    LockPoisoned,

    #[error("Path error: {0}")]
    PathError(String),

    #[error("Record not found with id: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryRecord {
    pub id: String,
    pub raw_text: String,
    pub final_text: String,
    pub created_at: String,
    pub duration_ms: u32,
    pub language: String,
    pub model_variant: String,
    pub provider: String,
    pub inference_ms: u32,
    /// Professional Mode: the LLM-refined text (None for normal dictation entries).
    pub refined_text: Option<String>,
    /// Recording mode: "normal" | "professional" (None for legacy entries).
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewHistoryRecord {
    pub raw_text: String,
    pub final_text: String,
    pub duration_ms: u32,
    pub language: Option<String>,
    pub model_variant: Option<String>,
    pub provider: Option<String>,
    pub inference_ms: Option<u32>,
    /// Professional Mode only: LLM-refined output text.
    pub refined_text: Option<String>,
    /// Recording mode: "normal" | "professional".
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryRecord {
    pub id: String,
    pub find_text: String,
    pub replace_text: String,
    pub is_case_sensitive: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewDictionaryEntry {
    pub find_text: String,
    pub replace_text: String,
    pub is_case_sensitive: Option<bool>,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open SQLite database at given path, ensuring directory exists and WAL mode is enabled.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| DbError::PathError(format!("Failed to create DB directory: {e}")))?;
            }
        }

        let conn = Connection::open(path)?;
        Self::init_connection(conn)
    }

    /// Open an in-memory database for unit testing.
    pub fn in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        Self::init_connection(conn)
    }

    fn init_connection(conn: Connection) -> Result<Self, DbError> {
        // Configure WAL mode, normal synchronous, and 5000ms busy timeout
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        // Create tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                raw_text TEXT NOT NULL,
                final_text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                language TEXT NOT NULL,
                model_variant TEXT NOT NULL,
                provider TEXT NOT NULL,
                inference_ms INTEGER NOT NULL,
                refined_text TEXT,
                mode TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_history_created_at ON history (created_at DESC);

            CREATE TABLE IF NOT EXISTS dictionary (
                id TEXT PRIMARY KEY,
                find_text TEXT NOT NULL,
                replace_text TEXT NOT NULL,
                is_case_sensitive INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_dictionary_find ON dictionary (find_text);",
        )?;

        // Migrate existing history table: add refined_text and mode columns if they don't exist
        // (SQLite ADD COLUMN is safe and idempotent for schema evolution)
        for migration in &[
            "ALTER TABLE history ADD COLUMN refined_text TEXT",
            "ALTER TABLE history ADD COLUMN mode TEXT",
        ] {
            if let Err(e) = conn.execute_batch(migration) {
                // "duplicate column name" means migration already applied — ignore
                let err_str = e.to_string().to_lowercase();
                if !err_str.contains("duplicate column name") {
                    return Err(DbError::Sqlite(e));
                }
            }
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // ==========================================
    // HISTORY CRUD
    // ==========================================

    /// Insert a new history record and auto-prune oldest entries if exceeding `max_retention`.
    pub fn insert_history(
        &self,
        record: NewHistoryRecord,
        max_retention: usize,
    ) -> Result<HistoryRecord, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;

        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let language = record.language.unwrap_or_else(|| "en".to_string());
        let model_variant = record
            .model_variant
            .unwrap_or_else(|| "parakeet-tdt-0.6b-v3-int8".to_string());
        let provider = record.provider.unwrap_or_else(|| "cpu".to_string());
        let inference_ms = record.inference_ms.unwrap_or(0);

        conn.execute(
            "INSERT INTO history (id, raw_text, final_text, created_at, duration_ms, language, model_variant, provider, inference_ms, refined_text, mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id,
                &record.raw_text,
                &record.final_text,
                &created_at,
                record.duration_ms,
                &language,
                &model_variant,
                &provider,
                inference_ms,
                &record.refined_text,
                &record.mode,
            ],
        )?;

        // Auto-prune if max_retention > 0
        if max_retention > 0 {
            conn.execute(
                "DELETE FROM history WHERE id NOT IN (
                    SELECT id FROM history ORDER BY created_at DESC, id DESC LIMIT ?1
                )",
                params![max_retention as i64],
            )?;
        }

        Ok(HistoryRecord {
            id,
            raw_text: record.raw_text,
            final_text: record.final_text,
            created_at,
            duration_ms: record.duration_ms,
            language,
            model_variant,
            provider,
            inference_ms,
            refined_text: record.refined_text,
            mode: record.mode,
        })
    }

    /// Retrieve paginated history entries ordered newest first.
    pub fn get_history(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<HistoryRecord>, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;

        let limit = limit.unwrap_or(100).min(500) as i64;
        let offset = offset.unwrap_or(0) as i64;

        let mut stmt = conn.prepare(
            "SELECT id, raw_text, final_text, created_at, duration_ms, language, model_variant, provider, inference_ms, refined_text, mode
             FROM history ORDER BY created_at DESC, id DESC LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(HistoryRecord {
                id: row.get(0)?,
                raw_text: row.get(1)?,
                final_text: row.get(2)?,
                created_at: row.get(3)?,
                duration_ms: row.get(4)?,
                language: row.get(5)?,
                model_variant: row.get(6)?,
                provider: row.get(7)?,
                inference_ms: row.get(8)?,
                refined_text: row.get(9)?,
                mode: row.get(10)?,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    /// Delete a single history entry by ID.
    pub fn delete_history(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        let count = conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    /// Update a history entry's final text.
    pub fn update_history(&self, id: &str, final_text: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        let count = conn.execute(
            "UPDATE history SET final_text = ?1 WHERE id = ?2",
            params![final_text, id],
        )?;
        Ok(count > 0)
    }

    /// Clear all history records.
    pub fn clear_history(&self) -> Result<usize, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        let count = conn.execute("DELETE FROM history", [])?;
        Ok(count)
    }

    /// Export all history records as formatted JSON.
    pub fn export_history_json(&self) -> Result<String, DbError> {
        let entries = self.get_history(Some(10000), Some(0))?;
        serde_json::to_string_pretty(&entries).map_err(DbError::from)
    }

    // ==========================================
    // DICTIONARY CRUD
    // ==========================================

    /// Insert a new dictionary rule.
    pub fn insert_dictionary(
        &self,
        entry: NewDictionaryEntry,
    ) -> Result<DictionaryRecord, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;

        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let is_case_sensitive = entry.is_case_sensitive.unwrap_or(false);

        conn.execute(
            "INSERT INTO dictionary (id, find_text, replace_text, is_case_sensitive, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &id,
                &entry.find_text,
                &entry.replace_text,
                if is_case_sensitive { 1 } else { 0 },
                &created_at,
            ],
        )?;

        Ok(DictionaryRecord {
            id,
            find_text: entry.find_text,
            replace_text: entry.replace_text,
            is_case_sensitive,
            created_at,
        })
    }

    /// List all dictionary replacement rules.
    pub fn get_dictionary(&self) -> Result<Vec<DictionaryRecord>, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;

        let mut stmt = conn.prepare(
            "SELECT id, find_text, replace_text, is_case_sensitive, created_at
             FROM dictionary ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let cs_num: i32 = row.get(3)?;
            Ok(DictionaryRecord {
                id: row.get(0)?,
                find_text: row.get(1)?,
                replace_text: row.get(2)?,
                is_case_sensitive: cs_num != 0,
                created_at: row.get(4)?,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    /// Update an existing dictionary rule.
    pub fn update_dictionary(&self, entry: DictionaryRecord) -> Result<bool, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;

        let count = conn.execute(
            "UPDATE dictionary
             SET find_text = ?1, replace_text = ?2, is_case_sensitive = ?3
             WHERE id = ?4",
            params![
                &entry.find_text,
                &entry.replace_text,
                if entry.is_case_sensitive { 1 } else { 0 },
                &entry.id,
            ],
        )?;

        Ok(count > 0)
    }

    /// Delete a dictionary rule by ID.
    pub fn delete_dictionary(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().map_err(|_| DbError::LockPoisoned)?;
        let count = conn.execute("DELETE FROM dictionary WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }
}

#[allow(dead_code)]
fn match_ignore_case_at(s: &str, prefix: &str) -> Option<usize> {
    let mut s_chars = s.chars();
    let mut p_chars = prefix.chars();
    let mut matched_bytes = 0;

    let mut s_pending: std::vec::IntoIter<char> = Vec::new().into_iter();
    let mut p_pending: std::vec::IntoIter<char> = Vec::new().into_iter();

    loop {
        let pc_opt = p_pending.next().or_else(|| {
            p_chars.next().map(|c| {
                let mut v: Vec<char> = c.to_lowercase().collect();
                let first = v.remove(0);
                p_pending = v.into_iter();
                first
            })
        });

        let pc = match pc_opt {
            Some(c) => c,
            None => {
                // Prefix is completely matched!
                return Some(matched_bytes);
            }
        };

        let sc_opt = s_pending.next().or_else(|| {
            s_chars.next().map(|c| {
                matched_bytes += c.len_utf8();
                let mut v: Vec<char> = c.to_lowercase().collect();
                let first = v.remove(0);
                s_pending = v.into_iter();
                first
            })
        });

        match sc_opt {
            Some(sc) if sc == pc => {}
            _ => return None,
        }
    }
}

/// Apply a sequence of dictionary replacement rules to transcribed text.
/// Enforces whole-word boundary matching (\b) so that substring matches (e.g., AMIN inside "contamination" or "vitamin")
/// are never inadvertently replaced.
pub fn apply_dictionary(text: &str, entries: &[DictionaryRecord]) -> String {
    let mut result = text.to_string();

    for entry in entries {
        let trimmed_find = entry.find_text.trim();
        if trimmed_find.is_empty() {
            continue;
        }

        // Determine word boundaries based on first and last characters of find_text
        let starts_word = trimmed_find
            .chars()
            .next()
            .map_or(false, |c| c.is_alphanumeric() || c == '_');
        let ends_word = trimmed_find
            .chars()
            .last()
            .map_or(false, |c| c.is_alphanumeric() || c == '_');

        let prefix = if starts_word { r"\b" } else { "" };
        let suffix = if ends_word { r"\b" } else { "" };
        let escaped = regex::escape(trimmed_find);

        let pattern = if entry.is_case_sensitive {
            format!(r"{prefix}{escaped}{suffix}")
        } else {
            format!(r"(?i){prefix}{escaped}{suffix}")
        };

        if let Ok(re) = regex::Regex::new(&pattern) {
            result = re
                .replace_all(&result, regex::NoExpand(&entry.replace_text))
                .into_owned();
        }
    }

    result
}

/// Locate the standard Murmur database path under %APPDATA%\Murmur\history.db.
pub fn get_default_db_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Murmur").join("history.db")
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
        proj_dirs.data_dir().join("history.db")
    } else {
        PathBuf::from("history.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_crud_and_autoprune() {
        let db = Database::in_memory().unwrap();

        // 1. Insert records
        for i in 1..=5 {
            db.insert_history(
                NewHistoryRecord {
                    raw_text: format!("Raw {i}"),
                    final_text: format!("Final {i}"),
                    duration_ms: i * 1000,
                    language: Some("en".into()),
                    model_variant: None,
                    provider: None,
                    inference_ms: Some(100),
                    refined_text: None,
                    mode: Some("normal".into()),
                },
                3, // max retention = 3
            )
            .unwrap();
        }

        // With max_retention = 3, only the latest 3 should remain (5, 4, 3)
        let entries = db.get_history(None, None).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].final_text, "Final 5");
        assert_eq!(entries[1].final_text, "Final 4");
        assert_eq!(entries[2].final_text, "Final 3");

        // 2. Update single entry
        let id_to_update = &entries[1].id;
        assert!(db.update_history(id_to_update, "Edited Final 4").unwrap());
        let entries_updated = db.get_history(None, None).unwrap();
        assert_eq!(entries_updated[1].final_text, "Edited Final 4");

        // 3. Delete single entry
        let id_to_delete = &entries[0].id;
        assert!(db.delete_history(id_to_delete).unwrap());
        let entries_after = db.get_history(None, None).unwrap();
        assert_eq!(entries_after.len(), 2);

        // 4. Clear history
        assert_eq!(db.clear_history().unwrap(), 2);
        assert_eq!(db.get_history(None, None).unwrap().len(), 0);
    }

    #[test]
    fn test_dictionary_crud() {
        let db = Database::in_memory().unwrap();

        let r1 = db
            .insert_dictionary(NewDictionaryEntry {
                find_text: "Murmer".into(),
                replace_text: "Murmur".into(),
                is_case_sensitive: Some(false),
            })
            .unwrap();

        assert_eq!(r1.find_text, "Murmer");
        assert_eq!(r1.replace_text, "Murmur");
        assert!(!r1.is_case_sensitive);

        let list = db.get_dictionary().unwrap();
        assert_eq!(list.len(), 1);

        // Update
        let mut updated = r1.clone();
        updated.replace_text = "Murmur ASR".into();
        assert!(db.update_dictionary(updated).unwrap());

        let list_after = db.get_dictionary().unwrap();
        assert_eq!(list_after[0].replace_text, "Murmur ASR");

        // Delete
        assert!(db.delete_dictionary(&r1.id).unwrap());
        assert_eq!(db.get_dictionary().unwrap().len(), 0);
    }

    #[test]
    fn test_apply_dictionary_replacement() {
        let entries = vec![
            DictionaryRecord {
                id: "1".into(),
                find_text: "murmer".into(),
                replace_text: "Murmur".into(),
                is_case_sensitive: false,
                created_at: "".into(),
            },
            DictionaryRecord {
                id: "2".into(),
                find_text: "rust lang".into(),
                replace_text: "Rust".into(),
                is_case_sensitive: true,
                created_at: "".into(),
            },
        ];

        let input = "I am using MURMER with rust lang and not Rust Lang.";
        let output = apply_dictionary(input, &entries);
        assert_eq!(output, "I am using Murmur with Rust and not Rust Lang.");

        // Unicode case-insensitive tests
        let unicode_entries = vec![
            DictionaryRecord {
                id: "3".into(),
                find_text: "café".into(),
                replace_text: "espresso".into(),
                is_case_sensitive: false,
                created_at: "".into(),
            },
            DictionaryRecord {
                id: "4".into(),
                find_text: "test".into(),
                replace_text: "passed".into(),
                is_case_sensitive: false,
                created_at: "".into(),
            },
        ];
        let res_unicode = apply_dictionary("Enjoying CAFÉ in İSTANBUL test.", &unicode_entries);
        assert_eq!(res_unicode, "Enjoying espresso in İSTANBUL passed.");

        // Word boundary protection test: PORT -> TERMINAL must not alter important or transport
        let port_rule = vec![DictionaryRecord {
            id: "5".into(),
            find_text: "PORT".into(),
            replace_text: "TERMINAL".into(),
            is_case_sensitive: false,
            created_at: "".into(),
        }];
        let test_sentence = "AMIN detected high contamination and took vitamin D with AMIN.";
        let res_port = apply_dictionary(test_sentence, &port_rule);
        assert_eq!(
            res_port,
            "TERMINAL detected important matters and took transport with TERMINAL."
        );
    }

    #[test]
    fn test_export_history_json() {
        let db = Database::in_memory().unwrap();
        db.insert_history(
            NewHistoryRecord {
                raw_text: "test".into(),
                final_text: "test".into(),
                duration_ms: 500,
                language: None,
                model_variant: None,
                provider: None,
                inference_ms: None,
                refined_text: None,
                mode: Some("normal".into()),
            },
            100,
        )
        .unwrap();

        let json = db.export_history_json().unwrap();
        assert!(json.contains("\"final_text\": \"test\""));
    }
}
