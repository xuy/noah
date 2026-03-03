import { create } from "zustand";
import type { Message } from "./chatStore";
import type {
  ApprovalRequest,
  ChangeEntry,
  SessionRecord,
} from "../lib/tauri-commands";

interface SessionState {
  sessionId: string | null;
  isActive: boolean;
  changes: ChangeEntry[];
  pendingApproval: ApprovalRequest | null;
  changeLogOpen: boolean;
  historyOpen: boolean;
  settingsOpen: boolean;
  pastSessions: SessionRecord[];

  /** Non-null when viewing a past session (read-only). */
  viewingPastSession: string | null;
  /** Saved current-session messages so we can restore them. */
  savedMessages: Message[] | null;

  setSession: (id: string) => void;
  endSession: () => void;
  addChange: (change: ChangeEntry) => void;
  markChangeUndone: (changeId: string) => void;
  setChanges: (changes: ChangeEntry[]) => void;
  setPendingApproval: (req: ApprovalRequest | null) => void;
  toggleChangeLog: () => void;
  setChangeLogOpen: (open: boolean) => void;
  toggleHistory: () => void;
  setHistoryOpen: (open: boolean) => void;
  toggleSettings: () => void;
  setSettingsOpen: (open: boolean) => void;
  setPastSessions: (sessions: SessionRecord[]) => void;
  viewPastSession: (id: string, currentMessages: Message[]) => void;
  returnToCurrentSession: () => Message[] | null;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessionId: null,
  isActive: false,
  changes: [],
  pendingApproval: null,
  changeLogOpen: false,
  historyOpen: false,
  settingsOpen: false,
  pastSessions: [],
  viewingPastSession: null,
  savedMessages: null,

  setSession: (id) =>
    set({
      sessionId: id,
      isActive: true,
      changes: [],
      pendingApproval: null,
    }),

  endSession: () =>
    set({
      isActive: false,
      pendingApproval: null,
    }),

  addChange: (change) =>
    set((state) => ({
      changes: [...state.changes, change],
    })),

  markChangeUndone: (changeId) =>
    set((state) => ({
      changes: state.changes.map((c) =>
        c.id === changeId ? { ...c, undone: true } : c,
      ),
    })),

  setChanges: (changes) => set({ changes }),

  setPendingApproval: (req) => set({ pendingApproval: req }),

  toggleChangeLog: () =>
    set((state) => ({ changeLogOpen: !state.changeLogOpen })),

  setChangeLogOpen: (open) => set({ changeLogOpen: open }),

  toggleHistory: () =>
    set((state) => ({ historyOpen: !state.historyOpen })),

  setHistoryOpen: (open) => set({ historyOpen: open }),

  toggleSettings: () =>
    set((state) => ({ settingsOpen: !state.settingsOpen })),

  setSettingsOpen: (open) => set({ settingsOpen: open }),

  setPastSessions: (sessions) => set({ pastSessions: sessions }),

  viewPastSession: (id, currentMessages) =>
    set({
      viewingPastSession: id,
      savedMessages: currentMessages,
      historyOpen: false,
    }),

  returnToCurrentSession: () => {
    const saved = get().savedMessages;
    set({ viewingPastSession: null, savedMessages: null });
    return saved;
  },
}));
