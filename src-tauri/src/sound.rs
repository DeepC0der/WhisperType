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

    fn mciGetErrorStringW(
        fdwError: u32,
        lpszErrorText: *mut u16,
        cchErrorText: u32,
    ) -> u32;
}

#[cfg(target_os = "windows")]
pub static MCI_MUTEX: Mutex<()> = Mutex::new(());

#[cfg(target_os = "windows")]
pub trait MciBackend: Send + Sync {
    fn send_cmd(&self, cmd: &str) -> Result<(), u32>;
}

#[cfg(target_os = "windows")]
pub struct RealMciBackend;

#[cfg(target_os = "windows")]
impl MciBackend for RealMciBackend {
    fn send_cmd(&self, cmd: &str) -> Result<(), u32> {
        let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
        let err = unsafe { mciSendStringW(wide.as_ptr(), std::ptr::null_mut(), 0, 0) };
        if err == 0 {
            Ok(())
        } else {
            Err(err)
        }
    }
}

#[cfg(target_os = "windows")]
static ACTIVE_MCI_BACKEND: Mutex<Option<Box<dyn MciBackend>>> = Mutex::new(None);

#[cfg(target_os = "windows")]
fn send_mci_cmd(cmd: &str) -> Result<(), u32> {
    let custom = {
        let guard = ACTIVE_MCI_BACKEND.lock().unwrap_or_else(|p| p.into_inner());
        guard.as_ref().map(|_| ())
    };
    if custom.is_some() {
        let guard = ACTIVE_MCI_BACKEND.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(ref backend) = *guard {
            return backend.send_cmd(cmd);
        }
    }
    RealMciBackend.send_cmd(cmd)
}

#[cfg(target_os = "windows")]
pub fn set_test_mci_backend(backend: Option<Box<dyn MciBackend>>) {
    let mut guard = ACTIVE_MCI_BACKEND.lock().unwrap_or_else(|p| p.into_inner());
    *guard = backend;
}

#[cfg(target_os = "windows")]
fn get_mci_error_message(err_code: u32) -> String {
    let mut buffer = [0u16; 256];
    let res = unsafe { mciGetErrorStringW(err_code, buffer.as_mut_ptr(), buffer.len() as u32) };
    if res != 0 {
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len])
    } else {
        format!("MCI error code {err_code}")
    }
}

/// Normalizes a path into a clean absolute Windows path suitable for legacy MCI commands.
/// Canonicalizes relative components, replaces forward slashes, and strips the extended prefix `\\?\`.
pub fn clean_path_for_mci(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();
    if let Some(stripped) = path_str.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{stripped}")
    } else if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path_str.to_string()
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
            let canonical = candidate.canonicalize().unwrap_or_else(|_| candidate.clone());
            return Ok(canonical);
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

/// Plays the specified audio cue file directly using native platform audio.
/// On Windows, this leverages winmm.dll mciSendStringW for zero-dependency, sub-millisecond asynchronous playback.
pub fn play_sound_file(sound_path: &Path, cue: &str) -> Result<(), String> {
    let cue_lower = cue.trim().to_lowercase();
    if cue_lower != "start" && cue_lower != "stop" {
        return Err(format!(
            "Unknown audio cue '{cue}'. Supported cues are 'start' and 'stop'"
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let _guard = MCI_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

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

            let clean_path = clean_path_for_mci(sound_path);
            let open_cmd = format!(r#"open "{clean_path}" type mpegvideo alias {alias}"#);

            send_mci_cmd(&open_cmd).map_err(|e| {
                format!(
                    "Failed to open audio device: {} (MCI code {e}) for {clean_path}",
                    get_mci_error_message(e)
                )
            })?;
            send_mci_cmd(&play_cmd).map_err(|e| {
                format!(
                    "Failed to play audio cue: {} (MCI code {e}) for {clean_path}",
                    get_mci_error_message(e)
                )
            })?;
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

/// Plays the specified audio cue ("start" or "stop") using the Tauri AppHandle to resolve the path.
pub fn play_audio_cue(app: &AppHandle, cue: &str) -> Result<(), String> {
    let sound_path = resolve_sound_path(app, cue)?;
    play_sound_file(&sound_path, cue)
}

/// Pre-opens both audio cue devices on application startup for sub-millisecond first playback latency.
pub fn preload_audio_cues(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let _guard = MCI_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        for cue in &["start", "stop"] {
            if let Ok(path) = resolve_sound_path(app, cue) {
                let clean_path = clean_path_for_mci(&path);
                let alias = format!("murmur_{cue}");
                let _ = send_mci_cmd(&format!("close {alias}"));
                let open_cmd = format!(r#"open "{clean_path}" type mpegvideo alias {alias}"#);
                if let Err(e) = send_mci_cmd(&open_cmd) {
                    log::debug!("MCI preload skipped for {alias}: code {e}");
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
}

/// Stops any currently playing audio cues.
pub fn stop_audio_cues() {
    #[cfg(target_os = "windows")]
    {
        let _guard = MCI_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let _ = send_mci_cmd("stop murmur_start");
        let _ = send_mci_cmd("stop murmur_stop");
    }
}

/// Closes the MCI audio cue devices, releasing underlying file handles and resources.
pub fn close_audio_cues() {
    #[cfg(target_os = "windows")]
    {
        let _guard = MCI_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let _ = send_mci_cmd("close murmur_start");
        let _ = send_mci_cmd("close murmur_stop");
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

    #[test]
    fn test_clean_path_for_mci() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let start_path = resolve_sound_path_with_dirs("start", None, manifest_dir).unwrap();
        let cleaned = clean_path_for_mci(&start_path);
        assert!(!cleaned.starts_with(r"\\?\"), "Clean path must not have extended prefix");
        assert!(!cleaned.contains('/'), "Clean path must not have forward slashes");
        assert!(cleaned.ends_with("start.mp3"));
    }

    #[cfg(target_os = "windows")]
    struct MockMciBackend {
        commands: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        opened: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    }

    #[cfg(target_os = "windows")]
    impl MockMciBackend {
        fn new(commands: std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> Self {
            Self {
                commands,
                opened: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            }
        }
    }

    #[cfg(target_os = "windows")]
    impl MciBackend for MockMciBackend {
        fn send_cmd(&self, cmd: &str) -> Result<(), u32> {
            self.commands.lock().unwrap().push(cmd.to_string());
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                return Ok(());
            }
            match parts[0] {
                "open" => {
                    if let Some(pos) = parts.iter().position(|&p| p == "alias") {
                        if let Some(alias) = parts.get(pos + 1) {
                            self.opened.lock().unwrap().insert(alias.to_string());
                        }
                    }
                    Ok(())
                }
                "close" => {
                    if let Some(alias) = parts.get(1) {
                        self.opened.lock().unwrap().remove(*alias);
                    }
                    Ok(())
                }
                "play" => {
                    if let Some(alias) = parts.get(1) {
                        if self.opened.lock().unwrap().contains(*alias) {
                            Ok(())
                        } else {
                            Err(263) // Device not open
                        }
                    } else {
                        Ok(())
                    }
                }
                _ => Ok(()),
            }
        }
    }

    #[cfg(target_os = "windows")]
    static TEST_SOUND_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(target_os = "windows")]
    #[test]
    fn test_play_sound_file_lifecycle_and_alternation() {
        let _test_lock = TEST_SOUND_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let commands = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockMciBackend::new(commands.clone());
        set_test_mci_backend(Some(Box::new(mock)));

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let start_path = resolve_sound_path_with_dirs("start", None, manifest_dir).unwrap();
        let stop_path = resolve_sound_path_with_dirs("stop", None, manifest_dir).unwrap();

        // 1. Play start cue
        let res_start = play_sound_file(&start_path, "start");
        assert_eq!(res_start, Ok(()), "play_sound_file for start should succeed");

        // 2. Play stop cue, which stops start cue
        let res_stop = play_sound_file(&stop_path, "stop");
        assert_eq!(res_stop, Ok(()), "play_sound_file for stop should succeed");

        // 3. Replay start cue immediately (active alternation)
        let res_start2 = play_sound_file(&start_path, "start");
        assert_eq!(res_start2, Ok(()), "Replaying start cue should succeed");

        // 4. Test stop_audio_cues
        stop_audio_cues();

        // 5. Test clean teardown
        close_audio_cues();

        set_test_mci_backend(None);

        let history = commands.lock().unwrap().clone();
        assert!(history.iter().any(|c| c.contains("open")));
        assert!(history.iter().any(|c| c.contains("play murmur_start")));
        assert!(history.iter().any(|c| c.contains("stop murmur_stop")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_mci_commands_playback_cycle() {
        let _test_lock = TEST_SOUND_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let commands = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockMciBackend::new(commands.clone());
        set_test_mci_backend(Some(Box::new(mock)));

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let start_path = resolve_sound_path_with_dirs("start", None, manifest_dir).unwrap();
        let clean_path = clean_path_for_mci(&start_path);
        let alias = "murmur_unit_test";

        let _ = send_mci_cmd(&format!("close {alias}"));
        let open_res = send_mci_cmd(&format!(r#"open "{clean_path}" type mpegvideo alias {alias}"#));
        assert_eq!(open_res, Ok(()));
        let play_res = send_mci_cmd(&format!("play {alias} from 0"));
        assert_eq!(play_res, Ok(()));
        let stop_res = send_mci_cmd(&format!("stop {alias}"));
        assert_eq!(stop_res, Ok(()));
        let close_res = send_mci_cmd(&format!("close {alias}"));
        assert_eq!(close_res, Ok(()));

        set_test_mci_backend(None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_mci_replay_lifecycle() {
        let _test_lock = TEST_SOUND_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let commands = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock = MockMciBackend::new(commands.clone());
        set_test_mci_backend(Some(Box::new(mock)));

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let start_path = resolve_sound_path_with_dirs("start", None, manifest_dir).unwrap();
        let clean_path = clean_path_for_mci(&start_path);
        let alias = "murmur_test_replay";

        let _ = send_mci_cmd(&format!("close {alias}"));
        let open_res = send_mci_cmd(&format!(r#"open "{clean_path}" type mpegvideo alias {alias}"#));
        assert_eq!(open_res, Ok(()));

        let play1 = send_mci_cmd(&format!("play {alias} from 0"));
        assert_eq!(play1, Ok(()));
        let play2 = send_mci_cmd(&format!("play {alias} from 0"));
        assert_eq!(play2, Ok(()));
        let stop = send_mci_cmd(&format!("stop {alias}"));
        assert_eq!(stop, Ok(()));
        let play3 = send_mci_cmd(&format!("play {alias} from 0"));
        assert_eq!(play3, Ok(()));
        let _ = send_mci_cmd(&format!("close {alias}"));

        set_test_mci_backend(None);
    }
}
