use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, Stream};
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Failed to query audio host: {0}")]
    HostError(String),

    #[error("No audio input device found")]
    NoDeviceFound,

    #[error("Microphone privacy restriction detected (0x80070005): Windows is blocking microphone access for desktop applications. Open ms-settings:privacy-microphone and enable 'Let desktop apps access your microphone'.")]
    PrivacyDenied,

    #[error("Audio device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to query input stream config: {0}")]
    ConfigError(String),

    #[error("Unsupported audio sample format: {0:?}")]
    UnsupportedFormat(SampleFormat),

    #[error("Failed to build audio input stream: {0}")]
    StreamBuildError(String),

    #[error("Audio stream playback/start error: {0}")]
    StreamPlayError(String),

    #[error("Audio capture is already running")]
    AlreadyRunning,

    #[error("Audio capture is not running")]
    NotRunning,

    #[error("Audio processing thread error: {0}")]
    ThreadError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct CapturedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Check if an error message indicates Windows microphone privacy denial (0x80070005 / Access Denied).
pub fn is_windows_privacy_error(err_str: &str) -> bool {
    let lower = err_str.to_lowercase();
    err_str.contains("0x80070005")
        || err_str.contains("80070005")
        || lower.contains("access is denied")
        || lower.contains("access denied")
        || lower.contains("privilege")
}

/// Enumerate available audio input devices via CPAL WASAPI.
pub fn enumerate_input_devices() -> Result<Vec<AudioDeviceInfo>, AudioError> {
    let host = cpal::default_host();
    let default_device_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let devices = match host.input_devices() {
        Ok(devs) => devs,
        Err(e) => {
            let err_str = e.to_string();
            if is_windows_privacy_error(&err_str) {
                return Err(AudioError::PrivacyDenied);
            }
            return Err(AudioError::HostError(err_str));
        }
    };

    let mut result = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            let is_default = default_device_name.as_deref() == Some(&name);
            result.push(AudioDeviceInfo { name, is_default });
        }
    }

    Ok(result)
}

/// Resolve target input device by name, falling back to default device if name is None.
pub fn resolve_input_device(
    host: &Host,
    device_name: Option<&str>,
) -> Result<Device, AudioError> {
    if let Some(target) = device_name {
        let trimmed = target.trim();
        if !trimmed.is_empty() {
            let mut devices = match host.input_devices() {
                Ok(devs) => devs,
                Err(e) => {
                    let err_str = e.to_string();
                    if is_windows_privacy_error(&err_str) {
                        return Err(AudioError::PrivacyDenied);
                    }
                    return Err(AudioError::HostError(err_str));
                }
            };
            if let Some(d) = devices.find(|d| {
                d.name()
                    .map(|n| n.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
            }) {
                return Ok(d);
            } else {
                return Err(AudioError::DeviceNotFound(trimmed.to_string()));
            }
        }
    }

    match host.default_input_device() {
        Some(d) => Ok(d),
        None => {
            if let Err(e) = host.input_devices() {
                let err_str = e.to_string();
                if is_windows_privacy_error(&err_str) {
                    return Err(AudioError::PrivacyDenied);
                }
            }
            Err(AudioError::NoDeviceFound)
        }
    }
}

/// Controller for capturing live microphone audio via CPAL WASAPI with lock-free buffering and RMS emission.
pub struct AudioCapture {
    is_recording: Arc<AtomicBool>,
    stream: Option<Stream>,
    worker_handle: Option<JoinHandle<Result<CapturedAudio, AudioError>>>,
    stop_signal_tx: Option<Sender<()>>,
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            stream: None,
            worker_handle: None,
            stop_signal_tx: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Start capturing audio. Buffers are allocated only during this session.
    pub fn start_recording(
        &mut self,
        device_name: Option<&str>,
        app_handle: Option<AppHandle>,
    ) -> Result<(), AudioError> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyRunning);
        }

        let host = cpal::default_host();
        let device = resolve_input_device(&host, device_name)?;

        let default_config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                let err_str = e.to_string();
                if is_windows_privacy_error(&err_str) {
                    return Err(AudioError::PrivacyDenied);
                }
                return Err(AudioError::ConfigError(err_str));
            }
        };

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();
        let sample_format = default_config.sample_format();

        // Lock-free unbounded channel from audio callback to collector thread
        let (audio_tx, audio_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) =
            crossbeam_channel::unbounded();
        let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);

        let err_fn = move |err: cpal::StreamError| {
            log::error!("CPAL audio stream error: {err}");
        };

        let stream_config: cpal::StreamConfig = default_config.into();

        // Build CPAL input stream with lock-free callback
        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx = audio_tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        let _ = tx.send(data.to_vec());
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let tx = audio_tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        let converted: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 32768.0).collect();
                        let _ = tx.send(converted);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let tx = audio_tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        let converted: Vec<f32> = data
                            .iter()
                            .map(|&s| ((s as i32) - 32768) as f32 / 32768.0)
                            .collect();
                        let _ = tx.send(converted);
                    },
                    err_fn,
                    None,
                )
            }
            _ => return Err(AudioError::UnsupportedFormat(sample_format)),
        }
        .map_err(|e| {
            let err_str = e.to_string();
            if is_windows_privacy_error(&err_str) {
                AudioError::PrivacyDenied
            } else {
                AudioError::StreamBuildError(err_str)
            }
        })?;

        stream
            .play()
            .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;

        self.stream = Some(stream);
        self.stop_signal_tx = Some(stop_tx);
        self.is_recording.store(true, Ordering::SeqCst);

        let is_recording_flag = Arc::clone(&self.is_recording);

        // Spawn background audio collector and RMS computation thread (~30 Hz)
        let worker_handle = std::thread::spawn(move || {
            let mut recorded_samples = Vec::<f32>::new();
            let mut rms_window = Vec::<f32>::new();
            let mut last_rms_emit = Instant::now();
            let rms_interval = Duration::from_millis(33); // ~30 Hz

            loop {
                // Check stop signal non-blockingly
                if stop_rx.try_recv().is_ok() || !is_recording_flag.load(Ordering::SeqCst) {
                    // Drain remaining samples in channel
                    while let Ok(chunk) = audio_rx.try_recv() {
                        recorded_samples.extend_from_slice(&chunk);
                    }
                    break;
                }

                // Wait for chunk with a short timeout to maintain 30 Hz RMS cadence even if stream is sparse
                match audio_rx.recv_timeout(Duration::from_millis(15)) {
                    Ok(chunk) => {
                        rms_window.extend_from_slice(&chunk);
                        recorded_samples.extend_from_slice(&chunk);
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }

                // Periodic RMS computation and emission
                if last_rms_emit.elapsed() >= rms_interval {
                    let level = if !rms_window.is_empty() {
                        let sum_sq: f32 = rms_window.iter().map(|&s| s * s).sum();
                        let rms = (sum_sq / rms_window.len() as f32).sqrt();
                        // Scale RMS (0.0 .. 0.5 typical speech) to a 0.0 .. 100.0 UI level
                        (rms * 200.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    };

                    if let Some(ref handle) = app_handle {
                        let _ = handle.emit("mic-level", level);
                    }

                    rms_window.clear();
                    last_rms_emit = Instant::now();
                }
            }

            // Reset UI mic level to 0.0 on stream completion
            if let Some(ref handle) = app_handle {
                let _ = handle.emit("mic-level", 0.0);
            }

            Ok(CapturedAudio {
                samples: recorded_samples,
                sample_rate,
                channels,
            })
        });

        self.worker_handle = Some(worker_handle);
        Ok(())
    }

    /// Stop recording, release audio stream & buffer memory, and return captured audio.
    pub fn stop_recording(&mut self) -> Result<CapturedAudio, AudioError> {
        if !self.is_recording.swap(false, Ordering::SeqCst) {
            return Err(AudioError::NotRunning);
        }

        // Send stop signal
        if let Some(tx) = self.stop_signal_tx.take() {
            let _ = tx.send(());
        }

        // Drop stream to stop OS audio driver
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // Wait for collector thread
        if let Some(handle) = self.worker_handle.take() {
            match handle.join() {
                Ok(res) => res,
                Err(_) => Err(AudioError::ThreadError(
                    "Worker collector thread panicked".into(),
                )),
            }
        } else {
            Err(AudioError::NotRunning)
        }
    }

    /// Cancel recording without returning audio, immediately freeing memory.
    pub fn cancel_recording(&mut self) {
        self.is_recording.store(false, Ordering::SeqCst);
        if let Some(tx) = self.stop_signal_tx.take() {
            let _ = tx.send(());
        }
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.cancel_recording();
    }
}

// CPAL Stream contains NotSendSyncAcrossAllPlatforms for cross-platform portability.
// On Windows WASAPI (and behind Mutex), it is safe to transfer and share across threads.
unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_error_detection() {
        assert!(is_windows_privacy_error("Error: 0x80070005"));
        assert!(is_windows_privacy_error("Access is denied"));
        assert!(is_windows_privacy_error("Access denied by system policy"));
        assert!(!is_windows_privacy_error("Device unplugged"));
        assert!(!is_windows_privacy_error("Buffer overrun"));
    }

    #[test]
    fn test_audio_capture_lifecycle_idle() {
        let capture = AudioCapture::new();
        assert!(!capture.is_recording());
    }

    #[test]
    fn test_enumerate_devices() {
        // Should not panic, returns Ok or PrivacyDenied
        let result = enumerate_input_devices();
        assert!(result.is_ok() || matches!(result, Err(AudioError::PrivacyDenied)));
    }

    #[test]
    fn test_resolve_input_device_not_found() {
        let host = cpal::default_host();
        let result = resolve_input_device(&host, Some("DefinitelyNonExistentMicrophone12345XYZ"));
        assert!(matches!(result, Err(AudioError::DeviceNotFound(_))));
    }
}
