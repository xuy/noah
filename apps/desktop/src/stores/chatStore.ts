import { create } from "zustand";
import type { AssistantUiPayload } from "../lib/tauri-commands";

export interface ToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
  result?: string;
  status: "pending" | "running" | "completed" | "denied";
}

export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  toolCalls?: ToolCall[];
  changeIds?: string[];
  actionTaken?: boolean;
  actionConfirmation?: boolean;
  assistantUi?: AssistantUiPayload;
}

interface ChatState {
  messages: Message[];
  addMessage: (msg: Omit<Message, "id" | "timestamp">) => void;
  setMessages: (msgs: Message[]) => void;
  updateMessage: (id: string, updates: Partial<Message>) => void;
  updateToolCall: (
    messageId: string,
    toolCallId: string,
    updates: Partial<ToolCall>,
  ) => void;
  markActionTaken: (messageId: string) => void;
  clearMessages: () => void;
}

let messageCounter = 0;

function generateId(): string {
  messageCounter += 1;
  return `msg_${Date.now()}_${messageCounter}`;
}

export const useChatStore = create<ChatState>((set) => ({
  messages: [],

  addMessage: (msg) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          ...msg,
          id: generateId(),
          timestamp: Date.now(),
        },
      ],
    })),

  setMessages: (msgs) => set({ messages: msgs }),

  updateMessage: (id, updates) =>
    set((state) => ({
      messages: state.messages.map((msg) =>
        msg.id === id ? { ...msg, ...updates } : msg,
      ),
    })),

  updateToolCall: (messageId, toolCallId, updates) =>
    set((state) => ({
      messages: state.messages.map((msg) => {
        if (msg.id !== messageId || !msg.toolCalls) return msg;
        return {
          ...msg,
          toolCalls: msg.toolCalls.map((tc) =>
            tc.id === toolCallId ? { ...tc, ...updates } : tc,
          ),
        };
      }),
    })),

  markActionTaken: (id) =>
    set((state) => ({
      messages: state.messages.map((msg) =>
        msg.id === id ? { ...msg, actionTaken: true } : msg,
      ),
    })),

  clearMessages: () => set({ messages: [] }),
}));

// ── Devtools helper ─────────────────────────────────────────────────
// Inspect the chat store from the browser console without needing
// React DevTools. Use when a card's contents seem to mutate after
// first render — take a snapshot, switch views, take another, diff.
//
//   __noahChatDebug.snapshot()         → [{id, role, content, assistantUi}]
//   __noahChatDebug.last()             → most recent message (full object)
//   __noahChatDebug.lastAssistantUi()  → just the last assistantUi
//   __noahChatDebug.diff(prev)         → fields that changed since `prev`
if (typeof window !== "undefined") {
  type Snap = {
    id: string;
    role: string;
    content: string;
    assistantUi: unknown;
  };
  const helper = {
    snapshot(): Snap[] {
      return useChatStore.getState().messages.map((m) => ({
        id: m.id,
        role: m.role,
        content: (m.content ?? "").slice(0, 80),
        assistantUi: m.assistantUi,
      }));
    },
    last(): Message | undefined {
      const msgs = useChatStore.getState().messages;
      return msgs[msgs.length - 1];
    },
    lastAssistantUi(): unknown {
      return helper.last()?.assistantUi;
    },
    diff(prev: Snap[]): unknown[] {
      const cur = helper.snapshot();
      const out: unknown[] = [];
      for (const c of cur) {
        const p = prev.find((x) => x.id === c.id);
        if (!p) {
          out.push({ id: c.id, change: "added", to: c });
          continue;
        }
        if (
          JSON.stringify(p.assistantUi) !== JSON.stringify(c.assistantUi)
        ) {
          out.push({
            id: c.id,
            change: "assistantUi",
            from: p.assistantUi,
            to: c.assistantUi,
          });
        }
        if (p.content !== c.content) {
          out.push({
            id: c.id,
            change: "content",
            from: p.content,
            to: c.content,
          });
        }
      }
      return out;
    },
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__noahChatDebug = helper;
}
