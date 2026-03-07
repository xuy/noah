import { create } from "zustand";
import type {
  ApprovalRequest,
  ChangeEntry,
  SessionRecord,
} from "../lib/tauri-commands";

type ActiveView = "chat" | "knowledge" | "diagnostics";

interface SessionState {
  sessionId: string | null;
  isActive: boolean;
  changes: ChangeEntry[];
  pendingApproval: ApprovalRequest | null;
  changeLogOpen: boolean;
  historyOpen: boolean;
  knowledgeOpen: boolean;
  settingsOpen: boolean;
  sidebarOpen: boolean;
  activeView: ActiveView;
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
  toggleKnowledge: () => void;
  setKnowledgeOpen: (open: boolean) => void;
  toggleSettings: () => void;
  setSettingsOpen: (open: boolean) => void;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  setActiveView: (view: ActiveView) => void;
  setPastSessions: (sessions: SessionRecord[]) => void;
}

// Helper: close all side panels.
const allPanelsClosed = {
  changeLogOpen: false,
  historyOpen: false,
  knowledgeOpen: false,
  settingsOpen: false,
};

export const useSessionStore = create<SessionState>((set) => ({
  sessionId: null,
  isActive: false,
  changes: [],
  pendingApproval: null,
  changeLogOpen: false,
  historyOpen: false,
  knowledgeOpen: false,
  settingsOpen: false,
  sidebarOpen: true,
  activeView: "chat",
  pastSessions: [],

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

  // Panels are mutually exclusive — opening one closes the others.
  toggleChangeLog: () =>
    set((state) => ({
      ...allPanelsClosed,
      changeLogOpen: !state.changeLogOpen,
    })),

  setChangeLogOpen: (open) =>
    set(open ? { ...allPanelsClosed, changeLogOpen: true } : { changeLogOpen: false }),

  toggleHistory: () =>
    set((state) => ({
      ...allPanelsClosed,
      historyOpen: !state.historyOpen,
    })),

  setHistoryOpen: (open) =>
    set(open ? { ...allPanelsClosed, historyOpen: true } : { historyOpen: false }),

  toggleKnowledge: () =>
    set((state) => ({
      ...allPanelsClosed,
      knowledgeOpen: !state.knowledgeOpen,
    })),

  setKnowledgeOpen: (open) =>
    set(open ? { ...allPanelsClosed, knowledgeOpen: true } : { knowledgeOpen: false }),

  toggleSettings: () =>
    set((state) => ({
      ...allPanelsClosed,
      settingsOpen: !state.settingsOpen,
    })),

  setSettingsOpen: (open) =>
    set(open ? { ...allPanelsClosed, settingsOpen: true } : { settingsOpen: false }),

  toggleSidebar: () =>
    set((state) => ({ sidebarOpen: !state.sidebarOpen })),

  setSidebarOpen: (open) => set({ sidebarOpen: open }),

  setActiveView: (view) => set({ activeView: view }),

  setPastSessions: (sessions) => set({ pastSessions: sessions }),
}));
