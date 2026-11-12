import React, { useEffect, useState, useRef } from "react";
import { useAppStore, StateKind, AppSettings, defaultSettings } from "@/store/useAppStore";
import { applyThemeToDocument } from "@/lib/theme";
import { Sparkles, Check, AlertTriangle } from "lucide-react";
import { motion } from "framer-motion";

export const OverlayView: React.FC = () => {
  const {
    appState,
    errorMessage,
    setAppState,
    micLevel,
    setMicLevel,
  } = useAppStore();
  const [seconds, setSeconds] = useState(0);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [isDimmed, setIsDimmed] = useState(false);
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);
  const lastSpeechTimeRef = useRef<number>(Date.now());
  const micLevelRef = useRef(0);
  const animLevelRef = useRef(0);
  const phaseRef = useRef(0);
  const [displayLevel, setDisplayLevel] = useState(0);
  const [animPhase, setAnimPhase] = useState(0);
  const rafRef = useRef<number>(0);
  const lastSetRef = useRef<number>(0);

  useEffect(() => {
    micLevelRef.current = micLevel;
  }, [micLevel]);

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
              applyThemeToDocument(s.theme || "dark");
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

          const unlistenSettings = listen<AppSettings>("settings-changed", (event) => {
            setSettings(event.payload);
            applyThemeToDocument(event.payload.theme || "dark");
          });

          unlistenAll = () => {
            unlistenState.then((fn) => fn()).catch(() => {});
            unlistenMic.then((fn) => fn()).catch(() => {});
            unlistenSettings.then((fn) => fn()).catch(() => {});
          };
        })
        .catch(() => {});
    }

    return () => {
      if (unlistenAll) unlistenAll();
    };
  }, [setAppState, setMicLevel]);

  // Auto self-healing reset: clear ERROR after 3 seconds
  useEffect(() => {
    if (appState !== "ERROR") return;
    const timer = setTimeout(() => {
      useAppStore.getState().setAppState("IDLE");
      if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
        import("@tauri-apps/api/core").then(({ invoke }) => {
          invoke("hide_overlay").catch(() => {});
        });
      }
    }, 3000);
    return () => clearTimeout(timer);
  }, [appState]);

  useEffect(() => {
    if (appState !== "RECORDING") {
      animLevelRef.current = 0;
      phaseRef.current = 0;
      setDisplayLevel(0);
      setAnimPhase(0);
      lastSetRef.current = 0;
      return;
    }
    lastSetRef.current = Date.now();
    const animate = () => {
      // Perceptual square-root scale: sensitive to whispering & conversational speech, resting below 2.0 noise floor
      const noiseFloor = 2.0;
      const currentMic = micLevelRef.current;
      const targetRaw =
        currentMic > noiseFloor
          ? Math.min(1, Math.sqrt((currentMic - noiseFloor) / 55.0))
          : 0;
      // Exponential moving average: fast attack (0.42), smooth release (0.18)
      const attack = targetRaw > animLevelRef.current ? 0.42 : 0.18;
      animLevelRef.current = animLevelRef.current * (1 - attack) + targetRaw * attack;
      if (animLevelRef.current < 0.005 && targetRaw === 0) {
        animLevelRef.current = 0;
      }

      // Throttle React state updates to ~30fps max
      const now = Date.now();
      const elapsed = now - (lastSetRef.current || now);
      if (elapsed >= 33) {
        // Increment animation phase smoothly for organic per-bar motion
        phaseRef.current += Math.min(elapsed / 1000, 0.1) * 7.5;
        setDisplayLevel(animLevelRef.current);
        setAnimPhase(phaseRef.current);
        lastSetRef.current = now;
      }
      rafRef.current = requestAnimationFrame(animate);
    };
    rafRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(rafRef.current);
  }, [appState]);

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

    // micLevel is 0.0 - 100.0 from WASAPI RMS; speech is detected above lowered noise floor 2.0
    const isSpeaking = micLevel > 2.0;
    if (isSpeaking) {
      lastSpeechTimeRef.current = Date.now();
      if (settings.brighten_on_speech) {
        setIsDimmed(false);
      }
    }
  }, [micLevel, appState, settings.brighten_on_speech]);

  // Size styling map (+15-20% spacious across all presets)
  const isMinimal = settings.ultra_minimal_mode;
  const sizeClasses = {
    S: isMinimal
      ? "h-[36px] px-3 gap-2.5 min-w-[95px]"
      : "h-[45px] px-3.5 text-xs gap-2.5 min-w-[180px] max-w-[200px]",
    M: isMinimal
      ? "h-[42px] px-3.5 gap-2.5 min-w-[110px]"
      : "h-[52px] px-4 text-[13px] gap-3 min-w-[230px] max-w-[252px]",
    L: isMinimal
      ? "h-[46px] px-4 gap-3 min-w-[125px]"
      : "h-[58px] px-5 text-sm gap-3.5 min-w-[270px] max-w-[298px]",
  }[settings.overlay_size || "M"] ||
    (isMinimal
      ? "h-[42px] px-3.5 gap-2.5 min-w-[110px]"
      : "h-[52px] px-4 text-[13px] gap-3 min-w-[230px] max-w-[252px]");

  // Dimming opacity fraction
  const dimOpacity = (settings.dim_opacity_percent || 40) / 100;

  if (appState === "IDLE") {
    if (!settings.ultra_minimal_mode) {
      return null;
    }
    // Ultra-minimal tiny dot when completely idle
    return (
      <div className="h-screen w-screen flex items-center justify-center bg-transparent select-none p-0 overflow-hidden pointer-events-none">
        <div className="h-2.5 w-2.5 rounded-full transition-colors duration-300 bg-primary/40" />
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-transparent select-none p-0 overflow-hidden pointer-events-none">
      <motion.div
        initial={prefersReducedMotion ? { opacity: 1 } : { opacity: 0, scale: 0.92 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.92 }}
        transition={{ duration: prefersReducedMotion ? 0.05 : 0.25 }}
        className={`relative flex items-center justify-between rounded-full border shadow-2xl transition-all duration-500 pixel-border ${sizeClasses}`}
        style={{
          backgroundColor: `hsl(var(--overlay-bg, var(--card)) / ${isDimmed ? dimOpacity : 0.9})`,
          borderColor: `hsl(var(--overlay-border, var(--border)) / ${isDimmed ? Math.min(1, dimOpacity * 1.6) : 0.85})`,
          backdropFilter: "blur(12px)",
        }}
      >
        {/* Status Indicator & Label */}
        <div className="flex items-center space-x-2 shrink-0">
          {appState === "RECORDING" ? (
            settings.ultra_minimal_mode ? (
              <div className="relative flex items-center justify-center shrink-0">
                <svg
                  width="20" height="20" viewBox="0 0 20 20"
                  className="absolute glow-recording"
                  style={{ overflow: 'visible' }}
                >
                  {/* Outer reactive ring */}
                  <circle
                    cx="10" cy="10"
                    r={5.5 + displayLevel * 5.5}
                    stroke="hsl(var(--status-recording))"
                    strokeWidth={1.5 + displayLevel * 2}
                    fill="none"
                    opacity={0.45 + displayLevel * 0.55}
                  />
                  {/* Inner stable dot */}
                  <circle
                    cx="10"
                    cy="10"
                    r="4.5"
                    fill="hsl(var(--status-recording))"
                  />
                </svg>
              </div>
            ) : null
          ) : appState === "TRANSCRIBING" ? (
            <Sparkles
              className={`h-4 w-4 glow-transcribing ${
                prefersReducedMotion ? "" : "animate-spin"
              }`}
              style={{ color: "hsl(var(--status-transcribing))" }}
            />
          ) : appState === "INSERTING" ? (
            <span
              className={`h-3 w-3 rounded-full bg-primary phosphor-glow ${
                prefersReducedMotion ? "" : "animate-pulse"
              }`}
            />
          ) : appState === "DONE" ? (
            <Check className="h-4 w-4 stroke-[3] text-primary phosphor-glow" />
          ) : appState === "ERROR" ? (
            <AlertTriangle
              className="h-4 w-4 glow-error"
              style={{ color: "hsl(var(--status-error))" }}
            />
          ) : (
            <span className="h-2.5 w-2.5 rounded-full bg-primary" />
          )}

          {/* Retro Pixel Font Silkscreen Status Text */}
          {!settings.ultra_minimal_mode && (
            <span
              className={`font-pixel text-[12px] sm:text-[13px] font-bold tracking-widest uppercase select-none transition-opacity duration-500 phosphor-glow-subtle ${
                isDimmed && appState === "RECORDING" ? "opacity-40" : "opacity-100"
              } ${
                appState === "ERROR"
                  ? "text-status-error glow-error"
                  : appState === "RECORDING"
                  ? "text-status-recording glow-recording"
                  : appState === "TRANSCRIBING"
                  ? "text-status-transcribing glow-transcribing"
                  : appState === "INSERTING" || appState === "DONE"
                  ? "text-primary phosphor-glow"
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

        {/* Dynamic Waveform Visualizer (9 vertical bars, organic voice motion, hidden in ultra-minimal) */}
        {appState === "RECORDING" && !settings.ultra_minimal_mode && (
          <div
            className={`flex items-center gap-[3px] h-6 px-1.5 transition-opacity duration-500 ${
              isDimmed ? "opacity-40" : "opacity-100"
            }`}
          >
            {[0, 1, 2, 3, 4, 5, 6, 7, 8].map((i) => {
              const minDotPx = isDimmed ? 3 : 4;
              let heightPx: number;

              if (displayLevel > 0.015) {
                // Symmetric center-weighted envelope (1.0 at center, tapering smoothly to ~0.45 at edges)
                const distFromCenter = Math.abs(i - 4);
                const envelope = 1 - (distFromCenter / 4) * 0.55; // 1.0 center -> 0.45 edge
                // Organic per-bar sine wave modulation using the smooth animation phase
                const wave = prefersReducedMotion ? 1.0 : 0.65 + 0.35 * Math.sin(i * 0.9 + animPhase);
                const maxExtraPx = isDimmed ? 10 : 18;
                heightPx = minDotPx + maxExtraPx * Math.min(1, displayLevel) * envelope * wave;
              } else {
                // Flat uniform dot-dot-dot minimum baseline when silent
                heightPx = minDotPx;
              }

              return (
                <div
                  key={i}
                  className={`${isDimmed ? "w-[3px]" : "w-[4px]"} rounded-full transition-[height] duration-75 ease-out shrink-0 motion-reduce:transition-none`}
                  style={{
                    height: `${Math.round(heightPx)}px`,
                    backgroundColor: "hsl(var(--primary))",
                  }}
                />
              );
            })}
          </div>
        )}

        {/* Timer readout (Always remains 100% crisp and bright) */}
        {appState === "RECORDING" && (
          <div className="font-pixel text-[12px] font-bold tracking-widest px-2 py-0.5 rounded opacity-100 shrink-0 select-none shadow-sm text-status-recording bg-status-recording/15 border border-status-recording/40 glow-recording">
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
