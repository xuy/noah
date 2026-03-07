import { useState, useRef, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";
import type { Message, ToolCall } from "../stores/chatStore";
import { useAgent } from "../hooks/useAgent";
import { parseResponse } from "../lib/parseResponse";
import * as commands from "../lib/tauri-commands";
import { NoahIcon } from "./NoahIcon";

const showToolCalls = import.meta.env.DEV;

function formatTime(ts: number) {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
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

// ── Action classification ────────────────────────────────────────────────────
// Dedicated tools: classify as diagnostic (read-only) or change (mutating).
// Only changes are shown to the user; diagnostics are counted but hidden.

const CHANGE_TOOLS: Record<string, string> = {
  mac_flush_dns: "Flushed DNS",
  mac_kill_process: "Stopped a process",
  mac_clear_caches: "Cleared caches",
  mac_clear_app_cache: "Cleared app cache",
  mac_restart_cups: "Restarted printing",
  mac_cancel_print_jobs: "Cancelled print jobs",
  mac_move_file: "Moved a file",
  win_flush_dns: "Flushed DNS",
  win_kill_process: "Stopped a process",
  win_clear_caches: "Cleared caches",
  win_clear_app_cache: "Cleared app cache",
  win_restart_spooler: "Restarted printing",
  win_cancel_print_jobs: "Cancelled print jobs",
  win_move_file: "Moved a file",
  win_restart_service: "Restarted a service",
  write_knowledge: "Saved a note",
};

// Shell command patterns that represent actual changes (not diagnostics).
// [pattern, label] — order matters (specific before general).
const SHELL_CHANGE_PATTERNS: [RegExp, string][] = [
  [/\bfind\b.*-exec\s+(mv|cp)\b/, "Organized files"],
  [/\bmkdir\b/, "Created folders"],
  [/\b(cp|rsync)\b/, "Copied files"],
  [/\bmv\b/, "Moved files"],
  [/\b(chmod|chown|icacls)\b/, "Changed permissions"],
  [/\brm\s/, "Cleaned up files"],
  [/\bnetworksetup\s+-setairportnetwork\b/, "Connected to WiFi"],
  [/\b(killall|taskkill)\s+(\S+)/, "Stopped $2"],
  [/\bpkill\b/, "Stopped a process"],
  [/\bopen\s+-a\s+(\S+)/, "Opened $1"],
  [/\b(launchctl|systemctl)\b.*\b(start|stop|restart)\b/, "Managed services"],
  [/\bdefaults\s+write\b/, "Changed preferences"],
  [/\b(brew|apt|yum|choco|winget|scoop)\s+install\b/, "Installed software"],
  [/\b(brew|apt|yum|choco|winget|scoop)\s+upgrade\b/, "Updated software"],
  [/\b(npm|yarn|pnpm)\s+cache\s+clean\b/, "Cleared caches"],
  [/\bdscacheutil\s+-flushcache\b/, "Cleared caches"],
  [/\bsoftwareupdate\s+-(i|d)\b/, "Installed updates"],
  [/\blpr\s/, "Printed a file"],
  [/\bopen\s+.*systempreferences/, "Opened Settings"],
  [/\b(open|start)\s/, "Opened a file"],
  [/\b(sfc|DISM|chkdsk)\b/i, "Ran repair tool"],
];

/** For a shell_run action, return its change label or null if diagnostic. */
function shellChangeLabel(description: string): string | null {
  if (!description.startsWith("Executed shell command:")) return null;
  const cmd = description.slice("Executed shell command:".length).trim();
  for (const [pattern, label] of SHELL_CHANGE_PATTERNS) {
    const m = cmd.match(pattern);
    if (m) return label.replace(/\$(\d+)/g, (_, i) => m[+i] || "");
  }
  return null; // diagnostic
}

/** Classify an action and return its label, or null if diagnostic. */
function changeLabel(c: {
  tool_name: string;
  description: string;
}): string | null {
  if (c.tool_name === "shell_run") return shellChangeLabel(c.description);
  return CHANGE_TOOLS[c.tool_name] || null;
}

/** Deduplicate change labels, preserving first-seen order. */
function dedupeChanges(
  actions: { tool_name: string; description: string }[],
): { changes: string[]; diagnosticCount: number } {
  const seen = new Set<string>();
  const changes: string[] = [];
  let diagnosticCount = 0;
  for (const a of actions) {
    const lbl = changeLabel(a);
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
  const [expanded, setExpanded] = useState(false);
  const allChanges = useSessionStore((s) => s.changes);
  const matched = allChanges.filter((c) => changeIds.includes(c.id));

  if (matched.length === 0) return null;

  const { changes, diagnosticCount } = dedupeChanges(matched);

  // If everything was diagnostic, show a simple one-liner
  if (changes.length === 0) {
    return (
      <div className="mt-2 rounded-xl border border-border-primary/50 bg-bg-primary/50 px-4 py-2 text-sm text-text-muted">
        Ran {diagnosticCount} diagnostic check{diagnosticCount !== 1 ? "s" : ""}
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
          {changes.length} action{changes.length !== 1 ? "s" : ""} taken
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
              + {diagnosticCount} diagnostic check{diagnosticCount !== 1 ? "s" : ""}
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
  actionTaken,
  isProcessing,
  timestamp,
  onDoIt,
}: {
  situation: string;
  plan: string;
  actionLabel: string;
  actionTaken?: boolean;
  isProcessing: boolean;
  timestamp: number;
  onDoIt: () => void;
}) {
  return (
    <div className="group animate-fade-in">
      <div className="rounded-xl border border-border-primary/50 bg-bg-secondary overflow-hidden">
        {/* Situation */}
        <div className="px-5 pt-4 pb-2">
          <div className="text-sm font-semibold text-text-secondary mb-1">
            Situation
          </div>
          <div className="text-base text-text-primary leading-relaxed">
            {situation}
          </div>
        </div>

        {/* Plan */}
        <div className="px-5 pb-3">
          <div className="text-sm font-semibold text-text-secondary mb-1">
            Plan
          </div>
          <div className="text-base text-text-secondary leading-relaxed">
            {plan}
          </div>
        </div>

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
                    : "bg-accent-blue text-white hover:bg-accent-blue/80"
              }
            `}
          >
            {actionTaken ? "Sent" : actionLabel}
          </button>
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
  const [resolved, setResolved] = useState<boolean | null>(null);
  const [loaded, setLoaded] = useState(false);

  // Load persisted resolution status on mount
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
      {/* Summary card */}
      <div className="rounded-xl border border-border-primary/50 bg-bg-secondary px-5 py-4">
        <div className="flex items-start gap-2.5">
          <span className="text-accent-green text-lg mt-0.5">{"\u2713"}</span>
          <div className="flex-1">
            <div className="text-base text-text-primary leading-relaxed">
              {summary}
            </div>
          </div>
        </div>
      </div>

      {/* Metadata row — hover to reveal timestamp & resolved status */}
      <div className="flex items-center gap-3 mt-1.5 min-h-[24px]">
        {/* Resolution prompt or status */}
        {isLatestDone && loaded && resolved === null && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-muted">Fixed?</span>
            <button
              onClick={() => handleResolve(true)}
              className="px-2.5 py-1 rounded-lg text-xs font-medium text-accent-blue bg-accent-blue/10 hover:bg-accent-blue/20 transition-colors cursor-pointer"
            >
              Yes
            </button>
            <button
              onClick={() => handleResolve(false)}
              className="px-2.5 py-1 rounded-lg text-xs text-text-muted hover:bg-bg-tertiary transition-colors cursor-pointer"
            >
              Not quite
            </button>
          </div>
        )}
        {resolved === true && (
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            <span className="text-accent-blue text-xs">{"\u2713"}</span>
            <span className="text-xs text-text-muted">Resolved</span>
          </div>
        )}
        {resolved === false && (
          <span className="text-xs text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            Still working on it
          </span>
        )}

        {/* Timestamp — hover reveal */}
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
      <div className="rounded-xl border border-border-primary/50 bg-bg-secondary px-5 py-4">
        <div className="flex items-start gap-2.5">
          <span className="text-accent-blue text-lg mt-0.5">{"\u2139"}</span>
          <div className="flex-1">
            <div className="text-base text-text-primary leading-relaxed">
              {summary}
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

function ConfirmationPill({ timestamp }: { timestamp: number }) {
  return (
    <div className="group flex flex-col items-end animate-fade-in">
      <div className="flex items-center gap-1.5 px-4 py-2 rounded-xl bg-bg-user-bubble/15 text-bg-user-bubble text-sm font-medium">
        <span>{"\u2713"}</span>
        <span>Go ahead</span>
      </div>
      <div className="text-[10px] mt-1 text-text-muted opacity-0 group-hover:opacity-100 transition-opacity duration-150">
        {formatTime(timestamp)}
      </div>
    </div>
  );
}

// ── Single Message Bubble (fallback for unstructured messages) ──

function MessageBubble({ message }: { message: Message }) {
  const isUser = message.role === "user";

  if (isUser) {
    return (
      <div className="group flex justify-end animate-fade-in">
        <div className="max-w-[75%]">
          <div className="rounded-2xl rounded-br-sm bg-bg-user-bubble text-white px-4 py-2.5">
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

  // Assistant: no bubble, text flows on background
  return (
    <div className="group animate-fade-in">
      <div className="text-base text-text-primary leading-relaxed whitespace-pre-wrap break-words">
        {message.content}
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

// ── Message Router (picks the right card for each message) ──

function MessageDisplay({
  message,
  isProcessing,
  isLatestDone,
  sessionId,
  onConfirm,
}: {
  message: Message;
  isProcessing: boolean;
  isLatestDone: boolean;
  sessionId: string | null;
  onConfirm: (messageId: string) => void;
}) {
  // User confirmation pill
  if (message.role === "user" && message.actionConfirmation) {
    return <ConfirmationPill timestamp={message.timestamp} />;
  }

  // Non-assistant messages use the regular bubble
  if (message.role !== "assistant") {
    return <MessageBubble message={message} />;
  }

  // Parse assistant messages for structured format
  const parsed = parseResponse(message.content);
  const hasActions = message.changeIds && message.changeIds.length > 0;

  let card: React.ReactNode;
  switch (parsed.type) {
    case "action":
      card = (
        <ActionCard
          situation={parsed.situation}
          plan={parsed.plan}
          actionLabel={parsed.actionLabel}
          actionTaken={message.actionTaken}
          isProcessing={isProcessing}
          timestamp={message.timestamp}
          onDoIt={() => onConfirm(message.id)}
        />
      );
      break;
    case "done":
      card = <DoneCard summary={parsed.summary} timestamp={message.timestamp} isLatestDone={isLatestDone} sessionId={sessionId} />;
      break;
    case "info":
      card = <InfoCard summary={parsed.summary} timestamp={message.timestamp} />;
      break;
    default:
      card = <MessageBubble message={message} />;
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

const TOOL_HUMAN_NAMES: Record<string, string> = {
  // macOS tools
  mac_network_info: "Checking network",
  mac_ping: "Testing connectivity",
  mac_dns_check: "Checking DNS",
  mac_http_check: "Testing web access",
  mac_flush_dns: "Flushing DNS cache",
  mac_system_info: "Checking system",
  mac_system_summary: "Running diagnostics",
  mac_process_list: "Listing processes",
  mac_disk_usage: "Checking disk space",
  mac_printer_list: "Checking printers",
  mac_print_queue: "Checking print queue",
  mac_app_list: "Listing applications",
  mac_app_logs: "Reading app logs",
  mac_read_file: "Reading file",
  mac_read_log: "Reading logs",
  shell_run: "Running command",
  mac_kill_process: "Stopping process",
  mac_clear_caches: "Clearing caches",
  mac_clear_app_cache: "Clearing app cache",
  mac_restart_cups: "Restarting print service",
  mac_cancel_print_jobs: "Cancelling print jobs",
  mac_move_file: "Moving file",
  // Windows tools
  win_network_info: "Checking network",
  win_ping: "Testing connectivity",
  win_dns_check: "Checking DNS",
  win_http_check: "Testing web access",
  win_flush_dns: "Flushing DNS cache",
  win_system_info: "Checking system",
  win_system_summary: "Running diagnostics",
  win_process_list: "Listing processes",
  win_disk_usage: "Checking disk space",
  win_printer_list: "Checking printers",
  win_print_queue: "Checking print queue",
  win_app_list: "Listing applications",
  win_app_logs: "Reading app logs",
  win_app_data_ls: "Browsing app data",
  win_read_file: "Reading file",
  win_read_log: "Reading logs",
  win_kill_process: "Stopping process",
  win_clear_caches: "Clearing caches",
  win_clear_app_cache: "Clearing app cache",
  win_restart_spooler: "Restarting print service",
  win_cancel_print_jobs: "Cancelling print jobs",
  win_move_file: "Moving file",
  win_startup_programs: "Checking startup programs",
  win_service_list: "Listing services",
  win_restart_service: "Restarting service",
  // Knowledge tools
  write_knowledge: "Saving knowledge",
  search_knowledge: "Searching knowledge",
  read_knowledge: "Reading knowledge",
  list_knowledge: "Listing knowledge",
};

function humanizeToolCall(summary: string): string {
  const match = summary.match(/Calling (\w+)/);
  if (!match) return "Working...";
  return TOOL_HUMAN_NAMES[match[1]] || "Working...";
}

// ── Thinking Indicator with live status ──

interface DebugLogPayload {
  event_type: string;
  summary: string;
}

function ThinkingIndicator() {
  const [status, setStatus] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const startRef = useRef(Date.now());

  useEffect(() => {
    const unlisten = listen<DebugLogPayload>("debug-log", (e) => {
      const evt = e.payload;
      if (evt.event_type === "tool_call") {
        setStatus(humanizeToolCall(evt.summary));
        startRef.current = Date.now(); // Reset timer on new tool
        setElapsed(0);
      } else if (evt.event_type === "llm_request") {
        setStatus("Thinking...");
        startRef.current = Date.now();
        setElapsed(0);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Tick elapsed time every second
  useEffect(() => {
    const timer = setInterval(() => {
      setElapsed(Math.floor((Date.now() - startRef.current) / 1000));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className="animate-fade-in py-1">
      <div className="flex items-center gap-2.5">
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
    </div>
  );
}

// ── Onboarding Suggestion Cards ──

const SUGGESTIONS = [
  { icon: "\uD83C\uDF10", label: "My internet is slow", description: "Diagnose network issues" },
  { icon: "\uD83D\uDC22", label: "My computer feels sluggish", description: "Check performance" },
  { icon: "\uD83D\uDCA5", label: "A program keeps crashing", description: "Find the cause" },
  { icon: "\uD83D\uDDA8\uFE0F", label: "Set up my printer", description: "Fix printing problems" },
];

function SuggestionCards({
  onSelect,
  disabled,
}: {
  onSelect: (text: string) => void;
  disabled: boolean;
}) {
  const [contextual, setContextual] = useState<
    { icon: string; label: string; description: string }[]
  >([]);

  useEffect(() => {
    commands.listKnowledge("issues").then((entries) => {
      setContextual(
        entries.slice(0, 2).map((e) => ({
          icon: "\uD83D\uDD04",
          label: `Check on: ${e.title}`,
          description: "Follow up on a previous issue",
        })),
      );
    }).catch(() => {});
  }, []);

  const allSuggestions = [...contextual, ...SUGGESTIONS].slice(0, 4);

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

function WelcomeHero({ hasContextual }: { hasContextual: boolean }) {
  return (
    <div className="flex flex-col items-center text-text-muted">
      <NoahIcon className="w-14 h-14 rounded-2xl mb-4" alt="Noah" />
      <p className="text-2xl font-semibold text-text-primary mb-1">
        Hey, I'm Noah
      </p>
      <p className="text-base text-text-secondary">
        {hasContextual
          ? "What's going on? Or check in on something I know about."
          : "Your computer helper. What's going on?"}
      </p>
    </div>
  );
}

// ── Chat Panel ──

export function ChatPanel() {
  const messages = useChatStore((s) => s.messages);
  const sessionId = useSessionStore((s) => s.sessionId);
  const { sendMessage, sendConfirmation, cancelProcessing, isProcessing } =
    useAgent();

  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isProcessing]);

  // Auto-resize textarea
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

  const showWelcome = messages.length === 0 || (messages.length === 1 && messages[0].role === "system");

  // Shared floating input card
  const inputCard = (
    <div className="max-w-4xl w-full mx-auto">
      <div className="flex items-end gap-2 bg-bg-secondary rounded-2xl border border-border-primary focus-within:border-accent-blue/40 transition-colors shadow-sm">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Tell Noah what you need help with..."
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
          /* Welcome: hero + cards + input centered in viewport */
          <div className="flex flex-col items-center justify-center h-full gap-8">
            <WelcomeHero hasContextual={false} />
            {!input.trim() && (
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
          /* Conversation: messages then input floating at bottom */
          <div className="flex flex-col min-h-full">
            <div className="max-w-3xl w-full mx-auto space-y-5 flex-1">
              {(() => {
                const latestDoneId = [...messages]
                  .reverse()
                  .find(
                    (m) =>
                      m.role === "assistant" && parseResponse(m.content).type === "done",
                  )?.id ?? null;

                return messages.map((msg) => (
                  <MessageDisplay
                    key={msg.id}
                    message={msg}
                    isProcessing={isProcessing}
                    isLatestDone={msg.id === latestDoneId}
                    sessionId={sessionId}
                    onConfirm={sendConfirmation}
                  />
                ));
              })()}
              {isProcessing && <ThinkingIndicator />}
            </div>
            <div className="sticky bottom-0 pt-6 pb-4 bg-gradient-to-t from-bg-primary from-90% to-transparent">
              {inputCard}
            </div>
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
