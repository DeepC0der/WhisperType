import { create } from "zustand";

export type StateKind = "IDLE" | "RECORDING" | "TRANSCRIBING" | "INSERTING" | "DONE" | "ERROR";

export interface HistoryEntry {
  id: string;
  timestamp: string;
  text: string;
  durationSeconds: number;
  wordCount: number;
}

export interface DictionaryEntry {
  id: string;
  original: string;
  replacement: string;
}

interface AppStore {
  appState: StateKind;
  errorMessage: string | null;
  lastTranscript: string;
  recordingDuration: number;
  hotkey: string;
  micLevel: number;
  autoInsert: boolean;
  launchOnStartup: boolean;
  history: HistoryEntry[];
  dictionary: DictionaryEntry[];

  setAppState: (state: StateKind, error?: string | null) => void;
  setLastTranscript: (text: string) => void;
  setRecordingDuration: (duration: number) => void;
  setHotkey: (hotkey: string) => void;
  setMicLevel: (level: number) => void;
  setAutoInsert: (enabled: boolean) => void;
  setLaunchOnStartup: (enabled: boolean) => void;
  addHistoryEntry: (entry: Omit<HistoryEntry, "id" | "timestamp">) => void;
  clearHistory: () => void;
  addDictionaryEntry: (original: string, replacement: string) => void;
  removeDictionaryEntry: (id: string) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  appState: "IDLE",
  errorMessage: null,
  lastTranscript: "",
  recordingDuration: 0,
  hotkey: "Ctrl+Shift+Space",
  micLevel: 0,
  autoInsert: true,
  launchOnStartup: false,
  history: [
    {
      id: "demo-1",
      timestamp: new Date().toLocaleTimeString(),
      text: "Murmur is an offline, privacy-first local dictation tool powered by sherpa-onnx and parakeet-tdt-0.6b.",
      durationSeconds: 3.2,
      wordCount: 15,
    },
  ],
  dictionary: [
    { id: "dict-1", original: "Murmer", replacement: "Murmur" },
    { id: "dict-2", original: "Rust lang", replacement: "Rust" },
  ],

  setAppState: (appState, errorMessage = null) => set({ appState, errorMessage }),
  setLastTranscript: (lastTranscript) => set({ lastTranscript }),
  setRecordingDuration: (recordingDuration) => set({ recordingDuration }),
  setHotkey: (hotkey) => set({ hotkey }),
  setMicLevel: (micLevel) => set({ micLevel }),
  setAutoInsert: (autoInsert) => set({ autoInsert }),
  setLaunchOnStartup: (launchOnStartup) => set({ launchOnStartup }),
  addHistoryEntry: (entry) =>
    set((state) => ({
      history: [
        {
          ...entry,
          id: `hist-${Date.now()}`,
          timestamp: new Date().toLocaleTimeString(),
        },
        ...state.history,
      ],
    })),
  clearHistory: () => set({ history: [] }),
  addDictionaryEntry: (original, replacement) =>
    set((state) => ({
      dictionary: [
        ...state.dictionary,
        { id: `dict-${Date.now()}`, original, replacement },
      ],
    })),
  removeDictionaryEntry: (id) =>
    set((state) => ({
      dictionary: state.dictionary.filter((item) => item.id !== id),
    })),
}));
