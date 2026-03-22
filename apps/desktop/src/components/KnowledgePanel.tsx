import { useState, useEffect, useCallback, useMemo, useRef, type ReactElement } from "react";
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

// ── Playbook parser: extracts semantic structure from markdown ──

interface PlaybookSection {
  type: "trigger" | "quick-check" | "step" | "caveats" | "tools" | "section";
  title: string;
  stepNumber?: number;
  lines: string[];
}

interface ParsedPlaybook {
  title: string;
  emoji?: string;
  description?: string;
  platform?: string;
  author?: string;
  lastReviewed?: string;
  sections: PlaybookSection[];
}

function parsePlaybookContent(content: string, entry?: KnowledgeEntry | null): ParsedPlaybook {
  let body = content;
  let emoji = entry?.emoji ?? undefined;
  let description = entry?.description ?? undefined;
  let platform: string | undefined;
  let author: string | undefined;
  let lastReviewed: string | undefined;
  if (body.trimStart().startsWith("---")) {
    const after = body.trimStart().slice(3);
    const endIdx = after.indexOf("\n---");
    if (endIdx !== -1) {
      const yaml = after.slice(0, endIdx);
      for (const line of yaml.split("\n")) {
        const t = line.trim();
        if (t.startsWith("emoji:")) emoji = emoji ?? t.slice(6).trim();
        if (t.startsWith("description:")) description = description ?? t.slice(12).trim();
        if (t.startsWith("platform:")) platform = t.slice(9).trim();
        if (t.startsWith("author:")) author = t.slice(7).trim();
        if (t.startsWith("last_reviewed:")) lastReviewed = t.slice(14).trim();
      }
      body = after.slice(endIdx + 4);
    }
  }

  const lines = body.split("\n");
  let title = "";
  let i = 0;
  while (i < lines.length) {
    const t = lines[i].trim();
    if (t.startsWith("# ")) { title = t.slice(2); i++; break; }
    i++;
  }

  const sections: PlaybookSection[] = [];
  let currentSection: PlaybookSection | null = null;
  const flushSection = () => { if (currentSection) sections.push(currentSection); currentSection = null; };
  const classifyH2 = (heading: string): PlaybookSection["type"] => {
    const h = heading.toLowerCase();
    if (h.includes("when to activate") || h.includes("trigger")) return "trigger";
    if (h.includes("quick check") || h.includes("triage")) return "quick-check";
    if (h.includes("caveat") || h.includes("notes") || h.includes("warning")) return "caveats";
    if (h.includes("tools referenced") || h.includes("tools used")) return "tools";
    return "section";
  };

  while (i < lines.length) {
    const line = lines[i];
    const t = line.trim();
    const stepMatch = t.match(/^###\s+(\d+)\.\s+(.+)/);
    if (stepMatch) {
      flushSection();
      currentSection = { type: "step", title: stepMatch[2], stepNumber: parseInt(stepMatch[1], 10), lines: [] };
      i++; continue;
    }
    const stepAltMatch = t.match(/^##\s+Step\s+(\d+)[:.—\-]\s*(.+)/i);
    if (stepAltMatch) {
      flushSection();
      currentSection = { type: "step", title: stepAltMatch[2], stepNumber: parseInt(stepAltMatch[1], 10), lines: [] };
      i++; continue;
    }
    if (t.startsWith("## ")) {
      flushSection();
      currentSection = { type: classifyH2(t.slice(3)), title: t.slice(3), lines: [] };
      i++; continue;
    }
    if (currentSection) currentSection.lines.push(line);
    i++;
  }
  flushSection();
  return { title, emoji, description, platform, author, lastReviewed, sections };
}

// ── Inline markdown renderer ──

function PlaybookInline({ text }: { text: string }) {
  const parts = text.split(/(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\([^)]+\))/g);
  return (
    <>
      {parts.map((part, i) => {
        const bold = part.match(/^\*\*([^*]+)\*\*$/);
        if (bold) return <strong key={i} className="font-semibold">{bold[1]}</strong>;
        const code = part.match(/^`([^`]+)`$/);
        if (code) return <code key={i} className="px-1.5 py-0.5 rounded bg-accent-blue/8 text-accent-blue text-[13px] font-mono">{code[1]}</code>;
        const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
        if (link) return <a key={i} href={link[2]} target="_blank" rel="noreferrer" className="text-accent-blue underline underline-offset-2">{link[1]}</a>;
        return <span key={i}>{part}</span>;
      })}
    </>
  );
}

function PlaybookBody({ lines }: { lines: string[] }) {
  const blocks: ReactElement[] = [];
  let idx = 0;
  while (idx < lines.length) {
    const line = lines[idx];
    const t = line.trim();
    if (!t) { idx++; continue; }
    if (t.startsWith("```") || t.startsWith("~~~")) {
      const fence = t.slice(0, 3);
      const codeLines: string[] = [];
      idx++;
      while (idx < lines.length && !lines[idx].trim().startsWith(fence)) { codeLines.push(lines[idx]); idx++; }
      blocks.push(<pre key={`code-${idx}`} className="bg-bg-tertiary/50 border border-border-primary rounded-lg p-3 overflow-x-auto text-sm text-text-primary font-mono my-2">{codeLines.join("\n")}</pre>);
      idx++; continue;
    }
    if (t.startsWith("> ")) {
      const quoteLines: string[] = [];
      while (idx < lines.length && lines[idx].trim().startsWith("> ")) { quoteLines.push(lines[idx].trim().slice(2)); idx++; }
      blocks.push(<div key={`bq-${idx}`} className="border-l-2 border-accent-blue/30 pl-3 py-1 my-2 text-sm text-text-secondary italic"><PlaybookInline text={quoteLines.join(" ")} /></div>);
      continue;
    }
    if (t.startsWith("- ") || t.startsWith("* ")) {
      const items: string[] = [];
      while (idx < lines.length) {
        const l = lines[idx].trim();
        if (l.startsWith("- ") || l.startsWith("* ")) { items.push(l.slice(2)); idx++; }
        else if (l.startsWith("  ") && items.length > 0) { items[items.length - 1] += " " + l.trim(); idx++; }
        else break;
      }
      blocks.push(
        <ul key={`ul-${idx}`} className="space-y-1.5 my-2">
          {items.map((item, j) => (
            <li key={j} className="flex gap-2 text-sm text-text-primary leading-relaxed">
              <span className="text-text-muted mt-1 flex-shrink-0">•</span>
              <span><PlaybookInline text={item} /></span>
            </li>
          ))}
        </ul>,
      );
      continue;
    }
    const para: string[] = [t];
    idx++;
    while (idx < lines.length) {
      const next = lines[idx].trim();
      if (!next || next.startsWith("#") || next.startsWith("- ") || next.startsWith("* ") || next.startsWith("```") || next.startsWith("~~~") || next.startsWith("> ")) break;
      para.push(next); idx++;
    }
    blocks.push(<p key={`p-${idx}`} className="text-sm text-text-primary leading-relaxed my-1.5"><PlaybookInline text={para.join(" ")} /></p>);
  }
  return <>{blocks}</>;
}

// ── PlaybookView: renders a playbook as a structured artifact ──

function PlaybookView({ content, entry }: { content: string; entry: KnowledgeEntry }) {
  const pb = parsePlaybookContent(content, entry);
  const steps = pb.sections.filter((s) => s.type === "step");
  const trigger = pb.sections.find((s) => s.type === "trigger");
  const quickCheck = pb.sections.find((s) => s.type === "quick-check");
  const caveats = pb.sections.find((s) => s.type === "caveats");
  const otherSections = pb.sections.filter(
    (s) => s.type === "section" && !s.title.toLowerCase().includes("fix path"),
  );
  const platformLabel: Record<string, string> = { macos: "macOS", windows: "Windows", linux: "Linux", all: "All platforms" };

  return (
    <div className="space-y-5">
      {/* Hero */}
      <div className="rounded-2xl border border-border-primary bg-gradient-to-b from-bg-secondary/80 to-bg-primary p-6">
        <div className="flex items-start gap-4">
          {pb.emoji && (
            <div className="w-14 h-14 rounded-xl bg-bg-tertiary/60 border border-border-primary flex items-center justify-center text-2xl flex-shrink-0">
              {pb.emoji}
            </div>
          )}
          <div className="flex-1 min-w-0">
            <h1 className="text-xl font-semibold text-text-primary">{pb.title}</h1>
            {pb.description && <p className="text-sm text-text-secondary mt-1">{pb.description}</p>}
            <div className="flex items-center gap-2 mt-3 flex-wrap">
              {pb.platform && <span className="inline-flex items-center text-[11px] text-text-muted bg-bg-tertiary/60 rounded-full px-2 py-0.5">{platformLabel[pb.platform] ?? pb.platform}</span>}
              {steps.length > 0 && <span className="inline-flex items-center text-[11px] text-text-muted bg-bg-tertiary/60 rounded-full px-2 py-0.5">{steps.length} step{steps.length !== 1 ? "s" : ""}</span>}
              {pb.author && <span className="inline-flex items-center text-[11px] text-text-muted bg-bg-tertiary/60 rounded-full px-2 py-0.5">{pb.author}</span>}
              {pb.lastReviewed && <span className="inline-flex items-center text-[11px] text-text-muted bg-bg-tertiary/60 rounded-full px-2 py-0.5">reviewed {pb.lastReviewed}</span>}
            </div>
          </div>
        </div>
      </div>

      {trigger && trigger.lines.some((l) => l.trim()) && (
        <div className="rounded-xl border border-accent-blue/15 bg-accent-blue/4 p-4">
          <div className="flex items-center gap-2 mb-2">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="text-accent-blue"><path d="M7 1L8.5 5H13L9.5 8L10.5 13L7 10L3.5 13L4.5 8L1 5H5.5L7 1Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/></svg>
            <span className="text-xs font-semibold text-accent-blue uppercase tracking-wider">Activates when</span>
          </div>
          <div className="text-sm text-text-primary"><PlaybookBody lines={trigger.lines} /></div>
        </div>
      )}

      {quickCheck && quickCheck.lines.some((l) => l.trim()) && (
        <div className="rounded-xl border border-border-primary bg-bg-secondary/40 p-4">
          <div className="flex items-center gap-2 mb-2">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="text-accent-purple"><circle cx="7" cy="7" r="5.5" stroke="currentColor" strokeWidth="1.2"/><path d="M5 7L6.5 8.5L9 5.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/></svg>
            <span className="text-xs font-semibold text-accent-purple uppercase tracking-wider">Quick check</span>
          </div>
          <div className="text-sm text-text-primary"><PlaybookBody lines={quickCheck.lines} /></div>
        </div>
      )}

      {steps.length > 0 && (
        <div className="space-y-0">
          <div className="text-xs font-semibold text-text-muted uppercase tracking-wider mb-3 px-1">Procedure</div>
          {steps.map((step, i) => (
            <div key={i} className="flex gap-3">
              <div className="flex flex-col items-center flex-shrink-0">
                <div className="w-7 h-7 rounded-full bg-bg-tertiary border border-border-primary flex items-center justify-center text-xs font-semibold text-text-secondary">{step.stepNumber ?? i + 1}</div>
                {i < steps.length - 1 && <div className="w-px flex-1 bg-border-primary my-1" />}
              </div>
              <div className={`flex-1 min-w-0 ${i < steps.length - 1 ? "pb-4" : "pb-1"}`}>
                <div className="text-sm font-semibold text-text-primary mb-1">{step.title}</div>
                <div className="text-text-secondary"><PlaybookBody lines={step.lines} /></div>
              </div>
            </div>
          ))}
        </div>
      )}

      {caveats && caveats.lines.some((l) => l.trim()) && (
        <div className="rounded-xl border border-accent-yellow/20 bg-accent-yellow/4 p-4">
          <div className="flex items-center gap-2 mb-2">
            <span className="text-sm">⚠</span>
            <span className="text-xs font-semibold text-text-secondary uppercase tracking-wider">Caveats</span>
          </div>
          <div className="text-sm text-text-primary"><PlaybookBody lines={caveats.lines} /></div>
        </div>
      )}

      {otherSections.map((sec, i) => (
        <div key={i} className="rounded-xl border border-border-primary bg-bg-secondary/30 p-4">
          <div className="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-2">{sec.title}</div>
          <div className="text-sm text-text-primary"><PlaybookBody lines={sec.lines} /></div>
        </div>
      ))}
    </div>
  );
}

// ── Search icon ──

function SearchIcon({ className }: { className?: string }) {
  return (
    <svg width="15" height="15" viewBox="0 0 15 15" fill="none" className={className}>
      <circle cx="6.5" cy="6.5" r="4.5" stroke="currentColor" strokeWidth="1.4" />
      <path d="M10 10L13.5 13.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

// ── Playbook list item (replaces the old card grid with a tighter list) ──

function PlaybookRow({
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
    <div className="group/row flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-bg-tertiary/30 transition-colors animate-fade-in">
      <button onClick={() => onSelect(entry)} className="flex items-center gap-3 flex-1 min-w-0 text-left cursor-pointer">
        <div className="w-10 h-10 rounded-lg bg-bg-tertiary/50 border border-border-primary/60 flex items-center justify-center text-lg flex-shrink-0">
          {icon}
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-[13px] font-medium text-text-primary truncate">{entry.title}</p>
          {entry.description && (
            <p className="text-[11px] text-text-muted truncate mt-0.5">{entry.description}</p>
          )}
        </div>
      </button>
      <div className="flex items-center gap-1 opacity-0 group-hover/row:opacity-100 transition-opacity duration-150 flex-shrink-0">
        {confirmDelete ? (
          <>
            <button
              onClick={() => { onDelete(entry.path); setConfirmDelete(false); }}
              className="px-2 py-1 rounded text-[11px] font-medium text-accent-red bg-accent-red/10 hover:bg-accent-red/20 transition-colors cursor-pointer"
            >
              {t("knowledgePanel.confirm")}
            </button>
            <button
              onClick={() => setConfirmDelete(false)}
              className="px-2 py-1 rounded text-[11px] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
            >
              {t("knowledgePanel.cancel")}
            </button>
          </>
        ) : (
          <button
            onClick={() => setConfirmDelete(true)}
            className="p-1 rounded text-text-muted hover:text-accent-red transition-colors cursor-pointer"
            title={t("knowledgePanel.delete")}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M3 4H11M5.5 4V3C5.5 2.45 5.95 2 6.5 2H7.5C8.05 2 8.5 2.45 8.5 3V4M4 4L4.5 11.5C4.5 12.05 4.95 12.5 5.5 12.5H8.5C9.05 12.5 9.5 12.05 9.5 11.5L10 4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}

// ── Card view for grid display ──

function PlaybookCard({
  entry,
  icon,
  onSelect,
}: {
  entry: KnowledgeEntry;
  icon: string;
  onSelect: (entry: KnowledgeEntry) => void;
}) {
  return (
    <button
      onClick={() => onSelect(entry)}
      className="group/card flex flex-col rounded-xl border border-border-primary/60 bg-bg-secondary/30 hover:bg-bg-tertiary/25 hover:border-border-primary transition-all cursor-pointer text-left animate-fade-in overflow-hidden"
    >
      <div className="h-20 bg-gradient-to-br from-bg-tertiary/40 to-bg-tertiary/10 flex items-center justify-center text-3xl border-b border-border-primary/40 group-hover/card:from-accent-blue/5 group-hover/card:to-transparent transition-colors">
        {icon}
      </div>
      <div className="px-3.5 py-3 flex-1 flex flex-col">
        <p className="text-[13px] font-medium text-text-primary line-clamp-2 leading-snug">{entry.title}</p>
        {entry.description && (
          <p className="text-[11px] text-text-muted mt-1.5 line-clamp-2 leading-relaxed">{entry.description}</p>
        )}
      </div>
    </button>
  );
}

// ── Main view ──

type ViewMode = "grid" | "list";

export function KnowledgeView({ onNewKnowledge }: { onNewKnowledge?: () => void } = {}) {
  const { t } = useLocale();
  const activeView = useSessionStore((s) => s.activeView);
  const [entries, setEntries] = useState<KnowledgeEntry[]>([]);
  const [selectedEntry, setSelectedEntry] = useState<KnowledgeEntry | null>(null);
  const [fileContent, setFileContent] = useState<string>("");
  const [activeTab, setActiveTab] = useState<KnowledgeTab>("builtin");
  const [searchQuery, setSearchQuery] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const searchRef = useRef<HTMLInputElement>(null);

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
      setSearchQuery("");
    }
  }, [activeView, loadEntries]);

  // Cmd+F / Ctrl+F focuses search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f" && activeView === "knowledge" && !selectedEntry) {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [activeView, selectedEntry]);

  const handleSelect = useCallback(async (entry: KnowledgeEntry) => {
    try {
      const content = await commands.readKnowledgeFile(entry.path);
      setFileContent(content);
      setSelectedEntry(entry);
    } catch (err) {
      console.error("Failed to read knowledge file:", err);
    }
  }, []);

  const handleDelete = useCallback(async (path: string) => {
    try {
      await commands.deleteKnowledgeFile(path);
      setEntries((prev) => prev.filter((e) => e.path !== path));
      setSelectedEntry((current) => (current?.path === path ? null : current));
    } catch (err) {
      console.error("Failed to delete knowledge file:", err);
    }
  }, []);

  const visibleEntries = useMemo(() => {
    const isTopLevel = (entry: KnowledgeEntry) => {
      const parts = entry.path.split("/");
      if (parts.length === 2) return true;
      if (parts.length === 3 && entry.filename === "playbook.md") return true;
      return false;
    };

    let filtered: KnowledgeEntry[];
    if (activeTab === "builtin") {
      filtered = entries.filter(
        (entry) => entry.category === "playbooks" && (entry.playbook_type ?? "system") === "system" && isTopLevel(entry),
      );
    } else if (activeTab === "yours") {
      filtered = entries.filter(
        (entry) => entry.category === "playbooks" && (entry.playbook_type ?? "system") !== "system" && isTopLevel(entry),
      );
    } else {
      filtered = entries.filter((entry) => entry.category !== "playbooks");
    }

    // Apply search filter
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase().trim();
      filtered = filtered.filter(
        (entry) =>
          (entry.title || "").toLowerCase().includes(q) ||
          (entry.description || "").toLowerCase().includes(q) ||
          entry.filename.toLowerCase().includes(q),
      );
    }

    return filtered;
  }, [activeTab, entries, searchQuery]);

  const getIcon = (entry: KnowledgeEntry) =>
    entry.emoji ??
    (entry.category === "playbooks"
      ? (entry.playbook_type ?? "system") === "system" ? "🧭" : "📘"
      : "🧠");

  // ── Detail view ──

  if (selectedEntry) {
    return (
      <div className="flex flex-col flex-1 min-h-0">
        <div className="flex-1 overflow-y-auto">
          <div className="max-w-3xl w-full mx-auto px-6 py-6">
            <button
              onClick={() => setSelectedEntry(null)}
              className="flex items-center gap-1.5 text-sm text-text-muted hover:text-text-primary transition-colors cursor-pointer mb-5"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M9 3L5 7L9 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              {t("knowledgePanel.backToKnowledge")}
            </button>
            {selectedEntry.category === "playbooks" ? (
              <PlaybookView content={fileContent} entry={selectedEntry} />
            ) : (
              <>
                <p className="text-xs text-text-muted font-mono mb-4">{selectedEntry.path}</p>
                <pre className="text-base text-text-primary whitespace-pre-wrap break-words leading-relaxed font-sans">
                  {fileContent}
                </pre>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ── Empty state ──

  if (entries.length === 0) {
    return (
      <div className="flex flex-col flex-1 min-h-0">
        <div className="flex flex-col items-center justify-center h-full text-text-muted px-6">
          <svg width="40" height="40" viewBox="0 0 32 32" fill="none" className="mb-4 opacity-50">
            <path d="M6 4H20L26 10V28H6V4Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
            <path d="M20 4V10H26" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
            <path d="M10 16H22M10 20H22M10 24H18" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
          <p className="text-base text-text-secondary mb-1">{t("knowledgePanel.noKnowledge")}</p>
          <p className="text-sm text-text-muted text-center max-w-xs">{t("knowledgePanel.noKnowledgeDesc")}</p>
          <button
            onClick={() => onNewKnowledge?.()}
            className="mt-4 px-4 py-2 rounded-lg border border-border-primary text-sm text-text-primary hover:bg-bg-tertiary/40 transition-colors cursor-pointer"
          >
            {t("knowledgePanel.newKnowledge")}
          </button>
        </div>
      </div>
    );
  }

  // ── Catalog view ──

  const tabDefs = [
    { key: "builtin" as const, label: t("knowledgePanel.builtin") },
    { key: "yours" as const, label: t("knowledgePanel.yourPlaybooks") },
    { key: "learned" as const, label: t("knowledgePanel.noahLearned") },
  ];

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* ── Sticky header with search ── */}
      <div className="flex-shrink-0 px-6 pt-5 pb-0">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h1 className="text-xl font-semibold text-text-primary tracking-tight">{t("knowledgePanel.title")}</h1>
            <p className="text-xs text-text-muted mt-0.5">{t("knowledgePanel.fileCount", { count: entries.length })}</p>
          </div>
          <button
            onClick={() => onNewKnowledge?.()}
            className="px-3 py-1.5 rounded-lg border border-border-primary text-[13px] text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/40 transition-colors cursor-pointer"
          >
            {t("knowledgePanel.newKnowledge")}
          </button>
        </div>

        {/* Search bar */}
        <div className="relative mb-4">
          <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted" />
          <input
            ref={searchRef}
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search playbooks..."
            className="w-full pl-9 pr-3 py-2 rounded-lg bg-bg-input border border-border-primary text-[13px] text-text-primary placeholder-text-muted outline-none focus:border-accent-blue/40 transition-colors"
            onKeyDown={(e) => {
              if (e.key === "Escape") { setSearchQuery(""); searchRef.current?.blur(); }
            }}
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery("")}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded text-text-muted hover:text-text-primary transition-colors cursor-pointer"
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path d="M3 3L9 9M9 3L3 9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
              </svg>
            </button>
          )}
        </div>

        {/* Tabs + view toggle */}
        <div className="flex items-center justify-between border-b border-border-primary">
          <div className="flex items-center gap-0.5">
            {tabDefs.map((tab) => {
              const active = tab.key === activeTab;
              return (
                <button
                  key={tab.key}
                  onClick={() => setActiveTab(tab.key)}
                  className={`px-3 py-2 text-[13px] border-b-2 -mb-px cursor-pointer transition-colors ${
                    active
                      ? "text-text-primary border-accent-blue font-medium"
                      : "text-text-muted border-transparent hover:text-text-secondary"
                  }`}
                >
                  {tab.label}
                </button>
              );
            })}
          </div>
          {/* View toggle */}
          <div className="flex items-center gap-0.5 mr-1 mb-px">
            <button
              onClick={() => setViewMode("grid")}
              className={`p-1.5 rounded transition-colors cursor-pointer ${viewMode === "grid" ? "text-text-primary bg-bg-tertiary/50" : "text-text-muted hover:text-text-secondary"}`}
              title="Grid view"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <rect x="1" y="1" width="5" height="5" rx="1" stroke="currentColor" strokeWidth="1.2" />
                <rect x="8" y="1" width="5" height="5" rx="1" stroke="currentColor" strokeWidth="1.2" />
                <rect x="1" y="8" width="5" height="5" rx="1" stroke="currentColor" strokeWidth="1.2" />
                <rect x="8" y="8" width="5" height="5" rx="1" stroke="currentColor" strokeWidth="1.2" />
              </svg>
            </button>
            <button
              onClick={() => setViewMode("list")}
              className={`p-1.5 rounded transition-colors cursor-pointer ${viewMode === "list" ? "text-text-primary bg-bg-tertiary/50" : "text-text-muted hover:text-text-secondary"}`}
              title="List view"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M1 3H13M1 7H13M1 11H13" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      {/* ── Scrollable content ── */}
      <div className="flex-1 overflow-y-auto px-6 pt-4 pb-6">
        {visibleEntries.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            {searchQuery ? (
              <>
                <SearchIcon className="text-text-muted mb-3 opacity-40" />
                <p className="text-sm text-text-muted">No results for &ldquo;{searchQuery}&rdquo;</p>
                <button
                  onClick={() => setSearchQuery("")}
                  className="text-xs text-accent-blue mt-2 hover:underline cursor-pointer"
                >
                  Clear search
                </button>
              </>
            ) : (
              <p className="text-sm text-text-muted">{t("knowledgePanel.noEntries")}</p>
            )}
          </div>
        ) : viewMode === "grid" ? (
          <div className="grid grid-cols-2 xl:grid-cols-3 gap-3">
            {visibleEntries.map((entry) => (
              <PlaybookCard
                key={entry.path}
                entry={{ ...entry, title: entry.title || toTitleCase(entry.filename.replace(".md", "")) }}
                icon={getIcon(entry)}
                onSelect={handleSelect}
              />
            ))}
          </div>
        ) : (
          <div className="space-y-0.5">
            {visibleEntries.map((entry) => (
              <PlaybookRow
                key={entry.path}
                entry={{ ...entry, title: entry.title || toTitleCase(entry.filename.replace(".md", "")) }}
                icon={getIcon(entry)}
                onSelect={handleSelect}
                onDelete={handleDelete}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
