import { create } from "zustand";
import { applyThemeToDocument } from "@/lib/theme";

export type StateKind = "IDLE" | "RECORDING" | "TRANSCRIBING" | "INSERTING" | "DONE" | "ERROR";
export type HotkeyMode = "hold-to-talk" | "toggle" | "hybrid";
export type InsertionMode = "paste-with-restore" | "unicode-typing";

export interface AppSettings {
  hotkey: string;
  hotkey_mode: HotkeyMode;
  audio_device: string | null;
  insertion_mode: InsertionMode;
  auto_insert: boolean;
  launch_on_startup: boolean;
  theme: string;
  history_retention_max: number;
  performance_provider: string;
  model_variant: string;
  sound_effects_enabled: boolean;
  overlay_position: string;
  trim_whitespace: boolean;
  preserve_punctuation: boolean;
  remove_fillers: boolean;
  apply_spoken_commands: boolean;
  apply_dictionary: boolean;
  add_trailing_space: boolean;
  idle_unload_minutes: number;
  audio_gain_percent: number;
  max_recording_seconds: number;
  persistent_history: boolean;
  store_audio: boolean;
  thread_count: number;
  overlay_size: string;
  dim_delay_seconds: number;
  dim_opacity_percent: number;
  ultra_minimal_mode: boolean;
  brighten_on_speech: boolean;
  fullscreen_behavior: string;
  first_run_completed: boolean;
  smart_structure_enabled: boolean;
  smart_sentence_structure: boolean;
  smart_auto_bullets: boolean;
  smart_aggressiveness: "conservative" | "balanced" | "aggressive" | string;
  smart_bullet_style: "bullet" | "dash" | "asterisk" | "numbered" | string;
}

export interface HistoryRecord {
  id: string;
  raw_text: string;
  final_text: string;
  created_at: string;
  duration_ms: number;
  language: string;
  model_variant: string;
  provider: string;
  inference_ms: number;
}

export interface DictionaryRecord {
  id: string;
  find_text: string;
  replace_text: string;
  is_case_sensitive: boolean;
  created_at: string;
}

export interface AudioDeviceInfo {
  name: string;
  is_default: boolean;
  sample_rate: number;
  channels: number;
}

export interface ModelStatus {
  status: { status: "not_loaded" | "loading" | "ready" | "error"; message?: string };
  active_provider: string;
  model_variant: string;
  model_dir: string | null;
  idle_unload_minutes: number;
  seconds_since_last_use: number | null;
  is_loaded?: boolean;
  is_loading?: boolean;
  provider?: string;
  last_error?: string | null;
}

export const defaultSettings: AppSettings = {
  hotkey: "Ctrl+Shift+Space",
  hotkey_mode: "hold-to-talk",
  audio_device: null,
  insertion_mode: "paste-with-restore",
  auto_insert: true,
  launch_on_startup: false,
  theme: "dark",
  history_retention_max: 100,
  performance_provider: "cpu",
  model_variant: "parakeet-tdt-0.6b-v3-int8",
  sound_effects_enabled: false,
  overlay_position: "bottom-center",
  trim_whitespace: true,
  preserve_punctuation: true,
  remove_fillers: true,
  apply_spoken_commands: true,
  apply_dictionary: true,
  add_trailing_space: false,
  idle_unload_minutes: 0,
  audio_gain_percent: 100,
  max_recording_seconds: 60,
  persistent_history: true,
  store_audio: false,
  thread_count: 4,
  overlay_size: "M",
  dim_delay_seconds: 2,
  dim_opacity_percent: 40,
  ultra_minimal_mode: false,
  brighten_on_speech: true,
  fullscreen_behavior: "show",
  first_run_completed: false,
  smart_structure_enabled: false,
  smart_sentence_structure: true,
  smart_auto_bullets: true,
  smart_aggressiveness: "balanced",
  smart_bullet_style: "bullet",
};

const isTauri = (): boolean =>
  typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

async function invokeTauri<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  throw new Error("Tauri API unavailable");
}

interface AppStore {
  appState: StateKind;
  errorMessage: string | null;
  lastTranscript: string;
  recordingDuration: number;
  micLevel: number;
  settings: AppSettings;
  history: HistoryRecord[];
  dictionary: DictionaryRecord[];
  audioDevices: AudioDeviceInfo[];
  modelStatus: ModelStatus | null;
  showOnboarding: boolean;

  setAppState: (state: StateKind, error?: string | null) => void;
  setLastTranscript: (text: string) => void;
  setRecordingDuration: (duration: number) => void;
  setMicLevel: (level: number) => void;
  setShowOnboarding: (show: boolean) => void;
  applySettingsUpdate: (settings: AppSettings) => void;

  addSessionTranscript: (transcript: string) => void;
  fetchSettings: () => Promise<void>;
  saveSettings: (partial: Partial<AppSettings>) => Promise<void>;
  fetchHistory: () => Promise<void>;
  deleteHistory: (id: string) => Promise<void>;
  clearAllHistory: () => Promise<void>;
  updateHistoryText: (id: string, text: string) => Promise<void>;
  reinsertHistory: (text: string) => Promise<void>;
  fetchDictionary: () => Promise<void>;
  saveDictionaryEntry: (
    id: string | null,
    find_text: string,
    replace_text: string,
    is_case_sensitive?: boolean
  ) => Promise<void>;
  deleteDictionary: (id: string) => Promise<void>;
  importDictionary: (
    entries: { find_text: string; replace_text: string; is_case_sensitive?: boolean }[]
  ) => Promise<number>;
  fetchAudioDevices: () => Promise<void>;
  fetchModelStatus: () => Promise<void>;
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<void>;
  cancelRecording: () => Promise<void>;
  toggleRecording: () => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  appState: "IDLE",
  errorMessage: null,
  lastTranscript: "",
  recordingDuration: 0,
  micLevel: 0,
  settings: defaultSettings,
  history: [
    {
      id: "demo-1",
      raw_text: "Murmur is an offline privacy-first local dictation tool.",
      final_text: "Murmur is an offline, privacy-first local dictation tool powered by sherpa-onnx and parakeet-tdt-0.6b.",
      created_at: new Date().toISOString(),
      duration_ms: 3200,
      language: "en",
      model_variant: "parakeet-tdt-0.6b-v3-int8",
      provider: "cpu",
      inference_ms: 210,
    },
  ],
  dictionary: [
    {
      id: "dict-1",
      find_text: "Murmer",
      replace_text: "Murmur",
      is_case_sensitive: false,
      created_at: new Date().toISOString(),
    },
    {
      id: "dict-2",
      find_text: "rust lang",
      replace_text: "Rust",
      is_case_sensitive: false,
      created_at: new Date().toISOString(),
    },
  ],
  audioDevices: [],
  modelStatus: {
    status: { status: "ready" },
    active_provider: "cpu",
    model_variant: "parakeet-tdt-0.6b-v3-int8",
    model_dir: null,
    idle_unload_minutes: 0,
    seconds_since_last_use: null,
    is_loaded: true,
    is_loading: false,
    provider: "cpu",
    last_error: null,
  },
  showOnboarding: false,

  setAppState: (appState, errorMessage = null) => set({ appState, errorMessage }),
  setLastTranscript: (lastTranscript) => set({ lastTranscript }),
  setRecordingDuration: (recordingDuration) => set({ recordingDuration }),
  setMicLevel: (micLevel) => set({ micLevel }),
  setShowOnboarding: (showOnboarding) => set({ showOnboarding }),
  applySettingsUpdate: (settings) => {
    set({ settings });
    applyThemeToDocument(settings.theme || "dark");
  },
  addSessionTranscript: (transcript: string) => {
    const entry: HistoryRecord = {
      id: `session-${Date.now()}`,
      raw_text: transcript,
      final_text: transcript,
      created_at: new Date().toISOString(),
      duration_ms: 2000,
      language: "en",
      model_variant: get().settings.model_variant,
      provider: get().settings.performance_provider,
      inference_ms: 180,
    };
    set((state) => ({ history: [entry, ...state.history] }));
  },

  fetchSettings: async () => {
    try {
      const s = await invokeTauri<AppSettings>("get_settings");
      set({
        settings: s,
        showOnboarding: !s.first_run_completed,
      });
      applyThemeToDocument(s.theme || "dark");
    } catch {
      // In dev browser fallback
    }
  },

  saveSettings: async (partial) => {
    const updated = { ...get().settings, ...partial };
    set({ settings: updated });
    // Apply theme change immediately
    if (partial.theme) {
      applyThemeToDocument(partial.theme);
    }
    try {
      await invokeTauri("update_settings", { newSettings: updated });
    } catch (err) {
      console.warn("Failed to persist settings to backend:", err);
    }
  },

  fetchHistory: async () => {
    try {
      const list = await invokeTauri<HistoryRecord[]>("get_history", {
        limit: 100,
        offset: 0,
      });
      set({ history: list });
    } catch {
      // Fallback
    }
  },

  deleteHistory: async (id: string) => {
    set((state) => ({ history: state.history.filter((h) => h.id !== id) }));
    try {
      await invokeTauri("delete_history", { id });
    } catch (err) {
      console.warn("delete_history error:", err);
    }
  },

  clearAllHistory: async () => {
    set({ history: [] });
    try {
      await invokeTauri("clear_history");
    } catch (err) {
      console.warn("clear_history error:", err);
    }
  },

  updateHistoryText: async (id: string, text: string) => {
    set((state) => ({
      history: state.history.map((h) => (h.id === id ? { ...h, final_text: text } : h)),
    }));
    try {
      await invokeTauri("update_history_entry", { id, text });
    } catch (err) {
      console.warn("update_history_entry error:", err);
    }
  },

  reinsertHistory: async (text: string) => {
    try {
      await invokeTauri("reinsert_text", { text });
    } catch (err) {
      console.warn("reinsert_text error:", err);
      // Fallback to clipboard
      await navigator.clipboard.writeText(text);
    }
  },

  fetchDictionary: async () => {
    try {
      const dict = await invokeTauri<DictionaryRecord[]>("get_dictionary_entries");
      set({ dictionary: dict });
    } catch {
      // Fallback
    }
  },

  saveDictionaryEntry: async (id, find_text, replace_text, is_case_sensitive = false) => {
    try {
      const saved = await invokeTauri<DictionaryRecord>("set_dictionary_entry", {
        id,
        findText: find_text,
        replaceText: replace_text,
        isCaseSensitive: is_case_sensitive,
      });
      set((state) => {
        const existingIdx = state.dictionary.findIndex((d) => d.id === saved.id);
        if (existingIdx >= 0) {
          const updated = [...state.dictionary];
          updated[existingIdx] = saved;
          return { dictionary: updated };
        }
        return { dictionary: [saved, ...state.dictionary] };
      });
    } catch {
      // Fallback mock
      const rec: DictionaryRecord = {
        id: id || `dict-${Date.now()}`,
        find_text,
        replace_text,
        is_case_sensitive,
        created_at: new Date().toISOString(),
      };
      set((state) => ({
        dictionary: [rec, ...state.dictionary.filter((d) => d.id !== rec.id)],
      }));
    }
  },

  deleteDictionary: async (id: string) => {
    set((state) => ({ dictionary: state.dictionary.filter((d) => d.id !== id) }));
    try {
      await invokeTauri("delete_dictionary_entry", { id });
    } catch (err) {
      console.warn("delete_dictionary_entry error:", err);
    }
  },

  importDictionary: async (entries) => {
    try {
      const count = await invokeTauri<number>("import_dictionary", {
        entries: entries.map((e) => ({
          find_text: e.find_text,
          replace_text: e.replace_text,
          is_case_sensitive: e.is_case_sensitive ?? false,
        })),
      });
      await get().fetchDictionary();
      return count;
    } catch {
      // Mock import
      const imported: DictionaryRecord[] = entries.map((e, idx) => ({
        id: `imp-${Date.now()}-${idx}`,
        find_text: e.find_text,
        replace_text: e.replace_text,
        is_case_sensitive: e.is_case_sensitive ?? false,
        created_at: new Date().toISOString(),
      }));
      set((state) => ({ dictionary: [...imported, ...state.dictionary] }));
      return entries.length;
    }
  },

  fetchAudioDevices: async () => {
    try {
      const devs = await invokeTauri<AudioDeviceInfo[]>("get_audio_devices");
      set({ audioDevices: devs });
    } catch {
      // Fallback mock
      set({
        audioDevices: [
          {
            name: "Default System Microphone (WASAPI)",
            is_default: true,
            sample_rate: 48000,
            channels: 2,
          },
        ],
      });
    }
  },

  fetchModelStatus: async () => {
    try {
      const raw = await invokeTauri<any>("get_model_status");
      const is_ready = raw?.status?.status === "ready";
      const is_loading = raw?.status?.status === "loading";
      const last_error =
        raw?.status?.status === "error" ? raw.status.message || "Model load error" : null;
      const status: ModelStatus = {
        ...raw,
        is_loaded: is_ready,
        is_loading: is_loading,
        provider: raw.active_provider || "cpu",
        last_error,
      };
      set({ modelStatus: status });
    } catch {
      // Fallback
    }
  },

  startRecording: async () => {
    try {
      await invokeTauri("start_recording");
    } catch (err) {
      console.warn("start_recording error:", err);
      set({ appState: "RECORDING" });
    }
  },

  stopRecording: async () => {
    try {
      await invokeTauri("stop_recording");
    } catch (err) {
      console.warn("stop_recording error:", err);
      set({ appState: "TRANSCRIBING" });
      setTimeout(() => {
        set({ appState: "DONE" });
        setTimeout(() => set({ appState: "IDLE" }), 600);
      }, 800);
    }
  },

  cancelRecording: async () => {
    try {
      await invokeTauri("cancel_recording");
    } catch (err) {
      console.warn("cancel_recording error:", err);
      set({ appState: "IDLE" });
    }
  },

  toggleRecording: async () => {
    const current = get().appState;
    if (current === "RECORDING") {
      await get().stopRecording();
    } else if (current === "IDLE" || current === "ERROR") {
      await get().startRecording();
    }
  },
}));
