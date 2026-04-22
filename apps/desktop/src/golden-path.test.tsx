// @vitest-environment jsdom
/**
 * Golden-path end-to-end test.
 *
 * Walks the one flow that matters most — the acquisition loop:
 *
 *   1. User has just installed Noah (fresh, no sessions).
 *   2. Tile picker appears; user picks a category + clarifier.
 *   3. Noah's backend responds with a Situation/Plan/Action card.
 *   4. User clicks the action button ("Please fix it").
 *   5. Subscribe modal appears (first-fix commitment moment).
 *   6. User clicks Subscribe → Stripe Checkout URL opens in browser.
 *   7. `noah://subscribed?session_id=…` deep link arrives → backend confirms.
 *
 * Unlike `onboarding.test.tsx` (which stubs ChatPanel / SubscribeModal to
 * isolate individual regressions), THIS file renders the real ChatPanel and
 * real SubscribeModal so we exercise the whole tree the user sees. If any
 * link in the chain breaks — seed pickup, SPA rendering, modal variant,
 * Stripe URL open, deep-link round-trip — this test goes red.
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
vi.mock("./lib/platform", () => ({ isMac: true }));

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
  redeemInviteCode: vi.fn().mockResolvedValue(undefined),
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
    period_start: null,
    period_end: null,
    usage_used: 0,
    usage_limit: 100,
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

describe("Golden path — install → tile → SPA → action → subscribe → pay", () => {
  it("walks the full acquisition loop end to end", async () => {
    const user = userEvent.setup();
    render(<App />);

    // ── Step 1: tile picker is what a first-time user sees ───────────────
    expect(
      await screen.findByText(/What's going on with your Mac/),
    ).toBeTruthy();

    // ── Step 2: pick a category, add a clarifier, continue ───────────────
    await user.click(screen.getByText("Wi-Fi or internet issues"));
    const clarifier = await screen.findByRole("textbox");
    await user.type(clarifier, "drops every 10 min at home");
    await user.click(screen.getByText("Continue"));

    // ── Step 3: ChatPanel mounts, picks up the seed, auto-sends it ───────
    await waitFor(
      () => {
        expect(commands.sendMessageV2).toHaveBeenCalled();
      },
      { timeout: 3000 },
    );
    const [, firstMessage] = vi.mocked(commands.sendMessageV2).mock.calls[0];
    expect(firstMessage).toContain("Wi-Fi or internet issues");
    expect(firstMessage).toContain("drops every 10 min at home");

    // ── Step 4: Noah renders its Situation + Plan + action button ────────
    expect(
      await screen.findByText(/Your Wi-Fi is dropping frequently/),
    ).toBeTruthy();
    expect(await screen.findByText(/Reset the Wi-Fi adapter/)).toBeTruthy();
    const fixButton = await screen.findByRole("button", {
      name: "Please fix it",
    });

    // Nothing should be paywalling yet — we're inside the trial.
    expect(useConsumerStore.getState().subscribeModal).toBeNull();

    // ── Step 5: user commits to the fix → subscribe modal appears ────────
    await user.click(fixButton);
    expect(
      await screen.findByText(/Nice — that's one fix down/),
    ).toBeTruthy();
    expect(useConsumerStore.getState().subscribeModal).toEqual({
      variant: "first_fix",
    });

    // ── Step 6: user subscribes → Stripe Checkout URL is opened ──────────
    await user.click(screen.getByRole("button", { name: "Subscribe" }));
    await waitFor(() => {
      // Default plan selection is "annual" — matches en.json.
      expect(commands.consumerBillingCheckoutUrl).toHaveBeenCalledWith(
        "annual",
      );
      expect(openUrlMock).toHaveBeenCalledWith(
        "https://checkout.stripe.com/cs_test_abc",
      );
    });

    // ── Step 7: Stripe returns via deep link → backend confirms ──────────
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

    await screen.findByText(/Your Wi-Fi is dropping frequently/);
    await user.click(
      await screen.findByRole("button", { name: "Please fix it" }),
    );

    // Switch the radio selection from annual (default) to monthly.
    await user.click(screen.getByLabelText(/Monthly/));
    await user.click(screen.getByRole("button", { name: "Subscribe" }));

    await waitFor(() => {
      expect(commands.consumerBillingCheckoutUrl).toHaveBeenCalledWith(
        "monthly",
      );
    });
  });

  it("'Not now' dismisses the modal and keeps the fix ready", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText(/What's going on with your Mac/);
    await user.click(screen.getByText("Battery drains fast"));
    await user.type(
      await screen.findByRole("textbox"),
      "used to get 8h now I get 3",
    );
    await user.click(screen.getByText("Continue"));

    await screen.findByText(/Your Wi-Fi is dropping frequently/);
    await user.click(
      await screen.findByRole("button", { name: "Please fix it" }),
    );
    await screen.findByText(/Nice — that's one fix down/);

    await user.click(screen.getByRole("button", { name: "Not now" }));
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
    // No Stripe call was made.
    expect(commands.consumerBillingCheckoutUrl).not.toHaveBeenCalled();
    expect(openUrlMock).not.toHaveBeenCalled();
  });
});
