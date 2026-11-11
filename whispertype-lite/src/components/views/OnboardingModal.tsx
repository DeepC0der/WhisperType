import React, { useState, useEffect } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Button } from "@/components/ui/button";
import {
  ShieldCheck,
  Cpu,
  Volume2,
  Keyboard,
  CheckCircle2,
  ArrowRight,
  ArrowLeft,
  Sparkles,
  Lock,
} from "lucide-react";

export const OnboardingModal: React.FC = () => {
  const {
    showOnboarding,
    setShowOnboarding,
    settings,
    audioDevices,
    modelStatus,
    micLevel,
    saveSettings,
    validateModelPath,
    importLocalModel,
    fetchModelStatus,
  } = useAppStore();

  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [testedHotkey, setTestedHotkey] = useState(false);
  const [modelImportPath, setModelImportPath] = useState("");
  const [importingModel, setImportingModel] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);
  const [modelSuccess, setModelSuccess] = useState<string | null>(null);

  const isModelReady = modelStatus?.is_loaded || modelStatus?.status?.status === "ready";
  const isModelLoading = modelStatus?.is_loading || modelStatus?.status?.status === "loading";

  const handleImportModel = async () => {
    if (!modelImportPath.trim()) return;
    setImportingModel(true);
    setModelError(null);
    setModelSuccess(null);
    try {
      const val = await validateModelPath(modelImportPath.trim());
      if (!val.valid) {
        setModelError(val.error_message || `Missing required files: ${val.missing_files.join(", ")}`);
        setImportingModel(false);
        return;
      }
      const importedTo = await importLocalModel(modelImportPath.trim());
      setModelSuccess(`Model imported successfully to ${importedTo}`);
      await fetchModelStatus();
    } catch (err) {
      setModelError(String(err));
    } finally {
      setImportingModel(false);
    }
  };

  useEffect(() => {
    if (!showOnboarding) {
      setStep(1);
      setTestedHotkey(false);
    }
  }, [showOnboarding]);

  // Live microphone monitor probe in Step 2
  useEffect(() => {
    if (!showOnboarding || step !== 2) {
      if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
        import("@tauri-apps/api/core").then(({ invoke }) => {
          invoke("stop_mic_test").catch(() => {});
        });
      }
      return;
    }

    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/core").then(({ invoke }) => {
        invoke("start_mic_test", {
          deviceName: settings.audio_device && settings.audio_device !== "default" ? settings.audio_device : null,
        }).catch(() => {});
      });
    }

    return () => {
      if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
        import("@tauri-apps/api/core").then(({ invoke }) => {
          invoke("stop_mic_test").catch(() => {});
        });
      }
    };
  }, [showOnboarding, step, settings.audio_device]);

  // Live hotkey test listener in Step 3
  useEffect(() => {
    if (!showOnboarding || step !== 3) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      const keys: string[] = [];
      if (e.ctrlKey) keys.push("Ctrl");
      if (e.altKey) keys.push("Alt");
      if (e.shiftKey) keys.push("Shift");
      if (e.metaKey) keys.push("Win");

      let keyName = e.key;
      if (keyName === " ") keyName = "Space";
      else if (keyName.length === 1) keyName = keyName.toUpperCase();

      if (!["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
        if (!keys.includes(keyName)) {
          keys.push(keyName);
        }
      }

      const pressed = keys.join("+");
      if (pressed.toLowerCase() === settings.hotkey.toLowerCase()) {
        e.preventDefault();
        setTestedHotkey(true);
      }
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => window.removeEventListener("keydown", handleKeyDown, { capture: true });
  }, [showOnboarding, step, settings.hotkey]);

  if (!showOnboarding) return null;

  const handleFinish = async () => {
    await saveSettings({ first_run_completed: true });
    setShowOnboarding(false);
  };

  const hotkeyTokens = (settings.hotkey || "Ctrl+Shift+Space").split("+");
  // Perceptual non-linear scaling for natural conversational speech visibility
  const normalizedMicLevel =
    micLevel <= 1.5
      ? 0
      : Math.min(100, Math.sqrt((micLevel - 1.5) / 85.0) * 100);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="onboarding-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/85 backdrop-blur-sm p-4 select-none"
    >
      <div className="w-full max-w-lg rounded-xl border border-border/80 bg-card p-6 shadow-2xl pixel-border relative space-y-6">
        {/* Progress indicator */}
        <div className="flex items-center justify-between pb-3 border-b border-border/60">
          <div className="flex items-center space-x-2">
            <div className="h-7 w-7 rounded-md bg-primary/10 border border-primary/40 flex items-center justify-center">
              <Sparkles className="h-4 w-4 text-primary" />
            </div>
            <div>
              <h2 id="onboarding-title" className="text-sm font-pixel font-bold tracking-wider text-foreground">
                WELCOME TO WHISPERTYPE
              </h2>
              <p className="text-[11px] text-muted-foreground">Step {step} of 3 • Quick Setup</p>
            </div>
          </div>

          <div className="flex items-center space-x-1.5">
            {[1, 2, 3].map((s) => (
              <div
                key={s}
                className={`h-2 rounded-full transition-all ${
                  step === s
                    ? "w-6 bg-primary"
                    : step > s
                    ? "w-2 bg-primary/50"
                    : "w-2 bg-secondary"
                }`}
              />
            ))}
          </div>
        </div>

        {/* STEP 1: Model & Offline Engine */}
        {step === 1 && (
          <div className="space-y-4">
            <div className="flex items-center space-x-3 p-3 rounded-lg bg-secondary/50 border border-border/60">
              <Cpu className="h-8 w-8 text-primary shrink-0" />
              <div>
                <h3 className="text-sm font-semibold text-foreground">
                  {isModelReady
                    ? "Offline Speech Model Verified"
                    : isModelLoading
                    ? "Loading Offline Model..."
                    : "Local Speech Model Setup"}
                </h3>
                <p className="text-xs text-muted-foreground">
                  Running NVIDIA Parakeet TDT 0.6B quantized to Int8.
                </p>
              </div>
            </div>

            <div className="space-y-2 text-xs leading-relaxed text-muted-foreground">
              <div className="flex items-center space-x-2 text-foreground">
                <ShieldCheck className="h-4 w-4 text-emerald-400" />
                <span className="font-semibold">Zero-Cloud, Complete Privacy Guarantee</span>
              </div>
              <p>
                All acoustic processing and transcription happens entirely in memory on your CPU.
                WhisperType makes zero network requests. No audio is ever uploaded to external servers.
              </p>
            </div>

            <div className="p-3 rounded-lg border border-border/50 bg-secondary/30 text-xs space-y-1.5">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Model Variant:</span>
                <span className="font-mono text-foreground font-medium">parakeet-tdt-0.6b-v3-int8</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-muted-foreground">Status:</span>
                <span
                  className={`font-mono font-semibold ${
                    isModelReady
                      ? "text-emerald-400"
                      : isModelLoading
                      ? "text-amber-400"
                      : "text-rose-400"
                  }`}
                >
                  {isModelReady
                    ? "READY IN MEMORY"
                    : isModelLoading
                    ? "LOADING..."
                    : "MODEL NOT FOUND"}
                </span>
              </div>
              {modelStatus?.model_dir && (
                <div className="flex justify-between text-[11px] text-muted-foreground truncate pt-1 border-t border-border/40">
                  <span>Location:</span>
                  <span className="font-mono truncate max-w-[260px]">{modelStatus.model_dir}</span>
                </div>
              )}
            </div>

            {/* Model Import Flow when model is not ready */}
            {!isModelReady && (
              <div className="p-3 rounded-lg border border-border/70 bg-card space-y-2 text-xs">
                <span className="font-semibold block text-foreground">
                  Import Model Folder
                </span>
                <p className="text-muted-foreground text-[11px]">
                  Select the folder containing <code className="bg-secondary px-1 rounded">encoder.int8.onnx</code>,{" "}
                  <code className="bg-secondary px-1 rounded">decoder.int8.onnx</code>,{" "}
                  <code className="bg-secondary px-1 rounded">joiner.int8.onnx</code>, and{" "}
                  <code className="bg-secondary px-1 rounded">tokens.txt</code>.
                </p>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={modelImportPath}
                    onChange={(e) => setModelImportPath(e.target.value)}
                    placeholder="e.g. C:\Downloads\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"
                    className="flex-1 px-2.5 py-1.5 rounded text-xs bg-secondary border border-border focus:outline-none focus:ring-1 focus:ring-primary font-mono"
                  />
                  <Button
                    size="sm"
                    variant="pixel"
                    onClick={handleImportModel}
                    disabled={importingModel || !modelImportPath.trim()}
                    className="text-xs shrink-0 font-pixel h-8"
                  >
                    {importingModel ? "Importing..." : "Validate & Import"}
                  </Button>
                </div>
                {modelError && (
                  <p className="text-[11px] text-rose-400 font-medium">{modelError}</p>
                )}
                {modelSuccess && (
                  <p className="text-[11px] text-emerald-400 font-medium">{modelSuccess}</p>
                )}
              </div>
            )}

            <div className="flex justify-between items-center pt-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => fetchModelStatus()}
                className="text-xs font-pixel h-9"
              >
                Refresh Status
              </Button>
              <Button
                variant="pixel"
                size="sm"
                onClick={() => setStep(2)}
                className="text-xs font-pixel px-4 h-9"
              >
                Next: Microphone Setup
                <ArrowRight className="h-3.5 w-3.5 ml-1.5" />
              </Button>
            </div>
          </div>
        )}

        {/* STEP 2: Microphone & Privacy Guidance */}
        {step === 2 && (
          <div className="space-y-4">
            <div className="flex items-center space-x-3 p-3 rounded-lg bg-secondary/50 border border-border/60">
              <Volume2 className="h-8 w-8 text-primary shrink-0" />
              <div>
                <h3 className="text-sm font-semibold text-foreground">Microphone & Audio Check</h3>
                <p className="text-xs text-muted-foreground">
                  Choose input source and verify audio capture levels.
                </p>
              </div>
            </div>

            <div>
              <label className="text-xs font-medium text-foreground block mb-1">
                Select Audio Input Device:
              </label>
              <select
                value={settings.audio_device || "default"}
                onChange={(e) =>
                  saveSettings({ audio_device: e.target.value === "default" ? null : e.target.value })
                }
                className="w-full h-9 rounded border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
              >
                <option value="default">Default System Microphone (WASAPI)</option>
                {audioDevices.map((dev) => (
                  <option key={dev.name} value={dev.name}>
                    {dev.name}
                  </option>
                ))}
              </select>
            </div>

            {/* Live VU Meter Bar */}
            <div className="space-y-1.5 p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex justify-between text-[11px] font-pixel text-muted-foreground">
                <span>SPEAK TO TEST VU METER</span>
                <span>{Math.round(normalizedMicLevel)}%</span>
              </div>
              <div className="h-2.5 w-full bg-secondary rounded-full overflow-hidden flex">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-75"
                  style={{ width: `${Math.max(5, normalizedMicLevel)}%` }}
                />
              </div>
            </div>

            <div className="flex items-start space-x-2 text-xs text-muted-foreground">
              <Lock className="h-3.5 w-3.5 text-primary shrink-0 mt-0.5" />
              <span>
                Audio is buffered in volatile memory only while actively recording, and purged
                immediately upon transcription.
              </span>
            </div>

            <div className="flex justify-between pt-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setStep(1)}
                className="text-xs h-9 text-muted-foreground"
              >
                <ArrowLeft className="h-3.5 w-3.5 mr-1.5" />
                Back
              </Button>
              <Button
                variant="pixel"
                size="sm"
                onClick={() => setStep(3)}
                className="text-xs font-pixel px-4 h-9"
              >
                Next: Hotkey Test
                <ArrowRight className="h-3.5 w-3.5 ml-1.5" />
              </Button>
            </div>
          </div>
        )}

        {/* STEP 3: Hotkey Interactive Test */}
        {step === 3 && (
          <div className="space-y-4">
            <div className="flex items-center space-x-3 p-3 rounded-lg bg-secondary/50 border border-border/60">
              <Keyboard className="h-8 w-8 text-primary shrink-0" />
              <div>
                <h3 className="text-sm font-semibold text-foreground">Interactive Hotkey Test</h3>
                <p className="text-xs text-muted-foreground">
                  Try triggering WhisperType using your configured global shortcut.
                </p>
              </div>
            </div>

            <div className="text-center py-4 space-y-3">
              <div className="flex items-center justify-center space-x-1.5">
                {hotkeyTokens.map((token, i) => (
                  <React.Fragment key={i}>
                    <span className="keycap-chip text-sm px-3 py-1">{token}</span>
                    {i < hotkeyTokens.length - 1 && (
                      <span className="text-muted-foreground font-bold text-sm">+</span>
                    )}
                  </React.Fragment>
                ))}
              </div>
              <p className="text-xs text-muted-foreground">
                Current mode:{" "}
                <b className="text-foreground capitalize">{settings.hotkey_mode.replace(/-/g, " ")}</b>
              </p>
            </div>

            <div className="flex items-center justify-center p-3 rounded-lg border border-border/60 bg-secondary/30">
              {testedHotkey ? (
                <div className="flex items-center space-x-2 text-emerald-400 font-pixel text-xs phosphor-glow">
                  <CheckCircle2 className="h-4 w-4" />
                  <span>HOTKEY TEST PASSED! READY TO DICTATE</span>
                </div>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setTestedHotkey(true)}
                  className="text-xs h-8"
                >
                  Simulate Hotkey Trigger
                </Button>
              )}
            </div>

            <div className="flex justify-between pt-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setStep(2)}
                className="text-xs h-9 text-muted-foreground"
              >
                <ArrowLeft className="h-3.5 w-3.5 mr-1.5" />
                Back
              </Button>
              <Button
                variant="pixel"
                size="sm"
                onClick={handleFinish}
                className="text-xs font-pixel px-5 h-9 bg-primary text-primary-foreground phosphor-glow"
              >
                Complete Setup & Launch
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
