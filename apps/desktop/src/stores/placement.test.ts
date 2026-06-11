// Tests for the scan-reveal paywall-placement A/B decision logic.
import { describe, expect, it } from "vitest";

import type { Entitlement } from "../lib/tauri-commands";
import { placementArm, scanRevealPaywallVariant } from "./consumerStore";

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

describe("placementArm", () => {
  it("reads the server arm when present", () => {
    expect(placementArm(ent({ paywall_placement: "launch" }))).toBe("launch");
    expect(placementArm(ent({ paywall_placement: "after_fix" }))).toBe("after_fix");
  });

  it("defaults to after_fix (value-first) when missing or null", () => {
    expect(placementArm(ent())).toBe("after_fix");
    expect(placementArm(ent({ paywall_placement: null }))).toBe("after_fix");
    expect(placementArm(null)).toBe("after_fix");
  });
});

describe("scanRevealPaywallVariant", () => {
  const scanned = { scanRevealed: true, firstFixReached: false };

  it("launch arm: shows scan_reveal once the scan has revealed proof", () => {
    expect(
      scanRevealPaywallVariant(ent({ paywall_placement: "launch" }), scanned),
    ).toBe("scan_reveal");
  });

  it("launch arm: stays silent until the scan reveals findings", () => {
    expect(
      scanRevealPaywallVariant(ent({ paywall_placement: "launch" }), {
        scanRevealed: false,
        firstFixReached: false,
      }),
    ).toBeNull();
  });

  it("after_fix arm: never fires the launch paywall (handled elsewhere)", () => {
    expect(
      scanRevealPaywallVariant(ent({ paywall_placement: "after_fix" }), scanned),
    ).toBeNull();
    // missing arm defaults to after_fix → also null
    expect(scanRevealPaywallVariant(ent(), scanned)).toBeNull();
  });

  it("never interrupts a user already trialing or active", () => {
    expect(
      scanRevealPaywallVariant(
        ent({ paywall_placement: "launch", status: "trialing" }),
        scanned,
      ),
    ).toBeNull();
    expect(
      scanRevealPaywallVariant(
        ent({ paywall_placement: "launch", status: "active" }),
        scanned,
      ),
    ).toBeNull();
  });

  it("does not interrupt with the launch paywall once a fix is already reached", () => {
    expect(
      scanRevealPaywallVariant(ent({ paywall_placement: "launch" }), {
        scanRevealed: true,
        firstFixReached: true,
      }),
    ).toBeNull();
  });

  it("handles a null entitlement safely (still loading)", () => {
    expect(scanRevealPaywallVariant(null, scanned)).toBeNull();
  });

  it("re-shows for an expired/canceled launch-arm user (re-acquisition)", () => {
    for (const status of ["expired", "canceled", "past_due"] as const) {
      expect(
        scanRevealPaywallVariant(ent({ paywall_placement: "launch", status }), scanned),
      ).toBe("scan_reveal");
    }
  });
});
