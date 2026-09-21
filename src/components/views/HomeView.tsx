import React from "react";
import { useAppStore, StateKind } from "@/store/useAppStore";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Mic, MicOff, Sparkles, Copy, Check, AlertCircle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

export const HomeView: React.FC = () => {
  const {
    appState,
    errorMessage,
    lastTranscript,
    hotkey,
    recordingDuration,
    setAppState,
    setLastTranscript,
    addHistoryEntry,
  } = useAppStore();

  const [copied, setCopied] = React.useState(false);

  const handleToggleRecord = () => {
    if (appState === "IDLE") {
      setAppState("RECORDING");
    } else if (appState === "RECORDING") {
      setAppState("TRANSCRIBING");
      // Simulate pipeline transition
      setTimeout(() => {
        const text = "Voice input successfully captured and transcribed via parakeet-tdt.";
        setLastTranscript(text);
        setAppState("INSERTING");
        addHistoryEntry({
          text,
          durationSeconds: 2.8,
          wordCount: 9,
        });
        setTimeout(() => {
          setAppState("IDLE");
        }, 600);
      }, 800);
    } else if (appState === "ERROR") {
      setAppState("IDLE");
    }
  };

  const copyTranscript = () => {
    if (!lastTranscript) return;
    navigator.clipboard.writeText(lastTranscript);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const getStateColor = (state: StateKind) => {
    switch (state) {
      case "RECORDING":
        return "text-red-500 border-red-500/40 bg-red-500/10";
      case "TRANSCRIBING":
        return "text-amber-400 border-amber-400/40 bg-amber-400/10";
      case "INSERTING":
      case "DONE":
        return "text-emerald-400 border-emerald-400/40 bg-emerald-400/10";
      case "ERROR":
        return "text-rose-500 border-rose-500/40 bg-rose-500/10";
      case "IDLE":
      default:
        return "text-primary border-primary/40 bg-primary/10";
    }
  };

  return (
    <div className="flex flex-col space-y-6">
      {/* Retro Pixel HUD Bar */}
      <div className="flex items-center justify-between border border-border bg-card/60 backdrop-blur px-4 py-3 rounded-lg">
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
            <span className="font-pixel text-xs tracking-wider text-muted-foreground uppercase">
              STATUS:
            </span>
            <Badge variant="pixel" className={getStateColor(appState)}>
              {appState}
            </Badge>
          </div>
          {appState === "RECORDING" && (
            <span className="font-pixel text-xs text-red-400">
              [REC 00:{recordingDuration.toString().padStart(2, "0")}]
            </span>
          )}
        </div>

        <div className="flex items-center space-x-3 text-xs">
          <span className="font-pixel text-[11px] text-muted-foreground uppercase">
            HOTKEY:
          </span>
          <code className="font-pixel text-[11px] bg-secondary/80 px-2 py-1 rounded border border-border text-foreground">
            {hotkey}
          </code>
        </div>
      </div>

      {/* Main Recording Center Card */}
      <Card className="border-border/60 bg-gradient-to-b from-card to-background/50 relative overflow-hidden">
        <CardContent className="p-8 flex flex-col items-center justify-center space-y-6 text-center">
          {/* Pulsing Mic Circle */}
          <div className="relative flex items-center justify-center">
            {appState === "RECORDING" && (
              <motion.div
                className="absolute h-36 w-36 rounded-full border border-red-500/30 bg-red-500/10"
                animate={{ scale: [1, 1.25, 1], opacity: [0.8, 0.2, 0.8] }}
                transition={{ repeat: Infinity, duration: 1.5 }}
              />
            )}
            <motion.button
              whileTap={{ scale: 0.95 }}
              onClick={handleToggleRecord}
              className={`h-24 w-24 rounded-full flex items-center justify-center border-2 transition-all shadow-lg ${
                appState === "RECORDING"
                  ? "bg-red-500/20 border-red-500 text-red-400 shadow-red-500/20"
                  : appState === "TRANSCRIBING"
                  ? "bg-amber-500/20 border-amber-400 text-amber-400 animate-spin"
                  : "bg-primary/10 border-primary text-primary hover:bg-primary/20 hover:shadow-primary/20"
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

          <div className="space-y-1">
            <h2 className="text-xl font-semibold tracking-tight">
              {appState === "RECORDING"
                ? "Listening..."
                : appState === "TRANSCRIBING"
                ? "Transcribing Audio..."
                : appState === "INSERTING"
                ? "Injecting Text via Clipboard..."
                : appState === "DONE"
                ? "Dictation Complete!"
                : appState === "ERROR"
                ? "Error Occurred"
                : "Ready to Dictate"}
            </h2>
            <p className="text-sm text-muted-foreground">
              {appState === "IDLE"
                ? `Press ${hotkey} anywhere or click above to start`
                : appState === "RECORDING"
                ? "Speak naturally. Release hotkey or click above to transcribe."
                : appState === "DONE"
                ? "Text inserted successfully."
                : appState === "ERROR"
                ? errorMessage || "An unexpected error occurred."
                : "Processing pipeline..."}
            </p>
          </div>

          {/* Quick Trigger Button */}
          <Button
            variant={appState === "RECORDING" ? "destructive" : "pixel"}
            onClick={handleToggleRecord}
            className="text-xs px-6"
          >
            {appState === "RECORDING"
              ? "Stop & Transcribe"
              : appState === "ERROR"
              ? "Reset Status"
              : "Push To Talk"}
          </Button>
        </CardContent>
      </Card>

      {/* Latest Transcript Box */}
      <AnimatePresence>
        {lastTranscript && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
          >
            <Card className="border-border/60 bg-card">
              <CardContent className="p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <span className="font-pixel text-[11px] uppercase tracking-wider text-muted-foreground">
                    LATEST TRANSCRIPTION
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={copyTranscript}
                    className="h-7 px-2 text-xs flex items-center space-x-1"
                  >
                    {copied ? (
                      <>
                        <Check className="h-3.5 w-3.5 text-primary" />
                        <span className="text-primary font-medium">Copied</span>
                      </>
                    ) : (
                      <>
                        <Copy className="h-3.5 w-3.5" />
                        <span>Copy</span>
                      </>
                    )}
                  </Button>
                </div>
                <p className="text-sm leading-relaxed text-foreground bg-secondary/40 p-3 rounded-md font-sans border border-border/50 select-text">
                  {lastTranscript}
                </p>
              </CardContent>
            </Card>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Error Banner */}
      {appState === "ERROR" && errorMessage && (
        <div className="flex items-center space-x-2 border border-destructive/50 bg-destructive/10 text-destructive p-3 rounded-lg text-sm">
          <AlertCircle className="h-4 w-4 shrink-0" />
          <span>{errorMessage}</span>
        </div>
      )}
    </div>
  );
};
