// Tests for the card-first onboarding paywall decision logic.
import { describe, expect, it } from "vitest";

import type { Entitlement } from "../lib/tauri-commands";
import { onboardingPaywallOn, scanRevealPaywallVariant } from "./consumerStore";

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

describe("onboardingPaywallOn", () => {
  it("true only when the server sends onboarding_paywall: true", () => {
    expect(onboardingPaywallOn(ent({ onboarding_paywall: true }))).toBe(true);
    expect(onboardingPaywallOn(ent({ onboarding_paywall: false }))).toBe(false);
    expect(onboardingPaywallOn(ent())).toBe(false); // missing / loading
    expect(onboardingPaywallOn(null)).toBe(false);
  });
});

describe("scanRevealPaywallVariant (card-first)", () => {
  const scanned = { scanRevealed: true, firstFixReached: false };

  it("shows scan_reveal once the scan reveals proof (toggle on)", () => {
    expect(scanRevealPaywallVariant(ent({ onboarding_paywall: true }), scanned)).toBe("scan_reveal");
  });

  it("stays silent until the scan reveals findings", () => {
    expect(
      scanRevealPaywallVariant(ent({ onboarding_paywall: true }), {
        scanRevealed: false,
        firstFixReached: false,
      }),
    ).toBeNull();
  });

  it("toggle off / missing → no onboarding paywall (legacy model)", () => {
    expect(scanRevealPaywallVariant(ent({ onboarding_paywall: false }), scanned)).toBeNull();
    expect(scanRevealPaywallVariant(ent(), scanned)).toBeNull();
  });

  it("never interrupts a user already trialing or active", () => {
    expect(
      scanRevealPaywallVariant(ent({ onboarding_paywall: true, status: "trialing" }), scanned),
    ).toBeNull();
    expect(
      scanRevealPaywallVariant(ent({ onboarding_paywall: true, status: "active" }), scanned),
    ).toBeNull();
  });

  it("re-shows for an expired/canceled user (re-acquisition)", () => {
    for (const status of ["expired", "canceled", "past_due"] as const) {
      expect(
        scanRevealPaywallVariant(ent({ onboarding_paywall: true, status }), scanned),
      ).toBe("scan_reveal");
    }
  });

  it("handles a null entitlement safely (still loading)", () => {
    expect(scanRevealPaywallVariant(null, scanned)).toBeNull();
  });
});
