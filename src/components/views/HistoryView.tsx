import React from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Copy, Trash2, Check, Clock, FileText } from "lucide-react";

export const HistoryView: React.FC = () => {
  const { history, clearHistory } = useAppStore();
  const [copiedId, setCopiedId] = React.useState<string | null>(null);

  const handleCopy = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  return (
    <div className="flex flex-col space-y-4">
      <div className="flex items-center justify-between pb-2 border-b border-border">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Dictation History</h2>
          <p className="text-xs text-muted-foreground">
            All transcripts recorded locally on this device.
          </p>
        </div>
        {history.length > 0 && (
          <Button
            variant="outline"
            size="sm"
            onClick={clearHistory}
            className="text-xs text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
          >
            <Trash2 className="h-3.5 w-3.5 mr-1.5" />
            Clear All
          </Button>
        )}
      </div>

      {history.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-12 text-center text-muted-foreground space-y-2">
          <FileText className="h-10 w-10 stroke-1 opacity-40" />
          <p className="text-sm">No dictations recorded yet.</p>
          <p className="text-xs">Your spoken transcripts will appear here automatically.</p>
        </div>
      ) : (
        <div className="flex flex-col space-y-3 overflow-y-auto max-h-[420px] pr-1">
          {history.map((entry) => (
            <Card key={entry.id} className="border-border/60 bg-card">
              <CardContent className="p-4 space-y-2">
                <div className="flex items-center justify-between text-xs text-muted-foreground">
                  <div className="flex items-center space-x-2">
                    <Clock className="h-3 w-3" />
                    <span>{entry.timestamp}</span>
                    <span>•</span>
                    <span>{entry.durationSeconds.toFixed(1)}s</span>
                    <span>•</span>
                    <span>{entry.wordCount} words</span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleCopy(entry.id, entry.text)}
                    className="h-6 px-2 text-xs"
                  >
                    {copiedId === entry.id ? (
                      <Check className="h-3 w-3 text-primary" />
                    ) : (
                      <Copy className="h-3 w-3" />
                    )}
                  </Button>
                </div>
                <p className="text-sm text-foreground select-text font-sans leading-relaxed">
                  {entry.text}
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
};
