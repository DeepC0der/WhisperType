use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, Wave,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

struct MemoryStats {
    _working_set_mb: f64,
    peak_working_set_mb: f64,
}

fn get_memory_stats() -> MemoryStats {
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let handle = GetCurrentProcess();
        let _ = GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
        MemoryStats {
            _working_set_mb: (counters.WorkingSetSize as f64) / (1024.0 * 1024.0),
            peak_working_set_mb: (counters.PeakWorkingSetSize as f64) / (1024.0 * 1024.0),
        }
    }
}

fn get_gpu_vram_mb() -> Option<u32> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout);
        let first_line = s.lines().next()?.trim();
        first_line.parse::<u32>().ok()
    } else {
        None
    }
}

#[derive(Debug)]
struct BenchmarkResult {
    provider: String,
    load_time_ms: u128,
    _audio_duration_s: f64,
    inference_time_ms: u128,
    rtf: f64,
    peak_ram_mb: f64,
    _vram_mb: Option<u32>,
    transcript: String,
    success: bool,
    error: Option<String>,
}

fn run_benchmark_for_provider(
    provider: &str,
    encoder: &Path,
    decoder: &Path,
    joiner: &Path,
    tokens: &Path,
    wav_path: &Path,
) -> BenchmarkResult {
    println!("\n========================================================");
    println!(">>> Testing Provider: [{provider}]");
    println!("========================================================");

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(encoder.to_string_lossy().to_string()),
        decoder: Some(decoder.to_string_lossy().to_string()),
        joiner: Some(joiner.to_string_lossy().to_string()),
    };
    config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
    config.model_config.model_type = Some("nemo_transducer".to_string());
    config.model_config.provider = Some(provider.to_string());
    config.model_config.num_threads = 4;
    config.model_config.debug = false;

    println!("Loading recognizer with provider '{provider}'...");
    let t0 = Instant::now();
    let recognizer = match OfflineRecognizer::create(&config) {
        Some(r) => r,
        None => {
            let err = format!("Failed to create OfflineRecognizer with provider '{provider}'");
            eprintln!("  [ERROR] {err}");
            return BenchmarkResult {
                provider: provider.to_string(),
                load_time_ms: t0.elapsed().as_millis(),
                _audio_duration_s: 0.0,
                inference_time_ms: 0,
                rtf: 0.0,
                peak_ram_mb: get_memory_stats().peak_working_set_mb,
                _vram_mb: None,
                transcript: String::new(),
                success: false,
                error: Some(err),
            };
        }
    };
    let load_time_ms = t0.elapsed().as_millis();
    println!("Model loaded in {load_time_ms} ms.");

    println!("Reading audio from {}...", wav_path.display());
    let wave = match Wave::read(&wav_path.to_string_lossy()) {
        Some(w) => w,
        None => {
            let err = format!("Failed to decode WAV file {}", wav_path.display());
            eprintln!("  [ERROR] {err}");
            return BenchmarkResult {
                provider: provider.to_string(),
                load_time_ms,
                _audio_duration_s: 0.0,
                inference_time_ms: 0,
                rtf: 0.0,
                peak_ram_mb: get_memory_stats().peak_working_set_mb,
                _vram_mb: None,
                transcript: String::new(),
                success: false,
                error: Some(err),
            };
        }
    };

    let sample_rate = wave.sample_rate();
    let num_samples = wave.num_samples();
    let audio_duration_s = (num_samples as f64) / (sample_rate as f64);
    println!("Audio info: {num_samples} samples, {sample_rate} Hz, {audio_duration_s:.2} seconds.");

    println!("Creating stream and feeding audio...");
    let stream = recognizer.create_stream();

    stream.accept_waveform(sample_rate, wave.samples());

    println!("Decoding audio (measuring inference latency)...");
    let t1 = Instant::now();
    recognizer.decode(&stream);
    let inference_time_ms = t1.elapsed().as_millis();

    let rtf = (inference_time_ms as f64 / 1000.0) / audio_duration_s;
    let mem = get_memory_stats();
    let vram_after = get_gpu_vram_mb();

    let result = stream.get_result();
    let transcript = result.map(|r| r.text).unwrap_or_default();

    println!(">>> INFERENCE COMPLETED <<<");
    println!("  Inference Time:   {inference_time_ms} ms");
    println!("  Audio Duration:   {audio_duration_s:.2} s");
    println!("  Real-Time Factor: {rtf:.4} (RTF < 1.0 means faster than real-time)");
    println!("  Peak RAM:         {:.1} MB", mem.peak_working_set_mb);
    if let Some(vram) = vram_after {
        println!("  GPU VRAM Used:    {vram} MB");
    }
    println!("  Transcript:       \"{transcript}\"");

    BenchmarkResult {
        provider: provider.to_string(),
        load_time_ms,
        _audio_duration_s: audio_duration_s,
        inference_time_ms,
        rtf,
        peak_ram_mb: mem.peak_working_set_mb,
        _vram_mb: vram_after,
        transcript,
        success: true,
        error: None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Murmur Phase 0: sherpa-onnx ASR Benchmark CLI ===");

    let args: Vec<String> = std::env::args().collect();

    // 1. Resolve model directory
    let default_model_dirs = [
        PathBuf::from(r"d:\Projects!@#\Murmur\models\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"),
        dirs_appdata(),
    ];

    let model_dir = default_model_dirs
        .iter()
        .find(|p| p.join("encoder.int8.onnx").exists())
        .cloned()
        .unwrap_or_else(|| default_model_dirs[0].clone());

    println!("Model Directory: {}", model_dir.display());

    let encoder = model_dir.join("encoder.int8.onnx");
    let decoder = model_dir.join("decoder.int8.onnx");
    let joiner = model_dir.join("joiner.int8.onnx");
    let tokens = model_dir.join("tokens.txt");

    if !encoder.exists() || !decoder.exists() || !joiner.exists() || !tokens.exists() {
        eprintln!("[ERROR] Model files missing in {}", model_dir.display());
        eprintln!("Expected: encoder.int8.onnx, decoder.int8.onnx, joiner.int8.onnx, tokens.txt");
        return Err("Model files missing".into());
    }

    // 2. Resolve test WAV file
    let default_wavs = [
        model_dir.join("test_wavs").join("en.wav"),
        PathBuf::from(r"d:\Projects!@#\Murmur\test.wav"),
    ];

    let mut wav_path = default_wavs[0].clone();
    let mut provider_choice = "all".to_string();

    let mut update_feasibility = false;
    let mut idx = 1;
    while idx < args.len() {
        if args[idx] == "--provider" && idx + 1 < args.len() {
            provider_choice = args[idx + 1].clone();
            idx += 2;
        } else if args[idx] == "--model-dir" && idx + 1 < args.len() {
            idx += 2;
        } else if args[idx] == "--update-feasibility" {
            update_feasibility = true;
            idx += 1;
        } else if !args[idx].starts_with("--") {
            wav_path = PathBuf::from(&args[idx]);
            idx += 1;
        } else {
            idx += 1;
        }
    }

    if !wav_path.exists() {
        if let Some(w) = default_wavs.iter().find(|p| p.exists()) {
            wav_path = w.clone();
        } else {
            eprintln!("[ERROR] Test WAV file not found at {}", wav_path.display());
            return Err("Test WAV not found".into());
        }
    }
    println!("Test WAV file:   {}", wav_path.display());

    // 3. Determine providers to test
    let providers_to_test: Vec<&str> = match provider_choice.as_str() {
        "all" => vec!["cpu", "cuda", "directml"],
        other => vec![other],
    };

    println!("Providers to benchmark: {:?}", providers_to_test);

    let mut results: Vec<BenchmarkResult> = Vec::new();

    for prov in providers_to_test {
        let res = run_benchmark_for_provider(
            prov, &encoder, &decoder, &joiner, &tokens, &wav_path,
        );
        results.push(res);
    }

    // 4. Output Summary Table
    println!("\n==========================================================================================");
    println!(">>> PHASE 0 BENCHMARK SUMMARY TABLE <<<");
    println!("==========================================================================================");
    println!(
        "{:<10} | {:<22} | {:<10} | {:<10} | {:<8} | {:<10}",
        "Provider", "Effective Provider", "Load (ms)", "Infer (ms)", "RTF", "Peak RAM"
    );
    println!("{:-<10}-+-{:-<22}-+-{:-<10}-+-{:-<10}-+-{:-<8}-+-{:-<10}", "", "", "", "", "", "");

    for r in &results {
        let eff_provider = match r.provider.as_str() {
            "cpu" => "Native CPU (int8)",
            "cuda" => "Fallback -> CPU int8",
            "directml" => "Fallback -> CPU int8",
            _ => "Unknown",
        };
        let rtf_str = if r.success {
            format!("{:.4}", r.rtf)
        } else {
            "N/A".to_string()
        };
        let infer_str = if r.success {
            format!("{}", r.inference_time_ms)
        } else {
            "N/A".to_string()
        };
        println!(
            "{:<10} | {:<22} | {:<10} | {:<10} | {:<8} | {:<10.1} MB",
            r.provider, eff_provider, r.load_time_ms, infer_str, rtf_str, r.peak_ram_mb
        );
    }

    // 5. Append findings to FEASIBILITY.md only if requested
    if update_feasibility {
        append_to_feasibility(&results, &wav_path)?;
        println!("\nUpdated FEASIBILITY.md with benchmark results.");
    }

    // Check if at least one provider succeeded
    if results.iter().any(|r| r.success) {
        println!("\n>>> RESULT: PASS - sherpa-onnx benchmark completed successfully! <<<\n");
        Ok(())
    } else {
        Err("All providers failed in benchmark".into())
    }
}

fn dirs_appdata() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("Murmur")
            .join("models")
            .join("parakeet-tdt-int8")
    } else {
        PathBuf::from(PathBuf::from("models").join("parakeet-tdt-int8"))
    }
}

fn append_to_feasibility(
    results: &[BenchmarkResult],
    wav_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let feasibility_path = PathBuf::from(r"d:\Projects!@#\Murmur\FEASIBILITY.md");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&feasibility_path)?;

    writeln!(file, "\n## ASR Inference Benchmark Results")?;
    writeln!(file, "Test Audio: `{}`\n", wav_path.display())?;
    writeln!(
        file,
        "| Execution Provider | Status | Model Load (ms) | Inference (ms) | RTF | Peak RAM (MB) | Transcript |"
    )?;
    writeln!(
        file,
        "|---|---|---|---|---|---|---|"
    )?;

    for r in results {
        if r.success {
            writeln!(
                file,
                "| **{}** | ✅ PASS | {} ms | {} ms | **{:.4}** | {:.1} MB | \"{}\" |",
                r.provider, r.load_time_ms, r.inference_time_ms, r.rtf, r.peak_ram_mb, r.transcript
            )?;
        } else {
            let err = r.error.as_deref().unwrap_or("Failed");
            writeln!(
                file,
                "| **{}** | ❌ UNAVAILABLE | {} ms | N/A | N/A | {:.1} MB | Error: {} |",
                r.provider, r.load_time_ms, r.peak_ram_mb, err
            )?;
        }
    }

    Ok(())
}
