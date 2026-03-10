import { useState, useEffect, useCallback, useMemo, type ReactElement } from "react";
import { useSessionStore } from "../stores/sessionStore";
import * as commands from "../lib/tauri-commands";
import type { KnowledgeEntry } from "../lib/tauri-commands";
import { useLocale } from "../i18n";

type KnowledgeTab = "builtin" | "yours" | "learned";

function toTitleCase(value: string): string {
  return value
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function MarkdownView({ content }: { content: string }) {
  const lines = content.split("\n");
  const blocks: ReactElement[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    if (line.trim().startsWith("```") || line.trim().startsWith("~~~")) {
      const fence = line.trim().slice(0, 3);
      const codeLines: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].trim().startsWith(fence)) {
        codeLines.push(lines[i]);
        i += 1;
      }
      blocks.push(
        <pre
          key={`code-${i}`}
          className="bg-bg-secondary/70 border border-border-primary rounded-lg p-4 overflow-x-auto text-sm text-text-primary font-mono"
        >
          {codeLines.join("\n")}
        </pre>,
      );
      i += 1;
      continue;
    }

    const trimmed = line.trim();
    if (!trimmed) {
      i += 1;
      continue;
    }

    if (trimmed.startsWith("### ")) {
      blocks.push(
        <h3 key={`h3-${i}`} className="text-lg font-semibold text-text-primary mt-6 mb-2">
          {trimmed.slice(4)}
        </h3>,
      );
      i += 1;
      continue;
    }

    if (trimmed.startsWith("## ")) {
      blocks.push(
        <h2 key={`h2-${i}`} className="text-xl font-semibold text-text-primary mt-6 mb-2">
          {trimmed.slice(3)}
        </h2>,
      );
      i += 1;
      continue;
    }

    if (trimmed.startsWith("# ")) {
      blocks.push(
        <h1 key={`h1-${i}`} className="text-2xl font-semibold text-text-primary mt-6 mb-3">
          {trimmed.slice(2)}
        </h1>,
      );
      i += 1;
      continue;
    }

    if (trimmed.startsWith("- ") || trimmed.startsWith("* ")) {
      const items: string[] = [];
      while (i < lines.length) {
        const l = lines[i].trim();
        if (l.startsWith("- ") || l.startsWith("* ")) {
          items.push(l.slice(2));
          i += 1;
          continue;
        }
        break;
      }
      blocks.push(
        <ul key={`ul-${i}`} className="list-disc pl-6 space-y-1 text-text-primary">
          {items.map((item, idx) => (
            <li key={idx}>{item}</li>
          ))}
        </ul>,
      );
      continue;
    }

    const paragraph: string[] = [trimmed];
    i += 1;
    while (i < lines.length) {
      const next = lines[i].trim();
      if (
        !next ||
        next.startsWith("#") ||
        next.startsWith("- ") ||
        next.startsWith("* ") ||
        next.startsWith("```") ||
        next.startsWith("~~~")
      ) {
        break;
      }
      paragraph.push(next);
      i += 1;
    }

    blocks.push(
      <p key={`p-${i}`} className="text-base text-text-primary leading-relaxed">
        {paragraph.join(" ")}
      </p>,
    );
  }

  return <div className="space-y-3">{blocks}</div>;
}

function KnowledgeCard({
  entry,
  icon,
  onSelect,
  onDelete,
}: {
  entry: KnowledgeEntry;
  icon: string;
  onSelect: (entry: KnowledgeEntry) => void;
  onDelete: (path: string) => void;
}) {
  const { t } = useLocale();
  const [confirmDelete, setConfirmDelete] = useState(false);

  return (
    <div className="rounded-xl border border-border-primary bg-bg-secondary/50 p-4 hover:bg-bg-tertiary/20 transition-colors">
      <button onClick={() => onSelect(entry)} className="w-full text-left cursor-pointer">
        <div className="h-24 rounded-lg bg-bg-tertiary/50 border border-border-primary/60 flex items-center justify-center text-3xl mb-3">
          {icon}
        </div>
        <p className="text-base text-text-primary font-medium line-clamp-2">{entry.title}</p>
        {entry.description ? (
          <p className="text-xs text-text-muted mt-1 line-clamp-2">{entry.description}</p>
        ) : (
          <p className="text-xs text-text-muted font-mono mt-1 truncate">{entry.path}</p>
        )}
      </button>
      <div className="flex justify-end mt-3 text-xs">
        {confirmDelete ? (
          <>
            <button
              onClick={() => {
                onDelete(entry.path);
                setConfirmDelete(false);
              }}
              className="text-accent-red font-medium cursor-pointer"
            >
              {t("knowledgePanel.confirm")}
            </button>
            <button
              onClick={() => setConfirmDelete(false)}
              className="text-text-muted ml-2 cursor-pointer"
            >
              {t("knowledgePanel.cancel")}
            </button>
          </>
        ) : (
          <button
            onClick={() => setConfirmDelete(true)}
            className="text-text-muted hover:text-accent-red transition-colors cursor-pointer"
          >
            {t("knowledgePanel.delete")}
          </button>
        )}
      </div>
    </div>
  );
}

export function KnowledgeView({ onNewKnowledge }: { onNewKnowledge?: () => void } = {}) {
  const { t } = useLocale();
  const activeView = useSessionStore((s) => s.activeView);
  const [entries, setEntries] = useState<KnowledgeEntry[]>([]);
  const [selectedEntry, setSelectedEntry] = useState<KnowledgeEntry | null>(null);
  const [fileContent, setFileContent] = useState<string>("");
  const [activeTab, setActiveTab] = useState<KnowledgeTab>("builtin");

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
      setSelectedEntry(null);
      setActiveTab("builtin");
    }
  }, [activeView, loadEntries]);

  const handleSelect = useCallback(async (entry: KnowledgeEntry) => {
    try {
      const content = await commands.readKnowledgeFile(entry.path);
      setFileContent(content);
      setSelectedEntry(entry);
    } catch (err) {
      console.error("Failed to read knowledge file:", err);
    }
  }, []);

  const handleDelete = useCallback(
    async (path: string) => {
      try {
        await commands.deleteKnowledgeFile(path);
        setEntries((prev) => prev.filter((e) => e.path !== path));
        setSelectedEntry((current) => (current?.path === path ? null : current));
      } catch (err) {
        console.error("Failed to delete knowledge file:", err);
      }
    },
    [],
  );

  const visibleEntries = useMemo(() => {
    // For playbook tabs, only show top-level entries:
    // - flat playbooks: playbooks/X.md (path has exactly one slash)
    // - folder playbooks: playbooks/X/playbook.md only (not sub-modules)
    const isTopLevel = (entry: KnowledgeEntry) => {
      const parts = entry.path.split("/");
      // playbooks/name.md → 2 parts
      if (parts.length === 2) return true;
      // playbooks/name/playbook.md → 3 parts, filename is playbook.md
      if (parts.length === 3 && entry.filename === "playbook.md") return true;
      return false;
    };

    if (activeTab === "builtin") {
      return entries.filter(
        (entry) =>
          entry.category === "playbooks" &&
          (entry.playbook_type ?? "system") === "system" &&
          isTopLevel(entry),
      );
    }

    if (activeTab === "yours") {
      return entries.filter(
        (entry) =>
          entry.category === "playbooks" &&
          (entry.playbook_type ?? "system") !== "system" &&
          isTopLevel(entry),
      );
    }

    return entries.filter((entry) => entry.category !== "playbooks");
  }, [activeTab, entries]);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex-1 overflow-y-auto">
        {selectedEntry ? (
          <div className="max-w-4xl w-full mx-auto px-6 py-6">
            <button
              onClick={() => setSelectedEntry(null)}
              className="flex items-center gap-1.5 text-sm text-text-muted hover:text-text-primary transition-colors cursor-pointer mb-4"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M9 3L5 7L9 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              {t("knowledgePanel.backToKnowledge")}
            </button>
            <p className="text-xs text-text-muted font-mono mb-4">{selectedEntry.path}</p>
            {selectedEntry.category === "playbooks" ? (
              <MarkdownView content={fileContent} />
            ) : (
              <pre className="text-base text-text-primary whitespace-pre-wrap break-words leading-relaxed font-sans">
                {fileContent}
              </pre>
            )}
          </div>
        ) : entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-text-muted px-6">
            <svg width="40" height="40" viewBox="0 0 32 32" fill="none" className="mb-4 opacity-50">
              <path d="M6 4H20L26 10V28H6V4Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
              <path d="M20 4V10H26" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
              <path d="M10 16H22M10 20H22M10 24H18" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
            <p className="text-base text-text-secondary mb-1">{t("knowledgePanel.noKnowledge")}</p>
            <p className="text-sm text-text-muted text-center max-w-xs">
              {t("knowledgePanel.noKnowledgeDesc")}
            </p>
          </div>
        ) : (
          <div className="max-w-6xl w-full mx-auto py-4 px-6">
            <div className="flex items-center justify-between pb-4">
              <div>
                <h1 className="text-2xl font-semibold text-text-primary">{t("knowledgePanel.title")}</h1>
                <p className="text-sm text-text-muted mt-1">{t("knowledgePanel.fileCount", { count: entries.length })}</p>
              </div>
              <button
                onClick={() => onNewKnowledge?.()}
                className="px-4 py-2 rounded-lg border border-border-primary text-sm text-text-primary hover:bg-bg-tertiary/40 transition-colors cursor-pointer"
              >
                {t("knowledgePanel.newKnowledge")}
              </button>
            </div>

            <div className="flex items-center gap-2 border-b border-border-primary mb-4">
              {[
                { key: "builtin", label: t("knowledgePanel.builtin") },
                { key: "yours", label: t("knowledgePanel.yourPlaybooks") },
                { key: "learned", label: t("knowledgePanel.noahLearned") },
              ].map((tab) => {
                const key = tab.key as KnowledgeTab;
                const active = key === activeTab;
                return (
                  <button
                    key={tab.key}
                    onClick={() => setActiveTab(key)}
                    className={`px-3 py-2 text-sm border-b-2 -mb-px cursor-pointer transition-colors ${
                      active
                        ? "text-text-primary border-text-primary"
                        : "text-text-muted border-transparent hover:text-text-primary"
                    }`}
                  >
                    {tab.label}
                  </button>
                );
              })}
            </div>

            {visibleEntries.length === 0 ? (
              <p className="text-sm text-text-muted py-8">{t("knowledgePanel.noEntries")}</p>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-4 pb-4">
                {visibleEntries.map((entry) => {
                  const icon =
                    entry.emoji ??
                    (entry.category === "playbooks"
                      ? (entry.playbook_type ?? "system") === "system"
                        ? "🧭"
                        : "📘"
                      : "🧠");
                  return (
                    <KnowledgeCard
                      key={entry.path}
                      entry={{ ...entry, title: entry.title || toTitleCase(entry.filename.replace(".md", "")) }}
                      icon={icon}
                      onSelect={handleSelect}
                      onDelete={handleDelete}
                    />
                  );
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
