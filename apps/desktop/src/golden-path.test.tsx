// @vitest-environment jsdom
/**
 * Golden-path end-to-end test.
 *
 * Walks the acquisition loop in the post-redesign trial model:
 *
 *   1. User installed Noah and has already had a prior issue (first-issue
 *      session id is pre-seeded into localStorage to simulate this).
 *   2. Tile picker appears; user picks a category + clarifier.
 *   3. ChatPanel auto-sends the seed → second-issue trigger fires →
 *      subscribe modal opens with variant="second_issue".
 *   4. User clicks Subscribe → Stripe Checkout URL opens in browser.
 *   5. `noah://subscribed?session_id=…` deep link arrives → backend confirms.
 *
 * Unlike `onboarding.test.tsx` (which stubs ChatPanel / SubscribeModal to
 * isolate individual regressions), THIS file renders the real ChatPanel and
 * real SubscribeModal so we exercise the whole tree the user sees.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup, act, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

Element.prototype.scrollIntoView = vi.fn();

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

// ── Tauri plugin / API mocks ──
// vi.mock is hoisted to the top of the file, so factories CANNOT reference
// module-level const/let. Use vi.hoisted to share state between the factory
// and the test body.

const { deepLinkRef, openUrlMock } = vi.hoisted(() => ({
  deepLinkRef: {
    current: null as
      | ((urls: string[]) => unknown | Promise<unknown>)
      | null,
  },
  openUrlMock: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-deep-link", () => ({
  onOpenUrl: vi.fn((cb: (urls: string[]) => unknown) => {
    deepLinkRef.current = cb;
    return Promise.resolve(() => {
      deepLinkRef.current = null;
    });
  }),
  getCurrent: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: openUrlMock }));
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
vi.mock("./lib/platform", () => ({
  isMac: true,
  isWindows: false,
  isLinux: false,
  deviceLabel: "Mac",
  osName: "macOS",
}));

// ── Full tauri-commands mock surface ──
// The real ChatPanel + SubscribeModal pull in a wider slice of commands than
// `onboarding.test.tsx`, so we stub every one used transitively. Each test
// rewires the few that matter (listSessions, createSession, sendMessageV2,
// consumerGetEntitlement, consumerBillingCheckoutUrl, consumerConfirmCheckout).
vi.mock("./lib/tauri-commands", () => ({
  // Session / chat history
  createSession: vi.fn(),
  listSessions: vi.fn().mockResolvedValue([]),
  endSession: vi.fn().mockResolvedValue(undefined),
  deleteSession: vi.fn().mockResolvedValue(undefined),
  renameSession: vi.fn().mockResolvedValue(undefined),
  getSessionMessages: vi.fn().mockResolvedValue([]),
  getChanges: vi.fn().mockResolvedValue([]),
  getSessionSummary: vi.fn().mockResolvedValue(""),
  markResolved: vi.fn().mockResolvedValue(undefined),
  exportSession: vi.fn().mockResolvedValue(""),
  sendMessage: vi.fn().mockResolvedValue(""),
  sendMessageV2: vi.fn(),
  sendUserEvent: vi
    .fn()
    .mockResolvedValue({ text: "ack", assistant_ui: undefined }),
  recordActionConfirmation: vi.fn().mockResolvedValue(undefined),
  cancelProcessing: vi.fn().mockResolvedValue(undefined),
  approveAction: vi.fn().mockResolvedValue(undefined),
  denyAction: vi.fn().mockResolvedValue(undefined),
  undoChange: vi.fn().mockResolvedValue(undefined),

  // Knowledge / locale / mode
  listKnowledge: vi.fn().mockResolvedValue([]),
  readKnowledgeFile: vi.fn().mockResolvedValue(""),
  deleteKnowledgeFile: vi.fn().mockResolvedValue(undefined),
  setLocale: vi.fn().mockResolvedValue(undefined),
  setSessionMode: vi.fn().mockResolvedValue(undefined),
  storeSecret: vi.fn().mockResolvedValue(undefined),

  // Consumer (auth + entitlement + billing)
  consumerHasSession: vi.fn().mockResolvedValue(false),
  consumerEnsureDeviceId: vi.fn().mockResolvedValue("device-abc"),
  consumerRequestMagicLink: vi.fn().mockResolvedValue(null),
  consumerCompleteSignIn: vi.fn(),
  consumerSignOut: vi.fn().mockResolvedValue(undefined),
  consumerGetEntitlement: vi.fn(),
  consumerNotifyIssueStarted: vi.fn(),
  consumerNotifyFixCompleted: vi.fn().mockResolvedValue(null),
  consumerBillingCheckoutUrl: vi.fn(),
  consumerBillingPortalUrl: vi.fn().mockResolvedValue(""),
  consumerConfirmCheckout: vi.fn(),

  // Misc commands that MainApp / Settings call on mount
  hasApiKey: vi.fn().mockResolvedValue(false),
  setApiKey: vi.fn().mockResolvedValue(undefined),
  clearAuth: vi.fn().mockResolvedValue(undefined),
  getAuthMode: vi.fn().mockResolvedValue("proxy"),
  checkProxyStatus: vi.fn().mockResolvedValue('{"status":"active"}'),
  getAppVersion: vi.fn().mockResolvedValue("0.18.0"),
  trackEvent: vi.fn().mockResolvedValue(undefined),
  getTelemetryConsent: vi.fn().mockResolvedValue(true),
  setTelemetryConsent: vi.fn().mockResolvedValue(undefined),
  getFeedbackContext: vi
    .fn()
    .mockResolvedValue({ version: "0.18.0", os: "mac", traces: [] }),
  getProactiveEnabled: vi.fn().mockResolvedValue(false),
  setProactiveEnabled: vi.fn().mockResolvedValue(undefined),
  dismissProactiveSuggestion: vi.fn().mockResolvedValue(undefined),
  actOnProactiveSuggestion: vi.fn().mockResolvedValue(undefined),
  getAutoHealEnabled: vi.fn().mockResolvedValue(false),
  setAutoHealEnabled: vi.fn().mockResolvedValue(undefined),
  getScanJobs: vi.fn().mockResolvedValue([]),
  triggerScan: vi.fn().mockResolvedValue(""),
  pauseScan: vi.fn().mockResolvedValue(undefined),
  resumeScan: vi.fn().mockResolvedValue(undefined),
  getHealthScore: vi.fn().mockResolvedValue("null"),
  runHealthCheck: vi.fn().mockResolvedValue("null"),
  openHealthFix: vi.fn().mockResolvedValue(undefined),
  getHealthHistory: vi.fn().mockResolvedValue("[]"),
  exportHealthReport: vi.fn().mockResolvedValue(""),
  linkDashboard: vi.fn().mockResolvedValue(""),
  unlinkDashboard: vi.fn().mockResolvedValue(undefined),
  getDashboardStatus: vi
    .fn()
    .mockResolvedValue('{"linked":false}'),
  getFleetActions: vi.fn().mockResolvedValue("[]"),
  resolveFleetAction: vi.fn().mockResolvedValue(undefined),
  startFleetPlaybook: vi.fn().mockResolvedValue(""),
  verifyRemediation: vi.fn().mockResolvedValue(""),
}));

function TRIAL_ENTITLEMENT() {
  return {
    plan: "trial",
    status: "trialing" as const,
    trial_started_at: Date.now() - 1000,
    trial_ends_at: Date.now() + 7 * 86_400_000,
    tz_offset_minutes: new Date().getTimezoneOffset(),
    period_start: null,
    period_end: null,
    usage_used: 0,
    usage_limit: 10,
    fix_count_total: 0,
  };
}
function ACTIVE_ENTITLEMENT() {
  return {
    plan: "annual",
    status: "active" as const,
    trial_started_at: null,
    trial_ends_at: null,
    period_start: Date.now(),
    period_end: Date.now() + 365 * 86_400_000,
    usage_used: 0,
    usage_limit: 100,
    fix_count_total: 0,
  };
}

// ── Real imports after mocks ──
import App from "./App";
import { useSessionStore } from "./stores/sessionStore";
import { useChatStore } from "./stores/chatStore";
import { useConsumerStore } from "./stores/consumerStore";
import * as commands from "./lib/tauri-commands";

function resetStores() {
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

beforeEach(() => {
  vi.clearAllMocks();
  resetStores();
  deepLinkRef.current = null;
  openUrlMock.mockClear();

  // Default: clean install, trialing entitlement, fresh session, Noah's
  // SPA response (situation + plan + RUN_STEP action). Individual tests
  // override as needed.
  vi.mocked(commands.listSessions).mockResolvedValue([]);
  vi.mocked(commands.consumerGetEntitlement).mockResolvedValue(
    TRIAL_ENTITLEMENT(),
  );
  vi.mocked(commands.createSession).mockResolvedValue({
    id: "s-golden",
    created_at: new Date().toISOString(),
    message_count: 0,
  });
  vi.mocked(commands.sendMessageV2).mockResolvedValue({
    text: "ack",
    assistant_ui: {
      kind: "spa",
      situation: "Your Wi-Fi is dropping frequently during streaming.",
      plan: "Reset the Wi-Fi adapter and flush DNS to clear bad cache entries.",
      action: { label: "Please fix it", type: "RUN_STEP" },
    },
  });
  vi.mocked(commands.consumerBillingCheckoutUrl).mockResolvedValue(
    "https://checkout.stripe.com/cs_test_abc",
  );
  vi.mocked(commands.consumerConfirmCheckout).mockResolvedValue(
    ACTIVE_ENTITLEMENT(),
  );
  vi.mocked(commands.consumerCompleteSignIn).mockResolvedValue(
    TRIAL_ENTITLEMENT(),
  );
  vi.mocked(commands.consumerNotifyIssueStarted).mockResolvedValue(
    TRIAL_ENTITLEMENT(),
  );
});

afterEach(() => {
  cleanup();
});

describe("Golden path — second-issue → subscribe → pay", () => {
  // Pre-seed localStorage so this user counts as already past their first
  // issue. Each test then exercises the second-issue trigger on the first
  // message of the new session.
  beforeEach(() => {
    localStorage.setItem("noah.firstIssueSessionId", "s-prior");
  });

  it("walks the full acquisition loop end to end", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Tile picker is what a returning user still sees on a fresh launch
    // when they have no in-flight session loaded.
    expect(
      await screen.findByText(/What's going on with your Mac/),
    ).toBeTruthy();

    await user.click(screen.getByText("Wi-Fi or internet issues"));
    const clarifier = await screen.findByRole("textbox");
    await user.type(clarifier, "drops every 10 min at home");
    await user.click(screen.getByText("Continue"));

    // Seed auto-sends → second-issue modal pops in parallel.
    await waitFor(() => {
      expect(commands.sendMessageV2).toHaveBeenCalled();
    });
    const [, firstMessage] = vi.mocked(commands.sendMessageV2).mock.calls[0];
    expect(firstMessage).toContain("Wi-Fi or internet issues");

    expect(
      await screen.findByText(/Keep Noah on your Mac/),
    ).toBeTruthy();
    expect(useConsumerStore.getState().subscribeModal).toEqual({
      variant: "second_issue",
    });

    await user.click(screen.getByRole("button", { name: "Keep Noah" }));
    await waitFor(() => {
      expect(commands.consumerBillingCheckoutUrl).toHaveBeenCalledWith(
        "annual",
      );
      expect(openUrlMock).toHaveBeenCalledWith(
        "https://checkout.stripe.com/cs_test_abc",
      );
    });

    expect(deepLinkRef.current).not.toBeNull();
    await act(async () => {
      await deepLinkRef.current!([
        "noah://subscribed?session_id=cs_test_abc",
      ]);
    });
    expect(commands.consumerConfirmCheckout).toHaveBeenCalledWith(
      "cs_test_abc",
    );
  });

  it("selecting monthly before Subscribe passes 'monthly' to checkout", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText(/What's going on with your Mac/);
    await user.click(screen.getByText("My Mac feels slow"));
    await user.type(
      await screen.findByRole("textbox"),
      "Safari lags for ten seconds every new tab",
    );
    await user.click(screen.getByText("Continue"));

    await screen.findByText(/Keep Noah on your Mac/);

    await user.click(screen.getByLabelText(/Monthly/));
    await user.click(screen.getByRole("button", { name: "Keep Noah" }));

    await waitFor(() => {
      expect(commands.consumerBillingCheckoutUrl).toHaveBeenCalledWith(
        "monthly",
      );
    });
  });

  it("'Keep my free trial' dismisses without opening checkout", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText(/What's going on with your Mac/);
    await user.click(screen.getByText("Battery drains fast"));
    await user.type(
      await screen.findByRole("textbox"),
      "used to get 8h now I get 3",
    );
    await user.click(screen.getByText("Continue"));

    await screen.findByText(/Keep Noah on your Mac/);

    await user.click(screen.getByRole("button", { name: "Not yet — keep my free trial" }));
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
    expect(commands.consumerBillingCheckoutUrl).not.toHaveBeenCalled();
    expect(openUrlMock).not.toHaveBeenCalled();
  });
});
