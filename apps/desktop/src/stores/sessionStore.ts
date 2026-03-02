import { create } from "zustand";
import type {
  ApprovalRequest,
  ChangeEntry,
  SessionRecord,
} from "../lib/tauri-commands";

interface SessionState {
  sessionId: string | null;
  isActive: boolean;
  startTime: number | null;
  changes: ChangeEntry[];
  pendingApproval: ApprovalRequest | null;
  changeLogOpen: boolean;
  historyOpen: boolean;
  pastSessions: SessionRecord[];

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
  setPastSessions: (sessions: SessionRecord[]) => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  sessionId: null,
  isActive: false,
  startTime: null,
  changes: [],
  pendingApproval: null,
  changeLogOpen: false,
  historyOpen: false,
  pastSessions: [],

  setSession: (id) =>
    set({
      sessionId: id,
      isActive: true,
      startTime: Date.now(),
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

  setPastSessions: (sessions) => set({ pastSessions: sessions }),
}));
