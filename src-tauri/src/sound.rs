use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::sync::Mutex;

#[cfg(target_os = "windows")]
#[link(name = "winmm")]
extern "system" {
    fn mciSendStringW(
        lpstrCommand: *const u16,
        lpstrReturnString: *mut u16,
        uReturnLength: u32,
        hwndCallback: isize,
    ) -> u32;
}

#[cfg(target_os = "windows")]
static MCI_MUTEX: Mutex<()> = Mutex::new(());

#[cfg(target_os = "windows")]
fn send_mci_cmd(cmd: &str) -> Result<(), u32> {
    let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
    let err = unsafe { mciSendStringW(wide.as_ptr(), std::ptr::null_mut(), 0, 0) };
    if err == 0 {
        Ok(())
    } else {
        Err(err)
    }
}

/// Resolves the absolute path to an audio cue file (e.g. `start.mp3` or `stop.mp3`).
/// Checks bundled resources first, then cargo manifest dev paths, current working dir, and executable dir.
pub fn resolve_sound_path_with_dirs(
    cue: &str,
    resource_dir: Option<&Path>,
    manifest_dir: &Path,
) -> Result<PathBuf, String> {
    let cue_lower = cue.trim().to_lowercase();
    if cue_lower != "start" && cue_lower != "stop" {
        return Err(format!(
            "Unknown audio cue '{cue}'. Supported cues are 'start' and 'stop'"
        ));
    }

    let filename = format!("{cue_lower}.mp3");
    let mut candidates = Vec::new();

    // 1. Release bundle resource directory
    if let Some(res_dir) = resource_dir {
        candidates.push(res_dir.join("sounds").join(&filename));
        candidates.push(res_dir.join(&filename));
        candidates.push(res_dir.join("src").join("assets").join("sounds").join(&filename));
    }

    // 2. Dev environment: CARGO_MANIFEST_DIR/../src/assets/sounds/
    candidates.push(manifest_dir.join("../src/assets/sounds").join(&filename));
    candidates.push(manifest_dir.join("src/assets/sounds").join(&filename));

    // 3. Current working directory
    candidates.push(PathBuf::from("src/assets/sounds").join(&filename));
    candidates.push(PathBuf::from("../src/assets/sounds").join(&filename));

    // 4. Relative to current executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("sounds").join(&filename));
            candidates.push(exe_dir.join(&filename));
            candidates.push(exe_dir.join("resources").join("sounds").join(&filename));
            candidates.push(exe_dir.join("resources").join(&filename));
        }
    }

    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }

    let searched = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");

    Err(format!(
        "Audio cue file '{filename}' not found. Searched locations:\n  {searched}"
    ))
}

/// Resolves the audio cue file path given the active Tauri AppHandle.
pub fn resolve_sound_path(app: &AppHandle, cue: &str) -> Result<PathBuf, String> {
    let res_dir = app.path().resource_dir().ok();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    resolve_sound_path_with_dirs(cue, res_dir.as_deref(), manifest_dir)
}

/// Plays the specified audio cue ("start" or "stop") using native platform audio.
/// On Windows, this leverages winmm.dll mciSendStringW for zero-dependency, low-latency playback.
pub fn play_audio_cue(app: &AppHandle, cue: &str) -> Result<(), String> {
    let cue_lower = cue.trim().to_lowercase();
    let sound_path = resolve_sound_path(app, &cue_lower)?;

    #[cfg(target_os = "windows")]
    {
        let _guard = MCI_MUTEX.lock().map_err(|_| "MCI lock poisoned".to_string())?;

        let alias = format!("murmur_{cue_lower}");
        let other_alias = if cue_lower == "start" {
            "murmur_stop"
        } else {
            "murmur_start"
        };

        // Stop the opposite cue if still playing so cues do not clash
        let _ = send_mci_cmd(&format!("stop {other_alias}"));

        // Try playing directly from 0 if already opened
        let play_cmd = format!("play {alias} from 0");
        if send_mci_cmd(&play_cmd).is_err() {
            // If device wasn't opened yet or failed, close any stale state and open
            let _ = send_mci_cmd(&format!("close {alias}"));

            let path_str = sound_path.to_string_lossy();
            let clean_path = path_str.strip_prefix(r"\\?\").unwrap_or(&path_str);
            let open_cmd = format!(r#"open "{clean_path}" type mpegvideo alias {alias}"#);

            send_mci_cmd(&open_cmd)
                .map_err(|e| format!("Failed to open audio device (MCI code {e}) for {clean_path}"))?;
            send_mci_cmd(&play_cmd)
                .map_err(|e| format!("Failed to play audio cue (MCI code {e}) for {clean_path}"))?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = sound_path;
        log::info!("Audio cues are natively supported via winmm on Windows. Cue '{cue_lower}' skipped.");
        Ok(())
    }
}

/// Stops any currently playing audio cues.
pub fn stop_audio_cues() {
    #[cfg(target_os = "windows")]
    {
        if let Ok(_guard) = MCI_MUTEX.lock() {
            let _ = send_mci_cmd("stop murmur_start");
            let _ = send_mci_cmd("stop murmur_stop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_sound_path_dev() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let start_path = resolve_sound_path_with_dirs("start", None, manifest_dir)
            .expect("start.mp3 should be found in dev path");
        assert!(start_path.exists(), "start.mp3 must exist on disk");
        assert!(start_path.metadata().unwrap().len() > 0);

        let stop_path = resolve_sound_path_with_dirs("stop", None, manifest_dir)
            .expect("stop.mp3 should be found in dev path");
        assert!(stop_path.exists(), "stop.mp3 must exist on disk");
        assert!(stop_path.metadata().unwrap().len() > 0);
    }

    #[test]
    fn test_resolve_sound_path_unknown_cue() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let err = resolve_sound_path_with_dirs("unknown_cue", None, manifest_dir)
            .expect_err("unknown cue should return error");
        assert!(err.contains("Unknown audio cue"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_mci_commands_playback_cycle() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let start_path = resolve_sound_path_with_dirs("start", None, manifest_dir).unwrap();

        let path_str = start_path.to_string_lossy();
        let clean_path = path_str.strip_prefix(r"\\?\").unwrap_or(&path_str);
        let alias = "murmur_unit_test";

        let _ = send_mci_cmd(&format!("close {alias}"));

        let open_res = send_mci_cmd(&format!(r#"open "{clean_path}" type mpegvideo alias {alias}"#));
        assert_eq!(open_res, Ok(()), "MCI open should succeed");

        let play_res = send_mci_cmd(&format!("play {alias} from 0"));
        assert_eq!(play_res, Ok(()), "MCI play should succeed");

        std::thread::sleep(std::time::Duration::from_millis(100));

        let stop_res = send_mci_cmd(&format!("stop {alias}"));
        assert_eq!(stop_res, Ok(()), "MCI stop should succeed");

        let close_res = send_mci_cmd(&format!("close {alias}"));
        assert_eq!(close_res, Ok(()), "MCI close should succeed");
    }
}
