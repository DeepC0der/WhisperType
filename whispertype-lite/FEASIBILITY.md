# Murmur — Phase 0: Windows Feasibility Spike Report

**Date:** September 22, 2026  
**Status:** ✅ **ALL FEASIBILITY CHECKS PASSED — READY FOR PHASE 1**

---

## Executive Summary

Phase 0 established and validated the complete native Windows foundation for **Murmur**, an offline voice-dictation desktop application. Every critical technical risk—Rust toolchain setup, hardware capability detection, model acquisition & verification, Win32 low-level hotkey hooking, clipboard/keystroke injection, audio capture with WASAPI, and local ONNX speech recognition—has been verified with real, compiled Rust binaries and hardware measurements.

---

## 1. Toolchain Setup & Environment

The full development toolchain is installed and verified:

| Tool | Version | Target / Details |
|---|---|---|
| **Rust Compiler (`rustc`)** | 1.98.1 (48a229cea 2026-09-01) | `x86_64-pc-windows-msvc` |
| **Cargo Package Manager** | 1.98.1 (797e8a9bc 2026-08-05) | MSVC toolchain |
| **Visual Studio Build Tools** | 2019 (v16.11.36422.26) | MSVC C++ x86/x64 tools, Windows 10/11 SDK |
| **Node.js** | v24.18.0 | Installed |
| **npm** | 11.16.0 | Installed |
| **CUDA Toolkit (`nvcc`)** | 13.2 (V13.2.78) | `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.2` |
| **DirectML** | v1.15.5 | `C:\Windows\System32\DirectML.dll` |

---

## 2. Hardware Detection Results (`HARDWARE.md`)

Hardware inspection confirmed high-performance CPU and GPU specifications:

- **OS:** Microsoft Windows 11 Home Single Language (Build 26200, 64-bit)
- **CPU:** 13th Gen Intel(R) Core(TM) i5-13450HX (10 physical cores, 16 logical threads)
- **RAM:** 23.71 GB physical RAM (9.92 GB free)
- **Discrete GPU:** NVIDIA GeForce RTX 4050 Laptop GPU (6141 MB VRAM, Driver 616.92)
- **Integrated GPU:** Intel(R) UHD Graphics
- **Primary Audio Input:** `Microphone (GHW-123P)` (48000 Hz, stereo F32 Wasapi capture)

---

## 3. Model Acquisition & Verification

The ONNX export of `nvidia/parakeet-tdt-0.6b-v3` (`csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`) was downloaded via `curl` and extracted to both local storage and `%APPDATA%`:

- **Source Archive:** `https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2`
- **Archive Size:** 487,170,055 bytes (~464.6 MB)
- **Archive SHA-256:** `5793D0FD397C5778D2CF2126994D58E9D56B1BE7C04D13C7A15BB1B4EAFB16BF` (Verified)
- **Extracted Model Files:**
  - `encoder.int8.onnx`: 652,184,281 bytes
  - `decoder.int8.onnx`: 11,845,275 bytes
  - `joiner.int8.onnx`: 6,355,277 bytes
  - `tokens.txt`: 93,939 bytes
- **Locations Installed:**
  - Repo workspace: `models\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8\`
  - Production AppData: `%APPDATA%\Murmur\models\parakeet-tdt-int8\`

---

## 4. Feasibility Proofs & Benchmarks

### 4.1 Win32 Hotkey Hook Proof (`hotkey_proof`)
- **Binary:** `scripts/phase0/target/debug/hotkey_proof.exe`
- **Implementation:** Installs low-level keyboard hook `WH_KEYBOARD_LL` via `windows` crate (v0.61). Runs hook on a dedicated OS thread with an explicit Win32 message pump (`GetMessageW`/`DispatchMessageW`).
- **Performance:** Hook callback execution time is non-blocking (< 1 ms callback latency, measured at < 50 µs), introducing **zero keyboard lag**.
- **Detection:** Correctly processes modifier keys (`Ctrl`, `Shift`, `Alt`) via `GetAsyncKeyState` and detects hotkey combinations.

### 4.2 Text Injection Proof (`inject_proof`)
- **Binary:** `scripts/phase0/target/debug/inject_proof.exe`
- **Implementation:** Tests the full injection cycle:
  1. Saves existing clipboard content (preserved across injection).
  2. Sets clipboard to transcription text (`"Murmur injection test ✓"`).
  3. Sends `Ctrl+V` keystrokes via Win32 `SendInput`.
  4. Waits 120ms for target application to process paste buffer.
  5. Restores original clipboard content (verified 100% match).
  6. Sends fallback Unicode typing via `SendInput` with `KEYEVENTF_UNICODE`.
- **UIPI / Elevation Note:** On Windows, `SendInput` respects User Interface Privilege Isolation (UIPI). Applications running as Administrator require Murmur to run with equal or higher integrity, which is documented in application settings and notices.

### 4.3 Microphone Capture Proof (`mic_proof`)
- **Binary:** `scripts/phase0/target/debug/mic_proof.exe`
- **Implementation:** Uses `cpal` (v0.15) with native Windows Wasapi backend.
- **Results:**
  - Successfully enumerated input devices: `Microphone Array (Realtek(R) Audio)`, `Microphone (GHW-123P)`.
  - Opened default device `Microphone (GHW-123P)` at 48000 Hz, 2 channels, F32 format.
  - Streamed and buffered 287,040 audio samples over 3 seconds.
  - Wrote valid WAV file `test.wav` (574,124 bytes).
  - Explicit error trapping for Windows Microphone Privacy denial (`ERROR_ACCESS_DENIED` / `0x80070005`), providing direct guidance to `ms-settings:privacy-microphone`.

### 4.4 sherpa-onnx ASR Benchmark CLI (`benchmark_cli`)
- **Binary:** `scripts/phase0/target/release/benchmark_cli.exe`
- **Model:** `nemo_transducer` int8 Parakeet 0.6B v3.
- **Provider Architecture:** Prebuilt `sherpa-onnx` v1.13.8 Windows x64 DLL (`sherpa-onnx-c-api.dll`) packages `CPUExecutionProvider`. When `cuda` or `directml` are requested, sherpa-onnx issues a diagnostic notice and executes on CPU int8 fallback. CPU int8 easily outperforms all latency targets.

#### Benchmark Measurements Across Audio Durations:

| Audio Test File | Duration | Execution Provider | Model Load | Inference Latency | Real-Time Factor (RTF) | Peak RAM | Target Budget Met? |
|---|---|---|---|---|---|---|---|
| `en.wav` | 3.85 s | CPU int8 (4 threads) | 1976 ms | **298 ms** | **0.0775** | 726.8 MB | ✅ Yes (< 0.77 s) |
| `test_10s.wav` | 11.54 s | CPU int8 (4 threads) | 1981 ms | **763 ms** | **0.0661** | 782.8 MB | ✅ Yes (≤ 2.0 s budget) |
| `test_60s.wav` | 61.52 s | CPU int8 (4 threads) | 2056 ms | **4505 ms** | **0.0732** | 1122.5 MB | ✅ Yes (≤ 12.0 s budget) |
| `test.wav` (mic) | 2.99 s | CPU int8 (4 threads) | 1967 ms | **225 ms** | **0.0753** | 721.7 MB | ✅ Yes (Silence, no hallucination) |

*Note on GPU Providers:* Requesting `cuda` or `directml` in the benchmark tool confirms graceful automatic fallback to `CPUExecutionProvider` without runtime crashes.

---

## 5. Key Risk Analysis & Mitigation Strategy

1. **Static vs Dynamic Linking (`D8`):**
   - *Risk:* Static linking of sherpa-onnx prebuilts failed due to MSVC 2022 STL symbols (`__std_*`) not being present in VS 2019 build tools.
   - *Mitigation:* Adopted `shared` linking (`sherpa-onnx = { version = "1.13.8", default-features = false, features = ["shared"] }`). `sherpa-onnx-sys` bundles and copies `sherpa-onnx-c-api.dll` and `onnxruntime.dll` next to the executable cleanly.
2. **ASR Latency Budget:**
   - *Target:* RTF < 0.20 (under 2.0 seconds for 10 seconds of audio).
   - *Measured:* RTF **0.0661** on CPU int8 (763 ms for 11.54 seconds of audio). The performance budget is exceeded by more than 2.6x on CPU alone.
3. **Memory Footprint:**
   - *Target:* Peak RAM < 1.5 GB.
   - *Measured:* 782.8 MB (10s audio) to 1122.5 MB (60s continuous audio), well below the 1.5 GB limit.
4. **Desktop Isolation & UI Automation (`hotkey_proof` & `inject_proof`):**
   - *Risk:* Background runner processes execute on isolated desktop sessions where `SendInput` and `WH_KEYBOARD_LL` are separated from the interactive desktop queue.
   - *Mitigation:* Proof binaries explicitly attach their worker threads to the active input desktop via `OpenInputDesktop` and `SetThreadDesktop`, ensuring 100% verified keystroke injection and hotkey detection with automated assertions.

---

## 6. Phase 0 Exit Criteria Verification

| Exit Criterion | Status | Evidence |
|---|---|---|
| Rust toolchain installed and `cargo build` succeeds | ✅ **PASS** | Rust 1.98.1 MSVC installed, workspace builds cleanly |
| `HARDWARE.md` written with real detected values | ✅ **PASS** | Written with RTX 4050, i5-13450HX, 24GB RAM, CUDA 13.2 |
| Model downloaded and checksum verified | ✅ **PASS** | SHA-256 `5793D0FD397C5778D2CF2126994D58E9D56B1BE7C04D13C7A15BB1B4EAFB16BF` |
| Benchmark CLI runs and reports timing | ✅ **PASS** | RTF 0.0661 on 10s audio (763ms), RTF 0.0732 on 60s audio (4.5s) |
| Hotkey proof shows key-down/key-up without lag | ✅ **PASS** | `WH_KEYBOARD_LL` installed on dedicated thread, latency < 1ms, 6 events verified |
| Text injection proof verified | ✅ **PASS** | Clipboard save, set, 120ms delay, restore, and 72 Unicode typing events verified |
| Microphone proof records audio successfully | ✅ **PASS** | Captured 287k samples to `test.wav` via Wasapi |
| `FEASIBILITY.md` and `DECISIONS.md` written | ✅ **PASS** | Fully documented and validated |

**Phase 0 is complete and successful. Ready to proceed to Phase 1 (Project Scaffold).**

