import React, { useEffect, useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Mic, Sparkles, Check } from "lucide-react";
import { motion } from "framer-motion";

export const OverlayView: React.FC = () => {
  const { appState, recordingDuration, micLevel } = useAppStore();
  const [seconds, setSeconds] = useState(0);

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    if (appState === "RECORDING") {
      interval = setInterval(() => {
        setSeconds((s) => s + 1);
      }, 1000);
    } else {
      setSeconds(0);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [appState]);

  const displaySeconds = recordingDuration || seconds;

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-transparent select-none p-2 pointer-events-none">
      <motion.div
        initial={{ opacity: 0, scale: 0.9 }}
        animate={{ opacity: 1, scale: 1 }}
        className="flex items-center space-x-3 rounded-full border border-border/80 bg-background/90 backdrop-blur-md px-4 py-2 shadow-2xl"
      >
        {/* State Icon / Indicator */}
        <div className="flex items-center space-x-2">
          {appState === "RECORDING" ? (
            <div className="relative flex items-center justify-center">
              <span className="absolute h-3 w-3 rounded-full bg-red-500 animate-ping" />
              <Mic className="h-4 w-4 text-red-500 relative z-10" />
            </div>
          ) : appState === "TRANSCRIBING" ? (
            <Sparkles className="h-4 w-4 text-amber-400 animate-spin" />
          ) : appState === "INSERTING" ? (
            <span className="h-2.5 w-2.5 rounded-full bg-emerald-400 animate-pulse" />
          ) : appState === "DONE" ? (
            <Check className="h-4 w-4 text-emerald-400 stroke-[2.5]" />
          ) : (
            <span className="h-2.5 w-2.5 rounded-full bg-primary" />
          )}

          {/* Retro Pixel Readout in Silkscreen font */}
          <span
            className={`font-pixel text-xs tracking-wider uppercase ${
              appState === "RECORDING"
                ? "text-red-400"
                : appState === "TRANSCRIBING"
                ? "text-amber-400"
                : appState === "INSERTING" || appState === "DONE"
                ? "text-emerald-400"
                : "text-foreground"
            }`}
          >
            {appState === "RECORDING"
              ? `REC 00:${displaySeconds.toString().padStart(2, "0")}`
              : appState === "TRANSCRIBING"
              ? "TRANSCRIBING..."
              : appState === "INSERTING"
              ? "INSERTING..."
              : appState === "DONE"
              ? "DONE"
              : "MURMUR IDLE"}
          </span>
        </div>

        {/* Audio VU Meter Simulator */}
        {appState === "RECORDING" && (
          <div className="flex items-center space-x-1 h-3">
            {[1, 2, 3, 4, 5].map((i) => (
              <motion.div
                key={i}
                className="w-1 bg-red-500 rounded-full"
                animate={{
                  height: [
                    "20%",
                    `${Math.min(100, Math.max(20, (micLevel || 30) * (i * 0.25)))}%`,
                    "30%",
                  ],
                }}
                transition={{
                  repeat: Infinity,
                  duration: 0.4 + i * 0.1,
                  ease: "easeInOut",
                }}
              />
            ))}
          </div>
        )}
      </motion.div>
    </div>
  );
};
