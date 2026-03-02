import { useEffect, useState, useCallback, useRef } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";

interface UseSessionReturn {
  sessionId: string | null;
  isActive: boolean;
  startTime: number | null;
  /** Elapsed time formatted as "HH:MM:SS" */
  elapsed: string;
  /** Create a new session */
  createSession: () => Promise<void>;
  /** End the current session */
  endSession: () => Promise<void>;
}

function formatElapsed(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  const pad = (n: number) => n.toString().padStart(2, "0");

  if (hours > 0) {
    return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
  }
  return `${pad(minutes)}:${pad(seconds)}`;
}

export function useSession(): UseSessionReturn {
  const {
    sessionId,
    isActive,
    startTime,
    setSession,
    endSession: endSessionState,
  } = useSessionStore();
  const addMessage = useChatStore((s) => s.addMessage);
  const clearMessages = useChatStore((s) => s.clearMessages);

  const [elapsed, setElapsed] = useState("00:00");
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Update elapsed timer every second while session is active
  useEffect(() => {
    if (isActive && startTime) {
      const tick = () => {
        setElapsed(formatElapsed(Date.now() - startTime));
      };
      tick(); // immediate first tick
      timerRef.current = setInterval(tick, 1000);
      return () => {
        if (timerRef.current) {
          clearInterval(timerRef.current);
          timerRef.current = null;
        }
      };
    } else {
      setElapsed("00:00");
    }
  }, [isActive, startTime]);

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
    startTime,
    elapsed,
    createSession,
    endSession,
  };
}
