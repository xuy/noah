import { useEffect, useCallback, useRef } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";

interface UseSessionReturn {
  sessionId: string | null;
  isActive: boolean;
  /** Create a new session */
  createSession: () => Promise<void>;
  /** End the current session */
  endSession: () => Promise<void>;
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

  const endSession = useCallback(async () => {
    if (!sessionId) return;
    try {
      await commands.endSession(sessionId);
      endSessionState();
      addMessage({
        role: "system",
        content: "Session ended.",
      });
    } catch (err) {
      console.error("Failed to end session:", err);
    }
  }, [sessionId, endSessionState, addMessage]);

  // Auto-create session on mount (guard against React Strict Mode double-fire)
  const creatingRef = useRef(false);
  useEffect(() => {
    if (!sessionId && !creatingRef.current) {
      creatingRef.current = true;
      createSession().finally(() => {
        creatingRef.current = false;
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    sessionId,
    isActive,
    createSession,
    endSession,
  };
}
