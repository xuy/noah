import { useState, useEffect, useCallback, useRef } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useSession } from "../hooks/useSession";
import type { SessionMode } from "../hooks/useSession";
import { useLocale } from "../i18n";
import * as commands from "../lib/tauri-commands";
import type { SessionRecord } from "../lib/tauri-commands";
import { isMac } from "../lib/platform";
import { SidebarToggleIcon, SettingsGearIcon } from "./MainTitleBar";

// Map app locale to BCP 47 tag for Intl date/time formatting.
const localeBcp47: Record<string, string> = { zh: "zh-CN", en: "en-US" };

function formatDate(iso: string, t: (key: string, params?: Record<string, string | number>) => string, locale: string): string {
  const bcp = localeBcp47[locale] || locale;
  const d = new Date(iso);
  const now = new Date();

  // Compare calendar dates in the user's local timezone (not raw ms difference,
  // which misclassifies e.g. yesterday 11 PM as "today" when viewed at 10 AM).
  const localDate = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const todayDate = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const diffDays = Math.round((todayDate.getTime() - localDate.getTime()) / (1000 * 60 * 60 * 24));

  const time = d.toLocaleTimeString(bcp, {
    hour: "2-digit",
    minute: "2-digit",
  });

  if (diffDays === 0) return t("sidebar.today", { time });
  if (diffDays === 1) return t("sidebar.yesterday", { time });
  if (diffDays < 7)
    return `${d.toLocaleDateString(bcp, { weekday: "short" })}, ${time}`;
  return d.toLocaleDateString(bcp, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Context menu state shared across all session items (only one open at a time). */
interface ContextMenuState {
  session: SessionRecord;
  x: number;
  y: number;
  confirmDelete: boolean;
}

function ContextMenu({
  menu,
  setMenu,
  onRename,
  onResolveToggle,
  onExport,
  onDelete,
  t,
}: {
  menu: ContextMenuState;
  setMenu: (m: ContextMenuState | null) => void;
  onRename: (sessionId: string) => void;
  onResolveToggle: (sessionId: string, resolved: boolean) => void;
  onExport: (sessionId: string, title: string) => void;
  onDelete: (sessionId: string) => void;
  t: (key: string) => string;
}) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenu(null);
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [setMenu]);

  return (
    <>
      {/* Transparent backdrop to catch outside clicks */}
      <div className="fixed inset-0 z-[9998]" onClick={() => setMenu(null)} />
      <div
        style={{ position: "fixed", left: menu.x, top: menu.y, zIndex: 9999 }}
        className="w-44 bg-bg-secondary border border-border-primary rounded-lg shadow-2xl py-1"
      >
        <button
          onClick={() => {
            onRename(menu.session.id);
            setMenu(null);
          }}
          className="w-full px-3 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-tertiary transition-colors cursor-pointer"
        >
          {t("sidebar.rename")}
        </button>
        {menu.session.resolved !== true && (
          <button
            onClick={() => {
              onResolveToggle(menu.session.id, true);
              setMenu(null);
            }}
            className="w-full px-3 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-tertiary transition-colors cursor-pointer"
          >
            {t("sidebar.markResolved")}
          </button>
        )}
        <button
          onClick={() => {
            onExport(menu.session.id, menu.session.title || "session");
            setMenu(null);
          }}
          className="w-full px-3 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-tertiary transition-colors cursor-pointer"
        >
          {t("sidebar.export")}
        </button>
        <div className="border-t border-border-primary mt-1 pt-1">
          {menu.confirmDelete ? (
            <div className="flex items-center gap-2 px-3 py-1.5">
              <button
                onClick={() => {
                  onDelete(menu.session.id);
                  setMenu(null);
                }}
                className="text-xs text-accent-red font-medium cursor-pointer hover:underline"
              >
                {t("sidebar.confirm")}
              </button>
              <button
                onClick={() => setMenu({ ...menu, confirmDelete: false })}
                className="text-xs text-text-muted cursor-pointer hover:underline"
              >
                {t("sidebar.cancel")}
              </button>
            </div>
          ) : (
            <button
              onClick={() => setMenu({ ...menu, confirmDelete: true })}
              className="w-full px-3 py-1.5 text-left text-xs text-accent-red hover:bg-bg-tertiary transition-colors cursor-pointer"
            >
              {t("sidebar.delete")}
            </button>
          )}
        </div>
      </div>
    </>
  );
}

function SessionItem({
  session,
  isActive,
  isRenaming,
  onSelect,
  onContextMenu,
  onRenameSubmit,
  onRenameCancel,
  t,
  locale,
}: {
  session: SessionRecord;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: (sessionId: string) => void;
  onContextMenu: (e: React.MouseEvent, session: SessionRecord) => void;
  onRenameSubmit: (sessionId: string, title: string) => void;
  onRenameCancel: () => void;
  t: (key: string, params?: Record<string, string | number>) => string;
  locale: string;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [renameValue, setRenameValue] = useState(session.title || "");

  useEffect(() => {
    if (isRenaming && inputRef.current) {
      setRenameValue(session.title || "");
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isRenaming, session.title]);

  if (isRenaming) {
    return (
      <div className="px-3 py-1.5 mx-2">
        <input
          ref={inputRef}
          value={renameValue}
          onChange={(e) => setRenameValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const trimmed = renameValue.trim();
              if (trimmed) onRenameSubmit(session.id, trimmed);
              else onRenameCancel();
            } else if (e.key === "Escape") {
              onRenameCancel();
            }
          }}
          onBlur={() => {
            const trimmed = renameValue.trim();
            if (trimmed) onRenameSubmit(session.id, trimmed);
            else onRenameCancel();
          }}
          className="w-full text-sm bg-bg-primary border border-accent-blue rounded px-1.5 py-1 text-text-primary outline-none"
        />
      </div>
    );
  }

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onSelect(session.id)}
      onKeyDown={(e) => { if (e.key === "Enter") onSelect(session.id); }}
      onContextMenu={(e) => onContextMenu(e, session)}
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
          {formatDate(session.created_at, t, locale)}
          {session.resolved === true && (
            <span className="text-accent-green ml-1.5">{"\u2713"}</span>
          )}
        </p>
      </div>
    </div>
  );
}

interface SidebarProps {
  session: {
    startNewProblem: (mode?: SessionMode) => Promise<void>;
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
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [renamingSessionId, setRenamingSessionId] = useState<string | null>(null);

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

  const handleRenameStart = useCallback((sessionId: string) => {
    setRenamingSessionId(sessionId);
  }, []);

  const handleRenameSubmit = useCallback(
    async (sessionId: string, title: string) => {
      setRenamingSessionId(null);
      try {
        await commands.renameSession(sessionId, title);
        setPastSessions(
          pastSessions.map((s) =>
            s.id === sessionId ? { ...s, title } : s,
          ),
        );
      } catch (err) {
        console.error("Failed to rename session:", err);
      }
    },
    [pastSessions, setPastSessions],
  );

  const handleRenameCancel = useCallback(() => {
    setRenamingSessionId(null);
  }, []);

  const { t, locale } = useLocale();

  const handleNewChat = useCallback(async () => {
    setActiveView("chat");
    await session.startNewProblem();
  }, [session, setActiveView]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, s: SessionRecord) => {
      e.preventDefault();
      setContextMenu({ session: s, x: e.clientX, y: e.clientY, confirmDelete: false });
    },
    [],
  );

  const toggleSidebar = useSessionStore((s) => s.toggleSidebar);
  const settingsActive = activeView === "settings";

  if (!sidebarOpen) {
    return (
      <div className="w-14 flex-shrink-0 bg-bg-secondary border-r border-border-primary flex flex-col h-full">
        <div className="px-2 pt-2 pb-2">
          {!isMac && (
            <button
              onClick={toggleSidebar}
              title="Show sidebar"
              aria-label="Show sidebar"
              className="flex items-center justify-center w-9 h-9 rounded-lg text-text-muted hover:text-text-primary hover:bg-bg-tertiary/50 transition-colors cursor-pointer"
            >
              <SidebarToggleIcon />
            </button>
          )}
        </div>

        <div className="mt-auto px-2 pb-2 pt-1 border-t border-border-primary">
          <button
            onClick={() => setActiveView(settingsActive ? "chat" : "settings")}
            title={t("sidebar.settings")}
            aria-label={t("sidebar.settings")}
            className={`flex items-center justify-center w-full h-9 rounded-lg transition-colors cursor-pointer ${
              settingsActive
                ? "bg-accent-blue/15 text-accent-blue"
                : "text-text-secondary hover:bg-bg-tertiary/50 hover:text-text-primary"
            }`}
          >
            <SettingsGearIcon />
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="w-64 flex-shrink-0 bg-bg-secondary border-r border-border-primary flex flex-col h-full">
      {/* Nav section */}
      <div className="px-2 pt-2 pb-2 space-y-1">
        {/* New chat + sidebar collapse (on non-macOS, collapse lives here) */}
        <div className="flex items-center gap-1">
          <button
            onClick={handleNewChat}
            className="flex items-center gap-2.5 flex-1 px-3 py-2 rounded-lg text-sm text-text-primary hover:bg-bg-tertiary/50 transition-colors cursor-pointer"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M7 3V11M3 7H11" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
            </svg>
            {t("sidebar.newChat")}
          </button>
          {!isMac && (
            <button
              onClick={toggleSidebar}
              title="Hide sidebar"
              className="flex items-center justify-center w-7 h-7 rounded-md text-text-muted hover:text-text-primary hover:bg-bg-tertiary/50 transition-colors cursor-pointer flex-shrink-0"
            >
              <SidebarToggleIcon />
            </button>
          )}
        </div>

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
                isRenaming={renamingSessionId === s.id}
                onSelect={handleSelectSession}
                onContextMenu={handleContextMenu}
                onRenameSubmit={handleRenameSubmit}
                onRenameCancel={handleRenameCancel}
                t={t}
                locale={locale}
              />
            ))}
          </div>
        )}
      </div>

      <div className="px-2 pb-2 pt-1 border-t border-border-primary mt-auto">
        <button
          onClick={() => setActiveView(settingsActive ? "chat" : "settings")}
          title={t("sidebar.settings")}
          aria-label={t("sidebar.settings")}
          className={`flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm transition-colors cursor-pointer ${
            settingsActive
              ? "bg-accent-blue/15 text-accent-blue"
              : "text-text-secondary hover:bg-bg-tertiary/50 hover:text-text-primary"
          }`}
        >
          <SettingsGearIcon />
          {t("sidebar.settings")}
        </button>
      </div>

      {contextMenu && (
        <ContextMenu
          menu={contextMenu}
          setMenu={setContextMenu}
          onRename={handleRenameStart}
          onResolveToggle={handleResolveToggle}
          onExport={handleExport}
          onDelete={handleDelete}
          t={t}
        />
      )}
    </div>
  );
}
