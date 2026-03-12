import { useState, useRef, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";
import type { Message, ToolCall } from "../stores/chatStore";
import { useAgent } from "../hooks/useAgent";
import { parseResponse } from "../lib/parseResponse";
import type { AssistantQuestion, AssistantUiPayload } from "../lib/tauri-commands";
import * as commands from "../lib/tauri-commands";
import { NoahIcon } from "./NoahIcon";
import { useLocale } from "../i18n";
import QRCode from "qrcode";

const showToolCalls = import.meta.env.DEV;

function QrCodeImage({ data }: { data: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    QRCode.toDataURL(data, { width: 200, margin: 2 }).then(setSrc).catch(() => setSrc(null));
  }, [data]);
  if (!src) return null;
  return (
    <div className="flex justify-center py-3">
      <img src={src} alt="QR Code" className="rounded-lg" width={200} height={200} />
    </div>
  );
}

function formatTime(ts: number) {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

// ── Text helpers ──

const URL_PATTERN = /((?:https?:\/\/)?(?:[a-zA-Z0-9-]+\.)+[a-zA-Z]{2,}(?:\/[^\s)]*)?)/g;

function normalizeSpaText(input: string): string {
  return input
    .replace(/\\n/g, "\n")           // unescape literal \n sequences
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function humanizeActionLabel(label: string, actionType: string | undefined, t: (key: string) => string): string {
  const raw = (label || "").trim();
  const type = (actionType || "").trim();

  const mapByType: Record<string, string> = {
    RUN_STEP: t("chat.continue"),
    WAIT_FOR_USER: t("chat.iveDoneThis"),
  };

  if (!raw) return mapByType[type] || t("chat.continue");

  const looksEnum = /^[A-Z0-9_]+$/.test(raw);
  if (looksEnum) {
    if (mapByType[raw]) return mapByType[raw];
    return raw
      .toLowerCase()
      .split("_")
      .map((w) => (w ? w[0].toUpperCase() + w.slice(1) : w))
      .join(" ");
  }

  return raw;
}

// ── Progress Bar (playbook step indicator) ──

function ProgressBar({ step, total, label }: { step: number; total: number; label: string }) {
  const { t } = useLocale();
  const pct = Math.min(100, Math.round((step / total) * 100));
  return (
    <div className="mb-3">
      <div className="flex items-center justify-between text-xs text-text-muted mb-1.5">
        <span className="font-medium text-text-secondary">{t("chat.stepOf", { step, total })}</span>
        <span>{label}</span>
      </div>
      <div className="w-full h-1.5 rounded-full bg-bg-tertiary overflow-hidden">
        <div
          className="h-full rounded-full bg-accent-blue transition-all duration-500 ease-out"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

function LinkedText({ text }: { text: string }) {
  const parts = text.split(URL_PATTERN);
  return (
    <>
      {parts.map((part, i) => {
        if (!part) return null;
        if (!URL_PATTERN.test(part)) {
          URL_PATTERN.lastIndex = 0;
          return <span key={i}>{part}</span>;
        }
        URL_PATTERN.lastIndex = 0;
        const href = part.startsWith("http://") || part.startsWith("https://")
          ? part
          : `https://${part}`;
        return (
          <a
            key={i}
            href={href}
            target="_blank"
            rel="noreferrer"
            className="underline decoration-accent-blue/50 underline-offset-2 hover:text-accent-blue"
          >
            {part}
          </a>
        );
      })}
    </>
  );
}

function InlineMarkdown({ text }: { text: string }) {
  // Split on **bold**, [label](url), and `code` patterns
  const parts = text.split(/(\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\)|`[^`]+`)/g);
  return (
    <>
      {parts.map((part, i) => {
        const bold = part.match(/^\*\*([^*]+)\*\*$/);
        if (bold) {
          return <strong key={i} className="font-semibold">{bold[1]}</strong>;
        }
        const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
        if (link) {
          return (
            <a
              key={i}
              href={link[2]}
              target="_blank"
              rel="noreferrer"
              className="underline decoration-accent-blue/50 underline-offset-2 hover:text-accent-blue"
            >
              {link[1]}
            </a>
          );
        }
        const code = part.match(/^`([^`]+)`$/);
        if (code) {
          return <code key={i} className="px-1 py-0.5 rounded bg-bg-tertiary text-sm font-mono">{code[1]}</code>;
        }
        return <LinkedText key={i} text={part} />;
      })}
    </>
  );
}

function MarkdownSummary({ text }: { text: string }) {
  // Unescape literal \n sequences that may come from JSON-serialized content.
  const lines = text.replace(/\\n/g, "\n").split("\n");
  const blocks: React.ReactNode[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i].trim();
    if (!line) {
      i += 1;
      continue;
    }
    if (line.startsWith("### ") || line.startsWith("## ") || line.startsWith("# ")) {
      const level = line.startsWith("### ") ? "h3" : line.startsWith("## ") ? "h2" : "h1";
      const content = line.replace(/^#{1,3}\s+/, "");
      const cls = level === "h1"
        ? "text-lg font-semibold text-text-primary"
        : level === "h2"
          ? "text-base font-semibold text-text-primary"
          : "text-sm font-semibold text-text-primary";
      blocks.push(
        <div key={`h-${i}`} className={cls}>
          <InlineMarkdown text={content} />
        </div>,
      );
      i += 1;
      continue;
    }

    if (line.startsWith("- ") || line.startsWith("* ") || /^\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length) {
        const current = lines[i].trim();
        if (current.startsWith("- ") || current.startsWith("* ") || /^\d+\.\s+/.test(current)) {
          items.push(current.replace(/^(-|\*|\d+\.)\s+/, ""));
          i += 1;
          continue;
        }
        break;
      }
      blocks.push(
        <ul key={`ul-${i}`} className="list-disc pl-5 space-y-1">
          {items.map((item, idx) => (
            <li key={idx}>
              <InlineMarkdown text={item} />
            </li>
          ))}
        </ul>,
      );
      continue;
    }

    const paragraph: string[] = [line];
    i += 1;
    while (i < lines.length) {
      const next = lines[i].trim();
      if (!next || next.startsWith("#") || next.startsWith("- ") || next.startsWith("* ") || /^\d+\.\s+/.test(next)) {
        break;
      }
      paragraph.push(next);
      i += 1;
    }
    blocks.push(
      <p key={`p-${i}`} className="whitespace-pre-wrap break-words">
        <InlineMarkdown text={paragraph.join(" ")} />
      </p>,
    );
  }

  return <div className="space-y-2">{blocks}</div>;
}

// ── Tool Call Display ──

function ToolCallItem({ toolCall }: { toolCall: ToolCall }) {
  const [expanded, setExpanded] = useState(false);

  const statusColor = {
    pending: "text-status-pending",
    running: "text-status-running",
    completed: "text-accent-green",
    denied: "text-status-denied",
  }[toolCall.status];

  const statusIcon = {
    pending: "\u25CB",
    running: "\u25D4",
    completed: "\u2713",
    denied: "\u2715",
  }[toolCall.status];

  return (
    <div className="mt-2 rounded-md border border-border-primary bg-bg-primary/50 overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left cursor-pointer hover:bg-bg-tertiary/30 transition-colors"
      >
        <span className={`${statusColor} font-mono`}>{statusIcon}</span>
        <span className="font-mono text-accent-purple">{toolCall.name}</span>
        <span className="text-text-muted ml-auto">
          {expanded ? "\u25B4" : "\u25BE"}
        </span>
      </button>
      {expanded && (
        <div className="px-3 py-2 border-t border-border-primary text-xs space-y-2">
          <div>
            <span className="text-text-muted">Input:</span>
            <pre className="mt-1 p-2 rounded bg-bg-primary text-text-secondary font-mono text-[11px] overflow-x-auto whitespace-pre-wrap break-all">
              {JSON.stringify(toolCall.input, null, 2)}
            </pre>
          </div>
          {toolCall.result && (
            <div>
              <span className="text-text-muted">Result:</span>
              <pre className="mt-1 p-2 rounded bg-bg-primary text-text-secondary font-mono text-[11px] overflow-x-auto whitespace-pre-wrap break-all">
                {toolCall.result}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Actions Block (inline per-message) ──

const CHANGE_TOOL_KEYS: Record<string, string> = {
  mac_flush_dns: "changes.mac_flush_dns",
  mac_kill_process: "changes.mac_kill_process",
  mac_clear_caches: "changes.mac_clear_caches",
  mac_clear_app_cache: "changes.mac_clear_app_cache",
  mac_restart_cups: "changes.mac_restart_cups",
  mac_cancel_print_jobs: "changes.mac_cancel_print_jobs",
  mac_move_file: "changes.mac_move_file",
  win_flush_dns: "changes.win_flush_dns",
  win_kill_process: "changes.win_kill_process",
  win_clear_caches: "changes.win_clear_caches",
  win_clear_app_cache: "changes.win_clear_app_cache",
  win_restart_spooler: "changes.win_restart_spooler",
  win_cancel_print_jobs: "changes.win_cancel_print_jobs",
  win_move_file: "changes.win_move_file",
  win_restart_service: "changes.win_restart_service",
  write_knowledge: "changes.write_knowledge",
};

// Shell change patterns: regex → i18n key. Some have dynamic captures (Stopped $2, Opened $1)
// which can't be translated with captures, so we use the generic i18n key for those.
const SHELL_CHANGE_PATTERN_KEYS: [RegExp, string][] = [
  [/\bfind\b.*-exec\s+(mv|cp)\b/, "shellChanges.organized"],
  [/\bmkdir\b/, "shellChanges.createdFolders"],
  [/\b(cp|rsync)\b/, "shellChanges.copiedFiles"],
  [/\bmv\b/, "shellChanges.movedFiles"],
  [/\b(chmod|chown|icacls)\b/, "shellChanges.changedPermissions"],
  [/\brm\s/, "shellChanges.cleanedUp"],
  [/\bnetworksetup\s+-setairportnetwork\b/, "shellChanges.connectedWifi"],
  [/\b(killall|taskkill)\s+(\S+)/, "shellChanges.stoppedProcess"],
  [/\bpkill\b/, "shellChanges.stoppedProcess"],
  [/\bopen\s+-a\s+(\S+)/, "shellChanges.openedFile"],
  [/\b(launchctl|systemctl)\b.*\b(start|stop|restart)\b/, "shellChanges.managedServices"],
  [/\bdefaults\s+write\b/, "shellChanges.changedPreferences"],
  [/\b(brew|apt|yum|choco|winget|scoop)\s+install\b/, "shellChanges.installedSoftware"],
  [/\b(brew|apt|yum|choco|winget|scoop)\s+upgrade\b/, "shellChanges.updatedSoftware"],
  [/\b(npm|yarn|pnpm)\s+cache\s+clean\b/, "shellChanges.clearedCaches"],
  [/\bdscacheutil\s+-flushcache\b/, "shellChanges.clearedCaches"],
  [/\bsoftwareupdate\s+-(i|d)\b/, "shellChanges.installedUpdates"],
  [/\blpr\s/, "shellChanges.printedFile"],
  [/\bopen\s+.*systempreferences/, "shellChanges.openedSettings"],
  [/\b(open|start)\s/, "shellChanges.openedFile"],
  [/\b(sfc|DISM|chkdsk)\b/i, "shellChanges.ranRepairTool"],
];

function shellChangeLabel(description: string, t: (key: string) => string): string | null {
  if (!description.startsWith("Executed shell command:")) return null;
  const cmd = description.slice("Executed shell command:".length).trim();
  for (const [pattern, key] of SHELL_CHANGE_PATTERN_KEYS) {
    const m = cmd.match(pattern);
    if (m) return t(key);
  }
  return null;
}

function changeLabel(c: {
  tool_name: string;
  description: string;
}, t: (key: string) => string): string | null {
  if (c.tool_name === "shell_run") return shellChangeLabel(c.description, t);
  const key = CHANGE_TOOL_KEYS[c.tool_name];
  return key ? t(key) : null;
}

function dedupeChanges(
  actions: { tool_name: string; description: string }[],
  t: (key: string, params?: Record<string, string | number>) => string,
): { changes: string[]; diagnosticCount: number } {
  const seen = new Set<string>();
  const changes: string[] = [];
  let diagnosticCount = 0;
  for (const a of actions) {
    const lbl = changeLabel(a, t);
    if (lbl === null) {
      diagnosticCount++;
    } else if (!seen.has(lbl)) {
      seen.add(lbl);
      changes.push(lbl);
    }
  }
  return { changes, diagnosticCount };
}

function ChangesBlock({ changeIds }: { changeIds: string[] }) {
  const { t } = useLocale();
  const [expanded, setExpanded] = useState(false);
  const allChanges = useSessionStore((s) => s.changes);
  const matched = allChanges.filter((c) => changeIds.includes(c.id));

  if (matched.length === 0) return null;

  const { changes, diagnosticCount } = dedupeChanges(matched, t);

  if (changes.length === 0) {
    return (
      <div className="mt-2 rounded-xl border border-border-primary/50 bg-bg-primary/50 px-4 py-2 text-sm text-text-muted">
        {t("chat.diagnosticChecks", { count: diagnosticCount, s: diagnosticCount !== 1 ? "s" : "" })}
      </div>
    );
  }

  return (
    <div className="mt-2 rounded-xl border border-border-primary/50 bg-bg-primary/50 overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-4 py-2 text-sm text-left cursor-pointer hover:bg-bg-tertiary/30 transition-colors"
      >
        <svg width="12" height="12" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M8.5 1.5L12.5 5.5L5 13H1V9L8.5 1.5Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
        </svg>
        <span className="text-accent-purple font-medium">
          {t("chat.actionsTaken", { count: changes.length, s: changes.length !== 1 ? "s" : "" })}
        </span>
        <span className="text-text-muted ml-auto">
          {expanded ? "\u25B4" : "\u25BE"}
        </span>
      </button>
      {expanded && (
        <div className="px-4 py-2.5 border-t border-border-primary/50 text-sm space-y-1.5">
          {changes.map((label, i) => (
            <div key={i} className="text-text-secondary leading-snug">
              {label}
            </div>
          ))}
          {diagnosticCount > 0 && (
            <div className="text-text-muted pt-0.5">
              {t("chat.plusDiagnostics", { count: diagnosticCount, s: diagnosticCount !== 1 ? "s" : "" })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Action Card ──

function ActionCard({
  situation,
  plan,
  actionLabel,
  actionType,
  actionTaken,
  isProcessing,
  timestamp,
  progress,
  qrData,
  onDoIt,
}: {
  situation: string;
  plan?: string;
  actionLabel: string;
  actionType?: string;
  actionTaken?: boolean;
  isProcessing: boolean;
  timestamp: number;
  progress?: { step: number; total: number; label: string };
  qrData?: string;
  onDoIt: () => void;
}) {
  const { t } = useLocale();
  const prettySituation = normalizeSpaText(situation);
  const prettyPlan = plan ? normalizeSpaText(plan) : null;
  const prettyActionLabel = humanizeActionLabel(actionLabel, actionType, t);
  const isWaitForUser = actionType === "WAIT_FOR_USER";

  return (
    <div className="group animate-fade-in">
      <div className="rounded-xl border border-border-primary/50 bg-bg-secondary overflow-hidden">
        <div className="px-5 pt-4">
          {progress && <ProgressBar step={progress.step} total={progress.total} label={progress.label} />}
        </div>

        {/* Situation / Instructions */}
        <div className={`px-5 ${prettyPlan ? "pb-2" : "pb-3"}`}>
          {!isWaitForUser && (
            <div className="text-sm font-semibold text-accent-blue mb-1.5 tracking-wide">
              {t("chat.situation")}
            </div>
          )}
          <div className={`rounded-lg px-3.5 py-3 text-base leading-relaxed ${
            isWaitForUser
              ? "border border-border-primary/50 bg-bg-primary text-text-primary"
              : "border border-accent-blue/20 bg-accent-blue/5 text-text-primary"
          }`}>
            <div className="whitespace-pre-wrap break-words">
              <MarkdownSummary text={prettySituation} />
            </div>
          </div>
          {qrData && <QrCodeImage data={qrData} />}
        </div>

        {/* Plan (only when present) */}
        {prettyPlan && (
          <div className="px-5 pb-3">
            <div className="text-sm font-semibold text-accent-purple mb-1.5 tracking-wide">
              {t("chat.plan")}
            </div>
            <div className="rounded-lg border border-accent-purple/20 bg-accent-purple/5 px-3.5 py-3 text-base text-text-secondary leading-relaxed">
              <div className="whitespace-pre-wrap break-words">
                <MarkdownSummary text={prettyPlan} />
              </div>
            </div>
          </div>
        )}

        {/* Action button */}
        <div className="px-5 pb-4">
          <button
            onClick={onDoIt}
            disabled={actionTaken || isProcessing}
            className={`
              w-full py-2.5 rounded-lg text-base font-medium transition-all cursor-pointer
              ${
                actionTaken
                  ? "bg-bg-tertiary text-text-muted cursor-default"
                  : isProcessing
                    ? "bg-bg-tertiary text-text-muted cursor-not-allowed"
                    : isWaitForUser
                      ? "border-2 border-accent-green text-accent-green hover:bg-accent-green/10"
                      : "bg-accent-blue text-white hover:bg-accent-blue/80"
              }
            `}
          >
            {actionTaken ? t("chat.sent") : prettyActionLabel}
          </button>
        </div>
      </div>
      <div className="text-[10px] mt-1 text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
        {formatTime(timestamp)}
      </div>
    </div>
  );
}

// ── User Question Card ──

function UserQuestionCard({
  questions,
  actionTaken,
  isProcessing,
  timestamp,
  progress,
  onAnswer,
  onSecureAnswer,
  onSendMessage,
}: {
  questions: AssistantQuestion[];
  actionTaken?: boolean;
  isProcessing: boolean;
  timestamp: number;
  progress?: { step: number; total: number; label: string };
  onAnswer: (answer: string) => void;
  onSecureAnswer?: (secretName: string, value: string) => void;
  onSendMessage?: (text: string) => void;
}) {
  const { t } = useLocale();
  const [selectedOption, setSelectedOption] = useState<string>("");
  const [textValue, setTextValue] = useState<string>("");
  const [secureValue, setSecureValue] = useState<string>("");
  const first = questions[0];
  if (!first) return null;

  const hasOptions = first.options && first.options.length > 0;
  const hasTextInput = !!first.text_input;
  const hasSecureInput = !!first.secure_input;

  // Initialize text input default
  const defaultValue = first.text_input?.default || "";

  const canSubmit = hasOptions
    ? !!selectedOption
    : hasTextInput
      ? (textValue || defaultValue).trim().length > 0
      : hasSecureInput
        ? secureValue.trim().length > 0
        : false;

  const handleSubmit = () => {
    if (hasOptions) {
      onAnswer(selectedOption);
    } else if (hasTextInput) {
      onAnswer((textValue || defaultValue).trim());
    } else if (hasSecureInput && onSecureAnswer) {
      onSecureAnswer(first.secure_input!.secret_name, secureValue);
    }
  };

  return (
    <div className="group animate-fade-in">
      <div className="rounded-xl border border-border-primary/50 bg-bg-secondary overflow-hidden">
        <div className="px-5 pt-4 pb-3">
          {progress && <ProgressBar step={progress.step} total={progress.total} label={progress.label} />}
          <div className="text-sm font-semibold text-accent-blue mb-1.5 tracking-wide">
            <InlineMarkdown text={first.header} />
          </div>
          <div className="text-base text-text-primary mb-3">
            <MarkdownSummary text={first.question} />
          </div>

          {/* Options mode */}
          {hasOptions && (
            <div className="space-y-2">
              {first.options!.map((opt) => (
                <button
                  key={opt.label}
                  onClick={() => setSelectedOption(opt.label)}
                  disabled={actionTaken || isProcessing}
                  className={`w-full text-left rounded-lg border px-3 py-2 transition-colors cursor-pointer ${
                    selectedOption === opt.label
                      ? "border-accent-blue bg-accent-blue/10"
                      : "border-border-primary bg-bg-secondary hover:bg-bg-tertiary"
                  }`}
                >
                  <div className="text-sm font-medium text-text-primary"><InlineMarkdown text={opt.label} /></div>
                  <div className="text-xs text-text-muted"><InlineMarkdown text={opt.description || ""} /></div>
                </button>
              ))}
            </div>
          )}

          {/* Text input mode */}
          {hasTextInput && (
            <input
              type="text"
              value={textValue || defaultValue}
              onChange={(e) => setTextValue(e.target.value)}
              placeholder={first.text_input?.placeholder || ""}
              disabled={actionTaken || isProcessing}
              className="w-full px-3.5 py-2.5 rounded-lg border border-border-primary bg-bg-primary text-base text-text-primary placeholder-text-muted outline-none focus:border-accent-blue/40 transition-colors"
              onKeyDown={(e) => {
                if (e.key === "Enter" && canSubmit && !actionTaken && !isProcessing) {
                  handleSubmit();
                }
              }}
            />
          )}

          {/* Secure input mode */}
          {hasSecureInput && (
            <input
              type="password"
              value={secureValue}
              onChange={(e) => setSecureValue(e.target.value)}
              placeholder={first.secure_input?.placeholder || ""}
              disabled={actionTaken || isProcessing}
              autoComplete="off"
              className="w-full px-3.5 py-2.5 rounded-lg border border-border-primary bg-bg-primary text-base text-text-primary placeholder-text-muted outline-none focus:border-accent-blue/40 transition-colors"
              onKeyDown={(e) => {
                if (e.key === "Enter" && canSubmit && !actionTaken && !isProcessing) {
                  handleSubmit();
                }
              }}
            />
          )}
        </div>
        <div className="px-5 pb-4 space-y-2">
          <button
            onClick={handleSubmit}
            disabled={actionTaken || isProcessing || !canSubmit}
            className="w-full py-2 rounded-lg text-sm font-medium transition-all cursor-pointer bg-accent-blue text-white hover:bg-accent-blue/80 disabled:opacity-60"
          >
            {actionTaken ? t("chat.sent") : t("chat.submit")}
          </button>
          {!actionTaken && !isProcessing && onSendMessage && (
            <button
              onClick={() => onSendMessage("")}
              className="w-full text-sm text-text-muted hover:text-accent-blue transition-colors cursor-pointer"
            >
              {t("chat.typeAnswerBelow")}
            </button>
          )}
        </div>
      </div>
      <div className="text-[10px] mt-1 text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
        {formatTime(timestamp)}
      </div>
    </div>
  );
}

// ── Done Card ──

function DoneCard({
  summary,
  timestamp,
  isLatestDone,
  sessionId,
}: {
  summary: string;
  timestamp: number;
  isLatestDone: boolean;
  sessionId: string | null;
}) {
  const { t } = useLocale();
  const [resolved, setResolved] = useState<boolean | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (!sessionId || !isLatestDone) return;
    commands
      .listSessions()
      .then((sessions) => {
        const current = sessions.find((s) => s.id === sessionId);
        if (current && current.resolved !== null) {
          setResolved(current.resolved);
        }
      })
      .catch(() => {})
      .finally(() => setLoaded(true));
  }, [sessionId, isLatestDone]);

  const handleResolve = async (value: boolean) => {
    if (!sessionId) return;
    setResolved(value);
    try {
      await commands.markResolved(sessionId, value);
    } catch (err) {
      console.error("Failed to mark resolved:", err);
    }
  };

  return (
    <div className="group animate-fade-in">
      <div className="rounded-xl border border-accent-green/20 bg-accent-green/5 px-5 py-4">
        <div className="flex items-start gap-2.5">
          <span className="text-accent-green text-lg mt-0.5">{"\u2713"}</span>
          <div className="flex-1">
            <div className="text-base text-text-primary leading-relaxed">
              <MarkdownSummary text={summary} />
            </div>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-3 mt-1.5 min-h-[24px]">
        {isLatestDone && loaded && resolved === null && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-muted">{t("chat.fixed")}</span>
            <button
              onClick={() => handleResolve(true)}
              className="px-2.5 py-1 rounded-lg text-xs font-medium text-accent-blue bg-accent-blue/10 hover:bg-accent-blue/20 transition-colors cursor-pointer"
            >
              {t("chat.yes")}
            </button>
            <button
              onClick={() => handleResolve(false)}
              className="px-2.5 py-1 rounded-lg text-xs text-text-muted hover:bg-bg-tertiary transition-colors cursor-pointer"
            >
              {t("chat.notQuite")}
            </button>
          </div>
        )}
        {resolved === true && (
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            <span className="text-accent-blue text-xs">{"\u2713"}</span>
            <span className="text-xs text-text-muted">{t("chat.resolved")}</span>
          </div>
        )}
        {resolved === false && (
          <span className="text-xs text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            {t("chat.stillWorking")}
          </span>
        )}

        <span className="text-[10px] text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
          {formatTime(timestamp)}
        </span>
      </div>
    </div>
  );
}

// ── Info Card ──

function InfoCard({
  summary,
  timestamp,
}: {
  summary: string;
  timestamp: number;
}) {
  return (
    <div className="group animate-fade-in">
      <div className="rounded-xl border border-accent-blue/20 bg-accent-blue/5 px-5 py-4">
        <div className="flex items-start gap-2.5">
          <span className="text-accent-blue text-lg mt-0.5">{"\u2139"}</span>
          <div className="flex-1">
            <div className="text-base text-text-primary leading-relaxed">
              <MarkdownSummary text={summary} />
            </div>
          </div>
        </div>
      </div>
      <div className="text-[10px] mt-1 text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
        {formatTime(timestamp)}
      </div>
    </div>
  );
}

// ── Confirmation Pill (for "Go ahead" user messages) ──

// ── Single Message Bubble (fallback for unstructured messages) ──

function MessageBubble({ message }: { message: Message }) {
  const isUser = message.role === "user";

  if (isUser) {
    return (
      <div className="group flex justify-end animate-fade-in">
        <div className="max-w-[75%]">
          <div className="rounded-2xl rounded-br-sm bg-bg-user-bubble text-text-user-bubble px-4 py-2.5">
            <div className="text-base leading-relaxed whitespace-pre-wrap break-words">
              {message.content}
            </div>
          </div>
          <div className="text-[10px] mt-1 text-text-muted text-right opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            {formatTime(message.timestamp)}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="group animate-fade-in">
      <div className="text-base text-text-primary leading-relaxed">
        <MarkdownSummary text={message.content} />
      </div>

      {showToolCalls && message.toolCalls && message.toolCalls.length > 0 && (
        <div className="mt-1">
          {message.toolCalls.map((tc) => (
            <ToolCallItem key={tc.id} toolCall={tc} />
          ))}
        </div>
      )}

      <div className="text-[10px] mt-1.5 text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
        {formatTime(message.timestamp)}
      </div>
    </div>
  );
}

// ── Helper to render from AssistantUiPayload ──

function renderFromUiPayload(
  ui: AssistantUiPayload,
  message: Message,
  isProcessing: boolean,
  isLatestDone: boolean,
  sessionId: string | null,
  onConfirm: (messageId: string, actionLabel?: string) => void,
  onEvent: (eventType: "USER_ANSWER_QUESTION", payload?: string) => void,
  onSecureAnswer?: (secretName: string, value: string) => void,
  onSendMessage?: (text: string) => void,
): React.ReactNode {
  const progress = "progress" in ui ? ui.progress : undefined;

  switch (ui.kind) {
    case "spa":
      return (
        <ActionCard
          situation={ui.situation}
          plan={ui.plan}
          actionLabel={ui.action.label}
          actionType={ui.action.type}
          actionTaken={message.actionTaken}
          isProcessing={isProcessing}
          timestamp={message.timestamp}
          progress={progress}
          qrData={ui.qr_data}
          onDoIt={() => onConfirm(message.id, ui.action.label)}
        />
      );
    case "user_question":
      return (
        <UserQuestionCard
          questions={ui.questions}
          actionTaken={message.actionTaken}
          isProcessing={isProcessing}
          timestamp={message.timestamp}
          progress={progress}
          onAnswer={(answer) =>
            onEvent("USER_ANSWER_QUESTION", JSON.stringify({ answer }))
          }
          onSecureAnswer={onSecureAnswer}
          onSendMessage={onSendMessage}
        />
      );

    case "done":
      return (
        <DoneCard
          summary={ui.summary}
          timestamp={message.timestamp}
          isLatestDone={isLatestDone}
          sessionId={sessionId}
        />
      );
    case "info":
      return (
        <InfoCard
          summary={ui.summary}
          timestamp={message.timestamp}
        />
      );
    default:
      return <MessageBubble message={message} />;
  }
}

// ── Message Router (picks the right card for each message) ──

function MessageDisplay({
  message,
  isProcessing,
  isLatestDone,
  sessionId,
  onConfirm,
  onEvent,
  onSecureAnswer,
  onSendMessage,
}: {
  message: Message;
  isProcessing: boolean;
  isLatestDone: boolean;
  sessionId: string | null;
  onConfirm: (messageId: string, actionLabel?: string) => void;
  onEvent: (eventType: "USER_ANSWER_QUESTION", payload?: string) => void;
  onSecureAnswer?: (secretName: string, value: string) => void;
  onSendMessage?: (text: string) => void;
}) {
  // Non-assistant messages use the regular bubble
  if (message.role !== "assistant") {
    return <MessageBubble message={message} />;
  }

  const hasActions = message.changeIds && message.changeIds.length > 0;

  // Prefer structured assistantUi if available
  let card: React.ReactNode;
  if (message.assistantUi) {
    card = renderFromUiPayload(
      message.assistantUi,
      message,
      isProcessing,
      isLatestDone,
      sessionId,
      onConfirm,
      onEvent,
      onSecureAnswer,
      onSendMessage,
    );
  } else {
    // Fall back to parsing text (backward compat for old sessions)
    const parsed = parseResponse(message.content);
    switch (parsed.type) {
      case "action":
        card = (
          <ActionCard
            situation={parsed.situation}
            plan={parsed.plan}
            actionLabel={parsed.actionLabel}
            actionType={parsed.actionType}
            actionTaken={message.actionTaken}
            isProcessing={isProcessing}
            timestamp={message.timestamp}
            onDoIt={() => onConfirm(message.id, parsed.actionLabel)}
          />
        );
        break;
      case "user_question":
        card = (
          <UserQuestionCard
            questions={parsed.questions}
            actionTaken={message.actionTaken}
            isProcessing={isProcessing}
            timestamp={message.timestamp}
            onAnswer={(answer) =>
              onEvent("USER_ANSWER_QUESTION", JSON.stringify({ answer }))
            }
            onSendMessage={onSendMessage}
          />
        );
        break;
      case "done":
        card = (
          <DoneCard
            summary={parsed.summary}
            timestamp={message.timestamp}
            isLatestDone={isLatestDone}
            sessionId={sessionId}
          />
        );
        break;
      case "info":
        card = (
          <InfoCard
            summary={parsed.summary}
            timestamp={message.timestamp}
          />
        );
        break;
      default:
        card = <MessageBubble message={message} />;
    }
  }

  if (!hasActions) return card;

  return (
    <div>
      {card}
      <div className="mt-1">
        <ChangesBlock changeIds={message.changeIds!} />
      </div>
    </div>
  );
}

// ── Humanize tool names for the thinking indicator ──

// Tool names are looked up via t("tools.<name>") at render time.
const TOOL_NAMES_WITH_I18N = new Set([
  "mac_network_info", "mac_ping", "mac_dns_check", "mac_http_check",
  "mac_flush_dns", "mac_system_info", "mac_system_summary", "mac_process_list",
  "mac_disk_usage", "mac_printer_list", "mac_print_queue", "mac_app_list",
  "mac_app_logs", "mac_read_file", "mac_read_log", "shell_run",
  "mac_kill_process", "mac_clear_caches", "mac_clear_app_cache",
  "mac_restart_cups", "mac_cancel_print_jobs", "mac_move_file",
  "win_network_info", "win_ping", "win_dns_check", "win_http_check",
  "win_flush_dns", "win_system_info", "win_system_summary", "win_process_list",
  "win_disk_usage", "win_printer_list", "win_print_queue", "win_app_list",
  "win_app_logs", "win_app_data_ls", "win_read_file", "win_read_log",
  "win_kill_process", "win_clear_caches", "win_clear_app_cache",
  "win_restart_spooler", "win_cancel_print_jobs", "win_move_file",
  "win_startup_programs", "win_service_list", "win_restart_service",
  "linux_network_info", "linux_ping", "linux_dns_check", "linux_http_check",
  "linux_flush_dns", "linux_system_info", "linux_system_summary",
  "linux_process_list", "linux_disk_usage", "linux_read_file", "linux_read_log",
  "linux_kill_process",
  "web_fetch", "activate_playbook",
  "write_knowledge", "knowledge_search", "knowledge_read",
]);

function humanizeToolCall(summary: string, detail: Record<string, unknown> | undefined, t: (key: string) => string): string {
  const toolName = (detail?.name as string) || summary.match(/Calling (\w+)/)?.[1];
  if (!toolName) return t("chat.working");

  // Try to add context from the tool input.
  const input = detail?.input as Record<string, unknown> | undefined;
  if (input) {
    const target = (input.host || input.domain || input.url || input.app_name ||
      input.process_name || input.service_name || input.path || input.name || input.query) as string | undefined;
    if (target) {
      const short = target.length > 40 ? target.slice(0, 37) + "..." : target;
      const base = TOOL_NAMES_WITH_I18N.has(toolName) ? t(`tools.${toolName}`) : t("chat.working");
      return `${base} \u2014 ${short}`;
    }
  }

  if (TOOL_NAMES_WITH_I18N.has(toolName)) {
    return t(`tools.${toolName}`);
  }
  return t("chat.working");
}

// ── Thinking Indicator with live status ──

interface DebugLogPayload {
  event_type: string;
  summary: string;
  detail?: Record<string, unknown>;
}

interface ActivityEntry {
  time: string;
  text: string;
  type: "command" | "result" | "thinking" | "error";
}

function formatActivityEntry(evt: DebugLogPayload, t: (key: string) => string): ActivityEntry | null {
  const time = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  switch (evt.event_type) {
    case "tool_call": {
      const name = evt.detail?.name as string | undefined;
      // Filter out ui_* protocol calls — these are LLM↔Noah internal
      if (name?.startsWith("ui_")) return null;
      const input = evt.detail?.input as Record<string, unknown> | undefined;
      const cmd = input?.command as string | undefined;
      return { time, text: cmd ? `$ ${cmd}` : `${name || "tool"}`, type: "command" };
    }
    case "tool_result": {
      const name = evt.detail?.name as string | undefined;
      if (name?.startsWith("ui_")) return null;
      const preview = (evt.detail?.output_preview as string || "").trim();
      if (!preview) return null;
      // Truncate long output
      const lines = preview.split("\n");
      const display = lines.length > 20 ? [...lines.slice(0, 18), `... (${lines.length - 18} more lines)`].join("\n") : preview;
      return { time, text: display, type: "result" };
    }
    case "llm_request":
      return { time, text: t("chat.thinking"), type: "thinking" };
    case "error":
      return { time, text: evt.summary, type: "error" };
    default:
      return null;
  }
}

/** Persistent activity log — survives across processing cycles in playbook mode. */
function useActivityLog(t: (key: string) => string) {
  const [activity, setActivity] = useState<ActivityEntry[]>([]);
  const [isPlaybook, setIsPlaybook] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const startRef = useRef(Date.now());
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    const unlisten = listen<DebugLogPayload>("debug-log", (e) => {
      const evt = e.payload;
      if (evt.event_type === "tool_call") {
        setStatus(humanizeToolCall(evt.summary, evt.detail, t));
        startRef.current = Date.now();

        setElapsed(0);
      } else if (evt.event_type === "llm_request") {
        setStatus(t("chat.thinking"));
        startRef.current = Date.now();

        setElapsed(0);
      } else if (evt.event_type === "playbook_activated") {
        setIsPlaybook(true);
      }
      const entry = formatActivityEntry(evt, t);
      if (entry) {
        setActivity(prev => [...prev.slice(-50), entry]);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [t]);

  useEffect(() => {
    if (!status) return;
    const timer = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startRef.current) / 1000));
    }, 1000);
    return () => clearInterval(timer);
  }, [status]);

  const clear = useCallback(() => { setActivity([]); setIsPlaybook(false); setStatus(null); }, []);

  return { activity, isPlaybook, status, elapsed, clear };
}

function ThinkingDots({ status, elapsed }: { status: string | null; elapsed: number }) {
  return (
    <div className="flex items-center gap-2.5 py-1">
      <div className="flex items-center gap-1">
        <div className="w-1.5 h-1.5 rounded-full bg-text-muted thinking-dot" />
        <div className="w-1.5 h-1.5 rounded-full bg-text-muted thinking-dot" />
        <div className="w-1.5 h-1.5 rounded-full bg-text-muted thinking-dot" />
      </div>
      {status && (
        <span className="text-sm text-text-muted">
          {status}
          {elapsed > 0 && (
            <span className="ml-1 text-text-muted/60">{elapsed}s</span>
          )}
        </span>
      )}
    </div>
  );
}

function ActivityLog({ activity, defaultExpanded, t }: { activity: ActivityEntry[]; defaultExpanded: boolean; t: (key: string) => string }) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const logRef = useRef<HTMLDivElement>(null);

  // Auto-expand when defaultExpanded changes to true (playbook activated)
  useEffect(() => {
    if (defaultExpanded) setExpanded(true);
  }, [defaultExpanded]);

  // Auto-scroll the log
  useEffect(() => {
    if (expanded && logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [activity, expanded]);

  if (activity.length === 0) return null;

  return (
    <div className="py-1">
      <div className="flex items-center gap-2">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 text-xs text-text-muted hover:text-text-secondary cursor-pointer"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" className={`transition-transform ${expanded ? "rotate-90" : ""}`}>
            <path d="M3 1L7 5L3 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          {expanded ? t("chat.hideUnderTheHood") : t("chat.underTheHood")}
        </button>
        {!expanded && (
          <span className="text-[10px] text-text-muted/50">{activity.length} events</span>
        )}
      </div>
      {expanded && (
        <div
          ref={logRef}
          className="mt-2 max-h-72 overflow-y-auto rounded-lg border border-border-primary/30 bg-[#1a1a2e] dark:bg-[#0d0d1a] p-3 font-mono text-xs leading-relaxed shadow-inner"
        >
          {activity.filter((e) => e.type !== "thinking").map((entry, i) => (
            <div key={i} className={`${
              entry.type === "command" ? "text-[#64b5f6]"
              : entry.type === "error" ? "text-[#ef5350]"
              : "text-[#aaa]"
            } ${entry.type === "result" ? "pl-4 whitespace-pre-wrap" : ""}`}>
              <span className="text-[#555] mr-2 select-none">{entry.time}</span>
              {entry.text}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Onboarding Suggestion Cards ──

const SUGGESTION_KEYS = [
  { icon: "\uD83C\uDF10", labelKey: "suggestions.internetSlow", descKey: "suggestions.internetSlowDesc" },
  { icon: "\uD83D\uDC22", labelKey: "suggestions.sluggish", descKey: "suggestions.sluggishDesc" },
  { icon: "\uD83D\uDCA5", labelKey: "suggestions.crashing", descKey: "suggestions.crashingDesc" },
  { icon: "\uD83D\uDDA8\uFE0F", labelKey: "suggestions.printer", descKey: "suggestions.printerDesc" },
];

function SuggestionCards({
  onSelect,
  disabled,
}: {
  onSelect: (text: string) => void;
  disabled: boolean;
}) {
  const { t } = useLocale();
  const [contextual, setContextual] = useState<
    { icon: string; label: string; description: string }[]
  >([]);

  useEffect(() => {
    commands.listKnowledge("issues").then((entries) => {
      setContextual(
        entries.slice(0, 2).map((e) => ({
          icon: "\uD83D\uDD04",
          label: t("chat.checkOn", { title: e.title }),
          description: t("chat.followUp"),
        })),
      );
    }).catch(() => {});
  }, [t]);

  const suggestions = SUGGESTION_KEYS.map((s) => ({
    icon: s.icon,
    label: t(s.labelKey),
    description: t(s.descKey),
  }));
  const allSuggestions = [...contextual, ...suggestions].slice(0, 4);

  return (
    <div className="flex flex-col items-center text-text-muted">
      <div className="grid grid-cols-2 gap-3 w-full max-w-md">
        {allSuggestions.map((s) => (
          <button
            key={s.label}
            onClick={() => onSelect(s.label)}
            disabled={disabled}
            className="flex items-start gap-3 px-4 py-4 rounded-xl border border-border-primary/50 bg-bg-secondary hover:bg-bg-tertiary hover:border-accent-blue/40 transition-all text-left cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <span className="text-lg mt-0.5">{s.icon}</span>
            <div className="min-w-0">
              <div className="text-sm font-medium text-text-primary leading-snug">
                {s.label}
              </div>
              <div className="text-xs text-text-muted mt-0.5">
                {s.description}
              </div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function WelcomeHero({ hasContextual, learnMode }: { hasContextual: boolean; learnMode?: boolean }) {
  const { t } = useLocale();
  return (
    <div className="flex flex-col items-center text-text-muted">
      <NoahIcon className="w-14 h-14 rounded-2xl mb-4" alt="Noah" />
      <p className="text-2xl font-semibold text-text-primary mb-1">
        {learnMode ? t("welcome.learnGreeting") : t("welcome.greeting")}
      </p>
      <p className="text-base text-text-secondary">
        {learnMode
          ? t("welcome.learnSubtitle")
          : hasContextual
            ? t("welcome.subtitleContextual")
            : t("welcome.subtitleDefault")}
      </p>
    </div>
  );
}

// ── Chat Panel ──

export function ChatPanel() {
  const messages = useChatStore((s) => s.messages);
  const sessionId = useSessionStore((s) => s.sessionId);
  const { sendMessage, sendConfirmation, sendEvent, cancelProcessing, isProcessing } =
    useAgent();

  const { t } = useLocale();
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const activityLog = useActivityLog(t);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isProcessing]);

  useEffect(() => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 300)}px`;
    }
  }, [input]);

  const handleSubmit = useCallback(async () => {
    const text = input.trim();
    if (!text || isProcessing) return;
    setInput("");
    await sendMessage(text);
  }, [input, isProcessing, sendMessage]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleEvent = useCallback(
    (eventType: "USER_ANSWER_QUESTION", payload?: string) => {
      sendEvent(eventType, payload);
    },
    [sendEvent],
  );

  const handleSecureAnswer = useCallback(
    async (secretName: string, value: string) => {
      if (!sessionId) return;
      try {
        // Store the secret in the orchestrator's secret store (never in LLM context).
        await commands.storeSecret(sessionId, secretName, value);
        // Tell the LLM the secret was collected (without the value).
        sendEvent("USER_ANSWER_QUESTION", JSON.stringify({
          answer: `[SECRET:${secretName}] stored securely`,
        }));
      } catch (err) {
        console.error("Failed to store secret:", err);
      }
    },
    [sessionId, sendEvent],
  );

  const showWelcome = messages.length === 0 || (messages.length === 1 && messages[0].role === "system");
  const sessionMode = useSessionStore((s) => s.sessionMode);

  const inputCard = (
    <div className="max-w-4xl w-full mx-auto">
      <div className="flex items-end gap-2 bg-bg-secondary rounded-2xl border border-border-primary focus-within:border-accent-blue/40 transition-colors shadow-sm">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={sessionMode === "learn" ? t("chat.learnPlaceholder") : t("chat.placeholder")}
          rows={1}
          disabled={isProcessing}
          className="flex-1 bg-transparent text-base text-text-primary placeholder-text-muted px-4 py-3 resize-none outline-none min-h-[44px] max-h-[300px]"
        />
        <div className="flex items-center gap-1 pr-2 pb-1.5">
          {isProcessing ? (
            <button
              onClick={cancelProcessing}
              title="Stop processing"
              className="flex items-center justify-center w-9 h-9 rounded-lg bg-accent-red/15 text-accent-red hover:bg-accent-red/25 transition-all duration-200 cursor-pointer"
            >
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <rect x="3" y="3" width="8" height="8" rx="1.5" fill="currentColor" />
              </svg>
            </button>
          ) : (
            <button
              onClick={handleSubmit}
              disabled={!input.trim()}
              className={`
                flex items-center justify-center w-9 h-9 rounded-lg
                transition-all duration-200 cursor-pointer
                ${
                  input.trim()
                    ? "bg-accent-blue text-white hover:bg-accent-blue/80"
                    : "text-text-muted cursor-not-allowed"
                }
              `}
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M2 8L14 2L8 14V8H2Z" fill="currentColor" />
              </svg>
            </button>
          )}
        </div>
      </div>
    </div>
  );

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex-1 overflow-y-auto px-6 py-4">
        {showWelcome ? (
          <div className="flex flex-col items-center justify-center h-full gap-8">
            <WelcomeHero hasContextual={false} learnMode={sessionMode === "learn"} />
            {sessionMode !== "learn" && !input.trim() && (
              <SuggestionCards
                onSelect={(text) => sendMessage(text)}
                disabled={isProcessing}
              />
            )}
            <div className="w-full mb-4">
              {inputCard}
            </div>
          </div>
        ) : (
          <div className="flex flex-col min-h-full">
            <div className="max-w-3xl w-full mx-auto space-y-5 flex-1">
              {(() => {
                const isLatestDoneMsg = (msg: Message) => {
                  if (msg.assistantUi) {
                    return msg.assistantUi.kind === "done";
                  }
                  return msg.role === "assistant" && parseResponse(msg.content).type === "done";
                };
                const latestDoneId = [...messages]
                  .reverse()
                  .find((m) => isLatestDoneMsg(m))?.id ?? null;

                return messages.map((msg) => (
                  <MessageDisplay
                    key={msg.id}
                    message={msg}
                    isProcessing={isProcessing}
                    isLatestDone={msg.id === latestDoneId}
                    sessionId={sessionId}
                    onConfirm={sendConfirmation}
                    onEvent={handleEvent}
                    onSecureAnswer={handleSecureAnswer}
                    onSendMessage={(text) => { setInput(text); setTimeout(() => textareaRef.current?.focus(), 0); }}
                  />
                ));
              })()}
              {isProcessing && <ThinkingDots status={activityLog.status} elapsed={activityLog.elapsed} />}
              {showToolCalls && activityLog.activity.length > 0 && (
                <ActivityLog activity={activityLog.activity} defaultExpanded={activityLog.isPlaybook} t={t} />
              )}
            </div>
            <div
              data-testid="chat-input-footer"
              className="sticky bottom-0 z-10 pt-3 pb-3 bg-bg-primary border-t border-border-primary/50 shadow-[0_-6px_18px_rgba(0,0,0,0.16)]"
            >
              {inputCard}
            </div>
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
