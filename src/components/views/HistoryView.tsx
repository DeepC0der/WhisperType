import React, { useEffect, useState, useMemo } from "react";
import { useAppStore, HistoryRecord } from "@/store/useAppStore";
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
} from "lucide-react";

export const HistoryView: React.FC = () => {
  const {
    history,
    fetchHistory,
    deleteHistory,
    clearAllHistory,
    updateHistoryText,
    reinsertHistory,
  } = useAppStore();

  const [searchQuery, setSearchQuery] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editText, setEditText] = useState("");
  const [showClearConfirm, setShowClearConfirm] = useState(false);

  useEffect(() => {
    fetchHistory();
  }, [fetchHistory]);

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

  // Filter history by search query
  const filteredHistory = useMemo(() => {
    if (!searchQuery.trim()) return history;
    const q = searchQuery.toLowerCase();
    return history.filter(
      (entry) =>
        entry.final_text.toLowerCase().includes(q) ||
        entry.raw_text.toLowerCase().includes(q) ||
        new Date(entry.created_at).toLocaleDateString().toLowerCase().includes(q)
    );
  }, [history, searchQuery]);

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

        <div className="flex items-center space-x-2">
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
                onClick={() => setShowClearConfirm(true)}
                className="h-8 px-2.5 text-xs text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
              >
                <Trash2 className="h-3.5 w-3.5 mr-1" />
                Clear All
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Confirmation Dialog for Clear All */}
      {showClearConfirm && (
        <div className="p-4 border border-destructive/40 bg-destructive/10 rounded-lg flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs">
          <div className="flex items-center space-x-2 text-destructive font-medium">
            <AlertTriangle className="h-4 w-4 shrink-0" />
            <span>Are you sure you want to permanently delete all {history.length} history records?</span>
          </div>
          <div className="flex items-center space-x-2 shrink-0">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowClearConfirm(false)}
              className="h-7 px-3 text-xs"
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={async () => {
                await clearAllHistory();
                setShowClearConfirm(false);
              }}
              className="h-7 px-3 text-xs"
            >
              Yes, Clear All
            </Button>
          </div>
        </div>
      )}

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

      {/* List of Transcripts */}
      {filteredHistory.length === 0 ? (
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
                    </div>

                    <div className="flex items-center space-x-1">
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
                        onClick={() => deleteHistory(entry.id)}
                        title="Delete this record"
                        className="h-6 px-1.5 text-xs text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>

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
                    <p className="text-sm text-foreground select-text font-sans leading-relaxed">
                      {entry.final_text}
                    </p>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
};
