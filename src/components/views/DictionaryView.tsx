import React, { useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Trash2, Plus, BookA, ArrowRight } from "lucide-react";

export const DictionaryView: React.FC = () => {
  const { dictionary, addDictionaryEntry, removeDictionaryEntry } = useAppStore();
  const [original, setOriginal] = useState("");
  const [replacement, setReplacement] = useState("");

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault();
    if (!original.trim() || !replacement.trim()) return;
    addDictionaryEntry(original.trim(), replacement.trim());
    setOriginal("");
    setReplacement("");
  };

  return (
    <div className="flex flex-col space-y-4">
      <div className="pb-2 border-b border-border">
        <h2 className="text-lg font-semibold tracking-tight">Custom Dictionary</h2>
        <p className="text-xs text-muted-foreground">
          Define text substitutions and custom phonetic mappings applied during post-processing.
        </p>
      </div>

      {/* Add New Rule Form */}
      <Card className="border-border/60 bg-card">
        <CardContent className="p-4">
          <form onSubmit={handleAdd} className="flex items-center space-x-2">
            <input
              type="text"
              placeholder="Spoken / Recognized (e.g. Murmer)"
              value={original}
              onChange={(e) => setOriginal(e.target.value)}
              className="flex-1 h-9 rounded-md border border-input bg-secondary/50 px-3 py-1 text-sm shadow-sm transition-colors focus:border-primary focus:outline-none"
            />
            <ArrowRight className="h-4 w-4 text-muted-foreground shrink-0" />
            <input
              type="text"
              placeholder="Replacement (e.g. Murmur)"
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
              className="flex-1 h-9 rounded-md border border-input bg-secondary/50 px-3 py-1 text-sm shadow-sm transition-colors focus:border-primary focus:outline-none"
            />
            <Button type="submit" size="sm" variant="pixel" className="h-9 px-3 text-xs">
              <Plus className="h-3.5 w-3.5 mr-1" />
              Add
            </Button>
          </form>
        </CardContent>
      </Card>

      {/* Existing Rules List */}
      {dictionary.length === 0 ? (
        <div className="flex flex-col items-center justify-center p-8 text-center text-muted-foreground space-y-2">
          <BookA className="h-9 w-9 stroke-1 opacity-40" />
          <p className="text-sm">No dictionary substitutions defined.</p>
        </div>
      ) : (
        <div className="flex flex-col space-y-2 overflow-y-auto max-h-[360px]">
          {dictionary.map((item) => (
            <div
              key={item.id}
              className="flex items-center justify-between p-3 rounded-lg border border-border bg-card/60"
            >
              <div className="flex items-center space-x-3 text-sm">
                <code className="bg-secondary/80 text-foreground px-2 py-0.5 rounded font-mono text-xs">
                  {item.original}
                </code>
                <ArrowRight className="h-3.5 w-3.5 text-muted-foreground" />
                <code className="bg-primary/10 text-primary font-semibold px-2 py-0.5 rounded font-mono text-xs">
                  {item.replacement}
                </code>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => removeDictionaryEntry(item.id)}
                className="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
