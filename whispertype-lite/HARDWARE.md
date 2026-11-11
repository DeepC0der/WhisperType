# Hardware & Environment Detection Report

Generated: 2026-09-22 02:37:09

## System Summary

| Component | Detected Specification |
|---|---|
| **OS** | Microsoft Windows 11 Home Single Language (Version 10.0.26200, Build 26200, 64-bit) |
| **CPU** | 13th Gen Intel(R) Core(TM) i5-13450HX (10 Cores, 16 Logical Processors) |
| **RAM** | 23.71 GB Total physical RAM (10.86 GB Available) |
| **Primary GPU** | NVIDIA GeForce RTX 4050 Laptop GPU (6141 MB VRAM, Driver 616.92) |
| **Integrated GPU** | Intel(R) UHD Graphics |
| **CUDA** | Available (v13.2) |
| **DirectML** | Available (C:\Windows\System32\DirectML.dll, v1.15.5+250417-0100.1.os-germanium.7c460ce) |
| **Audio Devices** | NVIDIA High Definition Audio, Realtek High Definition Audio(SST), NVIDIA Virtual Audio Device (Wave Extensible) (WDM), NVIDIA Broadcast, Nahimic VAD, Nahimic Easy Surround device, USB Audio Device, Nahimic mirroring device |

## Toolchain Status

| Tool | Version / Status |
|---|---|
| **Node.js** | v24.18.0 |
| **npm** | 11.16.0 |
| **Rustc** | rustc 1.98.1 (48a229cea 2026-09-01) |
| **Cargo** | cargo 1.98.1 (797e8a9bc 2026-08-05) |
| **CUDA Toolkit (nvcc)** | 13.2 |

## Hardware Capabilities & Recommended Inference Providers

1. **CUDA (Rank 1 - Recommended)**: NVIDIA RTX 4050 Laptop GPU with 6141 MB VRAM and CUDA 13.2 is available. This will provide the lowest latency and real-time factor (RTF << 0.1).
2. **DirectML (Rank 2)**: Windows DirectML is available (C:\Windows\System32\DirectML.dll) for hardware-accelerated DirectX 12 execution.
3. **CPU int8 (Rank 3 - Fallback)**: Intel/AMD CPU with 10 cores and 23.71 GB RAM provides robust CPU fallback with quantized int8 models.
