// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import type { Entitlement, HealthScore } from "../lib/tauri-commands";
import { useConsumerStore } from "../stores/consumerStore";
import { OnboardingFlow } from "./OnboardingFlow";

// Stub SignInScreen so these tests don't pull its i18n/command deps — we only
// care that the handoff renders it, seeds the problem, and routes onComplete.
vi.mock("./SignInScreen", () => ({
  SignInScreen: (props: {
    onComplete: () => void;
    seedContext?: { label: string; seedMessage: string } | null;
  }) => (
    <div data-testid="ob-signin" data-seed={props.seedContext?.seedMessage ?? ""}>
      <button onClick={props.onComplete}>mock-signin-complete</button>
    </div>
  ),
}));

const liveScore: HealthScore = {
  overall_score: 60, overall_grade: "C",
  categories: [{ category: "memory", score: 60, grade: "C", checks: [
    { id: "m", category: "memory", label: "RAM almost full", status: "fail", detail: "9 GB used" },
  ] }],
  computed_at: "now", device_id: null,
};

function setArm(placement: "launch" | "after_fix", extra: Partial<Entitlement> = {}) {
  useConsumerStore.setState({
    entitlement: {
      plan: null, status: "none", trial_started_at: null, trial_ends_at: null,
      period_start: null, period_end: null, usage_used: 0, usage_limit: 10,
      fix_count_total: 0, onboarding_paywall: placement === "launch", ...extra,
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

  it("uses LIVE diagnostics when the scan returns findings", async () => {
    setArm("after_fix");
    render(<OnboardingFlow onComplete={() => {}} scanDurationMs={0} fetchHealthScore={async () => liveScore} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByText("RAM almost full")).toBeTruthy());
    // the curated default must NOT show when real data is present
    expect(screen.queryByText("Tied up by Chrome + 47 tabs")).toBeNull();
  });

  it("falls back to curated defaults when live diagnostics are empty", async () => {
    setArm("after_fix");
    render(<OnboardingFlow onComplete={() => {}} scanDurationMs={0} fetchHealthScore={async () => null} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByText("Tied up by Chrome + 47 tabs")).toBeTruthy());
  });

  it("shows problem-appropriate findings for Wi-Fi (general-purpose, not a cleanup fallback)", async () => {
    setArm("after_fix");
    render(<OnboardingFlow onComplete={() => {}} initialProblem="wifi" scanDurationMs={0} fetchHealthScore={async () => null} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByText("DNS is timing out")).toBeTruthy());
    expect(screen.queryByText("Tied up by Chrome + 47 tabs")).toBeNull();
  });

  it("the reveal CTA hands off to sign-in, seeded with the problem", async () => {
    setArm("after_fix");
    render(<OnboardingFlow onComplete={() => {}} initialProblem="storage" scanDurationMs={0} fetchHealthScore={async () => null} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByTestId("ob-reveal")).toBeTruthy());
    fireEvent.click(screen.getByText("Fix it →"));
    const signin = screen.getByTestId("ob-signin");
    expect(signin).toBeTruthy();
    expect(signin.getAttribute("data-seed")).toBe("I'm running low on storage");
  });

  it("completing sign-in finishes onboarding", async () => {
    setArm("after_fix");
    const onComplete = vi.fn();
    render(<OnboardingFlow onComplete={onComplete} scanDurationMs={0} fetchHealthScore={async () => null} />);
    fireEvent.click(screen.getByText("Get started"));
    fireEvent.click(screen.getByText("Look into it →"));
    await waitFor(() => expect(screen.getByTestId("ob-reveal")).toBeTruthy());
    fireEvent.click(screen.getByText("Fix it →"));
    fireEvent.click(screen.getByText("mock-signin-complete"));
    expect(onComplete).toHaveBeenCalled();
  });
});
