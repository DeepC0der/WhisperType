import React, { useEffect } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Cpu, ShieldCheck, Keyboard, Volume2, Sparkles, Power } from "lucide-react";

export const SettingsView: React.FC = () => {
  const {
    hotkey,
    setHotkey,
    autoInsert,
    setAutoInsert,
    launchOnStartup,
    setLaunchOnStartup,
  } = useAppStore();

  useEffect(() => {
    // Attempt to read autostart status from Tauri backend if running in desktop context
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/core")
        .then(({ invoke }) => {
          invoke<boolean>("is_autostart_enabled")
            .then((enabled) => setLaunchOnStartup(enabled))
            .catch(() => {});
        })
        .catch(() => {});
    }
  }, [setLaunchOnStartup]);

  const handleToggleAutostart = async (checked: boolean) => {
    setLaunchOnStartup(checked);
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("set_autostart_enabled", { enabled: checked });
      } catch (err) {
        console.error("Failed to update autostart setting:", err);
      }
    }
  };

  return (
    <div className="flex flex-col space-y-4">
      <div className="pb-2 border-b border-border">
        <h2 className="text-lg font-semibold tracking-tight">Settings & Engine</h2>
        <p className="text-xs text-muted-foreground">
          Configure audio capture, keyboard shortcuts, text injection, and offline speech model.
        </p>
      </div>

      <div className="flex flex-col space-y-4 overflow-y-auto max-h-[420px] pr-1">
        {/* Hotkey Configuration */}
        <Card className="border-border/60 bg-card">
          <CardContent className="p-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <Keyboard className="h-4 w-4 text-primary" />
                <div>
                  <h3 className="text-sm font-medium">Global Push-To-Talk Hotkey</h3>
                  <p className="text-xs text-muted-foreground">
                    Low-level OS keyboard hook triggered system-wide.
                  </p>
                </div>
              </div>
              <Badge variant="pixel">{hotkey}</Badge>
            </div>
            <div className="flex items-center space-x-2 pt-1">
              <select
                value={hotkey}
                onChange={(e) => setHotkey(e.target.value)}
                className="h-8 rounded-md border border-input bg-secondary px-3 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
              >
                <option value="Ctrl+Shift+Space">Ctrl + Shift + Space (Default)</option>
                <option value="Alt+Space">Alt + Space</option>
                <option value="F8">F8 (Dedicated Key)</option>
                <option value="F9">F9 (Dedicated Key)</option>
              </select>
            </div>
          </CardContent>
        </Card>

        {/* Text Injection Mode */}
        <Card className="border-border/60 bg-card">
          <CardContent className="p-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <Sparkles className="h-4 w-4 text-primary" />
                <div>
                  <h3 className="text-sm font-medium">Automatic Text Insertion</h3>
                  <p className="text-xs text-muted-foreground">
                    Automatically pastes transcribed text into foreground application upon release.
                  </p>
                </div>
              </div>
              <Switch checked={autoInsert} onCheckedChange={setAutoInsert} />
            </div>
          </CardContent>
        </Card>

        {/* Launch On Startup */}
        <Card className="border-border/60 bg-card">
          <CardContent className="p-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <Power className="h-4 w-4 text-primary" />
                <div>
                  <h3 className="text-sm font-medium">Launch On Startup</h3>
                  <p className="text-xs text-muted-foreground">
                    Start Murmur minimized in the background on system login.
                  </p>
                </div>
              </div>
              <Switch
                checked={launchOnStartup}
                onCheckedChange={handleToggleAutostart}
              />
            </div>
          </CardContent>
        </Card>

        {/* Engine & Privacy Status */}
        <Card className="border-border/60 bg-card">
          <CardContent className="p-4 space-y-3">
            <div className="flex items-center justify-between pb-2 border-b border-border/40">
              <div className="flex items-center space-x-2">
                <Cpu className="h-4 w-4 text-primary" />
                <h3 className="text-sm font-medium">ASR Speech Engine</h3>
              </div>
              <Badge variant="outline" className="text-[10px] text-emerald-400 border-emerald-500/30">
                100% OFFLINE
              </Badge>
            </div>

            <div className="grid grid-cols-2 gap-3 text-xs">
              <div className="p-2.5 rounded bg-secondary/40 border border-border/40">
                <span className="text-muted-foreground block text-[10px] uppercase font-pixel">
                  MODEL
                </span>
                <span className="font-semibold text-foreground">parakeet-tdt-0.6b-v3</span>
                <span className="text-muted-foreground block text-[10px]">Int8 Quantized</span>
              </div>
              <div className="p-2.5 rounded bg-secondary/40 border border-border/40">
                <span className="text-muted-foreground block text-[10px] uppercase font-pixel">
                  RUNTIME
                </span>
                <span className="font-semibold text-foreground">sherpa-onnx CPU</span>
                <span className="text-muted-foreground block text-[10px]">Empirical RTF: ~0.066</span>
              </div>
            </div>

            <div className="flex items-center space-x-2 text-xs text-muted-foreground pt-1">
              <ShieldCheck className="h-4 w-4 text-emerald-400" />
              <span>Zero network requests. Audio and text never leave your machine.</span>
            </div>
          </CardContent>
        </Card>

        {/* Audio Input Device */}
        <Card className="border-border/60 bg-card">
          <CardContent className="p-4 space-y-2">
            <div className="flex items-center space-x-2">
              <Volume2 className="h-4 w-4 text-primary" />
              <h3 className="text-sm font-medium">Microphone Device</h3>
            </div>
            <p className="text-xs text-muted-foreground">
              Capturing via CPAL WASAPI at 16,000 Hz mono PCM.
            </p>
            <div className="text-xs bg-secondary/50 p-2.5 rounded border border-border/40 font-mono text-muted-foreground">
              Default System Input Device (WASAPI)
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
};
