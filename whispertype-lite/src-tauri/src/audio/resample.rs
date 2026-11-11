use rubato::{FftFixedInOut, Resampler};
use thiserror::Error;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;
pub const TARGET_PEAK_AMPLITUDE: f32 = 0.95;

#[derive(Debug, Error, PartialEq)]
pub enum ResampleError {
    #[error("Invalid sample rate: input={0}, target={1}")]
    InvalidSampleRate(u32, u32),

    #[error("Invalid channel count: {0}")]
    InvalidChannels(u16),

    #[error("Empty audio input")]
    EmptyInput,

    #[error("Resampler initialization error: {0}")]
    InitError(String),

    #[error("Resampling processing error: {0}")]
    ProcessError(String),
}

/// Downmix interleaved multi-channel audio to mono f32 samples.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Result<Vec<f32>, ResampleError> {
    if channels == 0 {
        return Err(ResampleError::InvalidChannels(0));
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if channels == 1 {
        return Ok(samples.to_vec());
    }

    let ch = channels as usize;
    let frame_count = samples.len() / ch;
    let mut mono = Vec::with_capacity(frame_count);
    let inv_ch = 1.0 / (channels as f32);

    for frame_idx in 0..frame_count {
        let start = frame_idx * ch;
        let mut sum = 0.0f32;
        for c in 0..ch {
            sum += samples[start + c];
        }
        mono.push(sum * inv_ch);
    }

    Ok(mono)
}

/// Gentle peak normalization to [-0.95, 0.95].
/// Prevents digital clipping while gently scaling speech signals.
/// Leaves near-silence (noise floor) untouched to prevent amplifying background hiss.
pub fn normalize_peak(samples: &mut [f32], target_peak: f32) {
    if samples.is_empty() {
        return;
    }

    // Sanitize non-finite values (NaN, Inf) to 0.0
    for s in samples.iter_mut() {
        if !s.is_finite() {
            *s = 0.0;
        }
    }

    let max_abs = samples.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));

    // Ambient noise floor threshold: do not amplify pure silence or very quiet noise
    if max_abs < 1e-4 {
        return;
    }

    // Limit maximum gain boost to 20x (+26dB) to avoid blowing up faint background noise
    let gain_factor = (target_peak / max_abs).min(20.0);

    for s in samples.iter_mut() {
        *s = (*s * gain_factor).clamp(-target_peak, target_peak);
    }
}

/// Resample arbitrary sample-rate and multi-channel audio into 16,000 Hz mono f32 PCM,
/// followed by gentle peak normalization to [-0.95, 0.95].
pub fn resample_and_normalize(
    samples: &[f32],
    from_rate: u32,
    channels: u16,
) -> Result<Vec<f32>, ResampleError> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if from_rate == 0 {
        return Err(ResampleError::InvalidSampleRate(0, TARGET_SAMPLE_RATE));
    }
    if channels == 0 {
        return Err(ResampleError::InvalidChannels(0));
    }

    // 1. Downmix to mono
    let mono = downmix_to_mono(samples, channels)?;
    if mono.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Resample to 16,000 Hz if needed
    let mut resampled = if from_rate == TARGET_SAMPLE_RATE {
        mono
    } else {
        resample_mono_to_16k(&mono, from_rate)?
    };

    // 3. Gentle peak normalization
    normalize_peak(&mut resampled, TARGET_PEAK_AMPLITUDE);

    Ok(resampled)
}

/// Resample single-channel (mono) f32 samples from `from_rate` to 16,000 Hz using rubato FftFixedInOut.
pub fn resample_mono_to_16k(samples: &[f32], from_rate: u32) -> Result<Vec<f32>, ResampleError> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if from_rate == TARGET_SAMPLE_RATE {
        return Ok(samples.to_vec());
    }

    let chunk_size = 1024;
    let mut resampler = FftFixedInOut::<f32>::new(
        from_rate as usize,
        TARGET_SAMPLE_RATE as usize,
        chunk_size,
        1,
    )
    .map_err(|e| ResampleError::InitError(e.to_string()))?;

    let in_frames_needed = resampler.input_frames_next();
    let expected_out_len = ((samples.len() as f64) * (TARGET_SAMPLE_RATE as f64)
        / (from_rate as f64))
        .round() as usize;

    let delay = resampler.output_delay();
    let total_frames_needed = delay + expected_out_len;

    let mut output = Vec::with_capacity(total_frames_needed + chunk_size);
    let mut offset = 0;
    let mut in_buffer = vec![0.0f32; in_frames_needed];

    // Feed audio chunks, flushing filter delay with zeros if necessary until required frames are gathered
    while output.len() < total_frames_needed {
        if offset < samples.len() {
            let remaining = samples.len() - offset;
            let take = remaining.min(in_frames_needed);
            in_buffer[..take].copy_from_slice(&samples[offset..offset + take]);
            if take < in_frames_needed {
                in_buffer[take..].fill(0.0);
            }
            offset += take;
        } else {
            in_buffer.fill(0.0);
        }

        let input_chunks = [&in_buffer[..]];
        let out_chunks = resampler
            .process(&input_chunks, None)
            .map_err(|e| ResampleError::ProcessError(e.to_string()))?;

        if !out_chunks.is_empty() && !out_chunks[0].is_empty() {
            output.extend_from_slice(&out_chunks[0]);
        }
    }

    // Discard the filter delay at the beginning so the resampled audio aligns with t=0
    if output.len() > delay {
        output.drain(..delay);
    } else {
        output.clear();
    }

    // Keep exactly expected_out_len samples
    if output.len() > expected_out_len {
        output.truncate(expected_out_len);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downmix_mono() {
        let samples = vec![0.1, 0.2, 0.3];
        let downmixed = downmix_to_mono(&samples, 1).unwrap();
        assert_eq!(downmixed, samples);
    }

    #[test]
    fn test_downmix_stereo() {
        // Interleaved L, R, L, R
        let samples = vec![0.2, 0.4, 0.6, 0.8];
        let downmixed = downmix_to_mono(&samples, 2).unwrap();
        assert_eq!(downmixed.len(), 2);
        assert!((downmixed[0] - 0.3).abs() < 1e-5);
        assert!((downmixed[1] - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_downmix_invalid_channels() {
        let samples = vec![0.1, 0.2];
        assert_eq!(
            downmix_to_mono(&samples, 0),
            Err(ResampleError::InvalidChannels(0))
        );
    }

    #[test]
    fn test_normalize_peak_clipping() {
        let mut samples = vec![0.5, 1.2, -1.5, 0.2];
        normalize_peak(&mut samples, 0.95);
        let max_abs = samples.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
        assert!((max_abs - 0.95).abs() < 1e-4);
        for s in &samples {
            assert!(s.abs() <= 0.95 + 1e-5);
        }
    }

    #[test]
    fn test_normalize_peak_quiet_speech() {
        let mut samples = vec![0.1, -0.2, 0.15, -0.05];
        normalize_peak(&mut samples, 0.95);
        let max_abs = samples.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
        assert!((max_abs - 0.95).abs() < 1e-4);
    }

    #[test]
    fn test_normalize_peak_silence() {
        let mut samples = vec![0.0, 0.00001, -0.00002, 0.0];
        let original = samples.clone();
        normalize_peak(&mut samples, 0.95);
        // Noise floor should not be boosted to 0.95
        assert_eq!(samples, original);
    }

    #[test]
    fn test_normalize_peak_non_finite() {
        let mut samples = vec![0.5, f32::NAN, f32::INFINITY, -0.5];
        normalize_peak(&mut samples, 0.95);
        assert!(samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_resample_identity_16k() {
        let samples = vec![0.2, -0.2, 0.4, -0.4];
        let resampled = resample_mono_to_16k(&samples, 16000).unwrap();
        assert_eq!(resampled, samples);
    }

    #[test]
    fn test_resample_48k_to_16k() {
        // 48,000 Hz input, 1 second of audio (48,000 samples)
        let sample_count = 48000;
        let mut samples = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let t = (i as f32) / 48000.0;
            // 440 Hz sine wave
            samples.push((2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5);
        }

        let resampled = resample_mono_to_16k(&samples, 48000).unwrap();
        // Signal starts without artificial filter delay padding
        assert!(resampled[1].abs() > 0.01);
        assert_eq!(resampled.len(), 16000);
    }

    #[test]
    fn test_resample_44_1k_to_16k() {
        // 44,100 Hz input, 0.5 seconds of audio (22,050 samples)
        let sample_count = 22050;
        let mut samples = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let t = (i as f32) / 44100.0;
            samples.push((2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.4);
        }

        let resampled = resample_mono_to_16k(&samples, 44100).unwrap();
        let expected_len = (22050.0f64 * 16000.0 / 44100.0).round() as usize;
        assert_eq!(resampled.len(), expected_len);
    }

    #[test]
    fn test_resample_and_normalize_pipeline() {
        // Stereo 48kHz signal with peak 0.3
        let frame_count = 9600; // 0.2s
        let mut interleaved = Vec::with_capacity(frame_count * 2);
        for i in 0..frame_count {
            let s = (i as f32 / 100.0).sin() * 0.3;
            interleaved.push(s);
            interleaved.push(s);
        }

        let processed = resample_and_normalize(&interleaved, 48000, 2).unwrap();
        let expected_len = (frame_count as f64 * 16000.0 / 48000.0).round() as usize;
        assert_eq!(processed.len(), expected_len);

        let max_abs = processed.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
        assert!((max_abs - 0.95).abs() < 1e-4);
    }
}
