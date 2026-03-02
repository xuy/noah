import { useEffect, useCallback } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";
import type { SessionRecord } from "../lib/tauri-commands";

function formatDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  const time = d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  if (diffDays === 0) return `Today, ${time}`;
  if (diffDays === 1) return `Yesterday, ${time}`;
  if (diffDays < 7)
    return `${d.toLocaleDateString([], { weekday: "short" })}, ${time}`;
  return d.toLocaleDateString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDuration(created: string, ended: string | null): string {
  if (!ended) return "In progress";
  const ms = new Date(ended).getTime() - new Date(created).getTime();
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) return `${seconds}s`;
  return `${minutes}m ${seconds}s`;
}

function SessionItem({
  session,
  onSelect,
}: {
  session: SessionRecord;
  onSelect: (sessionId: string) => void;
}) {
  const isActive = !session.ended_at;

  return (
    <div className="border-b border-border-primary last:border-b-0">
      <button
        onClick={() => onSelect(session.id)}
        className="w-full px-4 py-3 text-left hover:bg-bg-tertiary/50 transition-colors cursor-pointer"
      >
        <div className="flex items-start justify-between gap-2">
          <div className="flex-1 min-w-0">
            {/* Title */}
            <p className="text-sm text-text-primary leading-snug truncate">
              {session.title || "Untitled session"}
            </p>

            {/* Meta row */}
            <div className="flex items-center gap-2 mt-1">
              <span className="text-[10px] text-text-muted">
                {formatDate(session.created_at)}
              </span>
              <span className="text-[10px] text-text-muted">
                {formatDuration(session.created_at, session.ended_at)}
              </span>
            </div>

            {/* Stats */}
            <div className="flex items-center gap-3 mt-1.5">
              {session.message_count > 0 && (
                <span className="text-[10px] text-text-muted">
                  {session.message_count} msg{session.message_count !== 1 ? "s" : ""}
                </span>
              )}
              {session.change_count > 0 && (
                <span className="text-[10px] text-accent-purple">
                  {session.change_count} change{session.change_count !== 1 ? "s" : ""}
                </span>
              )}
            </div>
          </div>

          <div className="flex items-center gap-2 flex-shrink-0 mt-1">
            {isActive && (
              <span className="w-2 h-2 rounded-full bg-status-active" />
            )}
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
              className="text-text-muted"
            >
              <path
                d="M3 1.5L7 5L3 8.5"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </div>
        </div>
      </button>
    </div>
  );
}

export function SessionHistory() {
  const historyOpen = useSessionStore((s) => s.historyOpen);
  const setHistoryOpen = useSessionStore((s) => s.setHistoryOpen);
  const pastSessions = useSessionStore((s) => s.pastSessions);
  const setPastSessions = useSessionStore((s) => s.setPastSessions);
  const viewPastSession = useSessionStore((s) => s.viewPastSession);
  const setMessages = useChatStore((s) => s.setMessages);

  const loadSessions = useCallback(async () => {
    try {
      const sessions = await commands.listSessions();
      setPastSessions(sessions);
    } catch (err) {
      console.error("Failed to load session history:", err);
    }
  }, [setPastSessions]);

  const handleSelectSession = useCallback(
    async (sessionId: string) => {
      try {
        const records = await commands.getSessionMessages(sessionId);
        const loaded = records.map((r) => ({
          id: r.id,
          role: r.role as "user" | "assistant" | "system",
          content: r.content,
          timestamp: new Date(r.timestamp).getTime(),
        }));
        // Read current messages directly from store (avoids stale closure).
        // Set loaded messages BEFORE marking viewingPastSession so the
        // re-render never sees an empty message list.
        const currentMessages = useChatStore.getState().messages;
        setMessages(loaded);
        viewPastSession(sessionId, currentMessages);
      } catch (err) {
        console.error("Failed to load session messages:", err);
      }
    },
    [viewPastSession, setMessages],
  );

  // Load sessions when panel opens.
  useEffect(() => {
    if (historyOpen) {
      loadSessions();
    }
  }, [historyOpen, loadSessions]);

  if (!historyOpen) return null;

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-30 bg-black/20"
        onClick={() => setHistoryOpen(false)}
      />

      {/* Slide-out panel */}
      <div className="fixed top-0 left-0 bottom-0 z-40 w-80 bg-bg-secondary border-r border-border-primary shadow-2xl flex flex-col animate-slide-in-left">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-primary">
          <h2 className="text-sm font-semibold text-text-primary">
            Session History
          </h2>
          <button
            onClick={() => setHistoryOpen(false)}
            className="w-7 h-7 rounded-md flex items-center justify-center text-text-muted hover:text-text-primary hover:bg-bg-tertiary transition-colors cursor-pointer"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 14 14"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M3 3L11 11M11 3L3 11"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        {/* Sessions list */}
        <div className="flex-1 overflow-y-auto">
          {pastSessions.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-text-muted px-4">
              <svg
                width="32"
                height="32"
                viewBox="0 0 32 32"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
                className="mb-3 opacity-50"
              >
                <path
                  d="M16 6V16L22 22"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
                <circle
                  cx="16"
                  cy="16"
                  r="12"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
              </svg>
              <p className="text-xs text-center">
                No past sessions yet.
                <br />
                Sessions will appear here as you use the app.
              </p>
            </div>
          ) : (
            <div>
              {pastSessions.map((session) => (
                <SessionItem
                  key={session.id}
                  session={session}
                  onSelect={handleSelectSession}
                />
              ))}
            </div>
          )}
        </div>

        {/* Footer summary */}
        {pastSessions.length > 0 && (
          <div className="px-4 py-2.5 border-t border-border-primary">
            <p className="text-[10px] text-text-muted">
              {pastSessions.length} session{pastSessions.length !== 1 ? "s" : ""} total
            </p>
          </div>
        )}
      </div>
    </>
  );
}
