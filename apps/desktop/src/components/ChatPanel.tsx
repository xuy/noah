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

const ACTION_LABELS: Record<string, string> = {
  shell_run: "Ran a command",
  mac_network_info: "Checked network",
  mac_ping: "Tested connectivity",
  mac_dns_check: "Checked DNS",
  mac_http_check: "Tested web access",
  mac_flush_dns: "Flushed DNS cache",
  mac_system_info: "Checked system info",
  mac_system_summary: "Ran diagnostics",
  mac_process_list: "Listed processes",
  mac_disk_usage: "Checked disk space",
  mac_printer_list: "Checked printers",
  mac_print_queue: "Checked print queue",
  mac_app_list: "Listed applications",
  mac_app_logs: "Read app logs",
  mac_read_file: "Read a file",
  mac_read_log: "Read logs",
  mac_kill_process: "Stopped a process",
  mac_clear_caches: "Cleared caches",
  mac_clear_app_cache: "Cleared app cache",
  mac_restart_cups: "Restarted print service",
  mac_cancel_print_jobs: "Cancelled print jobs",
  mac_move_file: "Moved a file",
  win_network_info: "Checked network",
  win_ping: "Tested connectivity",
  win_dns_check: "Checked DNS",
  win_http_check: "Tested web access",
  win_flush_dns: "Flushed DNS cache",
  win_system_info: "Checked system info",
  win_system_summary: "Ran diagnostics",
  win_process_list: "Listed processes",
  win_disk_usage: "Checked disk space",
  win_printer_list: "Checked printers",
  win_print_queue: "Checked print queue",
  win_app_list: "Listed applications",
  win_app_logs: "Read app logs",
  win_app_data_ls: "Browsed app data",
  win_read_file: "Read a file",
  win_read_log: "Read logs",
  win_kill_process: "Stopped a process",
  win_clear_caches: "Cleared caches",
  win_clear_app_cache: "Cleared app cache",
  win_restart_spooler: "Restarted print service",
  win_cancel_print_jobs: "Cancelled print jobs",
  win_move_file: "Moved a file",
  win_startup_programs: "Checked startup programs",
  win_service_list: "Listed services",
  win_restart_service: "Restarted a service",
  write_knowledge: "Saved a note",
  search_knowledge: "Searched notes",
  read_knowledge: "Read a note",
  list_knowledge: "Listed notes",
};

/** Map common shell commands to human-readable descriptions. */
const SHELL_PATTERNS: [RegExp, string][] = [
  [/^uptime/, "Checked how long the computer has been running"],
  [/^(top|ps\b|activity)/, "Checked running processes"],
  [/^(df|diskutil|du)\b/, "Checked disk space"],
  [/^(ifconfig|networksetup|ipconfig|scutil)\b/, "Checked network settings"],
  [/^(ping|arping)\s/, "Tested network connection"],
  [/^(nslookup|dig|host)\s/, "Looked up DNS records"],
  [/^traceroute\s/, "Traced network route"],
  [/^(curl|wget)\s/, "Fetched data from the web"],
  [/^(cat|less|more|head|tail)\s/, "Read a file"],
  [/^ls\b/, "Listed files"],
  [/^(mkdir|mktemp)\b/, "Created a folder"],
  [/^(cp|rsync)\b/, "Copied files"],
  [/^(mv)\b/, "Moved files"],
  [/^(chmod|chown|icacls)\b/, "Changed file permissions"],
  [/^(killall|taskkill)\s+(.+)/, "Stopped $2"],
  [/^(launchctl|systemctl)\b/, "Managed system services"],
  [/^(defaults)\s+(read|write)\s+(.+)/, "Changed system preferences"],
  [/^(defaults)\s+read\b/, "Checked system preferences"],
  [/^(brew|apt|yum|choco|winget|scoop)\s+install\b/, "Installed software"],
  [/^(brew|apt|yum|choco|winget|scoop)\s+upgrade\b/, "Updated software"],
  [/^(brew|apt|yum|choco|winget|scoop)\s+(list|info)\b/, "Checked installed software"],
  [/^(softwareupdate|wuauclt)\b/, "Checked for system updates"],
  [/^(open|start)\s/, "Opened an application"],
  [/^(pmset|powercfg)\b/, "Checked power settings"],
  [/^(sysctl|system_profiler|systeminfo)\b/, "Checked system information"],
  [/^(sw_vers|ver|winver)\b/, "Checked OS version"],
  [/^(lsof)\b/, "Checked open files and connections"],
  [/^(netstat|ss)\b/, "Checked network connections"],
  [/^(mdutil|mdfind)\b/, "Checked Spotlight indexing"],
  [/^(tmutil)\b/, "Checked Time Machine"],
  [/^(spctl|csrutil)\b/, "Checked security settings"],
  [/^(dscacheutil)\b/, "Checked directory cache"],
  [/^(log\s+show|journalctl)\b/, "Read system logs"],
  [/^(wmic|Get-WmiObject|Get-CimInstance)/, "Checked system details"],
  [/^(netsh)\b/, "Checked network configuration"],
  [/^(sfc|DISM|chkdsk)\b/i, "Ran system repair tool"],
];

/** Extract a human-friendly label from a shell command string. */
function humanizeShellCommand(cmd: string): string {
  const trimmed = cmd.trim();
  for (const [pattern, label] of SHELL_PATTERNS) {
    const m = trimmed.match(pattern);
    if (m) return label.replace(/\$(\d+)/g, (_, i) => m[+i] || "");
  }
  return "";
}

/** Turn a raw tool description into something a non-technical user understands. */
function humanizeDescription(_toolName: string, description: string): string {
  // "Executed shell command: <cmd>" → parse the command for a friendly label
  if (description.startsWith("Executed shell command:")) {
    const cmd = description.slice("Executed shell command:".length).trim();
    return humanizeShellCommand(cmd);
  }
  // "Killed process 79514 with signal 15" → "Stopped a runaway process"
  if (/^Killed process \d+/.test(description)) return "Stopped a runaway process";
  // "Cleared contents of /Users/..." → "Cleared system caches"
  if (description.startsWith("Cleared contents of")) return "Cleared system caches";
  // "Set DNS to ..." → keep as-is, it's already clear
  return description;
}

function ChangesBlock({ changeIds }: { changeIds: string[] }) {
  const [expanded, setExpanded] = useState(false);
  const changes = useSessionStore((s) => s.changes);
  const matched = changes.filter((c) => changeIds.includes(c.id));

  if (matched.length === 0) return null;

  return (
    <div className="mt-2 rounded-md border border-border-primary bg-bg-primary/50 overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left cursor-pointer hover:bg-bg-tertiary/30 transition-colors"
      >
        <svg width="12" height="12" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M8.5 1.5L12.5 5.5L5 13H1V9L8.5 1.5Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
        </svg>
        <span className="text-accent-purple font-medium">
          {matched.length} action{matched.length !== 1 ? "s" : ""} taken
        </span>
        <span className="text-text-muted ml-auto">
          {expanded ? "\u25B4" : "\u25BE"}
        </span>
      </button>
      {expanded && (
        <div className="px-3 py-2 border-t border-border-primary text-xs space-y-1.5">
          {matched.map((c) => {
            const label = ACTION_LABELS[c.tool_name] || c.tool_name.replace(/_/g, " ");
            const raw = humanizeDescription(c.tool_name, c.description);
            // Don't repeat if the detail is the same as the label
            const detail = raw && raw.toLowerCase() !== label.toLowerCase() ? raw : "";
            return (
              <div key={c.id} className="flex items-start gap-2" title={c.description}>
                <span className="text-text-secondary leading-snug">
                  {label}{detail ? ` \u2014 ${detail}` : ""}
                </span>
              </div>
            );
          })}
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
    <div className="flex justify-start animate-fade-in">
      <div className="max-w-[80%] rounded-xl border border-border-primary bg-bg-assistant-bubble overflow-hidden">
        {/* Header */}
        <div className="px-4 pt-3 pb-1">
          <span className="text-[11px] font-semibold text-accent-blue">
            Noah
          </span>
        </div>

        {/* Situation */}
        <div className="px-4 pb-2">
          <div className="text-[10px] uppercase tracking-wider text-text-muted mb-1">
            Situation
          </div>
          <div className="text-sm text-text-primary leading-relaxed">
            {situation}
          </div>
        </div>

        {/* Plan */}
        <div className="px-4 pb-3">
          <div className="text-[10px] uppercase tracking-wider text-text-muted mb-1">
            Plan
          </div>
          <div className="text-sm text-text-secondary leading-relaxed">
            {plan}
          </div>
        </div>

        {/* Action button */}
        <div className="px-4 pb-3">
          <button
            onClick={onDoIt}
            disabled={actionTaken || isProcessing}
            className={`
              w-full py-2.5 rounded-lg text-sm font-medium transition-all cursor-pointer
              ${
                actionTaken
                  ? "bg-bg-tertiary text-text-muted cursor-default"
                  : isProcessing
                    ? "bg-bg-tertiary text-text-muted cursor-not-allowed"
                    : "bg-accent-green text-white hover:bg-accent-green/80"
              }
            `}
          >
            {actionTaken ? "Sent" : actionLabel}
          </button>
        </div>

        {/* Timestamp */}
        <div className="px-4 pb-2 text-[10px] text-text-muted">
          {new Date(timestamp).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </div>
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
    <div className="flex justify-start animate-fade-in">
      <div className="max-w-[80%]">
        <div className="rounded-xl border border-accent-green/30 bg-accent-green/5 px-4 py-3">
          <div className="flex items-start gap-2.5">
            <span className="text-accent-green text-base mt-0.5">{"\u2713"}</span>
            <div className="flex-1">
              <span className="text-[10px] font-medium uppercase tracking-wider text-accent-green">
                Noah
              </span>
              <div className="text-sm text-text-primary leading-relaxed mt-1">
                {summary}
              </div>
              <div className="text-[10px] text-text-muted mt-1.5">
                {new Date(timestamp).toLocaleTimeString([], {
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </div>
            </div>
          </div>
        </div>

        {/* Resolution prompt — only on the latest done card, after load */}
        {isLatestDone && loaded && resolved === null && (
          <div className="flex items-center gap-2 mt-2 ml-1">
            <span className="text-[11px] text-text-muted">
              Did this fix your issue?
            </span>
            <button
              onClick={() => handleResolve(true)}
              className="px-2.5 py-1 rounded-md text-[11px] font-medium text-accent-green bg-accent-green/10 hover:bg-accent-green/20 transition-colors cursor-pointer"
            >
              Yes, all good
            </button>
            <button
              onClick={() => handleResolve(false)}
              className="px-2.5 py-1 rounded-md text-[11px] text-text-muted hover:bg-bg-tertiary transition-colors cursor-pointer"
            >
              Not quite
            </button>
          </div>
        )}

        {/* Resolution confirmation */}
        {resolved === true && (
          <div className="flex items-center gap-1.5 mt-2 ml-1">
            <span className="text-accent-green text-xs">{"\u2713"}</span>
            <span className="text-[11px] text-text-muted">
              Marked as resolved
            </span>
          </div>
        )}
        {resolved === false && (
          <div className="mt-2 ml-1">
            <span className="text-[11px] text-text-muted">
              Got it &mdash; keep chatting and I'll keep working on it.
            </span>
          </div>
        )}
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
    <div className="flex justify-start animate-fade-in">
      <div className="max-w-[80%] rounded-xl border border-accent-blue/30 bg-accent-blue/5 px-4 py-3">
        <div className="flex items-start gap-2.5">
          <span className="text-accent-blue text-base mt-0.5">{"\u2139"}</span>
          <div className="flex-1">
            <span className="text-[10px] font-medium uppercase tracking-wider text-accent-green">
              Noah
            </span>
            <div className="text-sm text-text-primary leading-relaxed mt-1">
              {summary}
            </div>
            <div className="text-[10px] text-text-muted mt-1.5">
              {new Date(timestamp).toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Confirmation Pill (for "Go ahead" user messages) ──

function ConfirmationPill({ timestamp }: { timestamp: number }) {
  return (
    <div className="flex justify-end animate-fade-in">
      <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-accent-green/15 text-accent-green text-xs font-medium">
        <span>{"\u2713"}</span>
        <span>Go ahead</span>
        <span className="text-[10px] text-accent-green/60 ml-1">
          {new Date(timestamp).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </span>
      </div>
    </div>
  );
}

// ── Single Message Bubble (fallback for unstructured messages) ──

function MessageBubble({ message }: { message: Message }) {
  const isUser = message.role === "user";

  return (
    <div
      className={`flex animate-fade-in ${
        isUser ? "justify-end" : "justify-start"
      }`}
    >
      <div
        className={`
          max-w-[80%] rounded-xl px-4 py-2.5
          ${
            isUser
              ? "bg-bg-user-bubble text-white rounded-br-sm"
              : "bg-bg-assistant-bubble text-text-primary border border-border-primary rounded-bl-sm"
          }
        `}
      >
        {!isUser && (
          <div className="flex items-center gap-1.5 mb-1">
            <span className="text-[10px] font-medium uppercase tracking-wider text-accent-green">
              Noah
            </span>
          </div>
        )}

        <div className="text-sm leading-relaxed whitespace-pre-wrap break-words">
          {message.content}
        </div>

        {showToolCalls && message.toolCalls && message.toolCalls.length > 0 && (
          <div className="mt-1">
            {message.toolCalls.map((tc) => (
              <ToolCallItem key={tc.id} toolCall={tc} />
            ))}
          </div>
        )}

        <div
          className={`text-[10px] mt-1 ${
            isUser ? "text-white/50 text-right" : "text-text-muted"
          }`}
        >
          {new Date(message.timestamp).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </div>
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
      <div className="mt-1 ml-1 max-w-[80%]">
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
    <div className="flex justify-start animate-fade-in">
      <div className="bg-bg-assistant-bubble border border-border-primary rounded-xl rounded-bl-sm px-4 py-3">
        <div className="flex items-center gap-1.5">
          <span className="text-[10px] font-medium uppercase tracking-wider text-accent-green mb-1">
            Noah
          </span>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1">
            <div className="w-1.5 h-1.5 rounded-full bg-text-muted thinking-dot" />
            <div className="w-1.5 h-1.5 rounded-full bg-text-muted thinking-dot" />
            <div className="w-1.5 h-1.5 rounded-full bg-text-muted thinking-dot" />
          </div>
          {status && (
            <span className="text-xs text-text-muted">
              {status}
              {elapsed > 0 && (
                <span className="ml-1 text-text-muted/60">{elapsed}s</span>
              )}
            </span>
          )}
        </div>
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
    <div className="flex flex-col items-center justify-center h-full text-text-muted">
      <NoahIcon className="w-14 h-14 rounded-2xl mb-5" alt="Noah" />
      <p className="text-base font-semibold text-text-primary mb-1">
        How can I help?
      </p>
      <p className="text-sm text-text-secondary mb-6">
        {contextual.length > 0
          ? "Based on what I know about your system, or try something new."
          : "Describe your problem, or try one of these:"}
      </p>
      <div className="grid grid-cols-2 gap-2.5 w-full max-w-sm">
        {allSuggestions.map((s) => (
          <button
            key={s.label}
            onClick={() => onSelect(s.label)}
            disabled={disabled}
            className="flex items-start gap-2.5 px-3.5 py-3 rounded-xl border border-border-primary bg-bg-secondary hover:bg-bg-tertiary hover:border-accent-blue/40 transition-all text-left cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <span className="text-base mt-0.5">{s.icon}</span>
            <div className="min-w-0">
              <div className="text-xs font-medium text-text-primary leading-snug">
                {s.label}
              </div>
              <div className="text-[10px] text-text-muted mt-0.5">
                {s.description}
              </div>
            </div>
          </button>
        ))}
      </div>
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
      el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
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

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* Messages area */}
      <div className="flex-1 overflow-y-auto px-4 py-4">
        {(() => {
          // Find the last "done" message so only it shows the resolution prompt
          const latestDoneId = [...messages]
            .reverse()
            .find(
              (m) =>
                m.role === "assistant" && parseResponse(m.content).type === "done",
            )?.id ?? null;

          if (messages.length === 0) {
            return (
              <SuggestionCards
                onSelect={(text) => sendMessage(text)}
                disabled={isProcessing}
              />
            );
          }
          if (messages.length === 1 && messages[0].role === "system") {
            return (
              <div className="max-w-2xl mx-auto space-y-4">
                <MessageDisplay
                  message={messages[0]}
                  isProcessing={isProcessing}
                  isLatestDone={messages[0].id === latestDoneId}
                  sessionId={sessionId}
                  onConfirm={sendConfirmation}
                />
                <SuggestionCards
                  onSelect={(text) => sendMessage(text)}
                  disabled={isProcessing}
                />
              </div>
            );
          }
          return (
            <div className="max-w-2xl mx-auto space-y-3">
              {messages.map((msg) => (
                <MessageDisplay
                  key={msg.id}
                  message={msg}
                  isProcessing={isProcessing}
                  isLatestDone={msg.id === latestDoneId}
                  sessionId={sessionId}
                  onConfirm={sendConfirmation}
                />
              ))}
              {isProcessing && <ThinkingIndicator />}
              <div ref={messagesEndRef} />
            </div>
          );
        })()}
      </div>

      {/* Input area */}
      <div className="border-t border-border-primary bg-bg-secondary px-4 py-3">
          <div className="max-w-2xl mx-auto">
            <div className="flex items-end gap-2 bg-bg-input rounded-xl border border-border-primary focus-within:border-accent-blue/40 transition-colors">
              <textarea
                ref={textareaRef}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Tell Noah what you need help with..."
                rows={1}
                disabled={isProcessing}
                className="flex-1 bg-transparent text-sm text-text-primary placeholder-text-muted px-4 py-2.5 resize-none outline-none min-h-[38px] max-h-[120px]"
              />
              <div className="flex items-center gap-1 pr-2 pb-1.5">
                {isProcessing ? (
                  <button
                    onClick={cancelProcessing}
                    title="Stop processing"
                    className="flex items-center justify-center w-9 h-9 rounded-lg bg-accent-red/15 text-accent-red hover:bg-accent-red/25 transition-all duration-200 cursor-pointer"
                  >
                    <svg
                      width="14"
                      height="14"
                      viewBox="0 0 14 14"
                      fill="none"
                      xmlns="http://www.w3.org/2000/svg"
                    >
                      <rect
                        x="3"
                        y="3"
                        width="8"
                        height="8"
                        rx="1.5"
                        fill="currentColor"
                      />
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
                    <svg
                      width="16"
                      height="16"
                      viewBox="0 0 16 16"
                      fill="none"
                      xmlns="http://www.w3.org/2000/svg"
                    >
                      <path d="M2 8L14 2L8 14V8H2Z" fill="currentColor" />
                    </svg>
                  </button>
                )}
              </div>
            </div>
            <p className="text-[10px] text-text-muted mt-1.5 text-center">
              Press Enter to send, Shift+Enter for new line
            </p>
          </div>
        </div>
    </div>
  );
}
