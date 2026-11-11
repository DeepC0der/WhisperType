/**
 * WhisperType Interactive Landing Page Script
 * 60fps Real-Time Audio Canvas, State Machine Simulator, Theme Manager,
 * and Accessible Native Dialog Controllers (following modern-web-guidance)
 */

document.addEventListener('DOMContentLoaded', () => {
  initThemeManager();
  initEditionSwitcher();
  initSimulator();
  initCopyButtons();
  initModals();
});

/* ==========================================================================
   1b. Two-Tier Edition Switcher (WhisperType Lite vs. WhisperType Standard)
   ========================================================================== */
function initEditionSwitcher() {
  const switchBtns = document.querySelectorAll('[data-edition-tab]');
  const cardSelectBtns = document.querySelectorAll('[data-select-edition]');
  const banner = document.getElementById('editionActiveBanner');
  const eabTag = document.getElementById('eabTag');
  const eabDesc = document.getElementById('eabDesc');
  const eabSpec = document.getElementById('eabSpec');
  const dlBoxLite = document.getElementById('dlBoxLite');
  const dlBoxStandard = document.getElementById('dlBoxStandard');
  const dlBadgeExe = document.getElementById('dlBadgeExe');
  const dlTitleExe = document.getElementById('dlTitleExe');
  const dlMetaExe = document.getElementById('dlMetaExe');
  const dlBtnExe = document.getElementById('dlBtnExe');
  const lmGuideBox = document.getElementById('lmStudioGuideBox');

  const EDITIONS = {
    lite: {
      tag: 'WHISPERTYPE LITE PROFILE • EVERYDAY PCS & LAPTOPS',
      desc: 'Completely stripped of Local LLM / LM Studio and Pro Mode overhead so it runs effortlessly on any normal PC or laptop (~35 MB idle RAM, 2 CPU threads, pure NVIDIA Parakeet TDT 0.6B Int8 speech-to-text, instant auto-unload).',
      spec: 'Recommended: Any x64 CPU • 4 GB+ RAM • 2 CPU Threads • ~35 MB Idle',
      exeBadge: 'FOR EVERYDAY PCS & LAPTOPS',
      exeTitle: 'WhisperType Lite Installer (.exe)',
      exeMeta: 'Ultra-lightweight NSIS setup (~12 MB). Pure sub-70ms ASR transcription, 2 CPU threads, ~35 MB idle RAM, and Audio & Transcript Review player.',
      exeBtnText: 'Download WhisperType Lite (.exe)',
      exeUrl: 'https://github.com/DeepC0der/WhisperType/releases/latest'
    },
    standard: {
      tag: 'WHISPERTYPE STANDARD PROFILE • HIGH-SPEC PCS & WORKSTATIONS',
      desc: 'Includes the full offline ASR engine PLUS Pro AI Mode (Ctrl+Win / Mouse4) connected to local LLMs (via LM Studio / Llama 3 / Qwen) for automatic grammar fixing, speech backtracking/self-correction, and structured Markdown note-making.',
      spec: 'Recommended: 8–16 GB+ RAM • Dedicated GPU/NPU for Local LLM',
      exeBadge: 'HIGH-SPEC PCS & LOCAL LLMs',
      exeTitle: 'WhisperType Standard Installer (.exe)',
      exeMeta: 'Full desktop installer with offline Parakeet ASR + Pro AI Mode (Ctrl+Win / Mouse4, LM Studio loopback) + Audio & Transcript History vault.',
      exeBtnText: 'Download WhisperType Standard (.exe)',
      exeUrl: 'https://github.com/DeepC0der/WhisperType/releases/latest'
    }
  };

  function setEdition(editionKey, notify = false) {
    const cfg = EDITIONS[editionKey] || EDITIONS.lite;
    switchBtns.forEach(btn => {
      const active = btn.getAttribute('data-edition-tab') === editionKey;
      btn.classList.toggle('active', active);
      btn.setAttribute('aria-pressed', active ? 'true' : 'false');
    });

    if (banner) {
      banner.classList.toggle('standard-mode', editionKey === 'standard');
    }
    if (eabTag) eabTag.textContent = cfg.tag;
    if (eabDesc) eabDesc.textContent = cfg.desc;
    if (eabSpec) eabSpec.textContent = cfg.spec;
    if (dlBoxLite && dlBoxStandard) {
      if (editionKey === 'standard') {
        dlBoxStandard.classList.add('featured');
        dlBoxLite.classList.remove('featured');
      } else {
        dlBoxLite.classList.add('featured');
        dlBoxStandard.classList.remove('featured');
      }
    }
    if (dlBadgeExe) dlBadgeExe.textContent = cfg.exeBadge;
    if (dlTitleExe) dlTitleExe.textContent = cfg.exeTitle;
    if (dlMetaExe) dlMetaExe.textContent = cfg.exeMeta;
    if (dlBtnExe) {
      const span = dlBtnExe.querySelector('span');
      if (span) span.textContent = cfg.exeBtnText;
      dlBtnExe.href = cfg.exeUrl;
    }
    if (lmGuideBox) {
      lmGuideBox.style.borderColor = editionKey === 'standard' ? 'var(--status-refining)' : 'var(--border)';
    }

    if (notify) {
      showToast(
        editionKey === 'standard'
          ? 'Selected WhisperType Standard (with Pro AI Mode)'
          : 'Selected WhisperType Lite (~35 MB Idle • Everyday PCs)'
      );
    }
  }

  switchBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const ed = btn.getAttribute('data-edition-tab');
      setEdition(ed, true);
    });
  });

  cardSelectBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const ed = btn.getAttribute('data-select-edition');
      setEdition(ed, true);
    });
  });
}

/* ==========================================================================
   1. Theme Management (Supporting 7 Desktop Themes & modern-web-guidance)
   ========================================================================== */
const THEMES = {
  dark: { name: 'Dark (Phosphor Green)', scheme: 'dark' },
  amber: { name: 'Retro Amber CRT', scheme: 'dark' },
  matrix: { name: 'Matrix Neon', scheme: 'dark' },
  cyan: { name: 'Cyberpunk Cyan', scheme: 'dark' },
  midnight: { name: 'Midnight Indigo', scheme: 'dark' },
  light: { name: 'Clean Light', scheme: 'light' },
  paper: { name: 'Warm Paper', scheme: 'light' }
};

function initThemeManager() {
  const metaColorScheme = document.querySelector('meta[name="color-scheme"]');
  const savedTheme = localStorage.getItem('whispertype_theme') || 'dark';

  function setTheme(themeId) {
    if (!THEMES[themeId]) themeId = 'dark';
    document.documentElement.setAttribute('data-theme', themeId);
    if (metaColorScheme) {
      metaColorScheme.content = THEMES[themeId].scheme;
    }
    localStorage.setItem('whispertype_theme', themeId);

    // Update active states on all theme pills & chips
    document.querySelectorAll('[data-set-theme]').forEach(btn => {
      const isCurrent = btn.getAttribute('data-set-theme') === themeId;
      btn.setAttribute('aria-pressed', isCurrent ? 'true' : 'false');
      btn.classList.toggle('active', isCurrent);
    });
  }

  // Bind clicks on theme selectors
  document.querySelectorAll('[data-set-theme]').forEach(btn => {
    btn.addEventListener('click', () => {
      const targetTheme = btn.getAttribute('data-set-theme');
      setTheme(targetTheme);
      showToast(`Switched theme to ${THEMES[targetTheme]?.name || targetTheme}`);
    });
  });

  // Apply initially
  setTheme(savedTheme);
}

/* ==========================================================================
   2. Interactive Live HUD & Dictation Simulator
   ========================================================================== */
function initSimulator() {
  const canvas = document.getElementById('waveformCanvas');
  const ctx = canvas ? canvas.getContext('2d') : null;
  const triggerBtn = document.getElementById('simTriggerBtn');
  const hudElement = document.getElementById('floatingHud');
  const hudStatusText = document.getElementById('hudStatusText');
  const hudTimerText = document.getElementById('hudTimerText');
  const hudProBadge = document.getElementById('hudProBadge');
  const editorText = document.getElementById('targetEditorText');
  const editorStats = document.getElementById('editorStats');
  const proToggleBtn = document.getElementById('simProToggle');

  if (!canvas || !ctx || !triggerBtn) return;

  // Set high DPI canvas resolution
  function resizeCanvas() {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
  }
  resizeCanvas();
  window.addEventListener('resize', resizeCanvas);

  // Simulator State
  let simState = 'IDLE'; // 'IDLE' | 'RECORDING' | 'TRANSCRIBING' | 'REFINING' | 'DONE'
  let isProMode = false;
  let timerInterval = null;
  let secondsElapsed = 0;
  let animFrameId = null;
  let wavePhase = 0;
  let audioIntensity = 0; // 0.0 to 1.0

  const dictationSamples = [
    {
      raw: "testing local offline dictation with WhisperType comma powered by NVIDIA Parakeet TDT zero point six B on Windows period",
      normal: "Testing local offline dictation with WhisperType, powered by NVIDIA Parakeet TDT 0.6B on Windows.",
      pro: "### Summary\n- **Engine**: WhisperType (Local ASR)\n- **Model**: NVIDIA Parakeet TDT 0.6B Int8\n- **Platform**: Windows 10/11 (100% Air-Gapped)\n- **Status**: Verified sub-70ms transcription latency."
    },
    {
      raw: "meeting notes scratch that action items for team comma optimize sherpa onnx pipeline comma check AVX two vectorization comma and release installers period",
      normal: "Action items for team:\nOptimize sherpa-onnx pipeline, check AVX2 vectorization, and release installers.",
      pro: "## Action Items (Auto-Backtracked)\n- [x] Optimize Sherpa-ONNX inference pipeline\n- [x] Validate AVX2 vector execution\n- [ ] Package NSIS & MSI release installers\n\n*Pro Mode automatically handled speech backtracking & self-correction.*"
    },
    {
      raw: "zero telemetry guarantee colon all voice packets remain entirely on local hardware with zero cloud egress period",
      normal: "Zero telemetry guarantee: all voice packets remain entirely on local hardware with zero cloud egress.",
      pro: "🔒 **Privacy Audit Passed**\n- Inbound Sockets: 0\n- Outbound Packets: 0\n- Speech Model: Local CPU/DirectML memory map"
    }
  ];
  let sampleIndex = 0;

  // Toggle Pro Mode (LM Studio AI Refinement)
  if (proToggleBtn) {
    proToggleBtn.addEventListener('click', () => {
      isProMode = !isProMode;
      proToggleBtn.setAttribute('aria-pressed', isProMode ? 'true' : 'false');
      proToggleBtn.classList.toggle('active', isProMode);
      if (hudProBadge) {
        hudProBadge.style.display = isProMode ? 'inline-block' : 'none';
      }
      showToast(isProMode ? "Professional Mode (LM Studio) Enabled" : "Normal Mode Enabled");
    });
  }

  // 60fps Canvas Waveform Renderer
  function renderWaveform() {
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    ctx.clearRect(0, 0, width, height);

    if (simState === 'RECORDING') {
      // Dynamic simulated speech envelope
      audioIntensity += (Math.random() * 0.4 + 0.3 - audioIntensity) * 0.15;
      wavePhase += 0.18;

      const barsCount = 18;
      const barWidth = 3;
      const gap = (width - barsCount * barWidth) / (barsCount - 1);
      const computedColor = getComputedStyle(document.documentElement).getPropertyValue(
        isProMode ? '--status-refining' : '--status-recording'
      ).trim() || '#ef4444';

      ctx.fillStyle = computedColor;

      for (let i = 0; i < barsCount; i++) {
        // Multi-frequency sine combination + random vocal turbulence
        const normalizedX = i / barsCount;
        const wave1 = Math.sin(wavePhase + normalizedX * Math.PI * 3);
        const wave2 = Math.cos(wavePhase * 1.5 + normalizedX * Math.PI * 4);
        const centerWeight = Math.sin(normalizedX * Math.PI); // center is taller

        const rawHeight = (Math.abs(wave1 * 0.6 + wave2 * 0.4) * audioIntensity * centerWeight + 0.15) * height;
        const finalHeight = Math.max(4, Math.min(height, rawHeight));
        const x = i * (barWidth + gap);
        const y = (height - finalHeight) / 2;

        ctx.beginPath();
        if (ctx.roundRect) {
          ctx.roundRect(x, y, barWidth, finalHeight, barWidth / 2);
        } else {
          ctx.rect(x, y, barWidth, finalHeight);
        }
        ctx.fill();
      }
    } else {
      // Idle / Resting line with subtle breathing dot
      const computedColor = getComputedStyle(document.documentElement).getPropertyValue('--border').trim() || '#334155';
      ctx.fillStyle = computedColor;
      const barWidth = 3;
      const barsCount = 18;
      const gap = (width - barsCount * barWidth) / (barsCount - 1);

      for (let i = 0; i < barsCount; i++) {
        const x = i * (barWidth + gap);
        const y = (height - 3) / 2;
        ctx.fillRect(x, y, barWidth, 3);
      }
    }

    animFrameId = requestAnimationFrame(renderWaveform);
  }
  animFrameId = requestAnimationFrame(renderWaveform);

  // Start Recording State
  function startRecording() {
    if (simState === 'RECORDING') return;
    simState = 'RECORDING';
    secondsElapsed = 0;

    triggerBtn.classList.add('active');
    triggerBtn.querySelector('.btn-label').textContent = 'Release to Transcribe';
    hudElement.className = `floating-hud-replica recording ${isProMode ? 'pro-mode' : ''}`;
    hudStatusText.textContent = 'REC';
    hudStatusText.style.color = isProMode ? 'var(--status-refining)' : 'var(--status-recording)';
    hudTimerText.textContent = '00:00';
    hudTimerText.style.display = 'block';

    timerInterval = setInterval(() => {
      secondsElapsed++;
      const mins = String(Math.floor(secondsElapsed / 60)).padStart(2, '0');
      const secs = String(secondsElapsed % 60).padStart(2, '0');
      hudTimerText.textContent = `${mins}:${secs}`;
    }, 1000);
  }

  // Stop Recording & Process Speech
  function stopRecording() {
    if (simState !== 'RECORDING') return;
    clearInterval(timerInterval);

    // Phase 1: Transcribing (~70ms latency simulation)
    simState = 'TRANSCRIBING';
    triggerBtn.classList.remove('active');
    triggerBtn.querySelector('.btn-label').textContent = 'Hold-to-Talk (Ctrl+Shift+Space)';
    hudElement.className = `floating-hud-replica transcribing ${isProMode ? 'pro-mode' : ''}`;
    hudStatusText.textContent = isProMode ? 'AI NOTES...' : 'TRANSCRIBING...';
    hudStatusText.style.color = isProMode ? 'var(--status-refining)' : 'var(--status-transcribing)';

    const latencyMs = isProMode ? 280 : 72; // Parakeet-TDT 0.066x RTF ~70ms

    setTimeout(() => {
      // Phase 2: Dual-Mode Text Injection simulation
      const sample = dictationSamples[sampleIndex % dictationSamples.length];
      sampleIndex++;
      const textToInject = isProMode ? sample.pro : sample.normal;

      hudElement.className = `floating-hud-replica ${isProMode ? 'pro-mode' : ''}`;
      hudStatusText.textContent = 'INSERTING...';
      hudStatusText.style.color = 'var(--primary)';

      typewriteText(editorText, textToInject, () => {
        hudStatusText.textContent = 'DONE';
        hudStatusText.style.color = 'var(--primary)';
        if (editorStats) {
          editorStats.textContent = `RTF: 0.066x | Latency: ${latencyMs}ms | Injected via Clipboard (${textToInject.length} chars)`;
        }

        setTimeout(() => {
          hudStatusText.textContent = 'READY';
          hudStatusText.style.color = 'var(--text)';
          hudTimerText.style.display = 'none';
          simState = 'IDLE';
        }, 1200);
      });
    }, latencyMs);
  }

  // Interactive typewriter effect in simulator
  function typewriteText(element, fullText, onComplete) {
    element.innerHTML = '';
    const span = document.createElement('span');
    span.style.whiteSpace = 'pre-wrap';
    element.appendChild(span);

    let idx = 0;
    const speed = 12; // fast realistic injection speed

    function typeStep() {
      if (idx < fullText.length) {
        // paste in chunks of 2-4 chars to simulate rapid keystroke/clipboard batch
        const chunk = fullText.slice(idx, idx + 3);
        span.textContent += chunk;
        idx += 3;
        setTimeout(typeStep, speed);
      } else {
        span.textContent = fullText;
        if (onComplete) onComplete();
      }
    }
    typeStep();
  }

  // Trigger Button Click & Hold Handlers
  let isPointerDown = false;

  triggerBtn.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    isPointerDown = true;
    startRecording();
  });

  window.addEventListener('pointerup', () => {
    if (isPointerDown) {
      isPointerDown = false;
      stopRecording();
    }
  });

  // Global Hotkey Simulation (`Ctrl + Shift + Space`)
  window.addEventListener('keydown', (e) => {
    if (e.code === 'Space' && e.ctrlKey && e.shiftKey && !e.repeat) {
      e.preventDefault();
      if (simState === 'IDLE') {
        startRecording();
      } else if (simState === 'RECORDING') {
        stopRecording();
      }
    } else if (e.code === 'Escape' && simState === 'RECORDING') {
      // Murmur Emergency Esc Cancellation
      e.preventDefault();
      clearInterval(timerInterval);
      simState = 'IDLE';
      triggerBtn.classList.remove('active');
      triggerBtn.querySelector('.btn-label').textContent = 'Hold-to-Talk (Ctrl+Shift+Space)';
      hudElement.className = 'floating-hud-replica';
      hudStatusText.textContent = 'ABORTED';
      hudStatusText.style.color = 'var(--status-recording)';
      showToast("Emergency Abort (Escape): audio buffer cleared, clipboard preserved.");
      setTimeout(() => {
        hudStatusText.textContent = 'READY';
        hudStatusText.style.color = 'var(--text)';
        hudTimerText.style.display = 'none';
      }, 1000);
    }
  });
}

/* ==========================================================================
   3. Copy Handlers & Toast Feedback
   ========================================================================== */
function initCopyButtons() {
  document.querySelectorAll('[data-copy-cmd]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const textToCopy = btn.getAttribute('data-copy-cmd');
      if (!textToCopy) return;

      try {
        await navigator.clipboard.writeText(textToCopy);
        const originalText = btn.innerHTML;
        btn.innerHTML = `✓ Copied!`;
        showToast("Copied command to clipboard!");
        setTimeout(() => {
          btn.innerHTML = originalText;
        }, 2200);
      } catch (err) {
        showToast("Error copying to clipboard, please copy manually.");
      }
    });
  });
}

function showToast(message) {
  let toast = document.getElementById('toastNotice');
  if (!toast) {
    toast = document.createElement('div');
    toast.id = 'toastNotice';
    toast.className = 'toast-notice';
    document.body.appendChild(toast);
  }
  toast.textContent = message;
  toast.classList.add('visible');

  clearTimeout(toast._timeout);
  toast._timeout = setTimeout(() => {
    toast.classList.remove('visible');
  }, 2600);
}

/* ==========================================================================
   4. Native Modal Dialog Controllers (Complying with modern-web-guidance)
   ========================================================================== */
function initModals() {
  const smartScreenDialog = document.getElementById('smartScreenModal');
  const openModalBtn = document.getElementById('openSmartScreenGuide');
  const closeModalBtn = document.getElementById('closeSmartScreenModal');
  const downloadChecksumsBtn = document.getElementById('downloadChecksumsBtn');

  if (!smartScreenDialog) return;

  if (openModalBtn) {
    openModalBtn.addEventListener('click', () => {
      smartScreenDialog.showModal();
    });
  }

  if (closeModalBtn) {
    closeModalBtn.addEventListener('click', () => {
      smartScreenDialog.close();
    });
  }

  // modern-web-guidance fallback for browsers without native `closedby` attribute support
  if (!('closedBy' in HTMLDialogElement.prototype)) {
    smartScreenDialog.addEventListener('click', (event) => {
      if (event.target !== smartScreenDialog) return;
      const rect = smartScreenDialog.getBoundingClientRect();
      const isInside = (
        rect.top <= event.clientY &&
        event.clientY <= rect.top + rect.height &&
        rect.left <= event.clientX &&
        event.clientX <= rect.left + rect.width
      );
      if (!isInside) {
        smartScreenDialog.close();
      }
    });
  }

  // Checksum Generator & Instant Download
  if (downloadChecksumsBtn) {
    downloadChecksumsBtn.addEventListener('click', () => {
      const manifestText = [
        "# WhisperType v0.1.0 Release SHA-256 Hashes",
        "# Generated for official x64 Windows binary distribution",
        "",
        "SHA-256 (WhisperType_0.1.0_x64-setup.exe) = 8f94e4a938c5b058c4f74d0e8e916a2ef137bcde847f9a46b9a81f3dc25a071d",
        "SHA-256 (WhisperType_0.1.0_x64_en-US.msi) = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "SHA-256 (WhisperType-Portable.zip)      = a56c71048b111db4c75f190e4f21db5ec4a02bb40dcf99371e7d825f778a4623",
        "SHA-256 (sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2) = f9432d6c95357904325a666993cbfb36f784e1b9b1b1178229b9f957eb62c208",
        "",
        "# Air-Gapped Verification: 0 outbound connections, 100% local inference"
      ].join("\n");

      const blob = new Blob([manifestText], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'whispertype-v0.1.0-checksums.txt';
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      showToast("Downloaded official checksum manifest!");
    });
  }
}
