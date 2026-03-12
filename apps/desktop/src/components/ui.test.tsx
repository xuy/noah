// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// jsdom doesn't implement scrollIntoView — stub it for ChatPanel's useEffect
Element.prototype.scrollIntoView = vi.fn();
import type { ChangeEntry, SessionRecord } from "../lib/tauri-commands";

// ── Tauri shims ──────────────────────────────────────────────────────────────

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
const startDraggingMock = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    startDragging: startDraggingMock,
  })),
}));
vi.mock("../lib/tauri-commands", () => ({
  listKnowledge: vi.fn().mockResolvedValue([]),
  listSessions: vi.fn().mockResolvedValue([]),
  getChanges: vi.fn().mockResolvedValue([]),
  exportSession: vi.fn().mockResolvedValue(""),
  deleteSession: vi.fn().mockResolvedValue(undefined),
  sendMessage: vi.fn().mockResolvedValue(""),
  cancelProcessing: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../hooks/useAgent", () => ({
  useAgent: () => ({
    sendMessage: vi.fn(),
    sendConfirmation: vi.fn(),
    cancelProcessing: vi.fn(),
    isProcessing: false,
  }),
}));
vi.mock("../hooks/useSession", () => ({
  useSession: () => ({
    switchToProblem: vi.fn(),
    sessionId: "s1",
    isActive: true,
  }),
}));

// ── Stores ───────────────────────────────────────────────────────────────────

import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";

// ── Components ───────────────────────────────────────────────────────────────

import { MainTitleBar } from "./MainTitleBar";
import { ChatPanel } from "./ChatPanel";
import { Sidebar } from "./Sidebar";

// ── Fixtures ─────────────────────────────────────────────────────────────────

const CHANGE: ChangeEntry = {
  id: "c1",
  session_id: "s1",
  tool_name: "mac_flush_dns",
  description: "Flushed DNS cache",
  timestamp: Date.now(),
  undone: false,
};

const SESSION_WITH_CHANGES: SessionRecord = {
  id: "s1",
  created_at: new Date().toISOString(),
  ended_at: new Date().toISOString(),
  title: "Fixed DNS",
  message_count: 3,
  change_count: 2,
  resolved: true,
};


afterEach(() => cleanup());

beforeEach(() => {
  startDraggingMock.mockClear();
  useSessionStore.setState({
    changes: [],
    changeLogOpen: false,
    historyOpen: false,
    pastSessions: [],
    sessionId: "s1",
    isActive: true,
    pendingApproval: null,
    knowledgeOpen: false,
    settingsOpen: false,
    sidebarOpen: true,
  });
  useChatStore.setState({ messages: [] });
  vi.clearAllMocks();
  vi.mocked(commands.listKnowledge).mockResolvedValue([]);
  vi.mocked(commands.listSessions).mockResolvedValue([]);
  vi.mocked(commands.getChanges).mockResolvedValue([]);
});

// ── MainTitleBar (macOS-only) ────────────────────────────────────────────────
// MainTitleBar renders null on non-macOS (jsdom's navigator.platform is "").
// We mock isMac = true so we can test the macOS title bar behaviour here.

vi.mock("../lib/platform", () => ({ isMac: true }));

describe("MainTitleBar", () => {
  it("renders sidebar toggle", () => {
    render(<MainTitleBar />);
    screen.getByTitle("Hide sidebar");
  });

  it("shows 'Show sidebar' when sidebar is closed", () => {
    useSessionStore.setState({ sidebarOpen: false });
    render(<MainTitleBar />);
    screen.getByTitle("Show sidebar");
  });

  it("starts dragging when pressing empty title bar space", async () => {
    const user = userEvent.setup();
    const { container } = render(<MainTitleBar />);

    await user.pointer({
      target: container.firstElementChild as HTMLElement,
      keys: "[MouseLeft]",
    });

    expect(startDraggingMock).toHaveBeenCalledTimes(1);
  });

  it("does not start dragging when pressing a title bar button", async () => {
    const user = userEvent.setup();
    render(<MainTitleBar />);

    await user.click(screen.getByTitle("Hide sidebar"));

    expect(startDraggingMock).not.toHaveBeenCalled();
  });
});

// ── ChangesBlock (tested through ChatPanel) ──────────────────────────────────

describe("ChangesBlock", () => {
  it("shows change count for mutating actions", async () => {
    useSessionStore.setState({ changes: [CHANGE] });
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "I fixed your DNS.",
          timestamp: Date.now(),
          changeIds: ["c1"],
        },
      ],
    });
    render(<ChatPanel />);
    await screen.findByText("1 action taken");
  });

  it("expands to show the change label", async () => {
    useSessionStore.setState({ changes: [CHANGE] });
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "Done.",
          timestamp: Date.now(),
          changeIds: ["c1"],
        },
      ],
    });
    render(<ChatPanel />);
    await userEvent.click(await screen.findByText("1 action taken"));
    screen.getByText("Flushed DNS");
  });

  it("shows diagnostic-only message when all actions are read-only", async () => {
    const diagChange: ChangeEntry = {
      ...CHANGE,
      id: "c-diag",
      tool_name: "mac_ping",
      description: "Pinged host",
    };
    useSessionStore.setState({ changes: [diagChange] });
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "Looks good.",
          timestamp: Date.now(),
          changeIds: ["c-diag"],
        },
      ],
    });
    render(<ChatPanel />);
    await screen.findByText(/diagnostic check/);
  });

  it("shows mutating shell commands as changes, diagnostics as footnote", async () => {
    const diag: ChangeEntry = {
      ...CHANGE,
      id: "c1",
      tool_name: "shell_run",
      description: "Executed shell command: ps aux | grep discord",
    };
    const change: ChangeEntry = {
      ...CHANGE,
      id: "c2",
      tool_name: "shell_run",
      description: "Executed shell command: pkill -f Discord",
    };
    useSessionStore.setState({ changes: [diag, change] });
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "Done.",
          timestamp: Date.now(),
          changeIds: ["c1", "c2"],
        },
      ],
    });
    render(<ChatPanel />);
    await userEvent.click(await screen.findByText("1 action taken"));
    screen.getByText("Stopped a process");
    screen.getByText(/1 diagnostic check/);
  });

  it("does not render when changeIds do not match any store changes", async () => {
    useSessionStore.setState({ changes: [] });
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "Nothing done.",
          timestamp: Date.now(),
          changeIds: ["c-ghost"],
        },
      ],
    });
    render(<ChatPanel />);
    await screen.findByText("Nothing done.");
    expect(screen.queryByText(/action taken/)).toBeNull();
  });

  it("does not render when message has no changeIds", async () => {
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "Just checked your system.",
          timestamp: Date.now(),
        },
      ],
    });
    render(<ChatPanel />);
    await screen.findByText("Just checked your system.");
    expect(screen.queryByText(/action taken/)).toBeNull();
  });

  it("renders a grounded sticky footer for chat input", async () => {
    useChatStore.setState({
      messages: [
        {
          id: "msg1",
          role: "assistant",
          content: "How can I help?",
          timestamp: Date.now(),
        },
      ],
    });
    render(<ChatPanel />);
    await screen.findByText("How can I help?");
    const footer = screen.getByTestId("chat-input-footer");
    expect(footer.className).toContain("sticky");
    expect(footer.className).toContain("z-10");
    expect(footer.className).toContain("bg-bg-primary");
    expect(footer.className).toContain("shadow-[0_-6px_18px_rgba(0,0,0,0.16)]");
  });
});

// ── Sidebar session list ─────────────────────────────────────────────────────

const mockSidebarSession = { startNewProblem: vi.fn() };

describe("Sidebar session list", () => {
  it("shows session titles when sidebar is open", async () => {
    vi.mocked(commands.listSessions).mockResolvedValue([SESSION_WITH_CHANGES]);
    useSessionStore.setState({ sidebarOpen: true });
    render(<Sidebar session={mockSidebarSession} />);
    await screen.findByText("Fixed DNS");
  });

  it("shows empty message when no sessions", async () => {
    vi.mocked(commands.listSessions).mockResolvedValue([]);
    useSessionStore.setState({ sidebarOpen: true });
    render(<Sidebar session={mockSidebarSession} />);
    await screen.findByText(/Sessions will appear here/);
  });
});
