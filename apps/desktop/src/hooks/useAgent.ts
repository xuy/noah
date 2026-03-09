import { useState, useCallback } from "react";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";
import * as commands from "../lib/tauri-commands";
import type { UserEventType } from "../lib/tauri-commands";

interface UseAgentReturn {
  sendMessage: (text: string) => Promise<void>;
  sendConfirmation: (messageId: string, actionLabel?: string) => Promise<void>;
  sendEvent: (eventType: UserEventType, payload?: string) => Promise<void>;
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
        addMessage({
          role: "assistant",
          content: result.text,
          assistantUi: result.assistant_ui,
        });

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
    [sessionId, addMessage, updateMessage, setChanges, changes],
  );

  const sendConfirmation = useCallback(
    async (messageId: string, actionLabel?: string) => {
      if (!sessionId) return;

      const prevChangeIds = new Set(changes.map((c) => c.id));

      const confirmText = actionLabel || "Go ahead";
      markActionTaken(messageId);
      addMessage({
        role: "user",
        content: confirmText,
      });
      setIsProcessing(true);

      try {
        const result = await commands.sendMessageV2(
          sessionId,
          confirmText,
          true,
        );
        addMessage({
          role: "assistant",
          content: result.text,
          assistantUi: result.assistant_ui,
        });

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
    [sessionId, addMessage, updateMessage, markActionTaken, setChanges, changes],
  );

  const sendEvent = useCallback(
    async (eventType: UserEventType, payload?: string) => {
      if (!sessionId) return;

      // Show the user's answer in the chat — transparency: what user said = what LLM sees
      if (eventType === "USER_ANSWER_QUESTION" && payload) {
        try {
          const parsed = JSON.parse(payload);
          const answer = parsed.answer || parsed.answers?.toString() || "";
          if (answer) {
            addMessage({ role: "user", content: answer });
          }
        } catch { /* best-effort */ }
      }

      setIsProcessing(true);
      try {
        const result = await commands.sendUserEvent(
          sessionId,
          eventType,
          payload,
        );
        addMessage({
          role: "assistant",
          content: result.text,
          assistantUi: result.assistant_ui,
        });
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
