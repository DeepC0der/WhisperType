import React, { useEffect, useState, useMemo } from "react";
import { useAppStore, HistoryRecord, SavedRecordingInfo, invokeTauri } from "@/store/useAppStore";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Copy,
  Trash2,
  Check,
  Clock,
  FileText,
  Search,
  Download,
  CornerDownLeft,
  Edit2,
  X,
  AlertTriangle,
  Play,
  Volume2,
  VolumeX,
} from "lucide-react";

export const HistoryView: React.FC = () => {
  const {
    history,
    recordings,
    fetchHistory,
    deleteHistory,
    clearAllHistory,
    updateHistoryText,
    reinsertHistory,
    deleteRecording,
    clearAllRecordings,
  } = useAppStore();

  const [searchQuery, setSearchQuery] = useState("");
  const [historySubTab, setHistorySubTab] = useState<"all" | "with_audio" | "audio_only">("all");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editText, setEditText] = useState("");
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [showClearAudioConfirm, setShowClearAudioConfirm] = useState(false);
  const [confirmDeleteEntryId, setConfirmDeleteEntryId] = useState<string | null>(null);

  // Audio playback state
  const [audioUrls, setAudioUrls] = useState<Record<string, string>>({});
  const audioUrlsRef = React.useRef<Record<string, string>>({});
  const [loadingAudioId, setLoadingAudioId] = useState<string | null>(null);
  const [activeAudioId, setActiveAudioId] = useState<string | null>(null);
  const [isAudioPlaying, setIsAudioPlaying] = useState(false);
  const [audioProgress, setAudioProgress] = useState(0);
  const [audioDuration, setAudioDuration] = useState(0);
  const [audioPlaybackError, setAudioPlaybackError] = useState<{ id: string; message: string } | null>(null);
  const audioRef = React.useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    fetchHistory();
  }, [fetchHistory]);

  // Clean up object URLs and audio element strictly on unmount
  const teardownAudioElement = (clearRef: boolean = false) => {
    if (audioRef.current) {
      audioRef.current.onloadedmetadata = null;
      audioRef.current.ontimeupdate = null;
      audioRef.current.onplay = null;
      audioRef.current.onpause = null;
      audioRef.current.onended = null;
      audioRef.current.onerror = null;
      audioRef.current.pause();
      if (clearRef) {
        audioRef.current = null;
      }
    }
  };

  const revokeAllAudioUrls = () => {
    Object.values(audioUrlsRef.current).forEach((url) => {
      try {
        if (url.startsWith("blob:")) URL.revokeObjectURL(url);
      } catch {
        // ignore
      }
    });
    audioUrlsRef.current = {};
    setAudioUrls({});
  };

  const releaseAudioFor = (id: string) => {
    if (activeAudioId === id) {
      teardownAudioElement(true);
      setActiveAudioId(null);
      setIsAudioPlaying(false);
    }
    const url = audioUrlsRef.current[id] || audioUrls[id];
    if (url) {
      try {
        if (url.startsWith("blob:")) URL.revokeObjectURL(url);
      } catch {
        // ignore
      }
      delete audioUrlsRef.current[id];
      setAudioUrls((prev) => {
        const copy = { ...prev };
        delete copy[id];
        return copy;
      });
    }
    setAudioPlaybackError((prev) => (prev?.id === id ? null : prev));
  };

  useEffect(() => {
    return () => {
      teardownAudioElement(true);
      revokeAllAudioUrls();
    };
  }, []);

  const recordingMap = useMemo(() => {
    const map = new Map<string, SavedRecordingInfo>();
    recordings.forEach((r) => map.set(r.id, r));
    return map;
  }, [recordings]);

  const standaloneRecordings = useMemo(() => {
    const historyIds = new Set(history.map((h) => h.id));
    return recordings.filter((r) => !historyIds.has(r.id));
  }, [recordings, history]);

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  };

  const startPlaybackWithUrl = (id: string, url: string, audioEl: HTMLAudioElement) => {
    if (audioEl.src !== url) {
      audioEl.src = url;
    }
    setAudioProgress(0);
    setAudioDuration(0);

    audioEl.onloadedmetadata = () => {
      if (Number.isFinite(audioEl.duration) && audioEl.duration > 0) {
        setAudioDuration(audioEl.duration);
      }
    };
    audioEl.ontimeupdate = () => {
      setAudioProgress(audioEl.currentTime);
    };
    audioEl.onplay = () => setIsAudioPlaying(true);
    audioEl.onpause = () => setIsAudioPlaying(false);
    audioEl.onended = () => {
      setIsAudioPlaying(false);
      setAudioProgress(0);
    };
    audioEl.onerror = () => {
      setIsAudioPlaying(false);
      const errMessage = audioEl.error?.message || "Audio playback error occurred";
      setAudioPlaybackError({ id, message: errMessage });
    };

    setActiveAudioId(id);
    audioEl.play().catch((e) => {
      console.warn("Audio play warning:", e);
      setIsAudioPlaying(false);
      setAudioPlaybackError({
        id,
        message: e instanceof Error ? e.message : "Audio playback was blocked or failed",
      });
    });
  };

  const handleTogglePlayPauseCurrent = () => {
    if (!audioRef.current) return;
    if (audioRef.current.paused) {
      setAudioPlaybackError(null);
      audioRef.current.play().catch((e) => {
        console.warn("Audio play error on resume:", e);
        if (activeAudioId) {
          setAudioPlaybackError({
            id: activeAudioId,
            message: e instanceof Error ? e.message : "Failed to resume audio playback",
          });
        }
      });
    } else {
      audioRef.current.pause();
    }
  };

  const handleSeekAudio = (newTime: number) => {
    if (!audioRef.current) return;
    audioRef.current.currentTime = newTime;
    setAudioProgress(newTime);
  };

  const handleTogglePlayAudio = async (id: string) => {
    if (activeAudioId === id) {
      if (audioRef.current) {
        audioRef.current.pause();
      }
      setActiveAudioId(null);
      setIsAudioPlaying(false);
      return;
    }

    setAudioPlaybackError(null);

    // Stop and clear listeners on any previous audio playback
    teardownAudioElement(false);

    // Synchronously create/obtain HTMLAudioElement within the user click gesture
    const audio = audioRef.current || new Audio();
    audioRef.current = audio;

    const existingUrl = audioUrlsRef.current[id] || audioUrls[id];
    if (existingUrl) {
      startPlaybackWithUrl(id, existingUrl, audio);
      return;
    }

    try {
      setLoadingAudioId(id);
      const raw = await invokeTauri<number[] | Uint8Array | ArrayBuffer>("get_recording_audio", { id });

      // Guard against race condition if user clicked another track while IPC was resolving
      if (audioRef.current !== audio) {
        return;
      }

      let uint8: Uint8Array;
      if (raw instanceof Uint8Array) {
        uint8 = raw;
      } else if (raw instanceof ArrayBuffer) {
        uint8 = new Uint8Array(raw);
      } else if (Array.isArray(raw)) {
        uint8 = new Uint8Array(raw);
      } else {
        uint8 = new Uint8Array(raw as any);
      }

      // Verify valid RIFF/WAV header ('RIFF' = 0x52, 0x49, 0x46, 0x46)
      if (
        uint8.length < 44 ||
        uint8[0] !== 0x52 ||
        uint8[1] !== 0x49 ||
        uint8[2] !== 0x46 ||
        uint8[3] !== 0x46
      ) {
        throw new Error("Invalid audio recording: missing RIFF header");
      }

      const blob = new Blob([uint8 as unknown as BlobPart], { type: "audio/wav" });
      const url = URL.createObjectURL(blob);
      audioUrlsRef.current[id] = url;
      setAudioUrls((prev) => ({ ...prev, [id]: url }));
      startPlaybackWithUrl(id, url, audio);
    } catch (err) {
      console.warn("get_recording_audio error:", err);
      if (audioRef.current === audio) {
        setActiveAudioId(null);
        setIsAudioPlaying(false);
      }
      setAudioPlaybackError({
        id,
        message: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setLoadingAudioId(null);
    }
  };

  const renderInlineAudioPlayer = (id: string, fallbackDurationSec?: number) => {
    const url = audioUrls[id] || audioUrlsRef.current[id];
    if (activeAudioId !== id || !url) return null;
    const totalSec = audioDuration > 0 ? audioDuration : fallbackDurationSec || 1;
    const pct = Math.min(100, Math.max(0, (audioProgress / totalSec) * 100));

    return (
      <div className="mt-2.5 p-2.5 rounded-xl bg-secondary/75 border border-border flex flex-col gap-2">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={handleTogglePlayPauseCurrent}
            aria-label={isAudioPlaying ? "Pause recording" : "Play recording"}
            className="h-8 w-8 rounded-lg bg-primary text-primary-foreground flex items-center justify-center shrink-0 hover:bg-primary/90 transition-colors cursor-pointer shadow-sm"
          >
            {isAudioPlaying ? (
              <VolumeX className="h-4 w-4" aria-hidden="true" />
            ) : (
              <Play className="h-4 w-4 ml-0.5" aria-hidden="true" />
            )}
          </button>

          <div className="flex-1 flex flex-col gap-1">
            <input
              type="range"
              min={0}
              max={totalSec}
              step={0.05}
              value={Math.min(audioProgress, totalSec)}
              onChange={(e) => handleSeekAudio(Number(e.target.value))}
              aria-label="Seek audio position"
              className="w-full accent-primary h-1.5 bg-background rounded-lg cursor-pointer"
            />
            <div className="flex items-center justify-between text-[10px] font-mono text-muted-foreground">
              <span>{audioProgress.toFixed(1)}s</span>
              <span>{pct.toFixed(0)}%</span>
              <span>{totalSec.toFixed(1)}s</span>
            </div>
          </div>
        </div>

        {audioPlaybackError && audioPlaybackError.id === id && (
          <div className="text-[11px] text-destructive flex items-center gap-1.5 pl-0.5">
            <AlertTriangle className="h-3 w-3 shrink-0" />
            <span>Playback warning: {audioPlaybackError.message}</span>
          </div>
        )}
      </div>
    );
  };

  const handleDeleteAudio = async (id: string) => {
    releaseAudioFor(id);
    await deleteRecording(id);
  };

  const handleCopy = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const handleStartEdit = (entry: HistoryRecord) => {
    setEditingId(entry.id);
    setEditText(entry.final_text);
  };

  const handleSaveEdit = async (id: string) => {
    if (!editText.trim()) return;
    await updateHistoryText(id, editText.trim());
    setEditingId(null);
  };

  const handleReinsert = async (text: string) => {
    await reinsertHistory(text);
  };

  // Filter history by search query and active sub-tab
  const filteredHistory = useMemo(() => {
    let base = history;
    if (historySubTab === "with_audio") {
      base = base.filter((entry) => recordingMap.has(entry.id));
    }
    if (!searchQuery.trim()) return base;
    const q = searchQuery.toLowerCase();
    return base.filter(
      (entry) =>
        entry.final_text.toLowerCase().includes(q) ||
        entry.raw_text.toLowerCase().includes(q) ||
        new Date(entry.created_at).toLocaleDateString().toLowerCase().includes(q)
    );
  }, [history, searchQuery, historySubTab, recordingMap]);

  // Export as Plain Text
  const exportTxt = () => {
    if (history.length === 0) return;
    const content = history
      .map(
        (h) =>
          `[${new Date(h.created_at).toLocaleString()}] (${(h.duration_ms / 1000).toFixed(1)}s)\n${
            h.final_text
          }\n`
      )
      .join("\n----------------------------------------\n\n");

    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `murmur-transcripts-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  // Export as Markdown
  const exportMd = () => {
    if (history.length === 0) return;
    let content = `# Murmur Dictation History\n\nExported on ${new Date().toLocaleString()}\n\n`;
    history.forEach((h) => {
      content += `### ${new Date(h.created_at).toLocaleString()}\n`;
      content += `- **Duration**: ${(h.duration_ms / 1000).toFixed(1)}s\n`;
      content += `- **Words**: ${h.final_text.trim().split(/\s+/).filter(Boolean).length}\n`;
      content += `- **Model**: ${h.model_variant} (${h.provider})\n\n`;
      content += `> ${h.final_text}\n\n---\n\n`;
    });

    const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `murmur-transcripts-${new Date().toISOString().slice(0, 10)}.md`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="flex flex-col space-y-4 pb-8">
      {/* Header & Export Actions */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pb-3 border-b border-border">
        <div>
          <h2 className="font-pixel text-sm tracking-wider uppercase font-bold text-foreground">
            Dictation History
          </h2>
          <p className="text-xs text-muted-foreground">
            Locally persisted SQLite transcripts with zero cloud transmission.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {recordings.length > 0 && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setShowClearConfirm(false);
                setShowClearAudioConfirm(true);
              }}
              className="h-8 px-2.5 text-xs text-amber-400 hover:text-amber-300 hover:bg-amber-500/10 border-amber-500/35"
              title="Delete only the saved .wav audio files while keeping all text transcripts"
            >
              <Trash2 className="h-3.5 w-3.5 mr-1" />
              Delete All Audio Only ({recordings.length})
            </Button>
          )}
          {history.length > 0 && (
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={exportTxt}
                className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                title="Export as plain text"
              >
                <Download className="h-3.5 w-3.5 mr-1" />
                .txt
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={exportMd}
                className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                title="Export as markdown"
              >
                <Download className="h-3.5 w-3.5 mr-1" />
                .md
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  setShowClearAudioConfirm(false);
                  setShowClearConfirm(true);
                }}
                className="h-8 px-2.5 text-xs text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
              >
                <Trash2 className="h-3.5 w-3.5 mr-1" />
                Clear All
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Confirmation Dialog for Delete All Audio Only */}
      {showClearAudioConfirm && (
        <div className="p-4 border border-amber-500/40 bg-amber-500/10 rounded-xl flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs">
          <div className="flex items-center space-x-2 text-amber-300 font-medium">
            <AlertTriangle className="h-4 w-4 shrink-0" />
            <span>
              Delete all {recordings.length} saved <code>.wav</code> audio files from disk? Your text transcripts will be kept intact.
            </span>
          </div>
          <div className="flex items-center space-x-2 shrink-0">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowClearAudioConfirm(false)}
              className="h-7 px-3 text-xs"
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={async () => {
                teardownAudioElement(true);
                setActiveAudioId(null);
                setIsAudioPlaying(false);
                setAudioPlaybackError(null);
                revokeAllAudioUrls();
                await clearAllRecordings();
                setShowClearAudioConfirm(false);
              }}
              className="h-7 px-3 text-xs"
            >
              Yes, Delete Audio Only
            </Button>
          </div>
        </div>
      )}

      {/* Confirmation Dialog for Clear All (Offers Delete Audio Alone vs Delete Everything) */}
      {showClearConfirm && (
        <div className="p-4 border border-destructive/40 bg-destructive/10 rounded-xl flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs">
          <div className="flex items-center space-x-2 text-foreground font-medium">
            <AlertTriangle className="h-4 w-4 text-destructive shrink-0" />
            <span>
              Choose what to delete: remove <strong>only the saved audio files</strong> (keeping transcripts) or <strong>delete everything</strong>?
            </span>
          </div>
          <div className="flex flex-wrap items-center gap-2 shrink-0">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowClearConfirm(false)}
              className="h-7 px-3 text-xs"
            >
              Cancel
            </Button>
            {recordings.length > 0 && (
              <Button
                variant="outline"
                size="sm"
                onClick={async () => {
                  teardownAudioElement(true);
                  setActiveAudioId(null);
                  setIsAudioPlaying(false);
                  setAudioPlaybackError(null);
                  revokeAllAudioUrls();
                  await clearAllRecordings();
                  setShowClearConfirm(false);
                }}
                className="h-7 px-3 text-xs border-amber-500/50 text-amber-300 hover:bg-amber-500/15"
              >
                Delete Audio Alone ({recordings.length})
              </Button>
            )}
            <Button
              variant="destructive"
              size="sm"
              onClick={async () => {
                teardownAudioElement(true);
                setActiveAudioId(null);
                setIsAudioPlaying(false);
                setAudioPlaybackError(null);
                revokeAllAudioUrls();
                await clearAllHistory();
                setShowClearConfirm(false);
              }}
              className="h-7 px-3 text-xs"
            >
              Delete Everything (Transcripts + Audio)
            </Button>
          </div>
        </div>
      )}

      {/* Sub-Part Switcher: Transcripts vs. Audio & Transcript Review vs. Raw Audio Files */}
      <div className="flex items-center gap-2 p-1 rounded-lg bg-secondary/50 border border-border">
        <button
          type="button"
          onClick={() => setHistorySubTab("all")}
          className={`flex-1 py-1.5 px-3 rounded-md text-xs font-pixel uppercase tracking-wider transition-all ${
            historySubTab === "all"
              ? "bg-primary text-primary-foreground font-bold shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          All Transcripts ({history.length})
        </button>
        <button
          type="button"
          onClick={() => setHistorySubTab("with_audio")}
          className={`flex-1 py-1.5 px-3 rounded-md text-xs font-pixel uppercase tracking-wider transition-all ${
            historySubTab === "with_audio"
              ? "bg-primary text-primary-foreground font-bold shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          Audio + Transcripts ({history.filter((h) => recordingMap.has(h.id)).length})
        </button>
        <button
          type="button"
          onClick={() => setHistorySubTab("audio_only")}
          className={`flex-1 py-1.5 px-3 rounded-md text-xs font-pixel uppercase tracking-wider transition-all ${
            historySubTab === "audio_only"
              ? "bg-primary text-primary-foreground font-bold shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          All Saved Audio ({recordings.length})
        </button>
      </div>

      {/* Search Input Bar */}
      {history.length > 0 && (
        <div className="relative">
          <Search className="absolute left-3 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search through transcripts, dates, or keywords..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full h-9 pl-9 pr-8 rounded-md border border-input bg-secondary/40 text-xs text-foreground placeholder:text-muted-foreground/60 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery("")}
              className="absolute right-2.5 top-2.5 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      )}

      {/* Audio-Only Sub-View (All Saved .wav Files) */}
      {historySubTab === "audio_only" ? (
        recordings.length === 0 ? (
          <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground space-y-2 border border-dashed border-border/80 rounded-lg">
            <Volume2 className="h-10 w-10 stroke-1 opacity-40" />
            <p className="text-sm">No saved audio recordings found.</p>
            <p className="text-xs">
              Enable <strong>Store Audio Recordings</strong> in Settings &rarr; System &amp; Advanced to save local <code>.wav</code> files alongside your transcripts.
            </p>
          </div>
        ) : (
          <div className="flex flex-col space-y-2.5 overflow-y-auto max-h-[500px] pr-1">
            {recordings.map((rec) => {
              const isPlaying = activeAudioId === rec.id;
              const audioUrl = audioUrls[rec.id];
              const linkedTranscript = history.find((h) => h.id === rec.id);

              return (
                <Card key={rec.id} className="border-border/70 bg-card pixel-border">
                  <CardContent className="p-3.5 space-y-2">
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                      <div className="flex items-center space-x-2">
                        <Volume2 className="h-3.5 w-3.5 text-primary" />
                        <span className="font-mono text-foreground font-medium">{rec.file_name}</span>
                        <span>•</span>
                        <span>{rec.duration_seconds.toFixed(1)}s</span>
                        <span>•</span>
                        <span className="text-[10px] bg-secondary px-1.5 py-0.2 rounded font-mono text-primary font-medium">
                          {formatFileSize(rec.size_bytes)}
                        </span>
                      </div>

                      <div className="flex items-center space-x-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleTogglePlayAudio(rec.id)}
                          disabled={loadingAudioId === rec.id}
                          className="h-6 px-2 text-xs text-primary hover:text-primary"
                        >
                          {isPlaying ? (
                            <>
                              <VolumeX className="h-3.5 w-3.5 mr-1" />
                              <span>Hide Player</span>
                            </>
                          ) : (
                            <>
                              <Play className="h-3.5 w-3.5 mr-1" />
                              <span>{loadingAudioId === rec.id ? "Loading..." : "Play Audio"}</span>
                            </>
                          )}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => {
                            if (linkedTranscript) {
                              setConfirmDeleteEntryId(
                                confirmDeleteEntryId === rec.id ? null : rec.id
                              );
                            } else {
                              handleDeleteAudio(rec.id);
                            }
                          }}
                          className="h-6 px-2 text-xs text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="h-3.5 w-3.5 mr-1" />
                          <span>Delete...</span>
                        </Button>
                      </div>
                    </div>

                    {confirmDeleteEntryId === rec.id && linkedTranscript && (
                      <div className="p-2.5 rounded-lg border border-amber-500/40 bg-amber-500/10 flex flex-wrap items-center justify-between gap-2 text-xs">
                        <span className="text-foreground font-medium">
                          Delete audio recording alone or delete transcript too?
                        </span>
                        <div className="flex items-center gap-1.5">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setConfirmDeleteEntryId(null)}
                            className="h-6 px-2 text-xs"
                          >
                            Cancel
                          </Button>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={async () => {
                              await handleDeleteAudio(rec.id);
                              setConfirmDeleteEntryId(null);
                            }}
                            className="h-6 px-2.5 text-xs border-amber-500/50 text-amber-300 hover:bg-amber-500/15"
                          >
                            Delete Audio Alone
                          </Button>
                          <Button
                            variant="destructive"
                            size="sm"
                            onClick={async () => {
                              releaseAudioFor(rec.id);
                              await deleteHistory(rec.id);
                              setConfirmDeleteEntryId(null);
                            }}
                            className="h-6 px-2.5 text-xs"
                          >
                            Delete Everything
                          </Button>
                        </div>
                      </div>
                    )}

                    {linkedTranscript && (
                      <div className="p-2.5 rounded bg-secondary/40 border border-border/60 text-xs text-foreground">
                        <span className="font-pixel text-[10px] uppercase text-muted-foreground block mb-1">
                          Matched Transcript ({linkedTranscript.model_variant}):
                        </span>
                        {linkedTranscript.final_text}
                      </div>
                    )}

                    {isPlaying && audioUrl && renderInlineAudioPlayer(rec.id, rec.duration_seconds)}
                    {audioPlaybackError && audioPlaybackError.id === rec.id && !isPlaying && (
                      <div className="mt-1 text-xs text-destructive flex items-center gap-1.5">
                        <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
                        <span>Audio playback error: {audioPlaybackError.message}</span>
                      </div>
                    )}
                  </CardContent>
                </Card>
              );
            })}
          </div>
        )
      ) : filteredHistory.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground space-y-2 border border-dashed border-border/80 rounded-lg">
          <FileText className="h-10 w-10 stroke-1 opacity-40" />
          <p className="text-sm">
            {searchQuery ? "No transcripts match your search query." : "No dictations recorded yet."}
          </p>
          <p className="text-xs">
            {searchQuery ? "Try searching for a different word." : "Your spoken transcripts will appear here automatically."}
          </p>
        </div>
      ) : (
        <div className="flex flex-col space-y-3 overflow-y-auto max-h-[500px] pr-1">
          {filteredHistory.map((entry) => {
            const wordCount = entry.final_text.trim().split(/\s+/).filter(Boolean).length;
            const isEditing = editingId === entry.id;
            const matchingRecording = recordingMap.get(entry.id);
            const isPlaying = activeAudioId === entry.id;
            const audioUrl = audioUrls[entry.id];

            return (
              <Card key={entry.id} className="border-border/70 bg-card pixel-border">
                <CardContent className="p-4 space-y-2.5">
                  {/* Metadata & Actions Header */}
                  <div className="flex items-center justify-between text-xs text-muted-foreground">
                    <div className="flex items-center space-x-2">
                      <Clock className="h-3.5 w-3.5 text-muted-foreground" />
                      <span>{new Date(entry.created_at).toLocaleString()}</span>
                      <span>•</span>
                      <span>{(entry.duration_ms / 1000).toFixed(1)}s</span>
                      <span>•</span>
                      <span>{wordCount} words</span>
                      <span>•</span>
                      <span className="uppercase text-[10px] bg-secondary px-1.5 py-0.2 rounded">
                        {entry.language || "EN"}
                      </span>
                      {matchingRecording && (
                        <>
                          <span>•</span>
                          <span
                            className="text-[10px] bg-secondary px-1.5 py-0.2 rounded font-mono text-primary font-medium"
                            title="Saved Audio WAV size"
                          >
                            {formatFileSize(matchingRecording.size_bytes)}
                          </span>
                        </>
                      )}
                      {entry.mode === "professional" && (
                        <span className="font-pixel uppercase text-[9px] bg-violet-500/20 text-violet-300 border border-violet-500/50 px-1.5 py-0.2 rounded">
                          PRO AI
                        </span>
                      )}
                    </div>

                    <div className="flex items-center space-x-1">
                      {/* Audio controls if recording file exists */}
                      {matchingRecording && (
                        <>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => handleTogglePlayAudio(entry.id)}
                            disabled={loadingAudioId === entry.id}
                            title={isPlaying ? "Hide Player" : "Play Audio"}
                            className="h-6 px-1.5 text-xs text-muted-foreground hover:text-foreground"
                          >
                            {isPlaying ? (
                              <>
                                <VolumeX className="h-3.5 w-3.5 mr-1" />
                                <span>Hide Player</span>
                              </>
                            ) : (
                              <>
                                <Play className="h-3.5 w-3.5 mr-1" />
                                <span>{loadingAudioId === entry.id ? "Loading..." : "Play Audio"}</span>
                              </>
                            )}
                          </Button>

                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => handleDeleteAudio(entry.id)}
                            title="Delete only the .wav audio file and keep the text transcript"
                            className="h-6 px-1.5 text-xs text-muted-foreground hover:text-destructive"
                          >
                            <Trash2 className="h-3.5 w-3.5 mr-1" />
                            <span>Delete Audio Only</span>
                          </Button>
                        </>
                      )}

                      {/* Re-insert button */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleReinsert(entry.final_text)}
                        title="Re-insert into active application"
                        className="h-6 px-1.5 text-xs text-muted-foreground hover:text-primary"
                      >
                        <CornerDownLeft className="h-3.5 w-3.5" />
                      </Button>

                      {/* Inline edit button */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleStartEdit(entry)}
                        title="Edit transcript text"
                        className="h-6 px-1.5 text-xs text-muted-foreground hover:text-foreground"
                      >
                        <Edit2 className="h-3.5 w-3.5" />
                      </Button>

                      {/* Copy button */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleCopy(entry.id, entry.final_text)}
                        title="Copy to clipboard"
                        className="h-6 px-1.5 text-xs"
                      >
                        {copiedId === entry.id ? (
                          <Check className="h-3.5 w-3.5 text-primary" />
                        ) : (
                          <Copy className="h-3.5 w-3.5" />
                        )}
                      </Button>

                      {/* Delete button */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          if (matchingRecording) {
                            setConfirmDeleteEntryId(
                              confirmDeleteEntryId === entry.id ? null : entry.id
                            );
                          } else {
                            releaseAudioFor(entry.id);
                            deleteHistory(entry.id);
                          }
                        }}
                        title={
                          matchingRecording
                            ? "Choose to delete audio alone or delete everything"
                            : "Delete this transcript"
                        }
                        className="h-6 px-1.5 text-xs text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>

                  {/* Per-item Delete Choice Prompt when record has both Audio & Transcript */}
                  {confirmDeleteEntryId === entry.id && matchingRecording && (
                    <div className="p-2.5 rounded-lg border border-amber-500/40 bg-amber-500/10 flex flex-wrap items-center justify-between gap-2 text-xs">
                      <span className="text-foreground font-medium">
                        Delete audio recording alone or delete everything (transcript + audio)?
                      </span>
                      <div className="flex items-center gap-1.5">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setConfirmDeleteEntryId(null)}
                          className="h-6 px-2 text-xs"
                        >
                          Cancel
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={async () => {
                            await handleDeleteAudio(entry.id);
                            setConfirmDeleteEntryId(null);
                          }}
                          className="h-6 px-2.5 text-xs border-amber-500/50 text-amber-300 hover:bg-amber-500/15"
                        >
                          Delete Audio Alone
                        </Button>
                        <Button
                          variant="destructive"
                          size="sm"
                          onClick={async () => {
                            releaseAudioFor(entry.id);
                            await deleteHistory(entry.id);
                            setConfirmDeleteEntryId(null);
                          }}
                          className="h-6 px-2.5 text-xs"
                        >
                          Delete Everything
                        </Button>
                      </div>
                    </div>
                  )}

                  {/* Body Content / Inline Edit Form */}
                  {isEditing ? (
                    <div className="space-y-2 pt-1">
                      <textarea
                        value={editText}
                        onChange={(e) => setEditText(e.target.value)}
                        rows={3}
                        className="w-full rounded border border-primary bg-secondary/40 p-2.5 text-sm font-sans text-foreground focus:outline-none resize-none"
                      />
                      <div className="flex justify-end space-x-2">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setEditingId(null)}
                          className="h-7 px-3 text-xs"
                        >
                          Cancel
                        </Button>
                        <Button
                          variant="pixel"
                          size="sm"
                          onClick={() => handleSaveEdit(entry.id)}
                          className="h-7 px-3 text-xs"
                        >
                          Save Changes
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <p className="text-sm text-foreground select-text font-sans leading-relaxed whitespace-pre-wrap">
                      {entry.final_text}
                    </p>
                  )}

                  {/* Inline Audio Player */}
                  {isPlaying && audioUrl && renderInlineAudioPlayer(entry.id, entry.duration_ms / 1000)}
                  {audioPlaybackError && audioPlaybackError.id === entry.id && !isPlaying && (
                    <div className="mt-1 text-xs text-destructive flex items-center gap-1.5">
                      <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
                      <span>Audio playback error: {audioPlaybackError.message}</span>
                    </div>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      {/* Standalone Saved Audio Recordings */}
      {standaloneRecordings.length > 0 && (
        <div className="pt-6 border-t border-border space-y-3">
          <div>
            <h3 className="font-pixel text-xs tracking-wider uppercase font-bold text-foreground">
              Standalone Saved Audio Recordings ({standaloneRecordings.length})
            </h3>
            <p className="text-xs text-muted-foreground">
              Audio files saved on disk without an associated text transcript (e.g. while persistent history was disabled).
            </p>
          </div>

          <div className="flex flex-col space-y-2">
            {standaloneRecordings.map((rec) => {
              const isPlaying = activeAudioId === rec.id;
              const audioUrl = audioUrls[rec.id];

              return (
                <Card key={rec.id} className="border-border/70 bg-card pixel-border">
                  <CardContent className="p-3 space-y-2">
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                      <div className="flex items-center space-x-2">
                        <Volume2 className="h-3.5 w-3.5 text-primary" />
                        <span className="font-mono text-foreground font-medium">{rec.file_name}</span>
                        <span>•</span>
                        <span>{rec.duration_seconds.toFixed(1)}s</span>
                        <span>•</span>
                        <span className="text-[10px] bg-secondary px-1.5 py-0.2 rounded font-mono text-primary font-medium">
                          {formatFileSize(rec.size_bytes)}
                        </span>
                      </div>

                      <div className="flex items-center space-x-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleTogglePlayAudio(rec.id)}
                          disabled={loadingAudioId === rec.id}
                          className="h-6 px-1.5 text-xs text-muted-foreground hover:text-foreground"
                          title={isPlaying ? "Hide Player" : "Play Audio"}
                        >
                          {isPlaying ? (
                            <>
                              <VolumeX className="h-3.5 w-3.5 mr-1" />
                              <span>Hide Player</span>
                            </>
                          ) : (
                            <>
                              <Play className="h-3.5 w-3.5 mr-1" />
                              <span>{loadingAudioId === rec.id ? "Loading..." : "Play Audio"}</span>
                            </>
                          )}
                        </Button>

                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDeleteAudio(rec.id)}
                          className="h-6 px-1.5 text-xs text-muted-foreground hover:text-destructive"
                          title="Delete Audio"
                        >
                          <Trash2 className="h-3.5 w-3.5 mr-1" />
                          <span>Delete Audio</span>
                        </Button>
                      </div>
                    </div>

                    {isPlaying && audioUrl && renderInlineAudioPlayer(rec.id, rec.duration_seconds)}
                    {audioPlaybackError && audioPlaybackError.id === rec.id && !isPlaying && (
                      <div className="mt-1 text-xs text-destructive flex items-center gap-1.5">
                        <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
                        <span>Audio playback error: {audioPlaybackError.message}</span>
                      </div>
                    )}
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};
