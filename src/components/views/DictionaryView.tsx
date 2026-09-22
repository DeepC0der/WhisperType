import React, { useEffect, useState, useRef } from "react";
import { useAppStore, DictionaryRecord } from "@/store/useAppStore";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Trash2,
  Plus,
  BookA,
  ArrowRight,
  Download,
  Upload,
  Edit2,
  Check,
  X,
} from "lucide-react";

export const DictionaryView: React.FC = () => {
  const {
    dictionary,
    fetchDictionary,
    saveDictionaryEntry,
    deleteDictionary,
    importDictionary,
  } = useAppStore();

  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [isCaseSensitive, setIsCaseSensitive] = useState(false);

  // Edit state
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editFind, setEditFind] = useState("");
  const [editReplace, setEditReplace] = useState("");
  const [editCase, setEditCase] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetchDictionary();
  }, [fetchDictionary]);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!findText.trim() || !replaceText.trim()) return;
    await saveDictionaryEntry(null, findText.trim(), replaceText.trim(), isCaseSensitive);
    setFindText("");
    setReplaceText("");
    setIsCaseSensitive(false);
  };

  const handleStartEdit = (entry: DictionaryRecord) => {
    setEditingId(entry.id);
    setEditFind(entry.find_text);
    setEditReplace(entry.replace_text);
    setEditCase(entry.is_case_sensitive);
  };

  const handleSaveEdit = async (id: string) => {
    if (!editFind.trim() || !editReplace.trim()) return;
    await saveDictionaryEntry(id, editFind.trim(), editReplace.trim(), editCase);
    setEditingId(null);
  };

  // Export JSON
  const exportJson = () => {
    if (dictionary.length === 0) return;
    const data = JSON.stringify(dictionary, null, 2);
    const blob = new Blob([data], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `murmur-dictionary-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  // Export CSV
  const exportCsv = () => {
    if (dictionary.length === 0) return;
    const header = "find_text,replace_text,is_case_sensitive\n";
    const rows = dictionary
      .map(
        (d) =>
          `"${d.find_text.replace(/"/g, '""')}","${d.replace_text.replace(/"/g, '""')}",${
            d.is_case_sensitive
          }`
      )
      .join("\n");
    const blob = new Blob([header + rows], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `murmur-dictionary-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  // Import CSV / JSON
  const handleFileImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const text = await file.text();
    try {
      if (file.name.endsWith(".json")) {
        const parsed = JSON.parse(text);
        if (Array.isArray(parsed)) {
          const entries = parsed.map((item) => ({
            find_text: item.find_text || item.original || "",
            replace_text: item.replace_text || item.replacement || "",
            is_case_sensitive: Boolean(item.is_case_sensitive),
          }));
          await importDictionary(entries);
        }
      } else {
        // Assume CSV
        const lines = text.split(/\r?\n/).filter(Boolean);
        const entries: { find_text: string; replace_text: string; is_case_sensitive?: boolean }[] = [];
        for (let i = 0; i < lines.length; i++) {
          const line = lines[i].trim();
          if (i === 0 && line.toLowerCase().includes("find_text")) continue; // skip header
          const parts = line.split(",").map((p) => p.replace(/^"|"$/g, "").trim());
          if (parts.length >= 2 && parts[0] && parts[1]) {
            entries.push({
              find_text: parts[0],
              replace_text: parts[1],
              is_case_sensitive: parts[2] === "true" || parts[2] === "1",
            });
          }
        }
        if (entries.length > 0) {
          await importDictionary(entries);
        }
      }
    } catch (err) {
      console.error("Failed to import dictionary file:", err);
    }
    // Reset file input
    if (fileInputRef.current) fileInputRef.current.value = "";
  };

  return (
    <div className="flex flex-col space-y-4 pb-8">
      {/* Header & Import/Export */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 pb-3 border-b border-border">
        <div>
          <h2 className="font-pixel text-sm tracking-wider uppercase font-bold text-foreground">
            Custom Dictionary
          </h2>
          <p className="text-xs text-muted-foreground">
            Phonetic substitutions & domain-specific replacements applied during ASR post-processing.
          </p>
        </div>

        <div className="flex items-center space-x-2">
          <input
            type="file"
            ref={fileInputRef}
            onChange={handleFileImport}
            accept=".json,.csv"
            className="hidden"
          />
          <Button
            variant="outline"
            size="sm"
            onClick={() => fileInputRef.current?.click()}
            className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
            title="Import rules from JSON or CSV"
          >
            <Upload className="h-3.5 w-3.5 mr-1" />
            Import
          </Button>
          {dictionary.length > 0 && (
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={exportJson}
                className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                title="Export as JSON"
              >
                <Download className="h-3.5 w-3.5 mr-1" />
                .json
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={exportCsv}
                className="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                title="Export as CSV"
              >
                <Download className="h-3.5 w-3.5 mr-1" />
                .csv
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Add New Rule Card */}
      <Card className="border-border/70 bg-card pixel-border">
        <CardContent className="p-4">
          <form onSubmit={handleAdd} className="flex flex-col sm:flex-row items-stretch sm:items-center gap-2.5">
            <input
              type="text"
              placeholder="Find / Spoken (e.g. Murmer)"
              value={findText}
              onChange={(e) => setFindText(e.target.value)}
              className="flex-1 h-9 rounded-md border border-input bg-secondary/40 px-3 py-1 text-xs text-foreground placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
            />
            <div className="hidden sm:flex items-center justify-center">
              <ArrowRight className="h-4 w-4 text-muted-foreground shrink-0" />
            </div>
            <input
              type="text"
              placeholder="Replace with (e.g. Murmur)"
              value={replaceText}
              onChange={(e) => setReplaceText(e.target.value)}
              className="flex-1 h-9 rounded-md border border-input bg-secondary/40 px-3 py-1 text-xs text-foreground placeholder:text-muted-foreground/60 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
            />

            <label className="flex items-center space-x-1.5 text-xs text-muted-foreground shrink-0 cursor-pointer select-none px-1">
              <input
                type="checkbox"
                checked={isCaseSensitive}
                onChange={(e) => setIsCaseSensitive(e.target.checked)}
                className="rounded border-border text-primary focus:ring-primary"
              />
              <span className="text-[11px]">Match Case</span>
            </label>

            <Button
              type="submit"
              size="sm"
              variant="pixel"
              className="h-9 px-4 text-xs font-pixel uppercase tracking-wider shrink-0"
            >
              <Plus className="h-3.5 w-3.5 mr-1" />
              Add
            </Button>
          </form>
        </CardContent>
      </Card>

      {/* Rules Table / List */}
      {dictionary.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-10 text-center text-muted-foreground space-y-2 border border-dashed border-border/80 rounded-lg">
          <BookA className="h-9 w-9 stroke-1 opacity-40" />
          <p className="text-sm">No dictionary replacement rules defined yet.</p>
          <p className="text-xs">
            Add rules above to automatically correct technical terms, names, or homophones.
          </p>
        </div>
      ) : (
        <div className="flex flex-col space-y-2 overflow-y-auto max-h-[460px] pr-1">
          {dictionary.map((item) => {
            const isEditing = editingId === item.id;

            if (isEditing) {
              return (
                <div
                  key={item.id}
                  className="flex items-center justify-between p-3 rounded-lg border border-primary/60 bg-card pixel-border gap-2"
                >
                  <input
                    type="text"
                    value={editFind}
                    onChange={(e) => setEditFind(e.target.value)}
                    className="flex-1 h-8 rounded border border-input bg-secondary px-2 text-xs"
                  />
                  <ArrowRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                  <input
                    type="text"
                    value={editReplace}
                    onChange={(e) => setEditReplace(e.target.value)}
                    className="flex-1 h-8 rounded border border-input bg-secondary px-2 text-xs"
                  />
                  <label className="flex items-center space-x-1 text-xs text-muted-foreground shrink-0">
                    <input
                      type="checkbox"
                      checked={editCase}
                      onChange={(e) => setEditCase(e.target.checked)}
                    />
                    <span className="text-[10px]">Case</span>
                  </label>
                  <Button
                    variant="pixel"
                    size="sm"
                    onClick={() => handleSaveEdit(item.id)}
                    className="h-8 px-2.5 text-xs"
                  >
                    <Check className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setEditingId(null)}
                    className="h-8 px-2 text-xs"
                  >
                    <X className="h-3.5 w-3.5" />
                  </Button>
                </div>
              );
            }

            return (
              <div
                key={item.id}
                className="flex items-center justify-between p-3 rounded-lg border border-border/70 bg-card/70 pixel-border hover:border-border transition-colors"
              >
                <div className="flex items-center space-x-3 text-xs">
                  <code className="bg-secondary text-foreground px-2 py-1 rounded font-mono font-medium">
                    {item.find_text}
                  </code>
                  <ArrowRight className="h-3.5 w-3.5 text-muted-foreground" />
                  <code className="bg-primary/10 text-primary px-2 py-1 rounded font-mono font-semibold phosphor-glow-subtle">
                    {item.replace_text}
                  </code>
                  {item.is_case_sensitive && (
                    <span className="text-[10px] font-pixel text-amber-400 bg-amber-950/30 border border-amber-500/30 px-1.5 py-0.5 rounded">
                      CASE
                    </span>
                  )}
                </div>

                <div className="flex items-center space-x-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleStartEdit(item)}
                    className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                    title="Edit substitution"
                  >
                    <Edit2 className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => deleteDictionary(item.id)}
                    className="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
                    title="Delete rule"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
