import React, { useState } from "react";
import { HashRouter as Router, Routes, Route, Link, useLocation } from "react-router-dom";
import { HomeView } from "@/components/views/HomeView";
import { HistoryView } from "@/components/views/HistoryView";
import { DictionaryView } from "@/components/views/DictionaryView";
import { SettingsView } from "@/components/views/SettingsView";
import { OverlayView } from "@/components/views/OverlayView";
import {
  Mic,
  History,
  BookA,
  Settings,
  ShieldCheck,
  Minus,
  Square,
  Copy,
  X,
} from "lucide-react";
import { useAppStore } from "@/store/useAppStore";
import { THEMES, applyThemeToDocument } from "@/lib/theme";
import { OnboardingModal } from "@/components/views/OnboardingModal";

const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const MainLayout: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const location = useLocation();
  const { settings, saveSettings } = useAppStore();
  const [isMaximized, setIsMaximized] = useState(false);

  const navItems = [
    { path: "/", label: "Dictate", icon: Mic },
    { path: "/history", label: "History", icon: History },
    { path: "/dictionary", label: "Dictionary", icon: BookA },
    { path: "/settings", label: "Settings", icon: Settings },
  ];

  const handleHeaderMouseDown = async (e: React.MouseEvent<HTMLElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest("button, a, input, select, [data-no-drag]")) {
      return;
    }
    if (e.button === 0 && isTauri()) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("start_drag_main_window");
      } catch {
        // Ignore if outside Tauri
      }
    }
  };

  const handleHeaderDoubleClick = async (e: React.MouseEvent<HTMLElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest("button, a, input, select, [data-no-drag]")) {
      return;
    }
    await handleToggleMaximize();
  };

  const handleMinimize = async () => {
    if (!isTauri()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("minimize_main_window");
    } catch {
      // Ignore
    }
  };

  const handleToggleMaximize = async () => {
    if (!isTauri()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const maxState = await invoke<boolean>("toggle_maximize_main_window");
      setIsMaximized(Boolean(maxState));
    } catch {
      // Ignore
    }
  };

  const handleClose = async () => {
    if (!isTauri()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("close_main_window");
    } catch {
      // Ignore
    }
  };

  const handleQuickThemeSwitch = (themeId: string) => {
    applyThemeToDocument(themeId);
    saveSettings({ theme: themeId });
  };

  const quickThemes = THEMES.filter((t) => t.id !== "system");

  return (
    <div className="h-screen flex flex-col bg-background text-foreground overflow-hidden scanlines-overlay border border-border/80 rounded-none">
      {/* Integrated Matte Application Titlebar & Navigation Frame */}
      <header
        data-tauri-drag-region
        onMouseDown={handleHeaderMouseDown}
        onDoubleClick={handleHeaderDoubleClick}
        className="h-14 border-b border-border/80 bg-card/95 backdrop-blur-md px-4 flex items-center justify-between shrink-0 select-none z-20 cursor-default"
      >
        {/* Left: Brand Identity */}
        <div className="flex items-center space-x-3">
          <img
            src="/logo.png"
            alt="WhisperType logo"
            className="h-8 w-8 rounded-lg shadow-sm object-contain"
          />
          <div className="leading-tight">
            <div className="flex items-center gap-2">
              <h1 className="font-sans text-sm tracking-tight font-bold text-foreground">
                WhisperType Lite
              </h1>
              <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-primary/12 text-primary border border-primary/30 hidden md:inline-block">
                Lite • CPU Int8
              </span>
            </div>
            <p className="text-[10px] text-muted-foreground">Ultra-Lightweight Offline Voice Dictation</p>
          </div>
        </div>

        {/* Center: Matte Segmented Navigation Tabs */}
        <nav
          aria-label="Primary"
          data-no-drag
          className="flex items-center space-x-1 bg-secondary/70 p-1 rounded-xl border border-border/70"
        >
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = location.pathname === item.path;
            return (
              <Link
                key={item.path}
                to={item.path}
                className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                  isActive
                    ? "bg-card text-foreground shadow-sm border border-border/80"
                    : "text-muted-foreground hover:text-foreground hover:bg-secondary"
                }`}
              >
                <Icon className="h-3.5 w-3.5" aria-hidden="true" />
                <span>{item.label}</span>
              </Link>
            );
          })}
        </nav>

        {/* Right: Quick Theme Pills, Mode Badge & Integrated Window Controls */}
        <div className="flex items-center space-x-2 text-xs" data-no-drag>
          {/* Website-style Quick Theme Dots */}
          <div
            className="hidden lg:flex items-center gap-1 bg-secondary/70 border border-border/70 rounded-full px-2 py-1"
            role="group"
            aria-label="Quick theme switcher"
          >
            {quickThemes.map((t) => {
              const isCurrent = (settings.theme || "dark") === t.id;
              return (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => handleQuickThemeSwitch(t.id)}
                  aria-label={`Switch theme to ${t.name}`}
                  aria-pressed={isCurrent}
                  title={`${t.name}${t.category === "retro" ? " (Optional Retro)" : " (Matte)"}`}
                  className={`h-3.5 w-3.5 rounded-full transition-transform cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary ${
                    isCurrent ? "scale-125 ring-2 ring-foreground/80" : "opacity-75 hover:opacity-100 hover:scale-110"
                  }`}
                  style={{
                    backgroundColor: t.preview.primary,
                  }}
                />
              );
            })}
          </div>

          <div
            className="hidden xl:flex items-center space-x-1 px-2 py-1 rounded-full bg-secondary/50 border border-border/60"
            title="Zero network audio transmission"
          >
            <ShieldCheck className="h-3.5 w-3.5 text-primary" aria-hidden="true" />
            <span className="text-[10px] font-medium text-muted-foreground">Air-Gapped</span>
          </div>

          {/* Integrated Window Controls (Minimize, Maximize/Restore, Close) */}
          <div className="flex items-center pl-2 ml-1 border-l border-border/70 space-x-1">
            <button
              type="button"
              onClick={handleMinimize}
              aria-label="Minimize window"
              title="Minimize"
              className="h-7 w-7 rounded-lg flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-secondary/90 transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
            >
              <Minus className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={handleToggleMaximize}
              aria-label={isMaximized ? "Restore window" : "Maximize window"}
              title={isMaximized ? "Restore" : "Maximize"}
              className="h-7 w-7 rounded-lg flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-secondary/90 transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
            >
              {isMaximized ? (
                <Copy className="h-3 w-3" aria-hidden="true" />
              ) : (
                <Square className="h-3 w-3" aria-hidden="true" />
              )}
            </button>
            <button
              type="button"
              onClick={handleClose}
              aria-label="Close to system tray"
              title="Close to Tray"
              className="h-7 w-7 rounded-lg flex items-center justify-center text-muted-foreground hover:text-white hover:bg-red-500/90 transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-red-500"
            >
              <X className="h-4 w-4" aria-hidden="true" />
            </button>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto p-6 max-w-4xl mx-auto w-full z-10">
        {children}
      </main>

      {/* First-Run Onboarding Modal Wizard */}
      <OnboardingModal />
    </div>
  );
};

export const App: React.FC = () => {
  React.useEffect(() => {
    // Initial fetch of settings and history
    useAppStore.getState().fetchSettings();
    useAppStore.getState().fetchHistory();

    let unlistenAll: (() => void) | null = null;

    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/event")
        .then(({ listen }) => {
          const unlistenMic = listen<number>("mic-level", (event) => {
            useAppStore.getState().setMicLevel(event.payload);
          });
          const unlistenState = listen<{
            current_state: import("@/store/useAppStore").StateKind;
            previous_state?: import("@/store/useAppStore").StateKind;
            error_message?: string;
          }>("state-changed", (event) => {
            useAppStore
              .getState()
              .setAppState(event.payload.current_state, event.payload.error_message);
          });
          const unlistenTranscript = listen<string>("transcript-ready", (event) => {
            useAppStore.getState().setLastTranscript(event.payload);
            if (useAppStore.getState().settings.persistent_history) {
              useAppStore.getState().fetchHistory();
            } else {
              useAppStore.getState().addSessionTranscript(event.payload);
            }
          });
          const unlistenSettings = listen<import("@/store/useAppStore").AppSettings>(
            "settings-changed",
            (event) => {
              useAppStore.getState().applySettingsUpdate(event.payload);
            }
          );
          const unlistenMode = listen<"normal" | "professional">("dictation-mode-changed", (event) => {
            useAppStore.getState().setActiveDictationMode(event.payload);
          });

          unlistenAll = () => {
            unlistenMic.then((fn) => fn()).catch(() => {});
            unlistenState.then((fn) => fn()).catch(() => {});
            unlistenTranscript.then((fn) => fn()).catch(() => {});
            unlistenSettings.then((fn) => fn()).catch(() => {});
            unlistenMode.then((fn) => fn()).catch(() => {});
          };
        })
        .catch(() => {});
    }

    return () => {
      if (unlistenAll) unlistenAll();
    };
  }, []);

  return (
    <Router>
      <Routes>
        {/* Overlay window route */}
        <Route path="/overlay" element={<OverlayView />} />

        {/* Main window routes */}
        <Route
          path="/"
          element={
            <MainLayout>
              <HomeView />
            </MainLayout>
          }
        />
        <Route
          path="/history"
          element={
            <MainLayout>
              <HistoryView />
            </MainLayout>
          }
        />
        <Route
          path="/dictionary"
          element={
            <MainLayout>
              <DictionaryView />
            </MainLayout>
          }
        />
        <Route
          path="/settings"
          element={
            <MainLayout>
              <SettingsView />
            </MainLayout>
          }
        />
      </Routes>
    </Router>
  );
};

export default App;
