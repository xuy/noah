import { useState, useEffect, useCallback } from "react";
import { useSessionStore } from "../stores/sessionStore";
import * as commands from "../lib/tauri-commands";
import type { KnowledgeEntry } from "../lib/tauri-commands";

function KnowledgeItem({
  entry,
  onSelect,
  onDelete,
}: {
  entry: KnowledgeEntry;
  onSelect: (path: string) => void;
  onDelete: (path: string) => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <div className="border-b border-border-primary/50 last:border-b-0">
      <button
        onClick={() => onSelect(entry.path)}
        className="w-full px-6 py-3 text-left hover:bg-bg-tertiary/30 transition-colors cursor-pointer"
      >
        <p className="text-base text-text-primary leading-snug">
          {entry.title}
        </p>
        <div className="flex items-center gap-2 mt-1">
          <span className="text-xs text-text-muted font-mono">
            {entry.path}
          </span>
          <span className="ml-auto">
            {confirmDelete ? (
              <>
                <span
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(entry.path);
                    setConfirmDelete(false);
                  }}
                  className="text-xs text-accent-red font-medium cursor-pointer"
                >
                  Confirm
                </span>
                <span
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmDelete(false);
                  }}
                  className="text-xs text-text-muted cursor-pointer ml-2"
                >
                  Cancel
                </span>
              </>
            ) : (
              <span
                onClick={(e) => {
                  e.stopPropagation();
                  setConfirmDelete(true);
                }}
                className="text-xs text-text-muted hover:text-accent-red transition-colors cursor-pointer"
              >
                Delete
              </span>
            )}
          </span>
        </div>
      </button>
    </div>
  );
}

export function KnowledgeView() {
  const activeView = useSessionStore((s) => s.activeView);
  const [entries, setEntries] = useState<KnowledgeEntry[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string>("");

  const loadEntries = useCallback(async () => {
    try {
      const result = await commands.listKnowledge();
      setEntries(result);
    } catch (err) {
      console.error("Failed to load knowledge:", err);
    }
  }, []);

  useEffect(() => {
    if (activeView === "knowledge") {
      loadEntries();
      setSelectedPath(null);
    }
  }, [activeView, loadEntries]);

  const handleSelect = useCallback(async (path: string) => {
    try {
      const content = await commands.readKnowledgeFile(path);
      setFileContent(content);
      setSelectedPath(path);
    } catch (err) {
      console.error("Failed to read knowledge file:", err);
    }
  }, []);

  const handleDelete = useCallback(
    async (path: string) => {
      try {
        await commands.deleteKnowledgeFile(path);
        setEntries(entries.filter((e) => e.path !== path));
        if (selectedPath === path) {
          setSelectedPath(null);
        }
      } catch (err) {
        console.error("Failed to delete knowledge file:", err);
      }
    },
    [entries, selectedPath],
  );

  const handleBack = useCallback(() => {
    setSelectedPath(null);
  }, []);

  // Group entries by category.
  const grouped: Record<string, KnowledgeEntry[]> = {};
  for (const entry of entries) {
    if (!grouped[entry.category]) {
      grouped[entry.category] = [];
    }
    grouped[entry.category].push(entry);
  }
  const categories = Object.keys(grouped).sort();

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex-1 overflow-y-auto">
        {selectedPath ? (
          /* Detail view */
          <div className="max-w-3xl w-full mx-auto px-6 py-6">
            <button
              onClick={handleBack}
              className="flex items-center gap-1.5 text-sm text-text-muted hover:text-text-primary transition-colors cursor-pointer mb-4"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M9 3L5 7L9 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              Back to knowledge
            </button>
            <p className="text-xs text-text-muted font-mono mb-4">
              {selectedPath}
            </p>
            <pre className="text-base text-text-primary whitespace-pre-wrap break-words leading-relaxed font-sans">
              {fileContent}
            </pre>
          </div>
        ) : entries.length === 0 ? (
          /* Empty state */
          <div className="flex flex-col items-center justify-center h-full text-text-muted px-6">
            <svg
              width="40"
              height="40"
              viewBox="0 0 32 32"
              fill="none"
              className="mb-4 opacity-50"
            >
              <path
                d="M6 4H20L26 10V28H6V4Z"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinejoin="round"
              />
              <path
                d="M20 4V10H26"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinejoin="round"
              />
              <path
                d="M10 16H22M10 20H22M10 24H18"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
            <p className="text-base text-text-secondary mb-1">
              No knowledge yet
            </p>
            <p className="text-sm text-text-muted text-center max-w-xs">
              Noah hasn't learned anything about your system yet.
              Knowledge will build up as you use the app.
            </p>
          </div>
        ) : (
          /* List view */
          <div className="max-w-3xl w-full mx-auto py-4">
            <div className="px-6 pb-4">
              <h1 className="text-2xl font-semibold text-text-primary">Knowledge</h1>
              <p className="text-sm text-text-muted mt-1">
                {entries.length} file{entries.length !== 1 ? "s" : ""} across{" "}
                {categories.length} categor{categories.length !== 1 ? "ies" : "y"}
              </p>
            </div>
            {categories.map((cat) => (
              <div key={cat}>
                <div className="px-6 py-2 text-xs font-semibold text-text-secondary">
                  {cat.charAt(0).toUpperCase() + cat.slice(1)}
                </div>
                {grouped[cat].map((entry) => (
                  <KnowledgeItem
                    key={entry.path}
                    entry={entry}
                    onSelect={handleSelect}
                    onDelete={handleDelete}
                  />
                ))}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
