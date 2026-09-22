import React, { useEffect, useState } from "react";
import { useAppStore, StateKind } from "@/store/useAppStore";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import {
  Mic,
  MicOff,
  Sparkles,
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

  useEffect(() => {
    fetchSettings();
    fetchHistory();
    fetchAudioDevices();
    fetchModelStatus();
  }, [fetchSettings, fetchHistory, fetchAudioDevices, fetchModelStatus]);

  // When a new transcript arrives, also append to test textarea if empty
  useEffect(() => {
    if (lastTranscript) {
      setTestAreaText((prev) => (prev ? `${prev} ${lastTranscript}` : lastTranscript));
    }
  }, [lastTranscript]);

  const handleToggleRecord = async () => {
    if (appState === "IDLE") {
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

  const getStateColor = (state: StateKind) => {
    switch (state) {
      case "RECORDING":
        return "text-red-400 border-red-500/50 bg-red-500/10 glow-red";
      case "TRANSCRIBING":
        return "text-amber-400 border-amber-500/50 bg-amber-500/10 glow-amber";
      case "INSERTING":
      case "DONE":
        return "text-emerald-400 border-emerald-500/50 bg-emerald-500/10 phosphor-glow";
      case "ERROR":
        return "text-rose-400 border-rose-500/50 bg-rose-500/10";
      case "IDLE":
      default:
        return "text-primary border-primary/40 bg-primary/10 phosphor-glow-subtle";
    }
  };

  // Split hotkey into keycap tokens (e.g. "Ctrl+Shift+Space")
  const hotkeyTokens = (settings.hotkey || "Ctrl+Shift+Space").split("+");

  // Recent 3 transcripts
  const recentThree = history.slice(0, 3);

  // Normalize mic level (0 to 100 scale)
  const normalizedMicLevel = Math.min(100, Math.max(0, micLevel > 1 ? micLevel : micLevel * 100));

  return (
    <div className="flex flex-col space-y-5 pb-8">
      {/* Retro HUD Status Bar */}
      <div className="flex items-center justify-between border border-border/80 bg-card/70 backdrop-blur px-4 py-2.5 rounded-lg pixel-border select-none">
        {/* Left: State & Engine */}
        <div className="flex items-center space-x-3">
          <div className="flex items-center space-x-2">
            <span
              className={`h-2.5 w-2.5 rounded-full ${
                appState === "RECORDING"
                  ? "bg-red-500 animate-ping"
                  : appState === "TRANSCRIBING"
                  ? "bg-amber-400 animate-pulse"
                  : appState === "INSERTING" || appState === "DONE"
                  ? "bg-emerald-400"
                  : "bg-primary"
              }`}
            />
            <span className="font-pixel text-[11px] tracking-wider text-muted-foreground uppercase">
              STATE:
            </span>
            <Badge variant="outline" className={`font-pixel text-[11px] ${getStateColor(appState)}`}>
              {appState}
            </Badge>
          </div>

          {/* Model Status Badge */}
          <div className="hidden sm:flex items-center space-x-1.5 pl-3 border-l border-border/60 text-xs">
            <Cpu className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-pixel text-[10px] text-muted-foreground uppercase">MODEL:</span>
            {modelStatus?.is_loading || modelStatus?.status?.status === "loading" ? (
              <Badge variant="outline" className="font-pixel text-[10px] text-amber-400 border-amber-500/40">
                LOADING...
              </Badge>
            ) : modelStatus?.is_loaded || modelStatus?.status?.status === "ready" ? (
              <Badge variant="outline" className="font-pixel text-[10px] text-emerald-400 border-emerald-500/40 phosphor-glow-subtle">
                READY ({modelStatus?.active_provider ? modelStatus.active_provider.toUpperCase() : "INT8"})
              </Badge>
            ) : (
              <Badge variant="outline" className="font-pixel text-[10px] text-muted-foreground border-border">
                STANDBY
              </Badge>
            )}
          </div>
        </div>

        {/* Right: Keycap Chips for Hotkey */}
        <div className="flex items-center space-x-1.5">
          <span className="font-pixel text-[10px] text-muted-foreground uppercase mr-1 hidden sm:inline">
            HOTKEY:
          </span>
          <div className="flex items-center space-x-1">
            {hotkeyTokens.map((token, idx) => (
              <React.Fragment key={idx}>
                <span className="keycap-chip">{token}</span>
                {idx < hotkeyTokens.length - 1 && (
                  <span className="text-muted-foreground text-xs font-bold">+</span>
                )}
              </React.Fragment>
            ))}
          </div>
        </div>
      </div>

      {/* Main Dictation Deck */}
      <Card className="border-border/80 bg-gradient-to-b from-card/90 to-background/70 pixel-border relative overflow-hidden">
        <CardContent className="p-6 sm:p-8 flex flex-col items-center justify-center space-y-6 text-center">
          {/* Main Action Button with Pulsing Radar Ring */}
          <div className="relative flex items-center justify-center">
            {appState === "RECORDING" && (
              <motion.div
                className="absolute h-36 w-36 rounded-full border border-red-500/40 bg-red-500/10 glow-box-red"
                animate={{ scale: [1, 1.3, 1], opacity: [0.9, 0.2, 0.9] }}
                transition={{ repeat: Infinity, duration: 1.4, ease: "easeInOut" }}
              />
            )}
            <motion.button
              whileTap={{ scale: 0.94 }}
              onClick={handleToggleRecord}
              className={`h-24 w-24 rounded-full flex items-center justify-center border-2 transition-all duration-300 shadow-xl ${
                appState === "RECORDING"
                  ? "bg-red-500/20 border-red-500 text-red-400 glow-box-red scale-105"
                  : appState === "TRANSCRIBING"
                  ? "bg-amber-500/20 border-amber-400 text-amber-400 animate-spin"
                  : "bg-primary/10 border-primary text-primary hover:bg-primary/20 glow-box"
              }`}
            >
              {appState === "RECORDING" ? (
                <MicOff className="h-10 w-10" />
              ) : appState === "TRANSCRIBING" ? (
                <Sparkles className="h-10 w-10" />
              ) : (
                <Mic className="h-10 w-10" />
              )}
            </motion.button>
          </div>

          <div className="space-y-1.5 max-w-md">
            <h2 className="text-xl font-semibold tracking-tight">
              {appState === "RECORDING"
                ? "Listening to your voice..."
                : appState === "TRANSCRIBING"
                ? "Transcribing offline via Parakeet..."
                : appState === "INSERTING"
                ? "Injecting text into active window..."
                : appState === "DONE"
                ? "Dictation successfully processed!"
                : appState === "ERROR"
                ? "Audio or ASR Error"
                : "Push To Talk or Press Hotkey"}
            </h2>
            <p className="text-xs text-muted-foreground leading-relaxed">
              {appState === "IDLE"
                ? `Hold or toggle ${settings.hotkey} anywhere in any app to dictate.`
                : appState === "RECORDING"
                ? "Speak into your microphone. Click or release hotkey when finished."
                : appState === "DONE"
                ? "Captured audio transcribed with zero network latency."
                : appState === "ERROR"
                ? errorMessage || "An unexpected error occurred."
                : "Processing with sherpa-onnx local model..."}
            </p>
          </div>

          {/* Trigger Button */}
          <div className="flex items-center space-x-3">
            <Button
              variant={appState === "RECORDING" ? "destructive" : "pixel"}
              onClick={handleToggleRecord}
              className="text-xs px-6 h-9 font-pixel uppercase tracking-wider"
            >
              {appState === "RECORDING"
                ? "STOP & TRANSCRIBE"
                : appState === "ERROR"
                ? "RESET PIPELINE"
                : "CLICK TO DICTATE"}
            </Button>
            {appState === "RECORDING" && (
              <Button
                variant="outline"
                size="sm"
                onClick={cancelRecording}
                className="text-xs h-9 text-muted-foreground hover:text-foreground"
              >
                Cancel (Esc)
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Hardware Section: Microphone Selector & Live VU Meter */}
      <Card className="border-border/70 bg-card/60 pixel-border">
        <CardContent className="p-4 space-y-3">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs">
            <div className="flex items-center space-x-2">
              <Volume2 className="h-4 w-4 text-primary" />
              <span className="font-pixel text-[11px] tracking-wider uppercase text-foreground">Input Microphone</span>
            </div>

            <div className="flex items-center space-x-2 w-full sm:w-auto">
              <select
                value={settings.audio_device || "default"}
                onChange={(e) => handleDeviceChange(e.target.value)}
                className="h-8 rounded border border-input bg-secondary/80 px-2.5 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary w-full sm:w-64"
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
              <span>MIC VU METER</span>
              <span>{Math.round(normalizedMicLevel)}%</span>
            </div>
            <div className="h-2 w-full bg-secondary rounded-full overflow-hidden border border-border/50 flex">
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

      {/* "Try It Here" Testing Textarea */}
      <Card className="border-border/70 bg-card/60 pixel-border">
        <CardContent className="p-4 space-y-2.5">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <span className="font-pixel text-[11px] uppercase tracking-wider text-muted-foreground">
                TRY IT HERE (SCRATCHPAD)
              </span>
            </div>
            <div className="flex items-center space-x-2">
              {testAreaText && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setTestAreaText("")}
                  className="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground"
                >
                  <RotateCcw className="h-3 w-3 mr-1" />
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
                    <Check className="h-3 w-3 text-primary mr-1" />
                    <span className="text-primary font-medium">Copied</span>
                  </>
                ) : (
                  <>
                    <Copy className="h-3 w-3 mr-1" />
                    <span>Copy</span>
                  </>
                )}
              </Button>
            </div>
          </div>

          <textarea
            rows={3}
            value={testAreaText}
            onChange={(e) => setTestAreaText(e.target.value)}
            placeholder="Focus here and press your hotkey to test voice typing live..."
            className="w-full rounded-md border border-input bg-secondary/30 p-3 text-sm font-sans leading-relaxed text-foreground placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary resize-none"
          />
          <div className="flex justify-end text-[10px] text-muted-foreground">
            {testAreaText.trim().split(/\s+/).filter(Boolean).length} words • {testAreaText.length} chars
          </div>
        </CardContent>
      </Card>

      {/* Last 3 Transcriptions Section */}
      <div className="space-y-3 pt-1">
        <div className="flex items-center justify-between">
          <span className="font-pixel text-[11px] uppercase tracking-wider text-muted-foreground">
            RECENT TRANSCRIPTIONS
          </span>
          <span className="text-[11px] text-muted-foreground">Showing last {recentThree.length}</span>
        </div>

        {recentThree.length === 0 ? (
          <div className="p-6 text-center text-xs text-muted-foreground border border-dashed border-border rounded-lg">
            No dictations recorded yet. Speak above to create your first transcription.
          </div>
        ) : (
          <div className="flex flex-col space-y-2.5">
            {recentThree.map((item) => (
              <Card key={item.id} className="border-border/60 bg-card pixel-border">
                <CardContent className="p-3.5 space-y-2">
                  <div className="flex items-center justify-between text-xs text-muted-foreground">
                    <div className="flex items-center space-x-2">
                      <Clock className="h-3 w-3" />
                      <span>{new Date(item.created_at).toLocaleTimeString()}</span>
                      <span>•</span>
                      <span>{(item.duration_ms / 1000).toFixed(1)}s audio</span>
                      {item.inference_ms > 0 && (
                        <>
                          <span>•</span>
                          <span>{item.inference_ms}ms RTF</span>
                        </>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleCopy(item.id, item.final_text)}
                      className="h-6 px-2 text-xs"
                    >
                      {copiedId === item.id ? (
                        <Check className="h-3.5 w-3.5 text-primary" />
                      ) : (
                        <Copy className="h-3.5 w-3.5" />
                      )}
                    </Button>
                  </div>
                  <p className="text-sm font-sans leading-relaxed text-foreground select-text">
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
            className="flex items-center space-x-2 border border-destructive/50 bg-destructive/10 text-destructive p-3 rounded-lg text-xs"
          >
            <AlertCircle className="h-4 w-4 shrink-0" />
            <span>{errorMessage}</span>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
