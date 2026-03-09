import { useState, useEffect, useCallback, useRef } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useSession } from "../hooks/useSession";
import { useLocale } from "../i18n";
import * as commands from "../lib/tauri-commands";
import type { SessionRecord } from "../lib/tauri-commands";

function formatDate(iso: string, t: (key: string, params?: Record<string, string | number>) => string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  const time = d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  if (diffDays === 0) return t("sidebar.dateFormat.today", { time });
  if (diffDays === 1) return t("sidebar.dateFormat.yesterday", { time });
  if (diffDays < 7)
    return `${d.toLocaleDateString([], { weekday: "short" })}, ${time}`;
  return d.toLocaleDateString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function OverflowMenu({
  session,
  onResolveToggle,
  onExport,
  onDelete,
  t,
}: {
  session: SessionRecord;
  onResolveToggle: () => void;
  onExport: () => void;
  onDelete: () => void;
  t: (key: string) => string;
}) {
  const [open, setOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false);
        setConfirmDelete(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={(e) => {
          e.stopPropagation();
          setOpen(!open);
          setConfirmDelete(false);
        }}
        className="w-6 h-6 rounded flex items-center justify-center text-text-muted hover:text-text-primary hover:bg-bg-tertiary transition-colors cursor-pointer opacity-0 group-hover:opacity-100"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <circle cx="2" cy="6" r="1.2" fill="currentColor" />
          <circle cx="6" cy="6" r="1.2" fill="currentColor" />
          <circle cx="10" cy="6" r="1.2" fill="currentColor" />
        </svg>
      </button>

      {open && (
        <div className="absolute left-0 top-full mt-1 w-40 bg-bg-secondary border border-border-primary rounded-lg shadow-xl z-50 py-1 overflow-hidden">
          {session.resolved !== true && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onResolveToggle();
                setOpen(false);
              }}
              className="w-full px-3 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-tertiary transition-colors cursor-pointer"
            >
              {t("sidebar.markResolved")}
            </button>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onExport();
              setOpen(false);
            }}
            className="w-full px-3 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-tertiary transition-colors cursor-pointer"
          >
            {t("sidebar.export")}
          </button>
          <div className="border-t border-border-primary mt-1 pt-1">
            {confirmDelete ? (
              <div className="flex items-center gap-2 px-3 py-1.5">
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete();
                    setOpen(false);
                    setConfirmDelete(false);
                  }}
                  className="text-xs text-accent-red font-medium cursor-pointer hover:underline"
                >
                  {t("sidebar.confirm")}
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmDelete(false);
                  }}
                  className="text-xs text-text-muted cursor-pointer hover:underline"
                >
                  {t("sidebar.cancel")}
                </button>
              </div>
            ) : (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setConfirmDelete(true);
                }}
                className="w-full px-3 py-1.5 text-left text-xs text-accent-red hover:bg-bg-tertiary transition-colors cursor-pointer"
              >
                {t("sidebar.delete")}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function SessionItem({
  session,
  isActive,
  onSelect,
  onExport,
  onDelete,
  onResolveToggle,
  t,
}: {
  session: SessionRecord;
  isActive: boolean;
  onSelect: (sessionId: string) => void;
  onExport: (sessionId: string, title: string) => void;
  onDelete: (sessionId: string) => void;
  onResolveToggle: (sessionId: string, resolved: boolean) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onSelect(session.id)}
      onKeyDown={(e) => { if (e.key === "Enter") onSelect(session.id); }}
      className={`group flex items-center gap-2 px-3 py-2 rounded-lg mx-2 cursor-pointer transition-colors ${
        isActive
          ? "bg-bg-tertiary text-text-primary"
          : "text-text-secondary hover:bg-bg-tertiary/50 hover:text-text-primary"
      }`}
    >
      <div className="flex-1 min-w-0">
        <p className="text-sm leading-snug truncate">
          {session.title || t("sidebar.untitledSession")}
        </p>
        <p className="text-[10px] text-text-muted mt-0.5">
          {formatDate(session.created_at, t)}
          {session.resolved === true && (
            <span className="text-accent-green ml-1.5">{"\u2713"}</span>
          )}
        </p>
      </div>
      <OverflowMenu
        session={session}
        onResolveToggle={() =>
          onResolveToggle(session.id, session.resolved !== true)
        }
        onExport={() =>
          onExport(session.id, session.title || "session")
        }
        onDelete={() => onDelete(session.id)}
        t={t}
      />
    </div>
  );
}

interface SidebarProps {
  session: {
    startNewProblem: () => Promise<void>;
  };
}

export function Sidebar({ session }: SidebarProps) {
  const sidebarOpen = useSessionStore((s) => s.sidebarOpen);
  const activeView = useSessionStore((s) => s.activeView);
  const setActiveView = useSessionStore((s) => s.setActiveView);
  const currentSessionId = useSessionStore((s) => s.sessionId);
  const pastSessions = useSessionStore((s) => s.pastSessions);
  const setPastSessions = useSessionStore((s) => s.setPastSessions);
  const { switchToProblem } = useSession();

  const loadSessions = useCallback(async () => {
    try {
      const sessions = await commands.listSessions();
      setPastSessions(sessions);
    } catch (err) {
      console.error("Failed to load session history:", err);
    }
  }, [setPastSessions]);

  // Load sessions when sidebar opens, session changes, or periodically while open
  useEffect(() => {
    if (sidebarOpen) {
      loadSessions();
    }
  }, [sidebarOpen, currentSessionId, loadSessions]);

  // Refresh session list periodically while sidebar is open (picks up title changes)
  useEffect(() => {
    if (!sidebarOpen) return;
    const timer = setInterval(loadSessions, 5000);
    return () => clearInterval(timer);
  }, [sidebarOpen, loadSessions]);

  const handleSelectSession = useCallback(
    async (sessionId: string) => {
      setActiveView("chat");
      await switchToProblem(sessionId);
    },
    [switchToProblem, setActiveView],
  );

  const handleExport = useCallback(
    async (sessionId: string, title: string) => {
      try {
        const markdown = await commands.exportSession(sessionId);
        const blob = new Blob([markdown], { type: "text/markdown" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `${title.replace(/[^a-zA-Z0-9 ]/g, "").replace(/\s+/g, "-").toLowerCase()}.md`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      } catch (err) {
        console.error("Failed to export session:", err);
      }
    },
    [],
  );

  const handleDelete = useCallback(
    async (sessionId: string) => {
      try {
        await commands.deleteSession(sessionId);
        setPastSessions(pastSessions.filter((s) => s.id !== sessionId));
      } catch (err) {
        console.error("Failed to delete session:", err);
      }
    },
    [pastSessions, setPastSessions],
  );

  const handleResolveToggle = useCallback(
    async (sessionId: string, resolved: boolean) => {
      try {
        await commands.markResolved(sessionId, resolved);
        setPastSessions(
          pastSessions.map((s) =>
            s.id === sessionId ? { ...s, resolved } : s,
          ),
        );
      } catch (err) {
        console.error("Failed to update session:", err);
      }
    },
    [pastSessions, setPastSessions],
  );

  const { t } = useLocale();

  const handleNewChat = useCallback(async () => {
    setActiveView("chat");
    await session.startNewProblem();
  }, [session, setActiveView]);

  if (!sidebarOpen) return null;

  return (
    <div className="w-64 flex-shrink-0 bg-bg-secondary border-r border-border-primary flex flex-col h-full">
      {/* Nav section */}
      <div className="px-2 pt-2 pb-2 space-y-1">
        {/* New chat */}
        <button
          onClick={handleNewChat}
          className="flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm text-text-primary hover:bg-bg-tertiary/50 transition-colors cursor-pointer"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M7 3V11M3 7H11" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          </svg>
          {t("sidebar.newChat")}
        </button>

        {/* Knowledge */}
        <button
          onClick={() => setActiveView(activeView === "knowledge" ? "chat" : "knowledge")}
          className={`flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm transition-colors cursor-pointer ${
            activeView === "knowledge"
              ? "bg-bg-tertiary text-text-primary"
              : "text-text-secondary hover:bg-bg-tertiary/50 hover:text-text-primary"
          }`}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M2 2.5C2 2.5 3.5 1.5 7 1.5C10.5 1.5 12 2.5 12 2.5V11.5C12 11.5 10.5 10.5 7 10.5C3.5 10.5 2 11.5 2 11.5V2.5Z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" />
            <path d="M7 1.5V10.5" stroke="currentColor" strokeWidth="1.1" />
          </svg>
          {t("sidebar.knowledge")}
        </button>

        {/* Actions */}
        <button
          onClick={() => setActiveView(activeView === "diagnostics" ? "chat" : "diagnostics")}
          className={`flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm transition-colors cursor-pointer ${
            activeView === "diagnostics"
              ? "bg-bg-tertiary text-text-primary"
              : "text-text-secondary hover:bg-bg-tertiary/50 hover:text-text-primary"
          }`}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <rect x="1" y="7" width="2.5" height="5.5" rx="0.5" stroke="currentColor" strokeWidth="1.1" />
            <rect x="5.75" y="4" width="2.5" height="8.5" rx="0.5" stroke="currentColor" strokeWidth="1.1" />
            <rect x="10.5" y="1.5" width="2.5" height="11" rx="0.5" stroke="currentColor" strokeWidth="1.1" />
          </svg>
          {t("sidebar.actions")}
        </button>
      </div>

      {/* Divider */}
      <div className="px-4">
        <div className="border-t border-border-primary" />
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-y-auto pt-2 pb-2">
        {pastSessions.length === 0 ? (
          <div className="px-4 py-6 text-center">
            <p className="text-xs text-text-muted">
              {t("sidebar.sessionsEmpty")}
            </p>
          </div>
        ) : (
          <div className="space-y-0.5">
            {pastSessions.map((s) => (
              <SessionItem
                key={s.id}
                session={s}
                isActive={s.id === currentSessionId}
                onSelect={handleSelectSession}
                onExport={handleExport}
                onDelete={handleDelete}
                onResolveToggle={handleResolveToggle}
                t={t}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
