import { useState, useCallback } from "react";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";
import * as commands from "../lib/tauri-commands";

interface UseAgentReturn {
  sendMessage: (text: string) => Promise<void>;
  sendConfirmation: (messageId: string) => Promise<void>;
  sendEvent: (eventType: "USER_CONFIRM" | "USER_SKIP_OPTIONAL" | "USER_SUBMIT_SECURE_FORM" | "USER_ANSWER_QUESTION", payload?: string) => Promise<void>;
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
  const updateMessage = useChatStore((s) => s.updateMessage);
  const markActionTaken = useChatStore((s) => s.markActionTaken);
  const sessionId = useSessionStore((s) => s.sessionId);
  const setChanges = useSessionStore((s) => s.setChanges);
  const setPastSessions = useSessionStore((s) => s.setPastSessions);
  const changes = useSessionStore((s) => s.changes);

  const sendMessage = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || !sessionId) return;

      const prevChangeIds = new Set(changes.map((c) => c.id));

      addMessage({ role: "user", content: trimmed });
      setIsProcessing(true);

      try {
        const result = await commands.sendMessageV2(sessionId, trimmed);
        const stillActive = useSessionStore.getState().sessionId === sessionId;
        if (stillActive) {
          addMessage({ role: "assistant", content: result.text, assistantUi: result.assistant_ui });
        }

        if (stillActive) {
          try {
            const updatedChanges = await commands.getChanges(sessionId);
            setChanges(updatedChanges);
            const newChangeIds = updatedChanges
              .filter((c) => !prevChangeIds.has(c.id))
              .map((c) => c.id);
            if (newChangeIds.length > 0) {
              // Update the assistant message we just added
              const latestMsgs = useChatStore.getState().messages;
              const lastAssistant = latestMsgs[latestMsgs.length - 1];
              if (lastAssistant?.role === "assistant") {
                updateMessage(lastAssistant.id, { changeIds: newChangeIds });
              }
            }
          } catch {
            // best-effort
          }
        }

        try {
          const sessions = await commands.listSessions();
          setPastSessions(sessions);
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
    [sessionId, addMessage, updateMessage, setChanges, setPastSessions, changes],
  );

  const sendConfirmation = useCallback(
    async (messageId: string) => {
      if (!sessionId) return;

      const prevChangeIds = new Set(changes.map((c) => c.id));

      markActionTaken(messageId);
      addMessage({ role: "user", content: "Go ahead", actionConfirmation: true });
      setIsProcessing(true);

      try {
        const result = await commands.sendUserEvent(sessionId, "USER_CONFIRM");
        const stillActive = useSessionStore.getState().sessionId === sessionId;
        if (stillActive) {
          addMessage({ role: "assistant", content: result.text, assistantUi: result.assistant_ui });
        }

        if (stillActive) {
          try {
            const updatedChanges = await commands.getChanges(sessionId);
            setChanges(updatedChanges);
            const newChangeIds = updatedChanges
              .filter((c) => !prevChangeIds.has(c.id))
              .map((c) => c.id);
            if (newChangeIds.length > 0) {
              const latestMsgs = useChatStore.getState().messages;
              const lastAssistant = latestMsgs[latestMsgs.length - 1];
              if (lastAssistant?.role === "assistant") {
                updateMessage(lastAssistant.id, { changeIds: newChangeIds });
              }
            }
          } catch {
            // best-effort
          }
        }

        try {
          const sessions = await commands.listSessions();
          setPastSessions(sessions);
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
    [sessionId, addMessage, updateMessage, markActionTaken, setChanges, setPastSessions, changes],
  );

  const sendEvent = useCallback(
    async (
      eventType: "USER_CONFIRM" | "USER_SKIP_OPTIONAL" | "USER_SUBMIT_SECURE_FORM" | "USER_ANSWER_QUESTION",
      payload?: string,
    ) => {
      if (!sessionId) return;
      setIsProcessing(true);
      try {
        const result = await commands.sendUserEvent(sessionId, eventType, payload);
        const stillActive = useSessionStore.getState().sessionId === sessionId;
        if (stillActive) {
          addMessage({ role: "assistant", content: result.text, assistantUi: result.assistant_ui });
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
    [sessionId, addMessage],
  );

  const cancelProcessing = useCallback(async () => {
    try {
      await commands.cancelProcessing();
    } catch (err) {
      console.error("Failed to cancel:", err);
    }
  }, []);

  return { sendMessage, sendConfirmation, sendEvent, cancelProcessing, isProcessing };
}
