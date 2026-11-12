use crate::audio::enumerate_input_devices;
use crate::storage::AppSettings;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Locate default log directory under %APPDATA%\Murmur\logs.
pub fn get_default_log_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Murmur").join("logs")
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
        proj_dirs.data_dir().join("logs")
    } else {
        PathBuf::from("logs")
    }
}

/// Initialize tracing subscriber with daily rolling file appender and stdout formatting.
/// Returns WorkerGuard which must be retained for the lifetime of the application to flush logs on shutdown.
pub fn init_logging(
    log_dir: &Path,
) -> Result<tracing_appender::non_blocking::WorkerGuard, String> {
    if !log_dir.exists() {
        fs::create_dir_all(log_dir).map_err(|e| format!("Failed to create log dir: {e}"))?;
    }

    // Bridge log crate records to tracing
    let _ = tracing_log::LogTracer::init();

    let file_appender = tracing_appender::rolling::daily(log_dir, "murmur.log");
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,murmur=debug,whispertype=debug,whispertype_lib=debug"));

    let file_layer = fmt::layer()
        .with_writer(non_blocking_file)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true);

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_target(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init()
        .map_err(|e| format!("Failed to initialize tracing subscriber: {e}"))?;

    tracing::info!("Tracing logger initialized. Log directory: {}", log_dir.display());

    Ok(guard)
}

/// Redact sensitive user data (e.g. username, user profile directory) from text strings.
pub fn redact_sensitive_info(text: &str) -> String {
    let mut redacted = text.to_string();

    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        if !userprofile.is_empty() {
            redacted = redacted.replace(&userprofile, r"C:\Users\<USER>");
            let userprofile_slash = userprofile.replace('\\', "/");
            redacted = redacted.replace(&userprofile_slash, "C:/Users/<USER>");

            // Also check for lowercase drive letter (e.g. c:\Users\... or c:/Users/...)
            if let Some(first_char) = userprofile.chars().next() {
                if first_char.is_ascii_uppercase() {
                    let mut lower_drive = userprofile.clone();
                    let lower_first = first_char.to_ascii_lowercase();
                    lower_drive.replace_range(0..1, &lower_first.to_string());
                    redacted = redacted.replace(&lower_drive, r"C:\Users\<USER>");
                    let lower_slash = lower_drive.replace('\\', "/");
                    redacted = redacted.replace(&lower_slash, "C:/Users/<USER>");
                }
            }
        }
    }

    if let Ok(username) = std::env::var("USERNAME") {
        if !username.is_empty() && username.len() > 1 {
            redacted = redacted.replace(&username, "<USER>");
        }
    }

    redacted
}

/// Read the last `max_lines` from the most recent log file in `log_dir`.
pub fn read_log_tail(log_dir: &Path, max_lines: usize) -> Vec<String> {
    if !log_dir.exists() {
        return Vec::new();
    }

    let mut log_files = Vec::new();
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("murmur.log") {
                        if let Ok(meta) = entry.metadata() {
                            if let Ok(modified) = meta.modified() {
                                log_files.push((modified, path));
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort newest first with path as deterministic tie-breaker
    log_files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    if let Some((_, latest_path)) = log_files.first() {
        if let Ok(file) = File::open(latest_path) {
            let reader = BufReader::new(file);
            let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
            let start = if lines.len() > max_lines {
                lines.len() - max_lines
            } else {
                0
            };
            return lines[start..].to_vec();
        }
    }

    Vec::new()
}

/// Generate a full redacted diagnostic report including system info, settings, audio devices, and log tail.
pub fn generate_diagnostics_report(settings: &AppSettings, log_dir: &Path) -> String {
    let os_name = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let version = env!("CARGO_PKG_VERSION");
    let cpus = std::thread::available_parallelism()
        .map(|p| p.get().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let devices_str = match enumerate_input_devices() {
        Ok(devs) => {
            if devs.is_empty() {
                "  No input devices found.".to_string()
            } else {
                devs.iter()
                    .map(|d| {
                        format!(
                            "  - {}{}",
                            d.name,
                            if d.is_default { " (Default)" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Err(e) => format!("  Error querying devices: {e}"),
    };

    let settings_json = serde_json::to_string_pretty(settings)
        .unwrap_or_else(|_| "<failed to serialize settings>".to_string());

    let log_lines = read_log_tail(log_dir, 50);
    let log_tail = if log_lines.is_empty() {
        "  (No recent log entries found)".to_string()
    } else {
        log_lines
            .iter()
            .map(|l| format!("  {}", redact_sensitive_info(l)))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let raw_report = format!(
        r#"==================================================
MURMUR SYSTEM DIAGNOSTICS REPORT
Generated: {}
==================================================

[SYSTEM]
Version: {}
Platform: {} ({})
Logical Cores: {}

[AUDIO DEVICES]
{}

[SETTINGS]
{}

[RECENT LOGS (Last 50 lines)]
{}
==================================================
"#,
        chrono::Utc::now().to_rfc3339(),
        version,
        os_name,
        arch,
        cpus,
        devices_str,
        settings_json,
        log_tail
    );

    redact_sensitive_info(&raw_report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_sensitive_info() {
        if let Ok(username) = std::env::var("USERNAME") {
            let input = format!("Error at C:\\Users\\{}\\AppData\\Roaming\\Murmur", username);
            let redacted = redact_sensitive_info(&input);
            assert!(!redacted.contains(&username));
            assert!(redacted.contains("<USER>"));
        }
    }

    #[test]
    fn test_generate_diagnostics_report() {
        let settings = AppSettings::default();
        let temp_dir = std::env::temp_dir().join(format!("murmur_log_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let report = generate_diagnostics_report(&settings, &temp_dir);
        assert!(report.contains("MURMUR SYSTEM DIAGNOSTICS REPORT"));
        assert!(report.contains("[SYSTEM]"));
        assert!(report.contains("[SETTINGS]"));
        assert!(report.contains("Ctrl+Shift+Space"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_log_tail() {
        let temp_dir = std::env::temp_dir().join(format!("murmur_log_tail_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let log_file = temp_dir.join("murmur.log.2026-09-22");

        let mut lines = Vec::new();
        for i in 1..=60 {
            lines.push(format!("Log line {i}"));
        }
        fs::write(&log_file, lines.join("\n")).unwrap();

        let tail = read_log_tail(&temp_dir, 10);
        assert_eq!(tail.len(), 10);
        assert_eq!(tail.last().unwrap(), "Log line 60");
        assert_eq!(tail.first().unwrap(), "Log line 51");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
