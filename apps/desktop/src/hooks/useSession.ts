import { useEffect, useCallback } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";

// Module-level guard: shared across all useSession() instances
let creating = false;

interface UseSessionReturn {
  sessionId: string | null;
  isActive: boolean;
  /** End the current problem and start a fresh session. */
  startNewProblem: () => Promise<void>;
  /** Switch to an existing problem/session (loads its messages). */
  switchToProblem: (sessionId: string) => Promise<void>;
}

export function useSession(): UseSessionReturn {
  const {
    sessionId,
    isActive,
    setSession,
    endSession: endSessionState,
  } = useSessionStore();
  const addMessage = useChatStore((s) => s.addMessage);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const setMessages = useChatStore((s) => s.setMessages);

  const createSession = useCallback(async () => {
    try {
      clearMessages();
      const session = await commands.createSession();
      setSession(session.id);
      addMessage({
        role: "system",
        content:
          "Hey! I'm Noah, your computer helper. Just tell me what's going on and I'll take care of it.",
      });
    } catch (err) {
      console.error("Failed to create session:", err);
      addMessage({
        role: "system",
        content: `Failed to start session: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }, [setSession, addMessage, clearMessages]);

  const startNewProblem = useCallback(async () => {
    if (sessionId) {
      try {
        await commands.endSession(sessionId);
        endSessionState();
      } catch (err) {
        console.error("Failed to end session:", err);
      }
    }
    await createSession();
  }, [sessionId, endSessionState, createSession]);

  const switchToProblem = useCallback(
    async (targetId: string) => {
      try {
        const records = await commands.getSessionMessages(targetId);
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
          setMessages(
            records.map((r) => ({
              id: r.id,
              role: r.role as "user" | "assistant" | "system",
              content: r.content,
              timestamp: new Date(r.timestamp).getTime(),
              actionTaken: r.action_taken || undefined,
              actionConfirmation: r.action_confirmation || undefined,
            })),
          );
        }
        setSession(targetId);
      } catch (err) {
        console.error("Failed to switch session:", err);
      }
    },
    [setSession, setMessages],
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

  return {
    sessionId,
    isActive,
    startNewProblem,
    switchToProblem,
  };
}
