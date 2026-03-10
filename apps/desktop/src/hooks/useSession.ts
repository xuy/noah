import { useEffect, useCallback } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import { currentLocale, t as i18nT } from "../i18n";
import * as commands from "../lib/tauri-commands";

// Module-level guard: shared across all useSession() instances
let creating = false;

export type SessionMode = "default" | "learn";

interface UseSessionReturn {
  sessionId: string | null;
  isActive: boolean;
  /** End the current problem and start a fresh session. */
  startNewProblem: (mode?: SessionMode) => Promise<void>;
  /** Switch to an existing problem/session (loads its messages). */
  switchToProblem: (sessionId: string) => Promise<void>;
}

export function useSession(): UseSessionReturn {
  const {
    sessionId,
    isActive,
    setSession,
    endSession: endSessionState,
    prependSession,
  } = useSessionStore();
  const addMessage = useChatStore((s) => s.addMessage);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const setMessages = useChatStore((s) => s.setMessages);

  const createSession = useCallback(async (mode: SessionMode = "default") => {
    try {
      clearMessages();
      const session = await commands.createSession();
      setSession(session.id);
      // Immediately add to sidebar so the user sees a placeholder entry.
      prependSession({
        id: session.id,
        created_at: session.created_at,
        ended_at: null,
        title: null,
        message_count: 0,
        change_count: 0,
        resolved: null,
      });
      // Sync locale to backend so the LLM system prompt includes a language hint.
      commands.setLocale(session.id, currentLocale()).catch(() => {});
      // Set session mode so backend can tailor the system prompt.
      // Awaited to avoid race with the user's first message.
      if (mode === "learn") {
        await commands.setSessionMode(session.id, "learn");
      }

      const greeting = mode === "learn"
        ? `${i18nT("welcome.learnGreeting")} ${i18nT("welcome.learnSubtitle")}`
        : "Hey! I'm Noah, your computer helper. Just tell me what's going on and I'll take care of it.";
      addMessage({ role: "system", content: greeting });
    } catch (err) {
      console.error("Failed to create session:", err);
      addMessage({
        role: "system",
        content: `Failed to start session: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }, [setSession, prependSession, addMessage, clearMessages]);

  const startNewProblem = useCallback(async (mode?: SessionMode) => {
    if (sessionId) {
      try {
        await commands.endSession(sessionId);
        endSessionState();
      } catch (err) {
        console.error("Failed to end session:", err);
      }
    }
    await createSession(mode ?? "default");
  }, [sessionId, endSessionState, createSession]);

  const setChanges = useSessionStore((s) => s.setChanges);

  const switchToProblem = useCallback(
    async (targetId: string) => {
      try {
        const [records, changes] = await Promise.all([
          commands.getSessionMessages(targetId),
          commands.getChanges(targetId),
        ]);

        setChanges(changes);

        if (records.length === 0) {
          setMessages([
            {
              id: "no-messages",
              role: "system",
              content:
                "This session's conversation was not saved. (Message recording was added in a later version.)",
              timestamp: Date.now(),
            },
          ]);
        } else {
          // Attach all change IDs to the last assistant message so the
          // inline ChangesBlock renders when viewing past sessions.
          // (The per-message linkage isn't persisted in the DB.)
          const allChangeIds = changes.map((c) => c.id);
          let lastAssistantIdx = -1;
          for (let i = records.length - 1; i >= 0; i--) {
            if (records[i].role === "assistant") {
              lastAssistantIdx = i;
              break;
            }
          }

          setMessages(
            records.map((r, i) => ({
              id: r.id,
              role: r.role as "user" | "assistant" | "system",
              content: r.content,
              timestamp: new Date(r.timestamp).getTime(),
              actionTaken: r.action_taken || undefined,
              actionConfirmation: r.action_confirmation || undefined,
              changeIds:
                i === lastAssistantIdx && allChangeIds.length > 0
                  ? allChangeIds
                  : undefined,
            })),
          );
        }
        setSession(targetId);
      } catch (err) {
        console.error("Failed to switch session:", err);
      }
    },
    [setSession, setMessages, setChanges],
  );

  // Auto-create session on mount (module-level guard prevents duplicates
  // across multiple useSession() instances and StrictMode double-mounts)
  useEffect(() => {
    if (!sessionId && !creating) {
      creating = true;
      createSession().finally(() => {
        creating = false;
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Load changes whenever the active session changes (covers app restart,
  // HMR reload, and any other case where sessionId is set but store is empty)
  useEffect(() => {
    if (!sessionId) return;
    commands.getChanges(sessionId).then(setChanges).catch(() => {});
  }, [sessionId, setChanges]);

  return {
    sessionId,
    isActive,
    startNewProblem,
    switchToProblem,
  };
}
