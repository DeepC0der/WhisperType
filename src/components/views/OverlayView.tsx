import React, { useEffect, useState, useRef } from "react";
import { useAppStore, StateKind, AppSettings, defaultSettings } from "@/store/useAppStore";
import { Sparkles, Check, AlertTriangle } from "lucide-react";
import { motion } from "framer-motion";

export const OverlayView: React.FC = () => {
  const { appState, errorMessage, setAppState, micLevel, setMicLevel } = useAppStore();
  const [seconds, setSeconds] = useState(0);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [isDimmed, setIsDimmed] = useState(false);
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);
  const lastSpeechTimeRef = useRef<number>(Date.now());

  // Fetch initial settings & snapshot, and listen to Tauri events on mount
  useEffect(() => {
    // Check reduced motion preference
    if (typeof window !== "undefined" && window.matchMedia) {
      const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
      setPrefersReducedMotion(mq.matches);
      const listener = (e: MediaQueryListEvent) => setPrefersReducedMotion(e.matches);
      mq.addEventListener("change", listener);
    }

    let unlistenAll: (() => void) | null = null;

    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/core")
        .then(({ invoke }) => {
          invoke<AppSettings>("get_settings")
            .then((s) => {
              setSettings(s);
              if (typeof document !== "undefined" && s.theme && s.theme !== "dark") {
                document.documentElement.className = `theme-${s.theme}`;
              }
            })
            .catch(() => {});

          invoke<{ current_state: StateKind; error_message?: string }>("get_state_snapshot")
            .then((snap) => {
              if (snap) {
                setAppState(snap.current_state, snap.error_message);
              }
            })
            .catch(() => {});
        })
        .catch(() => {});

      import("@tauri-apps/api/event")
        .then(({ listen }) => {
          const unlistenState = listen<{
            current_state: StateKind;
            previous_state?: StateKind;
            error_message?: string;
          }>("state-changed", (event) => {
            setAppState(event.payload.current_state, event.payload.error_message);
          });

          const unlistenMic = listen<number>("mic-level", (event) => {
            setMicLevel(event.payload);
          });

          unlistenAll = () => {
            unlistenState.then((fn) => fn()).catch(() => {});
            unlistenMic.then((fn) => fn()).catch(() => {});
          };
        })
        .catch(() => {});
    }

    return () => {
      if (unlistenAll) unlistenAll();
    };
  }, [setAppState, setMicLevel]);

  // Dedicated recording timer (only ticks during RECORDING, independent of mic events)
  useEffect(() => {
    if (appState !== "RECORDING") {
      setSeconds(0);
      return;
    }

    setSeconds(0);
    const interval = setInterval(() => {
      setSeconds((s) => s + 1);
    }, 1000);

    return () => clearInterval(interval);
  }, [appState]);

  // Anti-distraction dimming timer
  useEffect(() => {
    if (appState !== "RECORDING") {
      setIsDimmed(false);
      return;
    }

    lastSpeechTimeRef.current = Date.now();
    setIsDimmed(false);

    const dimDelayMs = (settings.dim_delay_seconds || 2) * 1000;

    const checkInterval = setInterval(() => {
      const elapsed = Date.now() - lastSpeechTimeRef.current;
      if (elapsed >= dimDelayMs) {
        setIsDimmed(true);
      }
    }, 100);

    return () => clearInterval(checkInterval);
  }, [appState, settings.dim_delay_seconds]);

  // Brighten when speaking (reacts to mic level above ambient speech threshold)
  useEffect(() => {
    if (appState !== "RECORDING") return;

    // micLevel is 0.0 - 100.0 from WASAPI RMS; speech is typically > 4.0
    const isSpeaking = micLevel > 4.0;
    if (isSpeaking) {
      lastSpeechTimeRef.current = Date.now();
      if (settings.brighten_on_speech) {
        setIsDimmed(false);
      }
    }
  }, [micLevel, appState, settings.brighten_on_speech]);

  // Size styling map
  const isMinimal = settings.ultra_minimal_mode;
  const sizeClasses = {
    S: isMinimal ? "h-9 px-2.5 gap-2 min-w-[90px]" : "h-10 px-3 text-[11px] gap-2.5 min-w-[170px]",
    M: isMinimal ? "h-10 px-3 gap-2.5 min-w-[100px]" : "h-12 px-4 text-xs gap-3 min-w-[200px]",
    L: isMinimal ? "h-12 px-3.5 gap-3 min-w-[120px]" : "h-14 px-5 text-sm gap-3.5 min-w-[240px]",
  }[settings.overlay_size || "M"] || (isMinimal ? "h-10 px-3 gap-2.5 min-w-[100px]" : "h-12 px-4 text-xs gap-3 min-w-[200px]");

  // Dimming opacity fraction
  const dimOpacity = (settings.dim_opacity_percent || 40) / 100;

  if (appState === "IDLE") {
    // Ultra-minimal tiny dot when completely idle
    return (
      <div className="h-screen w-screen flex items-center justify-center bg-transparent select-none p-2 pointer-events-none">
        <div className="h-2 w-2 rounded-full bg-primary/40" />
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-transparent select-none p-2 pointer-events-none">
      <motion.div
        initial={prefersReducedMotion ? { opacity: 1 } : { opacity: 0, scale: 0.92, y: 4 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.92 }}
        transition={{ duration: prefersReducedMotion ? 0.05 : 0.25 }}
        className={`relative flex items-center justify-between rounded-full border shadow-2xl transition-all duration-500 pixel-border ${sizeClasses}`}
        style={{
          backgroundColor: `hsl(var(--background) / ${isDimmed ? dimOpacity : 0.88})`,
          borderColor: `hsl(var(--border) / ${isDimmed ? Math.min(1, dimOpacity * 1.6) : 0.85})`,
          backdropFilter: "blur(12px)",
        }}
      >
        {/* Status Indicator & Label */}
        <div className="flex items-center space-x-2 shrink-0">
          {appState === "RECORDING" ? (
            <div className="relative flex items-center justify-center shrink-0">
              {!prefersReducedMotion && (
                <span className="absolute h-3.5 w-3.5 rounded-full bg-red-500 animate-ping opacity-75" />
              )}
              {/* Red dot stays bright even when container is anti-distraction dimmed */}
              <span className="h-2.5 w-2.5 rounded-full bg-red-500 glow-red opacity-100" />
            </div>
          ) : appState === "TRANSCRIBING" ? (
            <Sparkles
              className={`h-3.5 w-3.5 text-amber-400 glow-amber ${
                prefersReducedMotion ? "" : "animate-spin"
              }`}
            />
          ) : appState === "INSERTING" ? (
            <span
              className={`h-2.5 w-2.5 rounded-full bg-emerald-400 phosphor-glow ${
                prefersReducedMotion ? "" : "animate-pulse"
              }`}
            />
          ) : appState === "DONE" ? (
            <Check className="h-3.5 w-3.5 text-emerald-400 stroke-[3] phosphor-glow" />
          ) : appState === "ERROR" ? (
            <AlertTriangle className="h-3.5 w-3.5 text-rose-500 glow-red" />
          ) : (
            <span className="h-2 w-2 rounded-full bg-primary" />
          )}

          {/* Retro Pixel Font Silkscreen Status Text */}
          {!settings.ultra_minimal_mode && (
            <span
              className={`font-pixel tracking-wider uppercase select-none transition-opacity duration-500 ${
                isDimmed && appState === "RECORDING" ? "opacity-40" : "opacity-100"
              } ${
                appState === "RECORDING"
                  ? "text-red-400 glow-red"
                  : appState === "TRANSCRIBING"
                  ? "text-amber-400 glow-amber"
                  : appState === "INSERTING" || appState === "DONE"
                  ? "text-emerald-400 phosphor-glow"
                  : appState === "ERROR"
                  ? "text-rose-400"
                  : "text-foreground"
              }`}
            >
              {appState === "RECORDING"
                ? "REC"
                : appState === "TRANSCRIBING"
                ? "TRANSCRIBING..."
                : appState === "INSERTING"
                ? "INSERTING..."
                : appState === "DONE"
                ? "DONE"
                : appState === "ERROR"
                ? (errorMessage || "ERROR").slice(0, 16)
                : "READY"}
            </span>
          )}
        </div>

        {/* Dynamic Waveform (Calmer/thinner when dimmed, hidden in ultra-minimal) */}
        {appState === "RECORDING" && !settings.ultra_minimal_mode && (
          <div
            className={`flex items-center space-x-1 h-3.5 px-1 transition-opacity duration-500 ${
              isDimmed ? "opacity-40" : "opacity-100"
            }`}
          >
            {[1, 2, 3, 4, 5].map((i) => {
              const baseLevel = micLevel ? (micLevel > 1 ? micLevel / 100 : micLevel) : 0.15;
              const rawHeight = Math.min(100, Math.max(15, baseLevel * 100 * (0.6 + i * 0.18)));
              // Calmer and thinner waveform when anti-distraction dimmed
              const finalHeight = isDimmed ? rawHeight * 0.45 : rawHeight;

              return (
                <div
                  key={i}
                  className={`bg-red-500 rounded-full transition-all duration-150 ${
                    isDimmed ? "w-0.5 opacity-60" : "w-1 opacity-90 shadow-sm"
                  }`}
                  style={{
                    height: `${Math.max(15, finalHeight)}%`,
                  }}
                />
              );
            })}
          </div>
        )}

        {/* Timer readout (Always remains 100% crisp and bright) */}
        {appState === "RECORDING" && (
          <div className="font-pixel text-[11px] font-bold tracking-widest text-red-400 bg-red-950/60 border border-red-500/40 px-1.5 py-0.5 rounded opacity-100 glow-red shrink-0 select-none">
            {Math.floor(seconds / 60)
              .toString()
              .padStart(2, "0")}
            :{(seconds % 60).toString().padStart(2, "0")}
          </div>
        )}
      </motion.div>
    </div>
  );
};
