import { useState, useRef, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";
import type { Message, ToolCall } from "../stores/chatStore";
import { useAgent } from "../hooks/useAgent";
import { VoiceButton } from "./VoiceButton";
import { parseResponse } from "../lib/parseResponse";
import * as commands from "../lib/tauri-commands";

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
          <span className="text-[10px] font-medium uppercase tracking-wider text-accent-green">
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
}: {
  summary: string;
  timestamp: number;
}) {
  return (
    <div className="flex justify-start animate-fade-in">
      <div className="max-w-[80%] rounded-xl border border-accent-green/30 bg-accent-green/5 px-4 py-3">
        <div className="flex items-start gap-2.5">
          <span className="text-accent-green text-base mt-0.5">{"\u2713"}</span>
          <div className="flex-1">
            <span className="text-[10px] font-medium uppercase tracking-wider text-accent-green">
              Done
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

        {message.toolCalls && message.toolCalls.length > 0 && (
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
  onConfirm,
}: {
  message: Message;
  isProcessing: boolean;
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

  switch (parsed.type) {
    case "action":
      return (
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
    case "done":
      return <DoneCard summary={parsed.summary} timestamp={message.timestamp} />;
    case "info":
      return <InfoCard summary={parsed.summary} timestamp={message.timestamp} />;
    default:
      return <MessageBubble message={message} />;
  }
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
  // Knowledge artifacts
  save_artifact: "Saving knowledge",
  query_artifacts: "Recalling knowledge",
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
    commands.getContextualSuggestions().then((suggestions) => {
      setContextual(
        suggestions.map((s) => ({
          icon: "\uD83D\uDD04",
          label: s.label,
          description: s.description,
        })),
      );
    }).catch(() => {});
  }, []);

  const allSuggestions = [...contextual, ...SUGGESTIONS].slice(0, 4);

  return (
    <div className="flex flex-col items-center justify-center h-full text-text-muted">
      <div className="w-16 h-16 rounded-2xl bg-accent-green/10 border border-accent-green/20 flex items-center justify-center mb-4">
        <svg
          width="28"
          height="28"
          viewBox="0 0 28 28"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M18 4a7 7 0 0 0-7.8 1.7L14 9.6l-1 2.8-2.8 1L6.3 9.5A7 7 0 0 0 8 17.3l7.8 7.8a1.7 1.7 0 0 0 2.4 0l6-6a1.7 1.7 0 0 0 0-2.4L18 4Z"
            stroke="currentColor"
            strokeWidth="1.5"
            fill="none"
            opacity="0.5"
          />
        </svg>
      </div>
      <p className="text-sm font-medium text-text-primary mb-1">
        What can I help with?
      </p>
      <p className="text-xs text-text-muted mb-5">
        {contextual.length > 0
          ? "Based on what I know about your system, or try something new."
          : "Try one of these, or type anything below."}
      </p>
      <div className="grid grid-cols-2 gap-2.5 w-full max-w-sm">
        {allSuggestions.map((s) => (
          <button
            key={s.label}
            onClick={() => onSelect(s.label)}
            disabled={disabled}
            className="flex items-start gap-2.5 px-3 py-2.5 rounded-xl border border-border-primary bg-bg-secondary hover:bg-bg-tertiary hover:border-border-focus transition-all text-left cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
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
  const setMessages = useChatStore((s) => s.setMessages);
  const viewingPastSession = useSessionStore((s) => s.viewingPastSession);
  const returnToCurrentSession = useSessionStore(
    (s) => s.returnToCurrentSession,
  );
  const pastSessions = useSessionStore((s) => s.pastSessions);
  const { sendMessage, sendConfirmation, cancelProcessing, isProcessing } =
    useAgent();

  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleBackToCurrent = useCallback(() => {
    const saved = returnToCurrentSession();
    if (saved) {
      setMessages(saved);
    }
  }, [returnToCurrentSession, setMessages]);

  const viewingSession = viewingPastSession
    ? pastSessions.find((s) => s.id === viewingPastSession)
    : null;

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

  const handleVoiceTranscript = useCallback((text: string) => {
    setInput((prev) => (prev ? prev + " " + text : text));
    textareaRef.current?.focus();
  }, []);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* Past session banner */}
      {viewingPastSession && (
        <div className="flex items-center justify-between px-4 py-2 bg-accent-purple/10 border-b border-accent-purple/20">
          <span className="text-xs text-text-secondary">
            Viewing past session
            {viewingSession?.title ? `: ${viewingSession.title}` : ""}
          </span>
          <button
            onClick={handleBackToCurrent}
            className="text-xs text-accent-green font-medium hover:underline cursor-pointer"
          >
            Back to current session
          </button>
        </div>
      )}

      {/* Messages area */}
      <div className="flex-1 overflow-y-auto px-4 py-4">
        {messages.length === 0 ? (
          <SuggestionCards
            onSelect={(text) => sendMessage(text)}
            disabled={isProcessing}
          />
        ) : messages.length === 1 && messages[0].role === "system" ? (
          <div className="max-w-2xl mx-auto space-y-4">
            <MessageDisplay
              message={messages[0]}
              isProcessing={isProcessing}
              onConfirm={sendConfirmation}
            />
            <SuggestionCards
              onSelect={(text) => sendMessage(text)}
              disabled={isProcessing}
            />
          </div>
        ) : (
          <div className="max-w-2xl mx-auto space-y-3">
            {messages.map((msg) => (
              <MessageDisplay
                key={msg.id}
                message={msg}
                isProcessing={isProcessing}
                onConfirm={sendConfirmation}
              />
            ))}
            {isProcessing && <ThinkingIndicator />}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* Input area — hidden when viewing past session */}
      {!viewingPastSession && (
        <div className="border-t border-border-primary bg-bg-secondary px-4 py-3">
          <div className="max-w-2xl mx-auto">
            <div className="flex items-end gap-2 bg-bg-input rounded-xl border border-border-primary focus-within:border-border-focus transition-colors">
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
                <VoiceButton onTranscript={handleVoiceTranscript} />
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
      )}
    </div>
  );
}
