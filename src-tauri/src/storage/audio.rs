use hound::{WavSpec, WavWriter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedRecordingInfo {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub duration_seconds: f32,
}

pub fn get_default_recordings_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Murmur").join("recordings")
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
        proj_dirs.data_dir().join("recordings")
    } else {
        PathBuf::from("recordings")
    }
}

/// Save 16kHz mono PCM f32 samples to disk as a 16-bit WAV file named `<history_id>.wav`.
pub fn save_recording_file(
    recordings_dir: &Path,
    history_id: &str,
    samples: &[f32],
) -> Result<PathBuf, String> {
    if !recordings_dir.exists() {
        fs::create_dir_all(recordings_dir).map_err(|e| e.to_string())?;
    }

    let file_path = recordings_dir.join(format!("{history_id}.wav"));
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(&file_path, spec).map_err(|e| e.to_string())?;
    for &sample in samples {
        let int_val = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(int_val).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    log::info!("Saved audio recording to: {}", file_path.display());
    Ok(file_path)
}

/// Lists all `.wav` files in `recordings_dir`, sorted by modification time (newest first).
pub fn list_saved_recordings(recordings_dir: &Path) -> Result<Vec<SavedRecordingInfo>, String> {
    if !recordings_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries_with_mtime = Vec::new();
    let read_dir = fs::read_dir(recordings_dir).map_err(|e| e.to_string())?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("wav"))
                == Some(true)
        {
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let size_bytes = metadata.len();
            let duration_seconds = ((size_bytes.saturating_sub(44)) as f32) / 32000.0;
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let file_path = path.to_string_lossy().to_string();

            entries_with_mtime.push((
                mtime,
                SavedRecordingInfo {
                    id,
                    file_name,
                    file_path,
                    size_bytes,
                    duration_seconds,
                },
            ));
        }
    }

    entries_with_mtime.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(entries_with_mtime.into_iter().map(|(_, info)| info).collect())
}

/// Reads `<id>.wav` from `recordings_dir` and returns its raw `Vec<u8>` bytes.
pub fn get_recording_wav_bytes(recordings_dir: &Path, id: &str) -> Result<Vec<u8>, String> {
    if id.contains('/') || id.contains('\\') {
        return Err("Invalid recording ID: path separators are not allowed".to_string());
    }
    let file_path = recordings_dir.join(format!("{id}.wav"));
    if !file_path.exists() {
        return Err(format!("Recording file not found: {id}.wav"));
    }
    fs::read(&file_path).map_err(|e| e.to_string())
}

/// Deletes `<id>.wav` if it exists, returning `Ok(true)` if removed or `Ok(false)` if not found.
pub fn delete_recording_file(recordings_dir: &Path, id: &str) -> Result<bool, String> {
    if id.contains('/') || id.contains('\\') {
        return Err("Invalid recording ID: path separators are not allowed".to_string());
    }
    let file_path = recordings_dir.join(format!("{id}.wav"));
    if file_path.exists() {
        fs::remove_file(&file_path).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Deletes all `.wav` files in `recordings_dir` and returns the count deleted.
pub fn clear_all_recordings(recordings_dir: &Path) -> Result<usize, String> {
    if !recordings_dir.exists() {
        return Ok(0);
    }
    let mut deleted_count = 0;
    let read_dir = fs::read_dir(recordings_dir).map_err(|e| e.to_string())?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("wav"))
                == Some(true)
        {
            if fs::remove_file(&path).is_ok() {
                deleted_count += 1;
            }
        }
    }
    Ok(deleted_count)
}

/// Delete audio files for recordings that no longer exist in history.
pub fn prune_unreferenced_recordings(
    recordings_dir: &Path,
    active_history_ids: &[String],
) -> Result<usize, String> {
    if !recordings_dir.exists() {
        return Ok(0);
    }

    let mut removed_count = 0;
    let read_dir = fs::read_dir(recordings_dir).map_err(|e| e.to_string())?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("wav") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !active_history_ids.iter().any(|id| id == stem) {
                    if fs::remove_file(&path).is_ok() {
                        removed_count += 1;
                    }
                }
            }
        }
    }

    if removed_count > 0 {
        log::info!("Pruned {removed_count} unreferenced audio recording(s).");
    }

    Ok(removed_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_prune_recordings() {
        let temp_dir = std::env::temp_dir().join(format!("murmur_rec_test_{}", uuid::Uuid::new_v4()));
        let id1 = "rec_test_1";
        let id2 = "rec_test_2";
        let samples = vec![0.0f32, 0.5f32, -0.5f32, 0.0f32];

        // 1. Save two recordings
        let p1 = save_recording_file(&temp_dir, id1, &samples).unwrap();
        let p2 = save_recording_file(&temp_dir, id2, &samples).unwrap();
        assert!(p1.exists());
        assert!(p2.exists());

        // 2. Prune keeping only id1
        let active = vec![id1.to_string()];
        let pruned = prune_unreferenced_recordings(&temp_dir, &active).unwrap();
        assert_eq!(pruned, 1);
        assert!(p1.exists());
        assert!(!p2.exists());

        // 3. Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_saved_recordings_nonexistent_dir() {
        let temp_dir = std::env::temp_dir().join(format!("nonexistent_dir_{}", uuid::Uuid::new_v4()));
        let list = list_saved_recordings(&temp_dir).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_recordings_crud_and_clear() {
        let temp_dir = std::env::temp_dir().join(format!("murmur_crud_test_{}", uuid::Uuid::new_v4()));
        let id_a = "rec_a";
        let id_b = "rec_b";
        // 16000 samples = exactly 1.0 second of audio
        let samples = vec![0.1f32; 16000];

        // Save two files
        save_recording_file(&temp_dir, id_a, &samples).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(15));
        save_recording_file(&temp_dir, id_b, &samples).unwrap();

        // 1. list_saved_recordings
        let list = list_saved_recordings(&temp_dir).unwrap();
        assert_eq!(list.len(), 2);
        // id_b is newer, should be first
        assert_eq!(list[0].id, id_b);
        assert_eq!(list[1].id, id_a);
        assert_eq!(list[0].file_name, format!("{id_b}.wav"));
        assert!((list[0].duration_seconds - 1.0).abs() < 0.01);
        assert_eq!(list[0].size_bytes, 44 + 32000);

        // 2. get_recording_wav_bytes
        let bytes_a = get_recording_wav_bytes(&temp_dir, id_a).unwrap();
        assert_eq!(bytes_a.len(), 44 + 32000);
        assert_eq!(&bytes_a[0..4], b"RIFF");
        assert_eq!(&bytes_a[8..12], b"WAVE");

        // Path separator validation
        assert!(get_recording_wav_bytes(&temp_dir, "../sneaky").is_err());
        assert!(get_recording_wav_bytes(&temp_dir, "sub\\file").is_err());
        assert!(get_recording_wav_bytes(&temp_dir, "nonexistent").is_err());

        // 3. delete_recording_file
        assert!(delete_recording_file(&temp_dir, "../sneaky").is_err());
        let deleted = delete_recording_file(&temp_dir, id_a).unwrap();
        assert!(deleted);
        let deleted_again = delete_recording_file(&temp_dir, id_a).unwrap();
        assert!(!deleted_again);
        let list_after_delete = list_saved_recordings(&temp_dir).unwrap();
        assert_eq!(list_after_delete.len(), 1);
        assert_eq!(list_after_delete[0].id, id_b);

        // 4. clear_all_recordings
        // Add another recording first
        save_recording_file(&temp_dir, "rec_c", &samples).unwrap();
        let cleared = clear_all_recordings(&temp_dir).unwrap();
        assert_eq!(cleared, 2);
        let list_after_clear = list_saved_recordings(&temp_dir).unwrap();
        assert!(list_after_clear.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
