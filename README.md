# Murmur — Private, Real-Time Local Speech-to-Text for Windows

```
   __  ___                               
  /  |/  /_ _________ _  __ _________    [ STATUS: OFFLINE ]
 / /|_/ / // / __/  ' \/ // / __/ _ \   [ MODEL:  PARAKEET-TDT INT8 ]
/_/  /_/\_,_/_/ /_/_/_/\_,_/_/  \___/   [ LATENCY: ~70ms RTF 0.066 ]
                                         [ PRIVACY: 100% AIR-GAPPED ]
```

**Murmur** is a high-performance, completely offline voice dictation desktop application for Windows. It provides global hold-to-talk, toggle, and hybrid speech-to-text with real-time waveform feedback and dual-mode text injection into any active desktop window.

Powered by NVIDIA's Parakeet-TDT 0.6B model quantized to 8-bit integers via `sherpa-onnx`, Murmur delivers instantaneous transcription without sending a single byte of audio or text over the network.

---

## Key Highlights

- 🔒 **100% Offline & Private:** Zero telemetry, zero external network calls, zero cloud dependencies. Your voice never leaves your machine.
- ⚡ **Sub-Second Latency:** Benchmarked at Real-Time Factor (RTF) **0.066** on 13th Gen Intel Core CPUs (~760 ms transcription time for ~11.5 seconds of speech).
- 🎙️ **Dual-Mode Text Injection:** Seamlessly pastes text into any active editor or input field via clipboard with instant state restoration, plus direct Unicode keystroke typing fallback.
- 🎛️ **Floating Anti-Distraction Overlay:** Frameless, transparent, click-through HUD (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`) that dynamically dims during silence and brightens on speech.
- ⌨️ **Universal Global Hotkeys:** Low-level Win32 keyboard hook (`WH_KEYBOARD_LL`) running on a dedicated message-pump thread with instant `Esc` cancellation.
- 🕹️ **Retro-Futuristic Pixel HUD:** Aesthetically styled CRT scanlines, phosphor indicators, pixel fonts (`Silkscreen`), and crisp readable body text (`Inter`).
- 📚 **Phonetic Dictionary & Smart Post-Processing:** Spoken punctuation conversion ("new paragraph", "comma"), filler-word removal ("um", "uh"), regex replacement rules, and SQLite history storage.

---

## Architecture Overview

Murmur combines a native Rust backend with a modern React frontend wrapped in Tauri 2:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           TAURI 2 DESKTOP RUNTIME                       │
│                                                                         │
│  ┌───────────────────────────────┐     ┌─────────────────────────────┐  │
│  │   React 18 + Vite Frontend    │     │    Floating Overlay HUD     │  │
│  │  - Silkscreen Pixel Font      │     │  - Click-through & Frameless│  │
│  │  - Inter Body Typography      │     │  - Dynamic Dimming / Glow   │  │
│  │  - Tailwind CSS + CRT Shader  │     │  - Live Audio Waveform      │  │
│  └──────────────┬────────────────┘     └──────────────┬──────────────┘  │
│                 │ IPC Commands & Event Bridge         │                 │
├─────────────────┼─────────────────────────────────────┼─────────────────┤
│                 ▼                                     ▼                 │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                       RUST BACKEND CORE                           │  │
│  │                                                                   │  │
│  │  ┌───────────────────┐  ┌──────────────────┐  ┌────────────────┐  │  │
│  │  │  Win32 Hook Host  │  │  Audio Pipeline  │  │  ASR Engine    │  │  │
│  │  │  - WH_KEYBOARD_LL │  │  - CPAL (WASAPI) │  │  - sherpa-onnx │  │  │
│  │  │  - Message Pump   │  │  - Rubato (16kHz)│  │  - Silero VAD  │  │  │
│  │  │  - Esc Cancel     │  │  - Peak Normalizer│ │  - Parakeet TDT│  │  │
│  │  └────────┬──────────┘  └────────┬─────────┘  └───────┬────────┘  │  │
│  │           │                      │                    │           │  │
│  │  ┌────────▼──────────┐  ┌────────▼─────────┐  ┌───────▼────────┐  │  │
│  │  │   Text Injector   │  │  SQLite Database │  │ Settings Mgmt  │  │  │
│  │  │  - Clipboard Paste│  │  - History Store │  │  - Atomic JSON │  │  │
│  │  │  - Clip Restore   │  │  - Custom Dict   │  │  - Migration   │  │  │
│  │  │  - Unicode Typing │  │  - Audio BLOBs   │  │  - Hot Reload  │  │  │
│  │  └───────────────────┘  └──────────────────┘  └────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Core Technologies

| Subsystem | Technology | Purpose |
|---|---|---|
| **App Framework** | [Tauri 2](https://v2.tauri.app/) | Native Windows host, multi-window management, tray icon, and secure IPC bridge |
| **Frontend UI** | React 18, Vite, Tailwind CSS | High-DPI user interface, settings panels, history viewer, and onboarding wizard |
| **Typography** | `Silkscreen` & `Inter` | Self-hosted fonts: 8-bit retro pixel aesthetics for telemetry; clean sans-serif for transcripts |
| **ASR Engine** | [`sherpa-onnx`](https://github.com/k2-fsa/sherpa-onnx) (v1.13.8) | Offline Transducer speech recognition using `parakeet-tdt-0.6b-v3-int8` and VAD |
| **Audio Capture** | [`cpal`](https://github.com/RustAudio/cpal) | Windows WASAPI audio capture device enumeration and real-time streaming |
| **Resampling** | [`rubato`](https://github.com/HEnquist/rubato) | High-fidelity asynchronous sinc resampling from hardware sample rate (e.g. 48kHz) to 16kHz mono |
| **Storage** | [`rusqlite`](https://github.com/rusqlite/rusqlite) | Local SQLite 3 database for transcription records, audio BLOB storage, and dictionary replacements |
| **Input Injection** | Windows `SendInput` API | Dual-mode clipboard paste with state restoration + fallback Unicode character typing |
| **Window Styles** | Win32 Extended Window Styles | `WS_EX_LAYERED`, `WS_EX_TRANSPARENT`, and `WS_EX_NOACTIVATE` for click-through HUD overlay |

---

## Retro-Futuristic Pixel Aesthetic

Murmur embraces an authentic retro-futuristic computing interface inspired by CRT military telemetry and vintage synth workstations:

- **Typography Balance:** Status badges, timers, audio levels, and diagnostic metrics use `@fontsource/silkscreen` (monospaced 8-bit pixel font). Dictation text and long-form transcripts use `@fontsource/inter` to guarantee effortless reading and editing.
- **CRT Scanlines & Glow:** Subtle scanline overlays and phosphor glow accents (`#22c55e` emerald, `#06b6d4` cyan, `#f59e0b` amber) indicate pipeline states without blinding the user.
- **Anti-Distraction Floating HUD:** Displays a compact live audio waveform during speech. When speech pauses, the HUD dims to 40% opacity after 2 seconds to keep your active workspace distraction-free.

---

## Setup & Installation

### Prerequisites

- **OS:** Windows 10 or Windows 11 (64-bit)
- **Rust:** `rustc` and `cargo` 1.78+ (MSVC toolchain: `x86_64-pc-windows-msvc`)
- **Node.js:** v18.0.0 or later with `npm`
- **Visual Studio Build Tools:** C++ x86/x64 tools and Windows 10/11 SDK
- **(Optional) WiX Toolset:** Tauri CLI automatically downloads and manages its own isolated WiX v3.14 toolset into `%LOCALAPPDATA%\tauri\WixTools314\`, so manual WiX installation is unnecessary.

### Quick Start (Development)

1. **Clone or navigate to the repository:**
   ```powershell
   cd "d:\Projects!@#\Murmur"
   ```

2. **Install frontend dependencies:**
   ```powershell
   npm install
   ```

3. **Verify offline model assets:**
   The model files must be present either in `models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/` or `%APPDATA%\Murmur\models\parakeet-tdt-int8\`:
   - `encoder.int8.onnx` (~652 MB)
   - `decoder.int8.onnx` (~11.8 MB)
   - `joiner.int8.onnx` (~6.3 MB)
   - `tokens.txt` (~94 KB)

   *To download the pre-quantized model package on a new machine:*
   ```powershell
   curl.exe -L -o sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2 "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2"
   tar.exe -xjf sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2 -C models/
   ```

4. **Launch the development environment:**
   ```powershell
   npm run tauri dev
   ```

---

## Building Production Installers

Murmur uses Tauri CLI to produce self-contained Windows installers. The installer bundles the required native dynamic libraries (`onnxruntime.dll`, `onnxruntime_providers_shared.dll`, `sherpa-onnx-c-api.dll`, `sherpa-onnx-cxx-api.dll`) and is configured with `embedBootstrapper` to ensure that machines lacking Microsoft Edge WebView2 automatically acquire it during setup.

### Build Command

```powershell
npm run tauri build
```

### Generated Artifacts

Upon completion, release installers are located in:
- **NSIS Installer (`.exe`):**
  ```
  src-tauri/target/release/bundle/nsis/Murmur_0.1.0_x64-setup.exe
  ```
- **MSI Installer (`.msi`):**
  ```
  src-tauri/target/release/bundle/msi/Murmur_0.1.0_x64_en-US.msi
  ```
- **Standalone Binary:**
  ```
  src-tauri/target/release/murmur.exe
  ```

*Note on WiX Toolset:* Tauri CLI automatically resolves WiX v3.14 toolset binaries into `%LOCALAPPDATA%\tauri\WixTools314\` during the build process, enabling both NSIS (`.exe`) and WiX (`.msi`) installers to be compiled in a single `tauri build` invocation without requiring manual WiX installation or PATH modifications.

---

## Global Hotkey Usage

Murmur monitors a system-wide hotkey using a dedicated Win32 low-level keyboard hook. The default hotkey is `Ctrl+Shift+Space`.

### Hotkey Modes

Configure your preferred operation mode in **Settings > General**:

| Mode | How to Use | Ideal For |
|---|---|---|
| **Hold-to-Talk** *(Default)* | Press and hold `Ctrl+Shift+Space` while speaking. Release to stop recording and inject text. | Quick commands, short phrases, single sentences. |
| **Toggle** | Tap `Ctrl+Shift+Space` once to begin recording. Tap it again when finished speaking. | Long-form dictation, paragraphs, hands-free speech. |
| **Hybrid** | Tap briefly (< 400ms) to toggle on/off. Hold longer (> 400ms) for hold-to-talk. | Mixed workflows without having to change settings. |

### Emergency Cancellation (`Esc`)

Pressing **`Escape`** at any point while recording or transcribing immediately aborts the pipeline:
- Discards recorded audio buffers.
- Cancels transcription.
- Restores the original system clipboard.
- Returns the HUD overlay to `Idle`.

---

## Settings & Customization Guide

Access settings via the main application window or right-click the system tray icon and select **Settings**.

### 1. Audio Settings
- **Input Device:** Select any enumerated WASAPI input microphone or leave on *Default System Device*.
- **Microphone Gain (0% – 200%):** Software pre-amplification applied before peak normalization and VAD.
- **Max Recording Duration (5s – 300s):** Prevents runaway recording if a toggle is left active.
- **Sound Effects:** Optional retro audio clicks when starting and finishing dictation.

### 2. Text Post-Processing
- **Trim Whitespace:** Strips extraneous leading/trailing spaces from transcribed segments.
- **Preserve Punctuation:** Keeps punctuation generated by the Parakeet-TDT acoustic model.
- **Remove Filler Words:** Filters hesitations ("um", "uh", "like", "you know", "kind of", "actually", etc.).
- **Spoken Commands:** Converts spoken phrases into characters:
  - `"period"` or `"full stop"` ➔ `.`
  - `"comma"` ➔ `,`
  - `"question mark"` ➔ `?`
  - `"exclamation mark"` ➔ `!`
  - `"new line"` ➔ `\n`
  - `"new paragraph"` ➔ `\n\n`
  - `"colon"` ➔ `:`
  - `"semicolon"` ➔ `;`
  - `"open quote"` / `"close quote"` ➔ `"` / `"`
- **Add Trailing Space:** Appends a trailing space so consecutive dictations flow together naturally.
- **Custom Dictionary:** Add custom phonetic or regex replacement rules (e.g., replace `"tauri"` with `"Tauri 2"` or `"docker compose"` with `"docker-compose"`).

### 3. Overlay Anti-Distraction
- **Overlay Position:** Choose from 7 screen anchors: `Bottom-Center` (default), `Top-Center`, `Bottom-Right`, `Top-Right`, `Bottom-Left`, `Top-Left`, or `Center`.
- **Overlay Size:** Compact (`S`), Medium (`M`), or Large (`L`).
- **Dim Delay & Inactivity Opacity:** Automatically dims the HUD to a low opacity (default 40%) after 2 seconds of silence.
- **Brighten on Speech:** Immediately restores full HUD brightness when Voice Activity Detection detects user speech.
- **Ultra-Minimal Mode:** Hides text previews entirely, displaying only a microscopic status dot and live waveform.
- **Fullscreen Behavior:** Choose whether the overlay displays, hides, or dims when a fullscreen game or presentation is running.

### 4. Performance & Hardware
- **Execution Provider:** Selects the ONNX runtime provider (`cpu` int8 optimized; with architecture support for `cuda` and `directml`).
- **Thread Count:** Configures CPU inference parallelism (defaults to 4 threads).
- **Idle Memory Unload:** Automatically frees model weights from RAM after a configurable period of inactivity (e.g. 15 minutes) and reloads seamlessly on next hotkey press.

### 5. Storage & History
- **History Retention Limit:** Configurable max items (default 100 entries).
- **Store Audio:** Optionally saves recording audio as WAV BLOBs in the SQLite database for review.
- **Export / Clear:** One-click export of transcription history to JSON or database purge.

---

## Microphone Privacy Guide

If Murmur cannot access your microphone, Windows privacy permissions may be blocking desktop apps:

1. Press `Win + R`, paste the following command, and hit Enter:
   ```powershell
   ms-settings:privacy-microphone
   ```
2. Ensure **Microphone access** is toggled **On**.
3. Scroll down to **Let desktop apps access your microphone** and ensure it is set to **On**.
4. Restart Murmur. Use **Settings > Diagnostics > Copy Diagnostics Report** to inspect detected audio devices and system status.

---

## Elevated Application Pasting Note (Windows UIPI)

Windows implements **User Interface Privilege Isolation (UIPI)**. This security architecture prevents lower-integrity processes from sending synthetic keyboard events (`SendInput`, `WM_PASTE`) to windows running with Administrator privileges (elevated command prompts, Task Manager, elevated IDEs).

- **Symptom:** You dictate text while an Administrator window is focused, but no text appears in the target input.
- **Resolution:**
  1. Either launch Murmur as Administrator (**Right-click Murmur > Run as administrator**), or
  2. Simply click on Murmur's history view to copy the transcribed text with one click.

---

## 100% Offline Privacy Guarantee

Murmur was engineered from day one under strict zero-telemetry principles:

- **Zero Outbound Sockets:** The Murmur application binary opens no outbound network sockets, tracking pixels, or diagnostic beacons.
- **Local Neural Inference:** All acoustic modeling, CTC decoding, and post-processing run strictly on local CPU/GPU hardware.
- **Local Storage Only:** Transcription history, user dictionary rules, and settings are saved to a local SQLite database in `%APPDATA%\Murmur\murmur.db`.
- **Self-Contained Frontend:** All UI assets, icon sets, and web fonts (`Silkscreen`, `Inter`) are bundled inside the binary without external CDN links.

---

## Verification & Testing

Murmur includes a comprehensive Rust and TypeScript test suite verifying the end-to-end audio pipeline, state machine, VAD thresholds, post-processing rules, and Win32 hooks:

```powershell
# Run backend test suite (87 unit & integration tests)
cargo test

# Run Rust linter (0 warnings)
cargo clippy --all-targets -- -D warnings

# Run frontend typecheck and production build
npm run build
```

---

## License

Murmur is licensed under the MIT License. Models are distributed under their respective open-source licenses (NVIDIA Parakeet license).
