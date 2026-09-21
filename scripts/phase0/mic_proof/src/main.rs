use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Murmur Phase 0: Microphone Capture Proof ===");

    let host = cpal::default_host();
    println!("Audio Host: {:?}", host.id());

    // 1. Enumerate devices
    println!("\n1. Enumerating input devices...");
    let devices = match host.input_devices() {
        Ok(devs) => devs.collect::<Vec<_>>(),
        Err(e) => {
            eprintln!("Failed to enumerate input devices: {e}");
            return Err(e.into());
        }
    };

    for (idx, dev) in devices.iter().enumerate() {
        let name = dev.name().unwrap_or_else(|_| "<unknown>".to_string());
        println!("  [{idx}] {name}");
    }

    // 2. Select default input device
    println!("\n2. Querying default input device...");
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            eprintln!("\n[ERROR] No default input device found!");
            eprintln!("Please check Windows Settings > Privacy & security > Microphone (`ms-settings:privacy-microphone`).");
            return Err("No default input audio device found".into());
        }
    };

    let dev_name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
    println!("Default input device: {dev_name}");

    // 3. Supported input configs
    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\n[ERROR] Failed to query default input config: {e}");
            eprintln!("If this is an Access Denied error, Windows Microphone privacy is blocking access.");
            eprintln!("Check `ms-settings:privacy-microphone`.\n");
            return Err(e.into());
        }
    };

    println!(
        "Config: Sample Rate = {} Hz, Channels = {}, Format = {:?}",
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    );

    let wav_spec = WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let recorded_samples = Arc::new(Mutex::new(Vec::<i16>::new()));
    let samples_clone = Arc::clone(&recorded_samples);

    let err_fn = |err| eprintln!("Audio stream error: {err}");

    let stream_result = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut buf = samples_clone.lock().unwrap();
                for &sample in data {
                    let s = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    buf.push(s);
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let mut buf = samples_clone.lock().unwrap();
                buf.extend_from_slice(data);
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| {
                let mut buf = samples_clone.lock().unwrap();
                for &sample in data {
                    let s = ((sample as i32) - 32768) as i16;
                    buf.push(s);
                }
            },
            err_fn,
            None,
        ),
        _ => return Err("Unsupported sample format".into()),
    };

    let stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            let err_str = e.to_string();
            eprintln!("\n[ERROR] Failed to build audio input stream: {err_str}");
            if err_str.contains("Access is denied") || err_str.contains("0x80070005") {
                eprintln!("\n>>> MICROPHONE PRIVACY RESTRICTION DETECTED <<<");
                eprintln!("Windows is blocking microphone access for desktop applications.");
                eprintln!("Resolution: Open Settings -> Privacy & security -> Microphone (`ms-settings:privacy-microphone`)");
                eprintln!("Ensure 'Microphone access' and 'Let desktop apps access your microphone' are ON.\n");
            }
            return Err(e.into());
        }
    };

    println!("\n3. Recording 3 seconds of audio...");
    stream.play()?;

    let start = Instant::now();
    let record_duration = Duration::from_secs(3);
    while start.elapsed() < record_duration {
        std::thread::sleep(Duration::from_millis(200));
        let count = recorded_samples.lock().unwrap().len();
        print!("\r   Captured samples: {count}...");
    }
    drop(stream);
    println!();

    let final_samples = recorded_samples.lock().unwrap().clone();
    println!("Total recorded samples: {}", final_samples.len());

    if final_samples.is_empty() {
        return Err("No audio samples captured".into());
    }

    // Compute RMS
    let sum_sq: f64 = final_samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / final_samples.len() as f64).sqrt();
    let peak = final_samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
    println!("Audio stats: RMS = {rms:.2}, Peak amplitude = {peak}");

    // Save to test.wav
    let out_path = std::env::args().nth(1).unwrap_or_else(|| {
        let root = std::path::Path::new(r"d:\Projects!@#\Murmur");
        if root.exists() {
            root.join("test.wav").to_string_lossy().to_string()
        } else {
            "test.wav".to_string()
        }
    });
    println!("4. Writing to {out_path}...");
    let mut writer = WavWriter::create(&out_path, wav_spec)?;
    for sample in &final_samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;

    let file_meta = std::fs::metadata(out_path)?;
    println!("WAV file size: {} bytes", file_meta.len());
    println!("\n>>> RESULT: PASS - Microphone capture functional <<<\n");

    Ok(())
}
