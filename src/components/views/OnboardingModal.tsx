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
  } = useAppStore();

  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [testedHotkey, setTestedHotkey] = useState(false);

  useEffect(() => {
    if (!showOnboarding) {
      setStep(1);
      setTestedHotkey(false);
    }
  }, [showOnboarding]);

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
  const normalizedMicLevel = Math.min(100, Math.max(0, micLevel > 1 ? micLevel : micLevel * 100));

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
                WELCOME TO MURMUR
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
                <h3 className="text-sm font-semibold text-foreground">Offline Speech Model Verified</h3>
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
                All acoustic processing and transcription happens entirely in memory on your CPU or GPU.
                Murmur makes zero network requests. No audio is ever uploaded to external servers.
              </p>
            </div>

            <div className="p-3 rounded-lg border border-border/50 bg-secondary/30 text-xs space-y-1">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Model Variant:</span>
                <span className="font-mono text-foreground font-medium">parakeet-tdt-0.6b-v3-int8</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Status:</span>
                <span className="font-mono text-emerald-400 font-semibold">
                  {modelStatus?.is_loaded || modelStatus?.status?.status === "ready"
                    ? "READY IN MEMORY"
                    : "LOADING..."}
                </span>
              </div>
            </div>

            <div className="flex justify-end pt-2">
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
                  Try triggering Murmur using your configured global shortcut.
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
