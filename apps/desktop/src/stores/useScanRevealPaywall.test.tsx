// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";

import type { Entitlement } from "../lib/tauri-commands";
import { useConsumerStore } from "./consumerStore";
import { useScanRevealPaywall } from "./useScanRevealPaywall";

function ent(overrides: Partial<Entitlement> = {}): Entitlement {
  return {
    plan: null,
    status: "none",
    trial_started_at: null,
    trial_ends_at: null,
    period_start: null,
    period_end: null,
    usage_used: 0,
    usage_limit: 10,
    fix_count_total: 0,
    ...overrides,
  };
}

afterEach(() => {
  useConsumerStore.setState({ entitlement: null, subscribeModal: null });
});

describe("useScanRevealPaywall", () => {
  it("launch arm: opens scan_reveal once the scan reveals findings", () => {
    useConsumerStore.setState({ entitlement: ent({ paywall_placement: "launch" }) });
    const { rerender } = renderHook(
      (p: { scanRevealed: boolean }) =>
        useScanRevealPaywall({ scanRevealed: p.scanRevealed, firstFixReached: false }),
      { initialProps: { scanRevealed: false } },
    );
    expect(useConsumerStore.getState().subscribeModal).toBeNull();

    rerender({ scanRevealed: true });
    expect(useConsumerStore.getState().subscribeModal).toEqual({ variant: "scan_reveal" });
  });

  it("is one-shot: dismissing it does not re-pop", () => {
    useConsumerStore.setState({ entitlement: ent({ paywall_placement: "launch" }) });
    const { rerender } = renderHook(
      () => useScanRevealPaywall({ scanRevealed: true, firstFixReached: false }),
    );
    expect(useConsumerStore.getState().subscribeModal).toEqual({ variant: "scan_reveal" });

    // User picks "Maybe later" → modal closes.
    useConsumerStore.getState().closeSubscribeModal();
    expect(useConsumerStore.getState().subscribeModal).toBeNull();

    // A later entitlement refresh re-runs the effect — must NOT re-open.
    rerender();
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("after_fix arm: never opens the launch paywall", () => {
    useConsumerStore.setState({ entitlement: ent({ paywall_placement: "after_fix" }) });
    renderHook(() => useScanRevealPaywall({ scanRevealed: true, firstFixReached: false }));
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("never interrupts a user already trialing", () => {
    useConsumerStore.setState({
      entitlement: ent({ paywall_placement: "launch", status: "trialing" }),
    });
    renderHook(() => useScanRevealPaywall({ scanRevealed: true, firstFixReached: false }));
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("does not fire before the entitlement has loaded", () => {
    useConsumerStore.setState({ entitlement: null });
    renderHook(() => useScanRevealPaywall({ scanRevealed: true, firstFixReached: false }));
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });
});
