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
