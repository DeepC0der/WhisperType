use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Live, lightweight, low-latency audio monitor probe used during setup tours and audio settings checks.
/// Computes real-time RMS levels (~30 Hz) and emits `mic-level` events without persisting audio buffers.
pub struct AudioMonitorProbe {
    host: cpal::Host,
    stream: Option<Stream>,
    is_active: Arc<AtomicBool>,
}

impl Default for AudioMonitorProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioMonitorProbe {
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
            stream: None,
            is_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Open low-latency stream on requested audio device (or system default) and pump real-time mic-level events.
    pub fn start(&mut self, device_name: Option<&str>, app: AppHandle) -> Result<(), String> {
        let _ = self.stop(&app);

        let device = match device_name {
            Some(name) if !name.is_empty() && name != "default" => {
                let mut matched = None;
                if let Ok(devices) = self.host.input_devices() {
                    for dev in devices {
                        if let Ok(n) = dev.name() {
                            if n == name {
                                matched = Some(dev);
                                break;
                            }
                        }
                    }
                }
                matched.or_else(|| self.host.default_input_device())
            }
            _ => self.host.default_input_device(),
        }
        .ok_or_else(|| "No audio input device available for monitor probe".to_string())?;

        let default_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to query device input config: {e}"))?;

        let sample_format = default_config.sample_format();
        let stream_config: cpal::StreamConfig = default_config.into();

        let (tx, rx) = crossbeam_channel::bounded::<f32>(8);
        let is_active = Arc::new(AtomicBool::new(true));
        self.is_active = is_active.clone();

        // Worker thread that receives RMS levels and emits to Tauri at ~30 Hz
        let app_clone = app.clone();
        let active_flag = is_active.clone();
        std::thread::Builder::new()
            .name("murmur-mic-probe-emitter".to_string())
            .spawn(move || {
                let mut last_emit = Instant::now();
                let emit_interval = Duration::from_millis(33); // ~30 Hz

                while active_flag.load(Ordering::SeqCst) {
                    match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(level) => {
                            if last_emit.elapsed() >= emit_interval {
                                let _ = app_clone.emit("mic-level", level);
                                last_emit = Instant::now();
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    }
                }
                let _ = app_clone.emit("mic-level", 0.0);
            })
            .map_err(|e| format!("Failed to spawn probe emitter: {e}"))?;

        let err_fn = |err| {
            log::warn!("CPAL mic test probe stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        if !data.is_empty() {
                            let sum_sq: f32 = data.iter().map(|&s| s * s).sum();
                            let rms = (sum_sq / data.len() as f32).sqrt();
                            let level = (rms * 200.0).clamp(0.0, 100.0);
                            let _ = tx.try_send(level);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        if !data.is_empty() {
                            let sum_sq: f32 = data
                                .iter()
                                .map(|&s| {
                                    let norm = s as f32 / 32768.0;
                                    norm * norm
                                })
                                .sum();
                            let rms = (sum_sq / data.len() as f32).sqrt();
                            let level = (rms * 200.0).clamp(0.0, 100.0);
                            let _ = tx.try_send(level);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let tx = tx.clone();
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        if !data.is_empty() {
                            let sum_sq: f32 = data
                                .iter()
                                .map(|&s| {
                                    let norm = ((s as i32) - 32768) as f32 / 32768.0;
                                    norm * norm
                                })
                                .sum();
                            let rms = (sum_sq / data.len() as f32).sqrt();
                            let level = (rms * 200.0).clamp(0.0, 100.0);
                            let _ = tx.try_send(level);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => return Err(format!("Unsupported audio sample format: {sample_format:?}")),
        }
        .map_err(|e| format!("Failed to build mic test stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start mic test stream: {e}"))?;

        self.stream = Some(stream);
        log::info!("Microphone live test probe started successfully");
        Ok(())
    }

    /// Stop probe stream immediately, drop the driver handle, and reset mic level to 0.0.
    pub fn stop(&mut self, app: &AppHandle) -> Result<(), String> {
        self.is_active.store(false, Ordering::SeqCst);
        if let Some(stream) = self.stream.take() {
            drop(stream);
            log::info!("Microphone live test probe stopped and stream released");
        }
        let _ = app.emit("mic-level", 0.0);
        Ok(())
    }
}

// Safety: The internal CPAL stream handle is managed exclusively through synchronized mutexes
unsafe impl Send for AudioMonitorProbe {}
unsafe impl Sync for AudioMonitorProbe {}

