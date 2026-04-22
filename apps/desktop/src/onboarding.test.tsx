// @vitest-environment jsdom
/**
 * Onboarding flow — end-to-end UI test.
 *
 * Guards against the silly regressions that have bitten us in manual testing:
 *   • Onboarding gate firing (or skipping) incorrectly — fresh user must see
 *     the tile picker; returning user must skip straight to MainApp.
 *   • Tile pick → clarifier → seed-to-localStorage handoff to the chat.
 *   • Magic-link email path + hidden BYOK path both reach onComplete.
 *   • Root-level deep-link handler for `noah://auth` AND `noah://subscribed`
 *     (we once had the listener inside SignInScreen, and it never fired when
 *     the user was stuck on the tile picker).
 *   • `data-tauri-drag-region` strip on unauthenticated screens (window was
 *     once unmovable on macOS because MainTitleBar wasn't mounted).
 *   • Subscribe modal fires on the RUN_STEP "commitment moment" — not after
 *     resolution — and only once per install (localStorage gate).
 *
 * The tests mock Tauri commands/plugins and render the real React tree,
 * stubbing only the heavy MainApp-internal components so we can exercise the
 * gate logic without pulling in the entire chat surface.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup, act, waitFor } from "@testing-library/react";
import { renderHook } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

Element.prototype.scrollIntoView = vi.fn();

// jsdom doesn't implement matchMedia; useTheme needs it.
if (!window.matchMedia) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }),
  });
}

// ── Capture deep-link handler so tests can drive it ──
let deepLinkCallback: ((urls: string[]) => unknown | Promise<unknown>) | null =
  null;
vi.mock("@tauri-apps/plugin-deep-link", () => ({
  onOpenUrl: vi.fn((cb: (urls: string[]) => unknown) => {
    deepLinkCallback = cb;
    return Promise.resolve(() => {
      deepLinkCallback = null;
    });
  }),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn().mockResolvedValue(null),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    startDragging: vi.fn().mockResolvedValue(undefined),
    setTitle: vi.fn().mockResolvedValue(undefined),
  })),
}));
vi.mock("./lib/platform", () => ({ isMac: true }));

// Stub MainApp-internal components — App.tsx renders these after the gate,
// and we don't care what's inside; we only care that we *left* the
// onboarding screens. Keep each stub text-distinct so assertions are clear.
vi.mock("./components/ChatPanel", () => ({
  ChatPanel: () => <div data-testid="chat-panel" />,
}));
vi.mock("./components/Sidebar", () => ({
  Sidebar: () => <div data-testid="sidebar" />,
}));
vi.mock("./components/MainTitleBar", () => ({
  MainTitleBar: () => <div data-testid="title-bar" />,
}));
vi.mock("./components/DebugPanel", () => ({
  DebugPanel: () => null,
}));
vi.mock("./components/ActionApproval", () => ({
  ActionApproval: () => null,
}));
vi.mock("./components/SessionSummary", () => ({
  SessionSummary: () => null,
}));
vi.mock("./components/SettingsPanel", () => ({
  SettingsPanel: () => null,
}));
vi.mock("./components/HealthDashboard", () => ({
  HealthDashboard: () => null,
}));
vi.mock("./components/KnowledgePanel", () => ({
  KnowledgeView: () => null,
}));
vi.mock("./components/UpdateBanner", () => ({
  UpdateBanner: () => null,
}));
vi.mock("./components/ProactiveSuggestionBanner", () => ({
  ProactiveSuggestionBanner: () => null,
}));
vi.mock("./components/SubscribeModal", () => ({
  SubscribeModal: ({ variant }: { variant: string }) => (
    <div data-testid="subscribe-modal" data-variant={variant} />
  ),
}));

// ── Mock the whole tauri-commands module ──
// Every command returns a sensible default; tests mutate per-test via
// `vi.mocked(commands.X).mockResolvedValue(...)`.
vi.mock("./lib/tauri-commands", () => ({
  listSessions: vi.fn().mockResolvedValue([]),
  consumerEnsureDeviceId: vi.fn().mockResolvedValue("device-abc"),
  consumerHasSession: vi.fn().mockResolvedValue(false),
  consumerRequestMagicLink: vi.fn().mockResolvedValue(null),
  consumerCompleteSignIn: vi.fn().mockResolvedValue(NEUTRAL_ENTITLEMENT()),
  consumerConfirmCheckout: vi
    .fn()
    .mockResolvedValue(ACTIVE_ENTITLEMENT()),
  consumerGetEntitlement: vi.fn().mockResolvedValue(null),
  consumerNotifyIssueStarted: vi
    .fn()
    .mockResolvedValue(TRIAL_ENTITLEMENT()),
  consumerNotifyFixCompleted: vi.fn().mockResolvedValue(null),
  consumerBillingCheckoutUrl: vi
    .fn()
    .mockResolvedValue("https://checkout.stripe.com/x"),
  consumerBillingPortalUrl: vi
    .fn()
    .mockResolvedValue("https://billing.stripe.com/x"),
  consumerSignOut: vi.fn().mockResolvedValue(undefined),
  setApiKey: vi.fn().mockResolvedValue(undefined),
  hasApiKey: vi.fn().mockResolvedValue(false),
  clearAuth: vi.fn().mockResolvedValue(undefined),
  createSession: vi.fn().mockResolvedValue({
    id: "s1",
    created_at: new Date().toISOString(),
    message_count: 0,
  }),
  endSession: vi.fn().mockResolvedValue(undefined),
  deleteSession: vi.fn().mockResolvedValue(undefined),
  getChanges: vi.fn().mockResolvedValue([]),
  getSessionMessages: vi.fn().mockResolvedValue([]),
  sendMessage: vi.fn().mockResolvedValue(""),
  sendMessageV2: vi
    .fn()
    .mockResolvedValue({ text: "ack", assistant_ui: undefined }),
  sendUserEvent: vi
    .fn()
    .mockResolvedValue({ text: "ack", assistant_ui: undefined }),
  cancelProcessing: vi.fn().mockResolvedValue(undefined),
  setLocale: vi.fn().mockResolvedValue(undefined),
  setSessionMode: vi.fn().mockResolvedValue(undefined),
  listKnowledge: vi.fn().mockResolvedValue([]),
  trackEvent: vi.fn().mockResolvedValue(undefined),
}));

// Entitlement fixtures — defined as functions so each use produces a fresh
// object (Zustand stores hold by reference).
function NEUTRAL_ENTITLEMENT() {
  return {
    plan: null,
    status: "none" as const,
    trial_started_at: null,
    trial_ends_at: null,
    period_start: null,
    period_end: null,
    usage_used: 0,
    usage_limit: 100,
    fix_count_total: 0,
  };
}
function TRIAL_ENTITLEMENT() {
  return {
    ...NEUTRAL_ENTITLEMENT(),
    plan: "trial",
    status: "trialing" as const,
    trial_started_at: Date.now() - 1000,
    trial_ends_at: Date.now() + 7 * 86_400_000,
  };
}
function ACTIVE_ENTITLEMENT() {
  return {
    ...NEUTRAL_ENTITLEMENT(),
    plan: "monthly",
    status: "active" as const,
    period_start: Date.now() - 1000,
    period_end: Date.now() + 30 * 86_400_000,
  };
}

// ── Real imports (after mocks are hoisted) ──
import App from "./App";
import { TilePickerScreen } from "./components/TilePickerScreen";
import { SignInScreen } from "./components/SignInScreen";
import { useAgent } from "./hooks/useAgent";
import { useSessionStore } from "./stores/sessionStore";
import { useChatStore } from "./stores/chatStore";
import { useConsumerStore } from "./stores/consumerStore";
import * as commands from "./lib/tauri-commands";

// ── Helpers ──

function resetAllStores() {
  localStorage.clear();
  useSessionStore.setState({
    sessionId: null,
    isActive: false,
    pastSessions: [],
    changes: [],
    changeLogOpen: false,
    historyOpen: false,
    knowledgeOpen: false,
    sidebarOpen: true,
    pendingApproval: null,
    activeView: "chat",
  });
  useChatStore.setState({ messages: [] });
  useConsumerStore.setState({
    entitlement: null,
    hydrated: false,
    subscribeModal: null,
  });
}

function resetTauriMocks() {
  vi.clearAllMocks();
  vi.mocked(commands.listSessions).mockResolvedValue([]);
  vi.mocked(commands.consumerGetEntitlement).mockResolvedValue(null);
  vi.mocked(commands.consumerEnsureDeviceId).mockResolvedValue("device-abc");
  vi.mocked(commands.consumerRequestMagicLink).mockResolvedValue(null);
  vi.mocked(commands.consumerCompleteSignIn).mockResolvedValue(
    NEUTRAL_ENTITLEMENT(),
  );
  vi.mocked(commands.consumerConfirmCheckout).mockResolvedValue(
    ACTIVE_ENTITLEMENT(),
  );
  vi.mocked(commands.sendMessageV2).mockResolvedValue({
    text: "ack",
    assistant_ui: undefined,
  });
  vi.mocked(commands.getChanges).mockResolvedValue([]);
}

beforeEach(() => {
  resetTauriMocks();
  resetAllStores();
  deepLinkCallback = null;
});

afterEach(() => {
  cleanup();
});

// ═══════════════════════════════════════════════════════════════════════════
// Gate logic
// ═══════════════════════════════════════════════════════════════════════════

describe("Onboarding gate", () => {
  it("shows tile picker when no prior sessions exist (fresh install)", async () => {
    vi.mocked(commands.listSessions).mockResolvedValue([]);
    render(<App />);
    // Tile picker greeting from i18n en.onboarding.greeting
    expect(
      await screen.findByText(/What's going on with your Mac/),
    ).toBeTruthy();
    // MainApp stubs should NOT be mounted
    expect(screen.queryByTestId("chat-panel")).toBeNull();
  });

  it("skips onboarding and renders MainApp when a session already exists", async () => {
    vi.mocked(commands.listSessions).mockResolvedValue([
      {
        id: "prev",
        created_at: new Date().toISOString(),
        ended_at: null,
        title: "Previous",
        message_count: 1,
        change_count: 0,
        resolved: null,
      },
    ]);
    render(<App />);
    expect(await screen.findByTestId("chat-panel")).toBeTruthy();
    expect(screen.queryByText(/What's going on with your Mac/)).toBeNull();
  });

  it("falls through to MainApp if listSessions errors (don't strand user on gate)", async () => {
    vi.mocked(commands.listSessions).mockRejectedValue(
      new Error("db locked"),
    );
    render(<App />);
    expect(await screen.findByTestId("chat-panel")).toBeTruthy();
  });

  it("ensures a device id is minted on first launch", async () => {
    render(<App />);
    await waitFor(() => {
      expect(commands.consumerEnsureDeviceId).toHaveBeenCalled();
    });
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// TilePickerScreen
// ═══════════════════════════════════════════════════════════════════════════

describe("TilePickerScreen → clarify → seed", () => {
  it("clicking a tile shows the clarify stage with the tile's title", async () => {
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<TilePickerScreen onComplete={onComplete} />);
    await user.click(screen.getByText("My Mac feels slow"));
    // Clarify stage echoes the title as a heading
    expect(
      screen.getAllByText("My Mac feels slow").length,
    ).toBeGreaterThan(0);
    // Continue button is disabled until text is entered
    const cont = screen.getByText("Continue") as HTMLButtonElement;
    expect(cont.hasAttribute("disabled")).toBe(true);
  });

  it("stashes a composed seed to localStorage and calls onComplete", async () => {
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<TilePickerScreen onComplete={onComplete} />);
    await user.click(screen.getByText("Wi-Fi or internet issues"));
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "drops every 10 min at home");
    await user.click(screen.getByText("Continue"));

    expect(onComplete).toHaveBeenCalledTimes(1);
    const stashed = localStorage.getItem("noah.pendingSeed");
    expect(stashed).not.toBeNull();
    const parsed = JSON.parse(stashed!);
    // Category title prefixed, then user's clarifier
    expect(parsed.message).toBe(
      "Wi-Fi or internet issues. drops every 10 min at home",
    );
    expect(parsed.expiresAt).toBeGreaterThan(Date.now());
  });

  it("'other' tile preserves the user's raw text without a category prefix", async () => {
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<TilePickerScreen onComplete={onComplete} />);
    await user.click(screen.getByText("Something else"));
    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "something weird with Finder");
    await user.click(screen.getByText("Continue"));

    const parsed = JSON.parse(localStorage.getItem("noah.pendingSeed")!);
    expect(parsed.message).toBe("something weird with Finder");
  });

  it("'Already have an account?' routes to the sign-in screen", async () => {
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<TilePickerScreen onComplete={onComplete} />);
    await user.click(screen.getByText(/Already have an account/));
    // Sign-in prompt comes from i18n en.signIn.prompt
    expect(await screen.findByText("What's your email?")).toBeTruthy();
  });

  it("Back from clarify returns to the tile grid", async () => {
    const user = userEvent.setup();
    render(<TilePickerScreen onComplete={vi.fn()} />);
    await user.click(screen.getByText("Battery drains fast"));
    // Use the Back button (there are two — text button and arrow link)
    const backBtns = screen.getAllByText("Back");
    await user.click(backBtns[0]);
    // Grid shows multiple tiles again
    expect(screen.getByText("My Mac feels slow")).toBeTruthy();
    expect(screen.getByText("Battery drains fast")).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// SignInScreen — magic link path
// ═══════════════════════════════════════════════════════════════════════════

describe("SignInScreen email path", () => {
  it("rejects an invalid email with an inline error (no browser alert)", async () => {
    const user = userEvent.setup();
    render(<SignInScreen onComplete={vi.fn()} />);
    await user.type(screen.getByPlaceholderText("you@example.com"), "not-an-email");
    await user.click(screen.getByText("Email me a sign-in link"));
    expect(
      await screen.findByText(/Please enter a valid email address/),
    ).toBeTruthy();
    expect(commands.consumerRequestMagicLink).not.toHaveBeenCalled();
  });

  it("calls consumerRequestMagicLink and completes on auto-sign-in", async () => {
    vi.mocked(commands.consumerRequestMagicLink).mockResolvedValue(
      TRIAL_ENTITLEMENT(),
    );
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<SignInScreen onComplete={onComplete} />);
    await user.type(
      screen.getByPlaceholderText("you@example.com"),
      "alice@example.com",
    );
    await user.click(screen.getByText("Email me a sign-in link"));
    await waitFor(() => {
      expect(commands.consumerRequestMagicLink).toHaveBeenCalledWith(
        "alice@example.com",
      );
      expect(onComplete).toHaveBeenCalledTimes(1);
    });
  });

  it("falls back to 'check your inbox' when server returns null", async () => {
    vi.mocked(commands.consumerRequestMagicLink).mockResolvedValue(null);
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<SignInScreen onComplete={onComplete} />);
    await user.type(
      screen.getByPlaceholderText("you@example.com"),
      "bob@example.com",
    );
    await user.click(screen.getByText("Email me a sign-in link"));
    expect(
      await screen.findByText(/We sent a sign-in link to bob@example.com/),
    ).toBeTruthy();
    expect(onComplete).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// SignInScreen — BYOK (advanced) path
// ═══════════════════════════════════════════════════════════════════════════

describe("SignInScreen BYOK path", () => {
  it("the advanced toggle reveals the BYOK stage", async () => {
    const user = userEvent.setup();
    render(<SignInScreen onComplete={vi.fn()} />);
    // The toggle is a small unlabelled button with aria-label "Advanced options"
    await user.click(screen.getByLabelText("Advanced options"));
    expect(await screen.findByText(/Use my own Anthropic key/)).toBeTruthy();
  });

  it("rejects non-'sk-ant-' keys without calling setApiKey", async () => {
    const user = userEvent.setup();
    render(<SignInScreen onComplete={vi.fn()} />);
    await user.click(screen.getByLabelText("Advanced options"));
    const keyInput = screen.getByPlaceholderText("sk-ant-...");
    await user.type(keyInput, "not-a-real-key");
    await user.click(screen.getByText("Save key"));
    expect(
      await screen.findByText(/doesn't look like an Anthropic API key/),
    ).toBeTruthy();
    expect(commands.setApiKey).not.toHaveBeenCalled();
  });

  it("accepts a valid sk-ant- key and completes", async () => {
    const onComplete = vi.fn();
    const user = userEvent.setup();
    render(<SignInScreen onComplete={onComplete} />);
    await user.click(screen.getByLabelText("Advanced options"));
    const keyInput = screen.getByPlaceholderText("sk-ant-...");
    await user.type(keyInput, "sk-ant-apikey-abc123");
    await user.click(screen.getByText("Save key"));
    await waitFor(() => {
      expect(commands.setApiKey).toHaveBeenCalledWith("sk-ant-apikey-abc123");
      expect(onComplete).toHaveBeenCalledTimes(1);
    });
  });

  it("'Use email sign-in instead' returns to the email stage", async () => {
    const user = userEvent.setup();
    render(<SignInScreen onComplete={vi.fn()} />);
    await user.click(screen.getByLabelText("Advanced options"));
    await user.click(screen.getByText("Use email sign-in instead"));
    expect(await screen.findByText("What's your email?")).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// Deep-link handler (root level, survives unauthenticated screens)
// ═══════════════════════════════════════════════════════════════════════════

describe("Deep-link handling at App root", () => {
  it("noah://auth?token=… completes sign-in and exits onboarding", async () => {
    vi.mocked(commands.consumerCompleteSignIn).mockResolvedValue(
      TRIAL_ENTITLEMENT(),
    );
    render(<App />);
    // Wait for the tile picker to mount so onOpenUrl has been registered
    await screen.findByText(/What's going on with your Mac/);
    expect(deepLinkCallback).not.toBeNull();

    await act(async () => {
      await deepLinkCallback!(["noah://auth?token=magic-xyz"]);
    });
    expect(commands.consumerCompleteSignIn).toHaveBeenCalledWith("magic-xyz");
    // User is escorted out of the tile picker into MainApp.
    await screen.findByTestId("chat-panel");
  });

  it("noah://subscribed?session_id=… confirms the checkout", async () => {
    render(<App />);
    await screen.findByText(/What's going on with your Mac/);
    expect(deepLinkCallback).not.toBeNull();

    await act(async () => {
      await deepLinkCallback!([
        "noah://subscribed?session_id=cs_test_abc",
      ]);
    });
    expect(commands.consumerConfirmCheckout).toHaveBeenCalledWith(
      "cs_test_abc",
    );
    await screen.findByTestId("chat-panel");
  });

  it("noah://subscribed updates the consumer store and closes the modal", async () => {
    vi.mocked(commands.consumerConfirmCheckout).mockResolvedValue(
      ACTIVE_ENTITLEMENT(),
    );
    useConsumerStore.setState({
      subscribeModal: { variant: "first_fix" },
      entitlement: TRIAL_ENTITLEMENT(),
    });
    render(<App />);
    await screen.findByText(/What's going on with your Mac/);

    await act(async () => {
      await deepLinkCallback!([
        "noah://subscribed?session_id=cs_test_abc",
      ]);
    });

    // Store flipped to active and the modal is dismissed.
    const state = useConsumerStore.getState();
    expect(state.entitlement?.status).toBe("active");
    expect(state.subscribeModal).toBeNull();
  });

  it("swallows a stale/invalid token instead of blowing up the UI", async () => {
    vi.mocked(commands.consumerCompleteSignIn).mockRejectedValue(
      new Error("token already used"),
    );
    render(<App />);
    await screen.findByText(/What's going on with your Mac/);

    await act(async () => {
      await deepLinkCallback!(["noah://auth?token=stale"]);
    });
    // Still on the tile picker; didn't crash.
    expect(screen.getByText(/What's going on with your Mac/)).toBeTruthy();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// Drag-region regression guards
// ═══════════════════════════════════════════════════════════════════════════

describe("Window drag region on unauthenticated screens", () => {
  // Both of these regressed in the past: MainTitleBar isn't mounted during
  // onboarding, so without an explicit drag strip the window becomes
  // unmovable on macOS's overlay title bar.

  it("TilePickerScreen has a data-tauri-drag-region strip", () => {
    const { container } = render(<TilePickerScreen onComplete={vi.fn()} />);
    const dragStrip = container.querySelector("[data-tauri-drag-region]");
    expect(dragStrip).not.toBeNull();
  });

  it("SignInScreen has a data-tauri-drag-region strip", () => {
    const { container } = render(<SignInScreen onComplete={vi.fn()} />);
    const dragStrip = container.querySelector("[data-tauri-drag-region]");
    expect(dragStrip).not.toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// Subscribe modal — "first fix" commitment-moment timing
// ═══════════════════════════════════════════════════════════════════════════

describe("Subscribe modal — first-fix prompt", () => {
  const FIRST_FIX_KEY = "noah.firstFixPromptShown";

  beforeEach(() => {
    useSessionStore.setState({ sessionId: "s1", isActive: true });
  });

  it("opens with variant 'first_fix' on RUN_STEP during trial", async () => {
    useConsumerStore.setState({ entitlement: TRIAL_ENTITLEMENT() });

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendConfirmation("msg-1", "Continue", "RUN_STEP");
    });

    expect(useConsumerStore.getState().subscribeModal).toEqual({
      variant: "first_fix",
    });
    expect(localStorage.getItem(FIRST_FIX_KEY)).toBe("1");
  });

  it("does NOT reopen on a subsequent RUN_STEP (shown-once per install)", async () => {
    useConsumerStore.setState({ entitlement: TRIAL_ENTITLEMENT() });

    const { result } = renderHook(() => useAgent());
    // First click — modal opens.
    await act(async () => {
      await result.current.sendConfirmation("msg-1", "Continue", "RUN_STEP");
    });
    expect(useConsumerStore.getState().subscribeModal).not.toBeNull();

    // User dismisses it.
    act(() => useConsumerStore.getState().closeSubscribeModal());
    expect(useConsumerStore.getState().subscribeModal).toBeNull();

    // Second click — must NOT reopen.
    await act(async () => {
      await result.current.sendConfirmation("msg-2", "Continue", "RUN_STEP");
    });
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("does NOT open for WAIT_FOR_USER confirmations", async () => {
    useConsumerStore.setState({ entitlement: TRIAL_ENTITLEMENT() });

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendConfirmation(
        "msg-1",
        "I've done this",
        "WAIT_FOR_USER",
      );
    });
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("does NOT open when the user is already on an active subscription", async () => {
    useConsumerStore.setState({ entitlement: ACTIVE_ENTITLEMENT() });

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendConfirmation("msg-1", "Continue", "RUN_STEP");
    });
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("does NOT open when entitlement has not hydrated yet", async () => {
    useConsumerStore.setState({ entitlement: null });

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendConfirmation("msg-1", "Continue", "RUN_STEP");
    });
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// Trial-start race on fresh install
// ═══════════════════════════════════════════════════════════════════════════

describe("Trial-start race guard", () => {
  // Regression: on fresh install, MainApp's refreshEntitlement() is async
  // and the ChatPanel seed auto-send can fire before ent hydrates. If
  // sendMessage skips notifyIssueStarted when ent is null, the server never
  // gets /events/issue-started → entitlement stays "none" → the "first fix"
  // subscribe modal never opens when the user later clicks the action.
  // Symptom from manual test: clicked action, no paywall, payment never
  // triggered. Fix: call notifyIssueStarted when ent is null OR "none".

  beforeEach(() => {
    useSessionStore.setState({ sessionId: "s1", isActive: true });
  });

  it("calls notifyIssueStarted when ent is null (pre-hydration race)", async () => {
    useConsumerStore.setState({ entitlement: null });
    vi.mocked(commands.consumerNotifyIssueStarted).mockResolvedValue(
      TRIAL_ENTITLEMENT(),
    );

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendMessage("I want more disk space");
    });

    expect(commands.consumerNotifyIssueStarted).toHaveBeenCalledTimes(1);
    // Entitlement in the store should reflect the returned trialing state.
    expect(useConsumerStore.getState().entitlement?.status).toBe("trialing");
  });

  it("calls notifyIssueStarted when ent.status is 'none'", async () => {
    useConsumerStore.setState({ entitlement: NEUTRAL_ENTITLEMENT() });
    vi.mocked(commands.consumerNotifyIssueStarted).mockResolvedValue(
      TRIAL_ENTITLEMENT(),
    );

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendMessage("I want more disk space");
    });

    expect(commands.consumerNotifyIssueStarted).toHaveBeenCalledTimes(1);
  });

  it("does NOT call notifyIssueStarted when already trialing", async () => {
    useConsumerStore.setState({ entitlement: TRIAL_ENTITLEMENT() });

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendMessage("another question");
    });

    expect(commands.consumerNotifyIssueStarted).not.toHaveBeenCalled();
  });

  it("does NOT call notifyIssueStarted when already active", async () => {
    useConsumerStore.setState({ entitlement: ACTIVE_ENTITLEMENT() });

    const { result } = renderHook(() => useAgent());
    await act(async () => {
      await result.current.sendMessage("another question");
    });

    expect(commands.consumerNotifyIssueStarted).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// Monotonic entitlement merge (guards against late-arriving stale GET)
// ═══════════════════════════════════════════════════════════════════════════

describe("Consumer store — monotonic entitlement", () => {
  // Regression: MainApp's refresh GET /entitlement can race with useAgent's
  // POST /events/issue-started. If the GET response is generated BEFORE the
  // POST commits but arrives at the client AFTER the POST response, setting
  // the store with the stale "none" clobbers the fresh "trialing" — and the
  // first-fix modal never fires on the next action click.

  it("refresh does not overwrite a started state with a 'none' response", async () => {
    useConsumerStore.setState({ entitlement: TRIAL_ENTITLEMENT() });
    vi.mocked(commands.consumerGetEntitlement).mockResolvedValue(
      NEUTRAL_ENTITLEMENT(),
    );

    await useConsumerStore.getState().refresh();

    expect(useConsumerStore.getState().entitlement?.status).toBe("trialing");
  });

  it("setEntitlement does not regress 'active' to 'none'", () => {
    useConsumerStore.setState({ entitlement: ACTIVE_ENTITLEMENT() });
    useConsumerStore.getState().setEntitlement(NEUTRAL_ENTITLEMENT());
    expect(useConsumerStore.getState().entitlement?.status).toBe("active");
  });

  it("refresh DOES apply legitimate state transitions (trialing → active)", async () => {
    useConsumerStore.setState({ entitlement: TRIAL_ENTITLEMENT() });
    vi.mocked(commands.consumerGetEntitlement).mockResolvedValue(
      ACTIVE_ENTITLEMENT(),
    );

    await useConsumerStore.getState().refresh();

    expect(useConsumerStore.getState().entitlement?.status).toBe("active");
  });

  it("refresh accepts 'none' when there is no prior started state", async () => {
    useConsumerStore.setState({ entitlement: null });
    vi.mocked(commands.consumerGetEntitlement).mockResolvedValue(
      NEUTRAL_ENTITLEMENT(),
    );

    await useConsumerStore.getState().refresh();

    expect(useConsumerStore.getState().entitlement?.status).toBe("none");
  });

  it("refresh with a null response does not clobber an active user", async () => {
    // Server unreachable / 401 / offline: consumerGetEntitlement returns null.
    // Must not overwrite the user's valid state; otherwise a paying user would
    // be kicked to the paywall on a single bad network call.
    useConsumerStore.setState({ entitlement: ACTIVE_ENTITLEMENT() });
    vi.mocked(commands.consumerGetEntitlement).mockResolvedValue(null);

    await useConsumerStore.getState().refresh();

    expect(useConsumerStore.getState().entitlement?.status).toBe("active");
  });
});
