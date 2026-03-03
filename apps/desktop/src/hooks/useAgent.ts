import { useState, useCallback } from "react";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";
import * as commands from "../lib/tauri-commands";

interface UseAgentReturn {
  sendMessage: (text: string) => Promise<void>;
  sendConfirmation: (messageId: string) => Promise<void>;
  cancelProcessing: () => Promise<void>;
  isProcessing: boolean;
}

/** Strip "Agent error: " prefix from backend errors since we already show friendly messages. */
function cleanError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  return raw.replace(/^Agent error:\s*/i, "");
}

export function useAgent(): UseAgentReturn {
  const [isProcessing, setIsProcessing] = useState(false);
  const addMessage = useChatStore((s) => s.addMessage);
  const markActionTaken = useChatStore((s) => s.markActionTaken);
  const sessionId = useSessionStore((s) => s.sessionId);
  const setChanges = useSessionStore((s) => s.setChanges);

  const sendMessage = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || !sessionId) return;

      addMessage({ role: "user", content: trimmed });
      setIsProcessing(true);

      try {
        const content = await commands.sendMessage(sessionId, trimmed);
        addMessage({ role: "assistant", content });

        try {
          const changes = await commands.getChanges(sessionId);
          setChanges(changes);
        } catch {
          // best-effort
        }
      } catch (err) {
        console.error("Agent communication error:", err);
        addMessage({
          role: "system",
          content: cleanError(err),
        });
      } finally {
        setIsProcessing(false);
      }
    },
    [sessionId, addMessage, setChanges],
  );

  const sendConfirmation = useCallback(
    async (messageId: string) => {
      if (!sessionId) return;

      markActionTaken(messageId);
      addMessage({ role: "user", content: "Go ahead", actionConfirmation: true });
      setIsProcessing(true);

      try {
        const content = await commands.sendMessage(sessionId, "Go ahead", true);
        addMessage({ role: "assistant", content });

        try {
          const changes = await commands.getChanges(sessionId);
          setChanges(changes);
        } catch {
          // best-effort
        }
      } catch (err) {
        console.error("Agent communication error:", err);
        addMessage({
          role: "system",
          content: cleanError(err),
        });
      } finally {
        setIsProcessing(false);
      }
    },
    [sessionId, addMessage, markActionTaken, setChanges],
  );

  const cancelProcessing = useCallback(async () => {
    try {
      await commands.cancelProcessing();
    } catch (err) {
      console.error("Failed to cancel:", err);
    }
  }, []);

  return { sendMessage, sendConfirmation, cancelProcessing, isProcessing };
}
