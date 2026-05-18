// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../lib/tauri-commands", () => ({
  consumerBillingCheckoutUrl: vi.fn().mockResolvedValue("https://example/checkout"),
  consumerGetEntitlement: vi.fn().mockResolvedValue(null),
}));
vi.mock("../lib/platform", () => ({
  isMac: true,
  isWindows: false,
  isLinux: false,
  deviceLabel: "Mac",
  osName: "macOS",
}));

import { SubscribeModal } from "./SubscribeModal";
import { useConsumerStore } from "../stores/consumerStore";

function resetStore() {
  useConsumerStore.setState({
    entitlement: {
      plan: null,
      status: "trialing",
      trial_started_at: 1_700_000_000,
      trial_ends_at: 1_700_604_800,
      period_start: null,
      period_end: null,
      usage_used: 0,
      usage_limit: 100,
      fix_count_total: 0,
    },
    hydrated: true,
    subscribeModal: null,
    postCheckoutPollUntil: null,
  });
  useConsumerStore.getState().stopPostCheckoutPolling();
}

describe("SubscribeModal — post-checkout reassurance footnote", () => {
  beforeEach(() => {
    resetStore();
  });
  afterEach(() => {
    cleanup();
    resetStore();
  });

  it("shows the normal trust footnote before checkout is opened", () => {
    render(
      <SubscribeModal variant="second_issue" onDismiss={() => {}} />,
    );
    // The "$0 today · cancel any time" footnote is visible.
    expect(
      screen.getByText(/cancel any time/i),
    ).toBeTruthy();
    // The reassurance line is NOT shown — would broadcast internal state
    // to users who haven't paid yet.
    expect(screen.queryByText(/already subscribed/i)).toBeNull();
  });

  it("swaps to the reassurance line once the post-checkout poll is running", () => {
    // Simulate "user clicked Subscribe → poll started".
    useConsumerStore.setState({
      postCheckoutPollUntil: Date.now() + 15 * 60 * 1000,
    });
    render(
      <SubscribeModal variant="second_issue" onDismiss={() => {}} />,
    );
    expect(
      screen.getByText(/already subscribed\?/i),
    ).toBeTruthy();
    // The normal footnote is gone — same visual slot, different message.
    expect(screen.queryByText(/cancel any time/i)).toBeNull();
  });
});
