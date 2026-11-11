# scripts/phase0/hardware_detect.ps1
# Murmur Phase 0: Hardware Detection Script

$ErrorActionPreference = "Continue"

Write-Host "=== Murmur Hardware & Environment Detection ===" -ForegroundColor Cyan

# 1. OS Info
$os = Get-CimInstance Win32_OperatingSystem
$osName = $os.Caption
$osVersion = $os.Version
$osBuild = $os.BuildNumber
$osArch = $os.OSArchitecture
Write-Host "OS: $osName (Version $osVersion, Build $osBuild, $osArch)"

# 2. CPU Info
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$cpuName = $cpu.Name.Trim()
$cpuCores = $cpu.NumberOfCores
$cpuLogical = $cpu.NumberOfLogicalProcessors
Write-Host "CPU: $cpuName ($cpuCores cores, $cpuLogical threads)"

# 3. RAM Info
$totalRamBytes = $os.TotalVisibleMemorySize * 1KB
$totalRamGB = [math]::Round($totalRamBytes / 1GB, 2)
$freeRamBytes = $os.FreePhysicalMemory * 1KB
$freeRamGB = [math]::Round($freeRamBytes / 1GB, 2)
Write-Host "RAM: Total $totalRamGB GB, Free $freeRamGB GB"

# 4. GPU Info via nvidia-smi
$gpuInfo = @()
$nvidiaSmi = Get-Command nvidia-smi -ErrorAction SilentlyContinue
if ($nvidiaSmi) {
    try {
        $gpuOut = & nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader,nounits
        foreach ($line in $gpuOut) {
            $parts = $line -split ',\s*'
            if ($parts.Count -ge 3) {
                $gpuInfo += [PSCustomObject]@{
                    Name = $parts[0]
                    VRAM_MB = [int]$parts[1]
                    Driver = $parts[2]
                }
            }
        }
    } catch {
        Write-Warning "Failed to query nvidia-smi: $_"
    }
}
$otherGpus = Get-CimInstance Win32_VideoController | Where-Object { $_.Name -notlike "*NVIDIA*" }
$iGpuNames = ($otherGpus | ForEach-Object { $_.Name }) -join ", "

foreach ($g in $gpuInfo) {
    Write-Host "GPU: $($g.Name) (VRAM: $($g.VRAM_MB) MB, Driver: $($g.Driver))"
}
if ($iGpuNames) {
    Write-Host "iGPU: $iGpuNames"
}

# 5. CUDA info
$cudaAvailable = $false
$cudaVersion = "Not detected"
$nvcc = Get-Command nvcc -ErrorAction SilentlyContinue
if ($nvcc) {
    $nvccOut = (& nvcc --version) -join "`n"
    if ($nvccOut -match "release ([0-9.]+)") {
        $cudaAvailable = $true
        $cudaVersion = $matches[1]
    }
}
Write-Host "CUDA: Available=$cudaAvailable (Version: $cudaVersion)"

# 6. DirectML info
$directMLPath = "$env:SystemRoot\System32\DirectML.dll"
$directMLAvailable = Test-Path $directMLPath
$directMLVersion = "N/A"
if ($directMLAvailable) {
    $verInfo = (Get-Item $directMLPath).VersionInfo
    $directMLVersion = $verInfo.FileVersion
}
Write-Host "DirectML: Available=$directMLAvailable (DirectML.dll version: $directMLVersion)"

# 7. Audio input devices (via Windows Audio Device enumeration)
$audioInputs = @()
try {
    # Using PnP / SoundDevice
    $soundDevices = Get-CimInstance Win32_SoundDevice
    foreach ($sd in $soundDevices) {
        if ($sd.Status -eq "OK") {
            $audioInputs += $sd.Name
        }
    }
} catch {
    Write-Warning "Could not query sound devices: $_"
}
Write-Host "Sound Devices detected: $($audioInputs -join '; ')"

# 8. Check Node.js and Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe") {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    }
}
$nodeVer = try { (node -v) } catch { "Not installed" }
$npmVer = try { (npm -v) } catch { "Not installed" }
$rustcVer = try { (rustc --version) } catch { "Not installed" }
$cargoVer = try { (cargo --version) } catch { "Not installed" }
Write-Host "Node: $nodeVer, npm: $npmVer"
Write-Host "Rust: $rustcVer, Cargo: $cargoVer"

# 9. Format output markdown
$hardwareMd = @"
# Hardware & Environment Detection Report

Generated: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")

## System Summary

| Component | Detected Specification |
|---|---|
| **OS** | $osName (Version $osVersion, Build $osBuild, $osArch) |
| **CPU** | $cpuName ($cpuCores Cores, $cpuLogical Logical Processors) |
| **RAM** | $totalRamGB GB Total physical RAM ($freeRamGB GB Available) |
| **Primary GPU** | $(if ($gpuInfo.Count -gt 0) { "$($gpuInfo[0].Name) ($($gpuInfo[0].VRAM_MB) MB VRAM, Driver $($gpuInfo[0].Driver))" } else { "None detected" }) |
| **Integrated GPU** | $(if ($iGpuNames) { $iGpuNames } else { "None detected" }) |
| **CUDA** | $(if ($cudaAvailable) { "Available (v$cudaVersion)" } else { "Not installed / not on PATH" }) |
| **DirectML** | $(if ($directMLAvailable) { "Available ($directMLPath, v$directMLVersion)" } else { "Not found in System32" }) |
| **Audio Devices** | $($audioInputs -join ', ') |

## Toolchain Status

| Tool | Version / Status |
|---|---|
| **Node.js** | $nodeVer |
| **npm** | $npmVer |
| **Rustc** | $rustcVer |
| **Cargo** | $cargoVer |
| **CUDA Toolkit (nvcc)** | $cudaVersion |

## Hardware Capabilities & Recommended Inference Providers

1. **CUDA (Rank 1 - Recommended)**: NVIDIA RTX 4050 Laptop GPU with $($gpuInfo[0].VRAM_MB) MB VRAM and CUDA $cudaVersion is available. This will provide the lowest latency and real-time factor (RTF << 0.1).
2. **DirectML (Rank 2)**: Windows DirectML is available ($directMLPath) for hardware-accelerated DirectX 12 execution.
3. **CPU int8 (Rank 3 - Fallback)**: Intel/AMD CPU with $cpuCores cores and $totalRamGB GB RAM provides robust CPU fallback with quantized int8 models.
"@

$outPath = Join-Path $PSScriptRoot "..\..\HARDWARE.md"
$hardwareMd | Set-Content -Path $outPath -Encoding utf8
Write-Host "`nWrote detection report to $outPath" -ForegroundColor Green
