use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, Stream};
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
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

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub device_name: Option<String>,
    pub max_recording_seconds: u32,
    pub audio_gain_percent: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            max_recording_seconds: 60,
            audio_gain_percent: 100,
        }
    }
}

/// Apply validated audio gain with clipping protection to f32 samples.
pub fn apply_audio_gain(samples: &mut [f32], gain_percent: u32) {
    if gain_percent == 100 {
        return;
    }
    let factor = (gain_percent as f32 / 100.0).clamp(0.0, 5.0);
    for s in samples.iter_mut() {
        *s = (*s * factor).clamp(-1.0, 1.0);
    }
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

    /// Start capturing audio with default configuration.
    pub fn start_recording(
        &mut self,
        device_name: Option<&str>,
        app_handle: Option<AppHandle>,
    ) -> Result<(), AudioError> {
        self.start_recording_with_config(
            CaptureConfig {
                device_name: device_name.map(str::to_string),
                max_recording_seconds: 60,
                audio_gain_percent: 100,
            },
            app_handle,
        )
    }

    /// Start capturing audio with explicit bounds (max recording seconds and gain).
    /// Uses bounded channels and recycling buffers to guarantee strictly bounded memory usage.
    pub fn start_recording_with_config(
        &mut self,
        config: CaptureConfig,
        app_handle: Option<AppHandle>,
    ) -> Result<(), AudioError> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyRunning);
        }

        let host = cpal::default_host();
        let device = resolve_input_device(&host, config.device_name.as_deref())?;

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

        // Bounded channel to prevent unbounded memory growth if worker stalls
        let channel_capacity = 256;
        let (audio_tx, audio_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) =
            crossbeam_channel::bounded(channel_capacity);
        let (recycle_tx, recycle_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) =
            crossbeam_channel::bounded(channel_capacity);

        // Pre-allocate recyclable buffers so the audio callback doesn't allocate fresh Vecs
        for _ in 0..16 {
            let _ = recycle_tx.try_send(Vec::with_capacity(1024));
        }

        let (stop_tx, stop_rx) = crossbeam_channel::bounded::<()>(1);

        let err_fn = move |err: cpal::StreamError| {
            log::error!("CPAL audio stream error: {err}");
        };

        let stream_config: cpal::StreamConfig = default_config.into();

        // Build CPAL input stream with non-blocking callback reusing recyclable buffers
        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx = audio_tx.clone();
                let rx_recycle = recycle_rx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        let mut buf = rx_recycle
                            .try_recv()
                            .unwrap_or_else(|_| Vec::with_capacity(data.len()));
                        buf.clear();
                        buf.extend_from_slice(data);
                        let _ = tx.try_send(buf);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let tx = audio_tx.clone();
                let rx_recycle = recycle_rx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        let mut buf = rx_recycle
                            .try_recv()
                            .unwrap_or_else(|_| Vec::with_capacity(data.len()));
                        buf.clear();
                        buf.extend(data.iter().map(|&s| s as f32 / 32768.0));
                        let _ = tx.try_send(buf);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let tx = audio_tx.clone();
                let rx_recycle = recycle_rx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        let mut buf = rx_recycle
                            .try_recv()
                            .unwrap_or_else(|_| Vec::with_capacity(data.len()));
                        buf.clear();
                        buf.extend(
                            data.iter()
                                .map(|&s| ((s as i32) - 32768) as f32 / 32768.0),
                        );
                        let _ = tx.try_send(buf);
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

        // Spawn bounded background audio collector and RMS computation thread (~30 Hz)
        let worker_handle = std::thread::spawn(move || {
            let max_seconds = config.max_recording_seconds.clamp(5, 600) as usize;
            let max_samples = sample_rate as usize * channels as usize * max_seconds;
            let initial_capacity = (sample_rate as usize * channels as usize * 10).min(max_samples);

            let mut recorded_samples = Vec::<f32>::with_capacity(initial_capacity);
            let mut rms_window = Vec::<f32>::with_capacity(sample_rate as usize / 10);
            let mut last_rms_emit = Instant::now();
            let rms_interval = Duration::from_millis(33); // ~30 Hz
            let gain_factor = (config.audio_gain_percent as f32 / 100.0).clamp(0.0, 5.0);

            loop {
                // Check stop signal non-blockingly
                if stop_rx.try_recv().is_ok() || !is_recording_flag.load(Ordering::SeqCst) {
                    while let Ok(chunk) = audio_rx.try_recv() {
                        if recorded_samples.len() < max_samples {
                            let remaining = max_samples - recorded_samples.len();
                            let take = remaining.min(chunk.len());
                            recorded_samples.extend_from_slice(&chunk[..take]);
                        }
                    }
                    break;
                }

                // Wait for chunk with short timeout to maintain 30 Hz RMS cadence
                match audio_rx.recv_timeout(Duration::from_millis(15)) {
                    Ok(chunk) => {
                        let remaining = max_samples.saturating_sub(recorded_samples.len());
                        let take = remaining.min(chunk.len());
                        if take > 0 {
                            rms_window.extend_from_slice(&chunk[..take]);
                            recorded_samples.extend_from_slice(&chunk[..take]);
                        }

                        // Stop automatically at max recording seconds to prevent unbounded memory growth
                        if recorded_samples.len() >= max_samples {
                            log::info!(
                                "Maximum recording duration reached ({}s / {} samples). Halting capture and initiating graceful pipeline completion.",
                                max_seconds,
                                max_samples
                            );
                            is_recording_flag.store(false, Ordering::SeqCst);
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("recording-max-duration-reached", max_seconds);
                                let app_for_stop = handle.clone();
                                std::thread::spawn(move || {
                                    let ctrl = app_for_stop.state::<crate::dictation::DictationController>();
                                    let _ = ctrl.stop(&app_for_stop);
                                });
                            }
                            let mut rec = chunk;
                            rec.clear();
                            let _ = recycle_tx.try_send(rec);
                            break;
                        }

                        let mut rec = chunk;
                        rec.clear();
                        let _ = recycle_tx.try_send(rec);
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }

                // Periodic RMS computation and emission with audio gain scaling
                if last_rms_emit.elapsed() >= rms_interval {
                    let level = if !rms_window.is_empty() {
                        let sum_sq: f32 = rms_window
                            .iter()
                            .map(|&s| {
                                let scaled = (s * gain_factor).clamp(-1.0, 1.0);
                                scaled * scaled
                            })
                            .sum();
                        let rms = (sum_sq / rms_window.len() as f32).sqrt();
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

            // Apply audio gain to the final recorded samples
            apply_audio_gain(&mut recorded_samples, config.audio_gain_percent);

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
        let was_recording = self.is_recording.swap(false, Ordering::SeqCst);
        if !was_recording && self.worker_handle.is_none() {
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

    #[test]
    fn test_audio_gain_scaling_and_clipping() {
        let mut samples = vec![0.1f32, -0.2f32, 0.7f32, -0.8f32];
        apply_audio_gain(&mut samples, 200); // 200% = 2x gain
        assert!((samples[0] - 0.2).abs() < 1e-6);
        assert!((samples[1] - (-0.4)).abs() < 1e-6);
        assert_eq!(samples[2], 1.0); // clipped at 1.0
        assert_eq!(samples[3], -1.0); // clipped at -1.0

        // 100% gain leaves samples unchanged
        let mut unchanged = vec![0.5f32, -0.5f32];
        apply_audio_gain(&mut unchanged, 100);
        assert_eq!(unchanged, vec![0.5f32, -0.5f32]);
    }

    #[test]
    fn test_capture_config_defaults() {
        let cfg = CaptureConfig::default();
        assert_eq!(cfg.max_recording_seconds, 60);
        assert_eq!(cfg.audio_gain_percent, 100);
        assert!(cfg.device_name.is_none());
    }
}
