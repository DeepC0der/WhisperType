import React from "react";
import { HashRouter as Router, Routes, Route, Link, useLocation } from "react-router-dom";
import { HomeView } from "@/components/views/HomeView";
import { HistoryView } from "@/components/views/HistoryView";
import { DictionaryView } from "@/components/views/DictionaryView";
import { SettingsView } from "@/components/views/SettingsView";
import { OverlayView } from "@/components/views/OverlayView";
import { Mic, History, BookA, Settings, ShieldCheck } from "lucide-react";
import { useAppStore } from "@/store/useAppStore";

const MainLayout: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const location = useLocation();

  const navItems = [
    { path: "/", label: "Dictate", icon: Mic },
    { path: "/history", label: "History", icon: History },
    { path: "/dictionary", label: "Dictionary", icon: BookA },
    { path: "/settings", label: "Settings", icon: Settings },
  ];

  return (
    <div className="h-screen flex flex-col bg-background text-foreground overflow-hidden">
      {/* App Header */}
      <header className="h-14 border-b border-border bg-card/60 backdrop-blur px-6 flex items-center justify-between shrink-0 select-none">
        <div className="flex items-center space-x-3">
          <div className="h-8 w-8 rounded-lg bg-primary/10 border border-primary/40 flex items-center justify-center">
            <Mic className="h-4 w-4 text-primary" />
          </div>
          <div>
            <h1 className="font-pixel text-sm tracking-wider font-bold uppercase text-foreground">
              MURMUR
            </h1>
            <p className="text-[10px] text-muted-foreground">Offline Local Dictation</p>
          </div>
        </div>

        {/* Navigation Tabs */}
        <nav className="flex items-center space-x-1 bg-secondary/50 p-1 rounded-lg border border-border/40">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = location.pathname === item.path;
            return (
              <Link
                key={item.path}
                to={item.path}
                className={`flex items-center space-x-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  isActive
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground hover:bg-secondary/60"
                }`}
              >
                <Icon className="h-3.5 w-3.5" />
                <span>{item.label}</span>
              </Link>
            );
          })}
        </nav>

        {/* Status Indicator */}
        <div className="flex items-center space-x-2 text-xs">
          <ShieldCheck className="h-4 w-4 text-primary" />
          <span className="font-pixel text-[10px] text-muted-foreground uppercase hidden sm:inline">
            100% OFFLINE
          </span>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto p-6 max-w-4xl mx-auto w-full">
        {children}
      </main>
    </div>
  );
};

export const App: React.FC = () => {
  React.useEffect(() => {
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
          return () => {
            unlistenMic.then((fn) => fn()).catch(() => {});
            unlistenState.then((fn) => fn()).catch(() => {});
          };
        })
        .catch(() => {});
    }
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
