import React, { useEffect, useState, useRef } from "react";
import { useAppStore, StateKind } from "@/store/useAppStore";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { THEMES, applyThemeToDocument } from "@/lib/theme";
import {
  Mic,
  MicOff,
  Copy,
  Check,
  AlertCircle,
  Cpu,
  Volume2,
  Clock,
  RotateCcw,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

export const HomeView: React.FC = () => {
  const {
    appState,
    errorMessage,
    lastTranscript,
    micLevel,
    settings,
    history,
    audioDevices,
    modelStatus,
    fetchSettings,
    fetchHistory,
    fetchAudioDevices,
    fetchModelStatus,
    saveSettings,
    startRecording,
    stopRecording,
    cancelRecording,
  } = useAppStore();

  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [testAreaText, setTestAreaText] = useState("");
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const settingsRef = useRef(settings);

  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  useEffect(() => {
    fetchSettings();
    fetchHistory();
    fetchAudioDevices();
    fetchModelStatus();
  }, [fetchSettings, fetchHistory, fetchAudioDevices, fetchModelStatus]);

  // Polling fallback while model is loading
  useEffect(() => {
    const isLoading = Boolean(modelStatus?.is_loading || modelStatus?.status?.status === "loading");
    if (!isLoading) return;

    const interval = setInterval(() => {
      fetchModelStatus();
    }, 600);

    return () => clearInterval(interval);
  }, [modelStatus?.is_loading, modelStatus?.status?.status, fetchModelStatus]);

  // Append new transcripts to the interactive scratchpad
  useEffect(() => {
    const handleTranscript = (transcript: string) => {
      if (!transcript) return;
      const isFocused =
        typeof document !== "undefined" && document.activeElement === textareaRef.current;

      if (isFocused && settingsRef.current.auto_insert) {
        // Textarea has focus and auto_insert is active -> WindowsTextInjector will paste via Ctrl+V.
        // Wait 180ms to check if textarea already received the paste before appending.
        setTimeout(() => {
          const currentVal = textareaRef.current?.value ?? "";
          const trimmed = transcript.trim();
          if (!currentVal.endsWith(trimmed) && !currentVal.includes(trimmed)) {
            setTestAreaText((prev) => (prev ? `${prev}\n\n${transcript}` : transcript));
          }
        }, 180);
      } else {
        // Not focused in scratchpad or auto_insert disabled -> append directly to scratchpad
        setTestAreaText((prev) => (prev ? `${prev}\n\n${transcript}` : transcript));
      }
    };

    let unlistenAll: (() => void) | null = null;

    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/event")
        .then(({ listen }) => {
          const unlistenNormal = listen<string>("transcript-ready", (event) => {
            handleTranscript(event.payload);
          });

          unlistenAll = () => {
            unlistenNormal.then((fn) => fn()).catch(() => {});
          };
        })
        .catch(() => {});
    }

    return () => {
      if (unlistenAll) {
        unlistenAll();
      }
    };
  }, []);

  // Fallback for non-Tauri / test environments
  useEffect(() => {
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      return;
    }
    if (lastTranscript) {
      setTestAreaText((prev) => (prev ? `${prev}\n\n${lastTranscript}` : lastTranscript));
    }
  }, [lastTranscript]);

  // Live recording timer matching the website HUD simulator
  useEffect(() => {
    if (appState !== "RECORDING") {
      setRecordingSeconds(0);
      return;
    }
    const interval = setInterval(() => {
      setRecordingSeconds((s) => s + 1);
    }, 1000);
    return () => clearInterval(interval);
  }, [appState]);

  const formatTimer = (secs: number) => {
    const mins = Math.floor(secs / 60)
      .toString()
      .padStart(2, "0");
    const rem = (secs % 60).toString().padStart(2, "0");
    return `${mins}:${rem}`;
  };

  const handleToggleRecord = async () => {
    if (appState === "IDLE" || appState === "DONE") {
      await startRecording();
    } else if (appState === "RECORDING") {
      await stopRecording();
    } else if (appState === "ERROR") {
      await cancelRecording();
    }
  };

  const handleCopy = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const handleDeviceChange = async (deviceName: string) => {
    await saveSettings({ audio_device: deviceName === "default" ? null : deviceName });
  };

  const handleThemeSelect = (themeId: string) => {
    applyThemeToDocument(themeId);
    saveSettings({ theme: themeId });
  };

  const getHudPillStyle = (state: StateKind) => {
    if (state === "RECORDING") {
      return "border-red-500/80 shadow-[0_0_25px_rgba(239,68,68,0.3)]";
    }
    if (state === "TRANSCRIBING") {
      return "border-amber-500/80 shadow-[0_0_25px_rgba(245,158,11,0.25)]";
    }
    return "border-border shadow-[0_10px_30px_rgba(0,0,0,0.3)]";
  };

  // Active hotkey tokens
  const activeHotkeyLabel = settings.hotkey || "Ctrl+Shift+Space";
  const hotkeyTokens = activeHotkeyLabel.split("+");

  // Recent 3 transcripts
  const recentThree = history.slice(0, 3);

  // Normalize mic level (0 to 100 scale)
  const normalizedMicLevel = Math.min(100, Math.max(0, micLevel > 1 ? micLevel : micLevel * 100));

  // Generate 18 reactive waveform bars matching the website HUD simulator
  const waveformBars = Array.from({ length: 18 }, (_, i) => {
    const centerDist = Math.abs(i - 8.5) / 8.5;
    const envelope = Math.max(0.2, 1 - centerDist * 0.75);
    if (appState === "RECORDING") {
      if (normalizedMicLevel > 2.0) {
        const waveOffset = Math.sin(i * 0.9 + recordingSeconds * 2.5) * 0.25 + 0.85;
        return Math.min(
          100,
          Math.round(
            14 +
              Math.min(1, Math.sqrt((normalizedMicLevel - 2.0) / 55.0)) *
                86 *
                envelope *
                waveOffset
          )
        );
      }
      return 14;
    }
    if (appState === "TRANSCRIBING") {
      return Math.round((35 + Math.sin(i * 0.8) * 20) * envelope);
    }
    return 14;
  });

  const matteThemes = THEMES.filter((t) => t.category === "matte" && t.id !== "system");
  const retroThemes = THEMES.filter((t) => t.category === "retro");

  return (
    <div className="flex flex-col space-y-5 pb-8">
      {/* Website-Matched Matte Dictation Stage Container */}
      <div className="rounded-2xl border border-border bg-card shadow-[0_20px_50px_rgba(0,0,0,0.35)] overflow-hidden pixel-border">
        {/* Top Integrated Stage Bar (matches .simulator-window-bar from website) */}
        <div className="flex flex-wrap items-center justify-between gap-2 px-5 py-3 bg-secondary/75 border-b border-border select-none">
          <div className="flex items-center space-x-3">
            <div className="flex items-center space-x-1.5" aria-hidden="true">
              <span
                className={`h-2.5 w-2.5 rounded-full transition-colors ${
                  appState === "RECORDING" ? "bg-red-500 animate-pulse" : "bg-red-500/75"
                }`}
              />
              <span
                className={`h-2.5 w-2.5 rounded-full transition-colors ${
                  appState === "TRANSCRIBING" ? "bg-amber-400 animate-pulse" : "bg-amber-500/75"
                }`}
              />
              <span
                className={`h-2.5 w-2.5 rounded-full transition-colors ${
                  modelStatus?.is_loaded || modelStatus?.status?.status === "ready"
                    ? "bg-emerald-500"
                    : "bg-emerald-500/50"
                }`}
              />
            </div>
            <span className="font-pixel text-[11px] text-muted-foreground tracking-wider uppercase">
              WHISPERTYPE LITE HUD &bull; PURE DICTATION
            </span>
          </div>

          <div className="flex items-center space-x-2.5">
            {/* Hotkey Keycap Chips */}
            <div className="hidden sm:flex items-center space-x-1">
              {hotkeyTokens.map((token, idx) => (
                <React.Fragment key={idx}>
                  <span className="keycap-chip">
                    {token}
                  </span>
                  {idx < hotkeyTokens.length - 1 && (
                    <span className="text-muted-foreground text-[10px] font-bold">+</span>
                  )}
                </React.Fragment>
              ))}
            </div>

            {/* Model Badge */}
            <div className="flex items-center">
              {modelStatus?.is_loading || modelStatus?.status?.status === "loading" ? (
                <span className="font-pixel text-[10px] px-2 py-0.5 rounded bg-amber-500/15 text-amber-400 border border-amber-500/40 uppercase">
                  LOADING MODEL...
                </span>
              ) : modelStatus?.is_loaded || modelStatus?.status?.status === "ready" ? (
                <span className="font-pixel text-[10px] px-2 py-0.5 rounded bg-primary/12 text-primary border border-primary/40 uppercase flex items-center gap-1">
                  <Cpu className="h-3 w-3" aria-hidden="true" />
                  <span>
                    NVIDIA PARAKEET TDT{" "}
                    {modelStatus?.active_provider ? modelStatus.active_provider.toUpperCase() : "INT8"}
                  </span>
                </span>
              ) : (
                <span className="font-pixel text-[10px] px-2 py-0.5 rounded bg-secondary text-muted-foreground border border-border uppercase">
                  MODEL STANDBY
                </span>
              )}
            </div>
          </div>
        </div>

        {/* Main Stage Body (matches .simulator-stage from website) */}
        <div className="p-6 sm:p-7 flex flex-col space-y-6 bg-gradient-to-b from-white/[0.015] to-transparent">
          {/* Live Floating HUD Replica Pill */}
          <div className="flex flex-col items-center space-y-3">
            <motion.button
              type="button"
              whileTap={{ scale: 0.98 }}
              onClick={handleToggleRecord}
              aria-label={
                appState === "RECORDING" ? "Stop dictation recording" : "Start dictation recording"
              }
              className={`inline-flex items-center justify-between gap-5 px-5 py-2.5 rounded-full bg-secondary/90 border transition-all duration-300 min-w-[290px] sm:min-w-[340px] cursor-pointer ${getHudPillStyle(
                appState
              )}`}
            >
              <div className="flex items-center gap-2">
                <span
                  className={`h-2 w-2 rounded-full ${
                    appState === "RECORDING"
                      ? "bg-red-500 animate-ping"
                      : appState === "TRANSCRIBING"
                      ? "bg-amber-400 animate-pulse"
                      : "bg-primary"
                  }`}
                />
                <span className="font-pixel text-xs font-bold tracking-wider uppercase text-foreground">
                  {appState === "IDLE" ? "READY" : appState}
                </span>
              </div>

              {/* Dynamic Live Waveform Bars */}
              <div
                className="flex items-center justify-center gap-1 h-7 w-32 sm:w-36"
                aria-hidden="true"
              >
                {waveformBars.map((heightPct, idx) => (
                  <span
                    key={idx}
                    className={`w-1 rounded-full transition-all duration-75 motion-reduce:transition-none ${
                      appState === "RECORDING"
                        ? "bg-red-500"
                        : appState === "TRANSCRIBING"
                        ? "bg-amber-400"
                        : "bg-primary/65"
                    }`}
                    style={{ height: `${heightPct}%` }}
                  />
                ))}
              </div>

              {/* Timer or Mode Indicator */}
              <div
                className={`font-pixel text-xs font-bold tabular-nums ${
                  appState === "RECORDING" ? "text-red-400" : "text-muted-foreground"
                }`}
              >
                {appState === "RECORDING" ? formatTimer(recordingSeconds) : "00:00"}
              </div>
            </motion.button>
          </div>

          {/* Interactive Target Scratchpad Box (matches .target-editor-box on website) */}
          <div className="rounded-xl border border-border bg-background p-4 space-y-3 shadow-inner">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="h-2 w-0.5 bg-primary animate-pulse" aria-hidden="true" />
                <label
                  htmlFor="dictation-scratchpad"
                  className="font-pixel text-[11px] uppercase tracking-wider text-muted-foreground cursor-pointer"
                >
                  Interactive Dictation Scratchpad
                </label>
              </div>
              <div className="flex items-center space-x-1.5">
                {testAreaText && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setTestAreaText("")}
                    className="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground"
                  >
                    <RotateCcw className="h-3 w-3 mr-1" aria-hidden="true" />
                    Clear
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    if (testAreaText) handleCopy("scratchpad", testAreaText);
                  }}
                  disabled={!testAreaText}
                  className="h-6 px-2 text-[11px]"
                >
                  {copiedId === "scratchpad" ? (
                    <>
                      <Check className="h-3 w-3 text-primary mr-1" aria-hidden="true" />
                      <span className="text-primary font-medium">Copied</span>
                    </>
                  ) : (
                    <>
                      <Copy className="h-3 w-3 mr-1" aria-hidden="true" />
                      <span>Copy</span>
                    </>
                  )}
                </Button>
              </div>
            </div>

            <textarea
              ref={textareaRef}
              id="dictation-scratchpad"
              rows={3}
              value={testAreaText}
              onChange={(e) => setTestAreaText(e.target.value)}
              placeholder={`Click the button below or press ${activeHotkeyLabel} anywhere to dictate directly into your active window or test right here...`}
              className="w-full rounded-lg border border-transparent bg-transparent p-1 text-sm font-sans leading-relaxed text-foreground placeholder:text-muted-foreground/65 focus:outline-none resize-none"
            />

            <div className="flex flex-wrap items-center justify-between gap-2 pt-2 border-t border-border/60 text-[11px] text-muted-foreground">
              <span>
                {appState === "RECORDING"
                  ? "Listening to microphone... Speak naturally."
                  : appState === "TRANSCRIBING"
                  ? "Transcribing locally with Parakeet TDT Int8..."
                  : `Active Hotkey: ${activeHotkeyLabel} • Ready for injection`}
              </span>
              <div className="flex items-center space-x-3">
                <span>
                  {testAreaText.trim().split(/\s+/).filter(Boolean).length} words &bull;{" "}
                  {testAreaText.length} chars
                </span>
                <span className="font-pixel text-[10px] text-muted-foreground/80">ESC TO ABORT</span>
              </div>
            </div>
          </div>

          {/* Action Buttons Bar */}
          <div className="flex flex-wrap items-center justify-center gap-3">
            <button
              type="button"
              onClick={handleToggleRecord}
              className={`inline-flex items-center justify-center gap-2.5 px-6 py-3 rounded-xl font-semibold text-xs transition-all cursor-pointer border shadow-sm ${
                appState === "RECORDING"
                  ? "bg-red-500/20 border-red-500 text-red-300 hover:bg-red-500/30"
                  : "bg-primary hover:bg-primary/90 text-primary-foreground border-primary/50 shadow-[0_4px_20px_hsl(var(--primary)/0.25)]"
              }`}
            >
              {appState === "RECORDING" ? (
                <MicOff className="h-4 w-4" aria-hidden="true" />
              ) : (
                <Mic className="h-4 w-4" aria-hidden="true" />
              )}
              <span>
                {appState === "RECORDING"
                  ? "Stop & Transcribe"
                  : appState === "ERROR"
                  ? "Reset Audio Pipeline"
                  : `Hold-to-Talk / Start Dictation (${activeHotkeyLabel})`}
              </span>
            </button>

            {appState === "RECORDING" && (
              <Button
                variant="outline"
                size="sm"
                onClick={cancelRecording}
                className="text-xs h-11 rounded-xl px-4 text-muted-foreground hover:text-foreground"
              >
                Cancel (Esc)
              </Button>
            )}
          </div>

          {/* Quick Theme Switcher Strip (Matte Default vs Optional Retro/Neon) */}
          <div className="pt-3 border-t border-border/60 flex flex-wrap items-center justify-center gap-2 text-xs">
            <span className="text-[11px] text-muted-foreground font-medium mr-1">Matte Finish:</span>
            {matteThemes.map((t) => {
              const isSelected = (settings.theme || "dark") === t.id;
              return (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => handleThemeSelect(t.id)}
                  className={`px-2.5 py-1 rounded-full text-[11px] font-medium border transition-all cursor-pointer ${
                    isSelected
                      ? "bg-primary/15 border-primary text-primary font-semibold"
                      : "bg-secondary/60 border-border/70 text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {t.name.replace("Matte ", "")}
                </button>
              );
            })}

            <span className="text-border mx-1 hidden sm:inline">|</span>
            <span className="text-[11px] text-muted-foreground font-medium mr-1">
              Optional Retro:
            </span>
            {retroThemes.map((t) => {
              const isSelected = settings.theme === t.id;
              return (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => handleThemeSelect(t.id)}
                  className={`px-2.5 py-1 rounded-full text-[11px] font-medium border transition-all cursor-pointer ${
                    isSelected
                      ? "bg-primary/15 border-primary text-primary font-semibold"
                      : "bg-secondary/40 border-border/60 text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {t.name.replace("Retro ", "").replace(" Neon", "")}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* Hardware Section: Microphone Selector & Live VU Meter */}
      <Card className="border-border bg-card pixel-border rounded-xl">
        <CardContent className="p-4 space-y-3">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs">
            <div className="flex items-center space-x-2">
              <Volume2 className="h-4 w-4 text-primary" aria-hidden="true" />
              <label
                htmlFor="home-mic-select"
                className="font-pixel text-[11px] tracking-wider uppercase text-foreground"
              >
                Input Microphone
              </label>
            </div>

            <div className="flex items-center space-x-2 w-full sm:w-auto">
              <select
                id="home-mic-select"
                value={settings.audio_device || "default"}
                onChange={(e) => handleDeviceChange(e.target.value)}
                className="h-8 rounded-lg border border-input bg-secondary/80 px-2.5 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary w-full sm:w-64"
              >
                <option value="default">Default System Device (WASAPI)</option>
                {audioDevices.map((dev) => (
                  <option key={dev.name} value={dev.name}>
                    {dev.name}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Live Audio Level VU Meter */}
          <div className="space-y-1 pt-1">
            <div className="flex items-center justify-between text-[10px] text-muted-foreground font-pixel">
              <span>MIC INPUT LEVEL</span>
              <span>{Math.round(normalizedMicLevel)}%</span>
            </div>
            <div className="h-2 w-full bg-secondary rounded-full overflow-hidden border border-border/60 flex">
              <div
                className={`h-full transition-all duration-75 rounded-full ${
                  normalizedMicLevel > 75
                    ? "bg-red-500"
                    : normalizedMicLevel > 40
                    ? "bg-amber-400"
                    : "bg-primary"
                }`}
                style={{ width: `${normalizedMicLevel}%` }}
              />
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Last 3 Transcriptions Section */}
      <div className="space-y-3 pt-1">
        <div className="flex items-center justify-between">
          <span className="font-pixel text-[11px] uppercase tracking-wider text-muted-foreground">
            Recent Transcriptions
          </span>
          <span className="text-[11px] text-muted-foreground">
            Showing last {recentThree.length}
          </span>
        </div>

        {recentThree.length === 0 ? (
          <div className="p-6 text-center text-xs text-muted-foreground border border-dashed border-border rounded-xl bg-card/40">
            No dictations recorded yet. Click the button above or press your hotkey to create your
            first transcription.
          </div>
        ) : (
          <div className="flex flex-col space-y-2.5">
            {recentThree.map((item) => (
              <Card key={item.id} className="border-border bg-card pixel-border rounded-xl">
                <CardContent className="p-3.5 space-y-2">
                  <div className="flex items-center justify-between text-xs text-muted-foreground">
                    <div className="flex items-center space-x-2">
                      <Clock className="h-3 w-3" aria-hidden="true" />
                      <span>{new Date(item.created_at).toLocaleTimeString()}</span>
                      <span>&bull;</span>
                      <span>{(item.duration_ms / 1000).toFixed(1)}s audio</span>
                      {item.inference_ms > 0 && (
                        <>
                          <span>&bull;</span>
                          <span>{item.inference_ms}ms RTF</span>
                        </>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleCopy(item.id, item.final_text)}
                      aria-label="Copy transcription"
                      className="h-6 px-2 text-xs"
                    >
                      {copiedId === item.id ? (
                        <Check className="h-3.5 w-3.5 text-primary" aria-hidden="true" />
                      ) : (
                        <Copy className="h-3.5 w-3.5" aria-hidden="true" />
                      )}
                    </Button>
                  </div>
                  <p className="text-sm font-sans leading-relaxed text-foreground select-text whitespace-pre-wrap">
                    {item.final_text}
                  </p>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>

      {/* Error Notice */}
      <AnimatePresence>
        {appState === "ERROR" && errorMessage && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            role="alert"
            className="flex items-center space-x-2 border border-destructive/50 bg-destructive/10 text-destructive p-3 rounded-xl text-xs"
          >
            <AlertCircle className="h-4 w-4 shrink-0" aria-hidden="true" />
            <span>{errorMessage}</span>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
