pub mod capture;
pub mod probe;
pub mod resample;

pub use capture::{
    apply_audio_gain, enumerate_input_devices, AudioCapture, AudioDeviceInfo, AudioError,
    CaptureConfig, CapturedAudio,
};
pub use probe::AudioMonitorProbe;
pub use resample::{
    downmix_to_mono, normalize_peak, resample_and_normalize, resample_mono_to_16k, ResampleError,
    TARGET_PEAK_AMPLITUDE, TARGET_SAMPLE_RATE,
};
