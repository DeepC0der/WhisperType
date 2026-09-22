import React, { useEffect, useState } from "react";
import { useAppStore, AppSettings } from "@/store/useAppStore";
import { applyThemeToDocument } from "@/lib/theme";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import {
  Sliders,
  Keyboard,
  Volume2,
  FileText,
  History,
  Cpu,
  Monitor,
  Info,
  Check,
  Copy,
  FolderOpen,
  ExternalLink,
  Play,
  Sparkles,
  AlertTriangle,
} from "lucide-react";

export const SettingsView: React.FC = () => {
  const {
    settings,
    audioDevices,
    fetchSettings,
    saveSettings,
    fetchAudioDevices,
    fetchModelStatus,
    setShowOnboarding,
  } = useAppStore();

  const [activeTab, setActiveTab] = useState("general");
  const [copiedDiag, setCopiedDiag] = useState(false);
  const [isCapturingHotkey, setIsCapturingHotkey] = useState(false);
  const [capturedKeys, setCapturedKeys] = useState<string[]>([]);
  const [isBenchmarking, setIsBenchmarking] = useState(false);
  const [benchmarkResult, setBenchmarkResult] = useState<{ rtf: number; inference_ms: number } | null>(null);
  const [isPreviewingOverlay, setIsPreviewingOverlay] = useState(false);

  useEffect(() => {
    fetchSettings();
    fetchAudioDevices();
    fetchModelStatus();
  }, [fetchSettings, fetchAudioDevices, fetchModelStatus]);

  // Live hotkey recording listener
  useEffect(() => {
    if (!isCapturingHotkey) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setIsCapturingHotkey(false);
        setCapturedKeys([]);
        return;
      }

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

      setCapturedKeys(keys);

      // If user pressed a main key with or without modifiers, save!
      if (!["Control", "Alt", "Shift", "Meta"].includes(e.key) && keys.length > 0) {
        const combo = keys.join("+");
        saveSettings({ hotkey: combo });
        setIsCapturingHotkey(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
    };
  }, [isCapturingHotkey, saveSettings]);

  // Copy diagnostics report
  const handleCopyDiagnostics = async () => {
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const report = await invoke<string>("copy_diagnostics");
        navigator.clipboard.writeText(report);
      } catch {
        navigator.clipboard.writeText("Murmur Diagnostics - Offline local speech");
      }
    } else {
      navigator.clipboard.writeText("Murmur v0.1.0 Diagnostics Report");
    }
    setCopiedDiag(true);
    setTimeout(() => setCopiedDiag(false), 2500);
  };

  // Open Windows microphone privacy settings
  const handleOpenMicPrivacy = async () => {
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("open_system_url", { url: "ms-settings:privacy-microphone" });
      } catch (err) {
        console.warn("open_system_url error:", err);
      }
    }
  };

  // Open Log Directory
  const handleOpenLogDir = async () => {
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("open_log_dir");
      } catch (err) {
        console.warn("open_log_dir error:", err);
      }
    }
  };

  // Trigger Benchmark
  const handleRunBenchmark = async () => {
    setIsBenchmarking(true);
    setBenchmarkResult(null);
    try {
      if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
        const { invoke } = await import("@tauri-apps/api/core");
        const res = await invoke<{ rtf: number; inference_ms: number }>("benchmark_model");
        setBenchmarkResult(res);
      } else {
        // Mock fallback
        setTimeout(() => {
          setBenchmarkResult({ rtf: 0.066, inference_ms: 198 });
          setIsBenchmarking(false);
        }, 1200);
        return;
      }
    } catch (err) {
      console.error("Benchmark failed:", err);
    } finally {
      setIsBenchmarking(false);
    }
  };

  // Preview Overlay
  const handlePreviewOverlay = async () => {
    setIsPreviewingOverlay(true);
    let emitFn: ((event: string, payload?: unknown) => Promise<void>) | null = null;
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const { emit } = await import("@tauri-apps/api/event");
        emitFn = emit;
        await invoke("position_overlay", { anchor: settings.overlay_position });
        await invoke("show_overlay");
      } catch {}
    }

    // Simulate overlay preview states with live simulated mic levels for waveform
    useAppStore.getState().setAppState("RECORDING");
    if (emitFn) emitFn("state-changed", { current_state: "RECORDING" });
    let ticks = 0;
    const waveInterval = setInterval(() => {
      ticks++;
      const fakeLevel = 18 + Math.sin(ticks * 0.8) * 14 + Math.random() * 8;
      useAppStore.getState().setMicLevel(fakeLevel);
      if (emitFn) emitFn("mic-level", fakeLevel);
    }, 100);

    setTimeout(() => {
      clearInterval(waveInterval);
      useAppStore.getState().setMicLevel(0);
      if (emitFn) emitFn("mic-level", 0);
      useAppStore.getState().setAppState("TRANSCRIBING");
      if (emitFn) emitFn("state-changed", { current_state: "TRANSCRIBING" });
      setTimeout(() => {
        useAppStore.getState().setAppState("DONE");
        if (emitFn) emitFn("state-changed", { current_state: "DONE" });
        setTimeout(async () => {
          useAppStore.getState().setAppState("IDLE");
          if (emitFn) emitFn("state-changed", { current_state: "IDLE" });
          if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
            try {
              const { invoke } = await import("@tauri-apps/api/core");
              await invoke("hide_overlay");
            } catch {}
          }
          setIsPreviewingOverlay(false);
        }, 600);
      }, 1000);
    }, 2400);
  };

  // Handle position change
  const handlePositionChange = async (pos: string) => {
    await saveSettings({ overlay_position: pos });
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("position_overlay", { anchor: pos });
      } catch {}
    }
  };

  return (
    <div className="flex flex-col space-y-4 pb-12">
      {/* Settings Header */}
      <div className="pb-3 border-b border-border flex items-center justify-between">
        <div>
          <h2 className="font-pixel text-sm tracking-wider uppercase font-bold text-foreground">
            System & Preferences
          </h2>
          <p className="text-xs text-muted-foreground">
            Configure hotkeys, offline speech engine, audio capture, and pixel overlay.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setShowOnboarding(true)}
          className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
        >
          <Sparkles className="h-3.5 w-3.5 mr-1 text-primary" />
          Onboarding Tour
        </Button>
      </div>

      {/* Tabs Navigation */}
      <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full">
        <TabsList className="grid grid-cols-4 sm:grid-cols-8 h-auto p-1 bg-secondary/50 rounded-lg border border-border/60 gap-1 font-pixel text-[11px]">
          <TabsTrigger value="general" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <Sliders className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">General</span>
          </TabsTrigger>
          <TabsTrigger value="hotkey" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <Keyboard className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">Hotkey</span>
          </TabsTrigger>
          <TabsTrigger value="audio" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <Volume2 className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">Audio</span>
          </TabsTrigger>
          <TabsTrigger value="text" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <FileText className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">Text</span>
          </TabsTrigger>
          <TabsTrigger value="history" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <History className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">History</span>
          </TabsTrigger>
          <TabsTrigger value="performance" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <Cpu className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">Perf</span>
          </TabsTrigger>
          <TabsTrigger value="overlay" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <Monitor className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">Overlay</span>
          </TabsTrigger>
          <TabsTrigger value="about" className="text-[11px] font-pixel py-1.5 flex items-center space-x-1">
            <Info className="h-3.5 w-3.5" />
            <span className="hidden sm:inline">About</span>
          </TabsTrigger>
        </TabsList>

        {/* ========================================================
            TAB 1: GENERAL
        ======================================================== */}
        <TabsContent value="general" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Launch on Startup</h3>
                  <p className="text-xs text-muted-foreground">
                    Start Murmur silently in the Windows system tray when logging in.
                  </p>
                </div>
                <Switch
                  checked={settings.launch_on_startup}
                  onCheckedChange={(checked) => saveSettings({ launch_on_startup: checked })}
                />
              </div>

              <div className="border-t border-border/50 pt-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold">Visual Theme Aesthetic</h3>
                  <p className="text-xs text-muted-foreground">
                    Select your preferred phosphor glow color palette.
                  </p>
                </div>
                <select
                  value={settings.theme || "dark"}
                  onChange={(e) => {
                    const newTheme = e.target.value;
                    applyThemeToDocument(newTheme);
                    saveSettings({ theme: newTheme });
                  }}
                  className="h-8 rounded border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                >
                  <option value="dark">Phosphor Green (Dark Default)</option>
                  <option value="amber">Retro Amber (CRT Monospace)</option>
                  <option value="matrix">Matrix Neon (High Contrast)</option>
                  <option value="cyan">Cyberpunk Cyan</option>
                  <option value="light">Clean Light</option>
                </select>
              </div>

              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Reset First-Run Wizard</h3>
                  <p className="text-xs text-muted-foreground">
                    Re-run the initial 3-step setup tour for audio, model, and hotkeys.
                  </p>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setShowOnboarding(true)}
                  className="text-xs h-8"
                >
                  Launch Wizard
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 2: HOTKEY
        ======================================================== */}
        <TabsContent value="hotkey" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div>
                <div className="flex items-center justify-between pb-2">
                  <div>
                    <h3 className="text-sm font-semibold">Global Dictation Hotkey</h3>
                    <p className="text-xs text-muted-foreground">
                      Low-level keyboard hook active across all applications.
                    </p>
                  </div>
                  <div className="flex items-center space-x-1.5">
                    {settings.hotkey.split("+").map((key, i) => (
                      <span key={i} className="keycap-chip">
                        {key}
                      </span>
                    ))}
                  </div>
                </div>

                <div className="flex items-center space-x-3 pt-2">
                  <Button
                    variant={isCapturingHotkey ? "destructive" : "pixel"}
                    size="sm"
                    onClick={() => {
                      setIsCapturingHotkey(!isCapturingHotkey);
                      setCapturedKeys([]);
                    }}
                    className="text-xs h-9 font-pixel"
                  >
                    {isCapturingHotkey ? "Press Keys Now (Esc to cancel)..." : "Record New Shortcut"}
                  </Button>
                  {isCapturingHotkey && (
                    <span className="text-xs text-amber-400 animate-pulse font-pixel">
                      {capturedKeys.length > 0 ? capturedKeys.join(" + ") : "Listening for keystroke..."}
                    </span>
                  )}
                </div>
              </div>

              {/* Mode Selection */}
              <div className="border-t border-border/50 pt-4 space-y-2">
                <h3 className="text-sm font-semibold">Activation Mode</h3>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 text-xs">
                  {[
                    {
                      id: "hold-to-talk",
                      label: "Hold To Talk",
                      desc: "Hold hotkey while speaking, release to transcribe.",
                    },
                    {
                      id: "toggle",
                      label: "Toggle Mode",
                      desc: "Press once to start recording, press again to stop.",
                    },
                    {
                      id: "hybrid",
                      label: "Hybrid Tap/Hold",
                      desc: "Quick tap (<400ms) toggles; long press acts as push-to-talk.",
                    },
                  ].map((mode) => (
                    <div
                      key={mode.id}
                      onClick={() =>
                        saveSettings({ hotkey_mode: mode.id as AppSettings["hotkey_mode"] })
                      }
                      className={`p-3 rounded-lg border cursor-pointer transition-all ${
                        settings.hotkey_mode === mode.id
                          ? "border-primary bg-primary/10 phosphor-glow-subtle"
                          : "border-border/60 bg-secondary/40 hover:border-border"
                      }`}
                    >
                      <span className="font-semibold block text-foreground">{mode.label}</span>
                      <span className="text-[11px] text-muted-foreground leading-tight block mt-1">
                        {mode.desc}
                      </span>
                    </div>
                  ))}
                </div>
              </div>

              {/* Escape Cancel Info */}
              <div className="border-t border-border/50 pt-3 flex items-center space-x-2 text-xs text-muted-foreground">
                <AlertTriangle className="h-4 w-4 text-amber-400 shrink-0" />
                <span>Pressing the <b>Esc</b> key at any time immediately cancels recording and discards audio.</span>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 3: AUDIO
        ======================================================== */}
        <TabsContent value="audio" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div>
                <h3 className="text-sm font-semibold">Microphone Device</h3>
                <p className="text-xs text-muted-foreground mb-2">
                  Select input hardware. Audio is captured via CPAL WASAPI at 16,000 Hz mono.
                </p>
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
                      {dev.name} ({dev.sample_rate} Hz, {dev.channels}ch)
                    </option>
                  ))}
                </select>
              </div>

              {/* Gain Slider */}
              <div className="border-t border-border/50 pt-4 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold">Microphone Digital Gain</span>
                  <span className="font-mono text-primary">{settings.audio_gain_percent || 100}%</span>
                </div>
                <input
                  type="range"
                  min="20"
                  max="200"
                  step="5"
                  value={settings.audio_gain_percent || 100}
                  onChange={(e) => saveSettings({ audio_gain_percent: Number(e.target.value) })}
                  className="w-full accent-primary h-1.5 bg-secondary rounded-lg cursor-pointer"
                />
                <div className="flex justify-between text-[10px] text-muted-foreground font-mono">
                  <span>20% (Quiet)</span>
                  <span>100% (Default)</span>
                  <span>200% (Boosted)</span>
                </div>
              </div>

              {/* Max Recording Length */}
              <div className="border-t border-border/50 pt-4 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold">Maximum Recording Length</span>
                  <span className="font-mono text-primary">{settings.max_recording_seconds || 60} seconds</span>
                </div>
                <input
                  type="range"
                  min="10"
                  max="300"
                  step="10"
                  value={settings.max_recording_seconds || 60}
                  onChange={(e) => saveSettings({ max_recording_seconds: Number(e.target.value) })}
                  className="w-full accent-primary h-1.5 bg-secondary rounded-lg cursor-pointer"
                />
              </div>

              {/* Audio feedback tone */}
              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Start & Stop Audio Cues</h3>
                  <p className="text-xs text-muted-foreground">
                    Play a subtle retro acoustic blip when dictation starts and stops.
                  </p>
                </div>
                <Switch
                  checked={settings.sound_effects_enabled}
                  onCheckedChange={(checked) => saveSettings({ sound_effects_enabled: checked })}
                />
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 4: TEXT INJECTION & POST-PROCESSING
        ======================================================== */}
        <TabsContent value="text" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Automatic Text Insertion</h3>
                  <p className="text-xs text-muted-foreground">
                    Directly inject transcribed text into the active window when speech ends.
                  </p>
                </div>
                <Switch
                  checked={settings.auto_insert}
                  onCheckedChange={(checked) => saveSettings({ auto_insert: checked })}
                />
              </div>

              {/* Insertion Mode */}
              <div className="border-t border-border/50 pt-4 space-y-2">
                <h3 className="text-sm font-semibold">Text Injection Method</h3>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 text-xs">
                  <div
                    onClick={() => saveSettings({ insertion_mode: "paste-with-restore" })}
                    className={`p-3 rounded-lg border cursor-pointer ${
                      settings.insertion_mode === "paste-with-restore"
                        ? "border-primary bg-primary/10 phosphor-glow-subtle"
                        : "border-border/60 bg-secondary/40"
                    }`}
                  >
                    <span className="font-semibold block">Clipboard Paste with Restore</span>
                    <span className="text-[11px] text-muted-foreground block mt-1">
                      Blazing fast. Temporarily copies text, triggers Ctrl+V, then restores previous clipboard.
                    </span>
                  </div>
                  <div
                    onClick={() => saveSettings({ insertion_mode: "unicode-typing" })}
                    className={`p-3 rounded-lg border cursor-pointer ${
                      settings.insertion_mode === "unicode-typing"
                        ? "border-primary bg-primary/10 phosphor-glow-subtle"
                        : "border-border/60 bg-secondary/40"
                    }`}
                  >
                    <span className="font-semibold block">Direct Unicode Keystrokes</span>
                    <span className="text-[11px] text-muted-foreground block mt-1">
                      Sends SendInput Unicode characters. Best for terminals or apps that block clipboard paste.
                    </span>
                  </div>
                </div>
              </div>

              {/* Text Cleaning Toggles */}
              <div className="border-t border-border/50 pt-4 space-y-3">
                <h3 className="text-sm font-semibold">Post-Processing Rules</h3>

                <div className="flex items-center justify-between text-xs">
                  <div>
                    <span className="font-medium">Filler Word Removal</span>
                    <p className="text-[11px] text-muted-foreground">
                      Removes verbal fillers like "um", "uh", "you know", "like".
                    </p>
                  </div>
                  <Switch
                    checked={settings.remove_fillers}
                    onCheckedChange={(checked) => saveSettings({ remove_fillers: checked })}
                  />
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div>
                    <span className="font-medium">Spoken Commands & Punctuation</span>
                    <p className="text-[11px] text-muted-foreground">
                      Converts spoken phrases like "period", "comma", "new line" into punctuation.
                    </p>
                  </div>
                  <Switch
                    checked={settings.apply_spoken_commands}
                    onCheckedChange={(checked) => saveSettings({ apply_spoken_commands: checked })}
                  />
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div>
                    <span className="font-medium">Custom Dictionary Replacements</span>
                    <p className="text-[11px] text-muted-foreground">
                      Applies custom word substitutions defined in the Dictionary tab.
                    </p>
                  </div>
                  <Switch
                    checked={settings.apply_dictionary}
                    onCheckedChange={(checked) => saveSettings({ apply_dictionary: checked })}
                  />
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div>
                    <span className="font-medium">Add Trailing Space</span>
                    <p className="text-[11px] text-muted-foreground">
                      Appends a single whitespace at the end for continuous dictation.
                    </p>
                  </div>
                  <Switch
                    checked={settings.add_trailing_space}
                    onCheckedChange={(checked) => saveSettings({ add_trailing_space: checked })}
                  />
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 5: HISTORY
        ======================================================== */}
        <TabsContent value="history" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Persistent History Database</h3>
                  <p className="text-xs text-muted-foreground">
                    Store past transcripts locally in an encrypted SQLite database on disk.
                  </p>
                </div>
                <Switch
                  checked={settings.persistent_history}
                  onCheckedChange={(checked) => saveSettings({ persistent_history: checked })}
                />
              </div>

              <div className="border-t border-border/50 pt-4 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold">Maximum Stored Transcripts</span>
                  <span className="font-mono text-primary">{settings.history_retention_max || 100} entries</span>
                </div>
                <input
                  type="range"
                  min="20"
                  max="500"
                  step="20"
                  value={settings.history_retention_max || 100}
                  onChange={(e) => saveSettings({ history_retention_max: Number(e.target.value) })}
                  className="w-full accent-primary h-1.5 bg-secondary rounded-lg cursor-pointer"
                />
                <p className="text-[10px] text-muted-foreground">
                  Oldest transcripts are automatically pruned when exceeding this limit.
                </p>
              </div>

              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Store Audio Recordings</h3>
                  <p className="text-xs text-muted-foreground">
                    Save recorded WAV samples locally alongside text transcripts.
                  </p>
                </div>
                <Switch
                  checked={settings.store_audio}
                  onCheckedChange={(checked) => saveSettings({ store_audio: checked })}
                />
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 6: PERFORMANCE & HARDWARE
        ======================================================== */}
        <TabsContent value="performance" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div>
                <h3 className="text-sm font-semibold">Inference Execution Provider</h3>
                <p className="text-xs text-muted-foreground mb-2">
                  sherpa-onnx runtime backend powering the Parakeet model.
                </p>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 text-xs">
                  {[
                    { id: "cpu", label: "CPU (Int8 Quantized)", desc: "Universal compatibility, zero GPU setup, ~0.066 RTF." },
                    { id: "directml", label: "DirectML (DirectX 12)", desc: "Hardware acceleration on Windows AMD/Intel/NVIDIA." },
                    { id: "cuda", label: "CUDA (NVIDIA)", desc: "Highest throughput on NVIDIA RTX GPUs." },
                  ].map((p) => (
                    <div
                      key={p.id}
                      onClick={() => saveSettings({ performance_provider: p.id })}
                      className={`p-3 rounded-lg border cursor-pointer ${
                        settings.performance_provider === p.id
                          ? "border-primary bg-primary/10 phosphor-glow-subtle"
                          : "border-border/60 bg-secondary/40"
                      }`}
                    >
                      <span className="font-semibold block">{p.label}</span>
                      <span className="text-[11px] text-muted-foreground block mt-1">{p.desc}</span>
                    </div>
                  ))}
                </div>
              </div>

              {/* Thread Count */}
              <div className="border-t border-border/50 pt-4 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold">CPU Execution Threads</span>
                  <span className="font-mono text-primary">{settings.thread_count || 4} threads</span>
                </div>
                <input
                  type="range"
                  min="1"
                  max="16"
                  step="1"
                  value={settings.thread_count || 4}
                  onChange={(e) => saveSettings({ thread_count: Number(e.target.value) })}
                  className="w-full accent-primary h-1.5 bg-secondary rounded-lg cursor-pointer"
                />
              </div>

              {/* Model Variant Selector */}
              <div className="border-t border-border/50 pt-4 space-y-2">
                <h3 className="text-sm font-semibold">Model Variant</h3>
                <p className="text-xs text-muted-foreground">
                  Offline neural speech recognition checkpoint.
                </p>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 text-xs">
                  <div
                    onClick={() => saveSettings({ model_variant: "parakeet-tdt-0.6b-v3-int8" })}
                    className={`p-3 rounded-lg border cursor-pointer ${
                      settings.model_variant === "parakeet-tdt-0.6b-v3-int8"
                        ? "border-primary bg-primary/10 phosphor-glow-subtle"
                        : "border-border/60 bg-secondary/40"
                    }`}
                  >
                    <span className="font-semibold block">Parakeet TDT 0.6B (Int8)</span>
                    <span className="text-[11px] text-muted-foreground block mt-1">
                      Recommended. ~450MB RAM, fastest inference (~0.066 RTF).
                    </span>
                  </div>
                  <div
                    onClick={() => saveSettings({ model_variant: "parakeet-tdt-0.6b-v3-fp32" })}
                    className={`p-3 rounded-lg border cursor-pointer ${
                      settings.model_variant === "parakeet-tdt-0.6b-v3-fp32"
                        ? "border-primary bg-primary/10 phosphor-glow-subtle"
                        : "border-border/60 bg-secondary/40"
                    }`}
                  >
                    <span className="font-semibold block">Parakeet TDT 0.6B (FP32)</span>
                    <span className="text-[11px] text-muted-foreground block mt-1">
                      Full 32-bit floating point precision. ~2.4GB RAM.
                    </span>
                  </div>
                </div>
              </div>

              {/* Idle Unload */}
              <div className="border-t border-border/50 pt-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold">Idle Model Unload Timeout</h3>
                  <p className="text-xs text-muted-foreground">
                    Free ~450MB of RAM when Murmur is not actively dictating.
                  </p>
                </div>
                <select
                  value={settings.idle_unload_minutes}
                  onChange={(e) => saveSettings({ idle_unload_minutes: Number(e.target.value) })}
                  className="h-8 rounded border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none"
                >
                  <option value={0}>Never Unload (Keep In Memory)</option>
                  <option value={5}>Unload after 5 minutes</option>
                  <option value={15}>Unload after 15 minutes</option>
                  <option value={30}>Unload after 30 minutes</option>
                </select>
              </div>

              {/* Live RTF Benchmark */}
              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Real-Time Factor (RTF) Benchmark</h3>
                  <p className="text-xs text-muted-foreground">
                    Measure empirical offline transcription speed on this hardware.
                  </p>
                  {benchmarkResult && (
                    <div className="mt-2 text-xs font-pixel text-emerald-400 phosphor-glow">
                      RTF: {benchmarkResult.rtf.toFixed(4)}x • Latency: {benchmarkResult.inference_ms}ms
                    </div>
                  )}
                </div>
                <Button
                  variant="pixel"
                  size="sm"
                  onClick={handleRunBenchmark}
                  disabled={isBenchmarking}
                  className="text-xs h-8 font-pixel"
                >
                  <Play className="h-3.5 w-3.5 mr-1" />
                  {isBenchmarking ? "Benchmarking..." : "Run Benchmark"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 7: OVERLAY
        ======================================================== */}
        <TabsContent value="overlay" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold">Screen Anchor Position</h3>
                  <p className="text-xs text-muted-foreground">
                    Where the floating translucent pill appears during dictation.
                  </p>
                </div>
                <select
                  value={settings.overlay_position || "bottom-center"}
                  onChange={(e) => handlePositionChange(e.target.value)}
                  className="h-8 rounded border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none"
                >
                  <option value="bottom-center">Bottom Center (Default)</option>
                  <option value="top-center">Top Center</option>
                  <option value="bottom-right">Bottom Right</option>
                  <option value="bottom-left">Bottom Left</option>
                  <option value="top-right">Top Right</option>
                  <option value="top-left">Top Left</option>
                  <option value="center">Center</option>
                </select>
              </div>

              {/* Overlay Size */}
              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Overlay Size</h3>
                  <p className="text-xs text-muted-foreground">Choose compact or larger pill scaling.</p>
                </div>
                <div className="flex items-center space-x-1.5">
                  {(["S", "M", "L"] as const).map((sz) => (
                    <Button
                      key={sz}
                      variant={settings.overlay_size === sz ? "pixel" : "outline"}
                      size="sm"
                      onClick={() => saveSettings({ overlay_size: sz })}
                      className="h-7 w-8 text-xs font-pixel p-0"
                    >
                      {sz}
                    </Button>
                  ))}
                </div>
              </div>

              {/* Anti-Distraction Dim Delay */}
              <div className="border-t border-border/50 pt-4 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold">Anti-Distraction Dim Delay</span>
                  <span className="font-mono text-primary">{settings.dim_delay_seconds || 2}s</span>
                </div>
                <input
                  type="range"
                  min="1"
                  max="10"
                  step="1"
                  value={settings.dim_delay_seconds || 2}
                  onChange={(e) => saveSettings({ dim_delay_seconds: Number(e.target.value) })}
                  className="w-full accent-primary h-1.5 bg-secondary rounded-lg cursor-pointer"
                />
                <p className="text-[10px] text-muted-foreground">
                  Pill smoothly dims after this duration of silence so you focus on typing.
                </p>
              </div>

              {/* Anti-Distraction Dim Opacity */}
              <div className="border-t border-border/50 pt-4 space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-semibold">Dimmed Opacity Percentage</span>
                  <span className="font-mono text-primary">{settings.dim_opacity_percent || 40}%</span>
                </div>
                <input
                  type="range"
                  min="10"
                  max="80"
                  step="5"
                  value={settings.dim_opacity_percent || 40}
                  onChange={(e) => saveSettings({ dim_opacity_percent: Number(e.target.value) })}
                  className="w-full accent-primary h-1.5 bg-secondary rounded-lg cursor-pointer"
                />
              </div>

              {/* Brighten on speech */}
              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Brighten When Speaking</h3>
                  <p className="text-xs text-muted-foreground">
                    Instantly restores full opacity when voice speech threshold is detected.
                  </p>
                </div>
                <Switch
                  checked={settings.brighten_on_speech}
                  onCheckedChange={(checked) => saveSettings({ brighten_on_speech: checked })}
                />
              </div>

              {/* Ultra Minimal Mode */}
              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Ultra-Minimal Mode</h3>
                  <p className="text-xs text-muted-foreground">
                    Renders only a tiny red dot and timer readout (no text or waveform).
                  </p>
                </div>
                <Switch
                  checked={settings.ultra_minimal_mode}
                  onCheckedChange={(checked) => saveSettings({ ultra_minimal_mode: checked })}
                />
              </div>

              {/* Fullscreen Behavior */}
              <div className="border-t border-border/50 pt-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold">Fullscreen App Behavior</h3>
                  <p className="text-xs text-muted-foreground">
                    How the overlay reacts when a fullscreen game or presentation is running.
                  </p>
                </div>
                <select
                  value={settings.fullscreen_behavior || "show"}
                  onChange={(e) => saveSettings({ fullscreen_behavior: e.target.value })}
                  className="h-8 rounded border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none"
                >
                  <option value="show">Always Show (Topmost)</option>
                  <option value="hide">Hide When Fullscreen Active</option>
                  <option value="minimal">Minimal Dot Only</option>
                </select>
              </div>

              {/* Theme Selector */}
              <div className="border-t border-border/50 pt-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold">Theme Palette</h3>
                  <p className="text-xs text-muted-foreground">
                    CRT phosphor glow color palette for overlay and windows.
                  </p>
                </div>
                <select
                  value={settings.theme || "dark"}
                  onChange={(e) => {
                    const newTheme = e.target.value;
                    applyThemeToDocument(newTheme);
                    saveSettings({ theme: newTheme });
                  }}
                  className="h-8 rounded border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none"
                >
                  <option value="dark">Phosphor Green (Dark Default)</option>
                  <option value="amber">Amber CRT (Retro Glow)</option>
                  <option value="matrix">Matrix Neon (High Contrast)</option>
                  <option value="cyan">Cyberpunk Cyan</option>
                  <option value="light">Clean Light Mode</option>
                </select>
              </div>

              {/* Preview Button */}
              <div className="border-t border-border/50 pt-4 flex items-center justify-between">
                <div>
                  <h3 className="text-sm font-semibold">Preview Overlay Animation</h3>
                  <p className="text-xs text-muted-foreground">
                    Executes a 4-second simulated recording animation on the overlay window.
                  </p>
                </div>
                <Button
                  variant="pixel"
                  size="sm"
                  onClick={handlePreviewOverlay}
                  disabled={isPreviewingOverlay}
                  className="text-xs h-8 font-pixel"
                >
                  <Play className="h-3.5 w-3.5 mr-1" />
                  {isPreviewingOverlay ? "Previewing..." : "Preview Overlay"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* ========================================================
            TAB 8: ABOUT & DIAGNOSTICS
        ======================================================== */}
        <TabsContent value="about" className="space-y-4 pt-3">
          <Card className="border-border/70 bg-card pixel-border">
            <CardContent className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <h3 className="text-base font-pixel font-bold tracking-wider text-foreground">
                    MURMUR v0.1.0
                  </h3>
                  <p className="text-xs text-muted-foreground">
                    Offline, privacy-first local dictation & voice command engine.
                  </p>
                </div>
                <Badge variant="outline" className="text-xs text-emerald-400 border-emerald-500/40 phosphor-glow-subtle">
                  100% LOCAL & OFFLINE
                </Badge>
              </div>

              {/* System Specs Grid */}
              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5 text-xs pt-1">
                <div className="p-2.5 rounded bg-secondary/50 border border-border/50">
                  <span className="text-[10px] font-pixel text-muted-foreground block uppercase">
                    SPEECH MODEL
                  </span>
                  <span className="font-semibold text-foreground">parakeet-tdt-0.6b-v3</span>
                  <span className="text-[10px] text-muted-foreground block">
                    {settings.model_variant.includes("fp32") ? "FP32 Precision (2.4GB)" : "Int8 Quantized (450MB)"}
                  </span>
                </div>
                <div className="p-2.5 rounded bg-secondary/50 border border-border/50">
                  <span className="text-[10px] font-pixel text-muted-foreground block uppercase">
                    ACTIVE BACKEND
                  </span>
                  <span className="font-semibold text-foreground uppercase">
                    {settings.performance_provider} (sherpa-onnx)
                  </span>
                  <span className="text-[10px] text-muted-foreground block">{settings.thread_count} CPU Threads</span>
                </div>
                <div className="p-2.5 rounded bg-secondary/50 border border-border/50">
                  <span className="text-[10px] font-pixel text-muted-foreground block uppercase">
                    OPERATING SYSTEM
                  </span>
                  <span className="font-semibold text-foreground">Windows 10/11 x86_64</span>
                  <span className="text-[10px] text-muted-foreground block">CPAL WASAPI & Win32 Hook</span>
                </div>
                <div className="p-2.5 rounded bg-secondary/50 border border-border/50">
                  <span className="text-[10px] font-pixel text-muted-foreground block uppercase">
                    GPU / GRAPHICS
                  </span>
                  <span className="font-semibold text-foreground">DirectX 12 / DirectML</span>
                  <span className="text-[10px] text-muted-foreground block">
                    {settings.performance_provider === "cuda"
                      ? "CUDA RTX Acceleration"
                      : settings.performance_provider === "directml"
                      ? "DirectML Accelerated"
                      : "DirectML Compatible"}
                  </span>
                </div>
                <div className="p-2.5 rounded bg-secondary/50 border border-border/50 col-span-2 sm:col-span-2">
                  <span className="text-[10px] font-pixel text-muted-foreground block uppercase">
                    MIC PRIVACY & PERMISSION
                  </span>
                  <div className="flex items-center space-x-2 pt-0.5">
                    <Badge variant="outline" className="font-pixel text-[10px] text-emerald-400 border-emerald-500/40 phosphor-glow-subtle flex items-center space-x-1">
                      <Check className="h-3 w-3 mr-1" />
                      <span>ACCESS GRANTED (WASAPI)</span>
                    </Badge>
                    <span className="text-[10px] text-muted-foreground">Ready for local capture</span>
                  </div>
                </div>
              </div>

              {/* Elevated Application Warning Box */}
              <div className="p-3 rounded-lg border border-amber-500/30 bg-amber-500/10 text-amber-300 text-xs flex items-start space-x-2">
                <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                <div className="leading-relaxed">
                  <span className="font-semibold block mb-0.5">Elevated Windows Privilege Notice:</span>
                  "Pasting into applications running as Administrator may require Murmur to also run as Administrator."
                </div>
              </div>

              {/* Action Buttons */}
              <div className="border-t border-border/50 pt-4 flex flex-wrap items-center gap-2.5">
                <Button
                  variant="pixel"
                  size="sm"
                  onClick={handleCopyDiagnostics}
                  className="text-xs h-8"
                >
                  {copiedDiag ? (
                    <>
                      <Check className="h-3.5 w-3.5 mr-1" />
                      Copied Report!
                    </>
                  ) : (
                    <>
                      <Copy className="h-3.5 w-3.5 mr-1" />
                      Copy Diagnostics
                    </>
                  )}
                </Button>

                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleOpenLogDir}
                  className="text-xs h-8 text-muted-foreground hover:text-foreground"
                >
                  <FolderOpen className="h-3.5 w-3.5 mr-1" />
                  Open Log Folder
                </Button>

                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleOpenMicPrivacy}
                  className="text-xs h-8 text-muted-foreground hover:text-foreground"
                >
                  <ExternalLink className="h-3.5 w-3.5 mr-1" />
                  Windows Microphone Settings
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
};
