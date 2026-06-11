// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import type { Entitlement } from "../lib/tauri-commands";
import { useConsumerStore } from "../stores/consumerStore";
import { OnboardingFlow } from "./OnboardingFlow";

function setArm(placement: "launch" | "after_fix", extra: Partial<Entitlement> = {}) {
  useConsumerStore.setState({
    entitlement: {
      plan: null, status: "none", trial_started_at: null, trial_ends_at: null,
      period_start: null, period_end: null, usage_used: 0, usage_limit: 10,
      fix_count_total: 0, paywall_placement: placement, ...extra,
    },
  });
}

afterEach(() => {
  cleanup();
  useConsumerStore.setState({ entitlement: null, subscribeModal: null });
});

describe("OnboardingFlow", () => {
  it("walks welcome → pick → scan → reveal (problem-led)", async () => {
    setArm("after_fix"); // no paywall in the way for the navigation check
    render(<OnboardingFlow onComplete={() => {}} scanDurationMs={0} />);

    expect(screen.getByTestId("ob-welcome")).toBeTruthy();
    fireEvent.click(screen.getByText("Get started"));
    expect(screen.getByTestId("ob-pick")).toBeTruthy();
    expect(screen.getByText("What's bugging you?")).toBeTruthy();

    fireEvent.click(screen.getByText("Look into it →"));
    expect(screen.getByTestId("ob-scan")).toBeTruthy();

    await waitFor(() => expect(screen.getByTestId("ob-reveal")).toBeTruthy());
    // diagnosis, not a junk list
    expect(screen.getByText("Tied up by Chrome + 47 tabs")).toBeTruthy();
  });

  it("launch arm: surfaces the scan_reveal paywall at the reveal", async () => {
    setArm("launch");
    render(<OnboardingFlow onComplete={() => {}} scanDurationMs={0} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));

    await waitFor(() =>
      expect(useConsumerStore.getState().subscribeModal).toEqual({ variant: "scan_reveal" }),
    );
  });

  it("after_fix arm: no paywall during onboarding", async () => {
    setArm("after_fix");
    render(<OnboardingFlow onComplete={() => {}} scanDurationMs={0} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByTestId("ob-reveal")).toBeTruthy());
    expect(useConsumerStore.getState().subscribeModal).toBeNull();
  });

  it("honors the ad's pre-selected problem (storage)", async () => {
    setArm("after_fix");
    render(<OnboardingFlow onComplete={() => {}} initialProblem="storage" scanDurationMs={0} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    // storage-specific diagnosis card (headline text is split across spans)
    await waitFor(() => expect(screen.getByText("Reclaimable now")).toBeTruthy());
  });

  it("the reveal CTA completes onboarding", async () => {
    setArm("after_fix");
    const onComplete = vi.fn();
    render(<OnboardingFlow onComplete={onComplete} scanDurationMs={0} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByTestId("ob-reveal")).toBeTruthy());
    fireEvent.click(screen.getByText("Fix it →"));
    expect(onComplete).toHaveBeenCalled();
  });
});
