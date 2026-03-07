import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import * as commands from "../lib/tauri-commands";
import { useChatStore } from "../stores/chatStore";
import { useSessionStore } from "../stores/sessionStore";

interface SuggestionPayload {
  id: string;
  category: string;
  headline: string;
  detail: string;
}

export function ProactiveSuggestionBanner() {
  const [suggestion, setSuggestion] = useState<SuggestionPayload | null>(null);
  const addMessage = useChatStore((s) => s.addMessage);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const sessionId = useSessionStore((s) => s.sessionId);
  const setSession = useSessionStore((s) => s.setSession);
  const endSession = useSessionStore((s) => s.endSession);

  useEffect(() => {
    const unlisten = listen<SuggestionPayload>(
      "proactive-suggestion",
      (event) => {
        setSuggestion(event.payload);
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (!suggestion) return null;

  const handleDismiss = async () => {
    try {
      await commands.dismissProactiveSuggestion(suggestion.id);
    } catch (err) {
      console.error("Failed to dismiss suggestion:", err);
    }
    setSuggestion(null);
  };

  const handleTellMore = async () => {
    try {
      await commands.actOnProactiveSuggestion(suggestion.id);
    } catch (err) {
      console.error("Failed to mark suggestion:", err);
    }

    const noahMessage = `I noticed something: ${suggestion.headline}\n\n${suggestion.detail}`;
    const userMessage = "Tell me more and help me fix it.";

    // End current session if active.
    if (sessionId) {
      try {
        await commands.endSession(sessionId);
        endSession();
      } catch (err) {
        console.error("Failed to end session:", err);
      }
    }

    // Create a new session directly (skip the default greeting).
    try {
      clearMessages();
      const session = await commands.createSession();
      setSession(session.id);

      // Noah's opening: the proactive finding.
      addMessage({ role: "assistant", content: noahMessage });
      // User's reply.
      addMessage({ role: "user", content: userMessage });

      // Send to backend so Noah can investigate further.
      const reply = await commands.sendMessageV2(session.id, userMessage);
      addMessage({ role: "assistant", content: reply.text, assistantUi: reply.assistant_ui });
    } catch (err) {
      console.error("Failed to start proactive session:", err);
      addMessage({
        role: "system",
        content: `Failed to get response: ${err instanceof Error ? err.message : String(err)}`,
      });
    }

    setSuggestion(null);
  };

  return (
    <div className="flex items-center justify-between gap-3 px-4 py-2 bg-accent-purple/10 border-b border-accent-purple/20">
      <div className="flex items-center gap-2 min-w-0">
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          className="shrink-0 text-accent-purple"
        >
          <path
            d="M8 1C5.24 1 3 3.24 3 6c0 1.83 1 3.43 2.5 4.3V12a1 1 0 001 1h3a1 1 0 001-1v-1.7C12 9.43 13 7.83 13 6c0-2.76-2.24-5-5-5zM6.5 14h3v.5a.5.5 0 01-.5.5H7a.5.5 0 01-.5-.5V14z"
            fill="currentColor"
          />
        </svg>
        <p className="text-xs text-text-primary truncate">
          <span className="font-medium">{suggestion.headline}</span>
          <span className="text-text-muted ml-1.5">{suggestion.detail}</span>
        </p>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <button
          onClick={handleDismiss}
          className="text-[10px] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
        >
          Dismiss
        </button>
        <button
          onClick={handleTellMore}
          className="px-3 py-1 rounded-md bg-accent-purple text-white text-[11px] font-medium hover:bg-accent-purple/80 transition-colors cursor-pointer"
        >
          Tell me more
        </button>
      </div>
    </div>
  );
}
