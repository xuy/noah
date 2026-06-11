import { create } from "zustand";
import * as commands from "../lib/tauri-commands";
import type { Entitlement } from "../lib/tauri-commands";

export type SubscribeModalVariant =
  | "first_fix"
  | "second_issue"
  | "paywall"
  | "cap_hit"
  // Launch-arm of the placement A/B: shown right after the onboarding scan
  // reveals personalized findings — proof first, then "start your free trial
  // to fix it." The card-on-file trial it opens is identical to every other
  // variant; only the timing differs.
  | "scan_reveal";

interface ConsumerState {
  entitlement: Entitlement | null;
  /** True once we've done the initial fetch (or given up after an error). */
  hydrated: boolean;
  subscribeModal: { variant: SubscribeModalVariant } | null;
  /**
   * Wall-clock ms at which the post-checkout poll loop should stop, or
   * null when no poll is in flight. Used by tests + UI ("verifying…")
   * to show a deterministic state during the activation window.
   */
  postCheckoutPollUntil: number | null;
  refresh: () => Promise<Entitlement | null>;
  setEntitlement: (e: Entitlement | null) => void;
  openSubscribeModal: (variant: SubscribeModalVariant) => void;
  closeSubscribeModal: () => void;
  /**
   * Start polling /entitlement every 3s until status flips to "active"
   * or 15 minutes elapse. Idempotent — calling again while a poll is
   * already running just extends the deadline. Returns a cancel fn so
   * callers can stop the loop early (e.g. on unmount).
   *
   * This is the Windows / fallback path for "you just paid in the
   * browser — when do we know?". On Mac the noah://subscribed deep
   * link still wins the race; on Windows the deep link is unreliable
   * (second-instance launches without single-instance plugin), so the
   * poll is the activation mechanism.
   */
  startPostCheckoutPolling: () => () => void;
  stopPostCheckoutPolling: () => void;
}

/**
 * Derived helper: is the user currently paywalled from starting a new fix?
 */
export function isPaywalled(ent: Entitlement | null): boolean {
  if (!ent) return false;
  if (ent.status === "none" || ent.status === "trialing") return false;
  if (ent.status === "active") return ent.usage_used >= ent.usage_limit;
  return true;
}

/**
 * Card-first: is the onboarding paywall active? Reads the single backend lever
 * (`onboarding_paywall`). Missing/false → no onboarding paywall (legacy no-card
 * model, or an older/loading server) — we never paywall on missing data.
 */
export function onboardingPaywallOn(ent: Entitlement | null): boolean {
  return ent?.onboarding_paywall === true;
}

/** Signals the onboarding orchestrator feeds the placement decision. */
export interface PaywallSignals {
  /** The onboarding scan finished and personalized findings are on screen. */
  scanRevealed: boolean;
  /** Kept for the caller; unused under card-first (the wall is at the reveal). */
  firstFixReached: boolean;
}

/**
 * Card-first onboarding paywall decision (pure). Returns "scan_reveal" — the
 * card-on-file trial paywall — to surface at the diagnosis, or null.
 *
 * The launch/after_fix A/B is retired: *everyone* with the backend toggle on
 * sees it once the scan has revealed proof, except users already trialing or
 * paying. "Maybe later" drops them into the silent 1-fix taste; the paywall
 * returns naturally when they reach for more (no "X fixes left" counter).
 */
export function scanRevealPaywallVariant(
  ent: Entitlement | null,
  signals: PaywallSignals,
): SubscribeModalVariant | null {
  if (!onboardingPaywallOn(ent)) return null;
  if (ent!.status === "trialing" || ent!.status === "active") return null;
  if (!signals.scanRevealed) return null; // wait for proof
  return "scan_reveal";
}

/**
 * Merge a new entitlement response with the current one, ignoring stale
 * responses that would regress known state.
 *
 * The race this guards against: MainApp's initial GET /entitlement can be
 * in flight when useAgent fires POST /events/issue-started. The server
 * processes the GET while the trial is still "none" and the POST right
 * after, so the GET response is *accurate for its read time* but *stale
 * by the time it reaches the client*. Without this guard, the late GET
 * overwrites "trialing" back to "none" and the subscribe modal never fires
 * on the next RUN_STEP click.
 *
 * Rule: if we already observed `trial_started_at` or `period_start`, a
 * later response that *lacks* that field is stale — keep what we have.
 */
function mergeEntitlement(
  prev: Entitlement | null,
  next: Entitlement | null,
): Entitlement | null {
  // A null response means "no info" (network error, 401, etc.) — never
  // clobber a valid entitlement with null, that would kick a paying user
  // straight to the paywall.
  if (!next) return prev;
  if (!prev) return next;
  // Stale-GET guard: if prev has a started timestamp and next doesn't,
  // next is stale (it was read on the server before issue-started/confirm
  // committed). Keep prev.
  const prevStarted = prev.trial_started_at || prev.period_start;
  const nextStarted = next.trial_started_at || next.period_start;
  if (prevStarted && !nextStarted) return prev;
  return next;
}

/** 3 seconds between polls — fast enough that activation feels instant
 *  once the Stripe webhook lands, slow enough that 15 min of polling is
 *  only ~300 GETs against a tiny endpoint. */
const POLL_INTERVAL_MS = 3_000;
/** Hard cap on poll duration. After this we stop and trust the next
 *  natural refresh (window focus, periodic poll) to pick up activation.
 *  15 min covers nearly every realistic Stripe webhook delivery window. */
const POLL_MAX_MS = 15 * 60 * 1000;

// Lives outside the store so React strict mode double-mounts don't
// spawn duplicate intervals — there is only one global poll handle.
let pollTimer: ReturnType<typeof setInterval> | null = null;

export const useConsumerStore = create<ConsumerState>((set, get) => ({
  entitlement: null,
  hydrated: false,
  subscribeModal: null,
  postCheckoutPollUntil: null,
  refresh: async () => {
    try {
      const ent = await commands.consumerGetEntitlement();
      set((state) => ({
        entitlement: mergeEntitlement(state.entitlement, ent),
        hydrated: true,
      }));
      return ent;
    } catch {
      set({ hydrated: true });
      return null;
    }
  },
  setEntitlement: (e) =>
    set((state) => ({ entitlement: mergeEntitlement(state.entitlement, e) })),
  openSubscribeModal: (variant) => set({ subscribeModal: { variant } }),
  closeSubscribeModal: () => set({ subscribeModal: null }),
  startPostCheckoutPolling: () => {
    const deadline = Date.now() + POLL_MAX_MS;
    // If a poll is already in flight, just push the deadline out — no
    // need to tear it down and rebuild.
    if (pollTimer != null) {
      set({ postCheckoutPollUntil: deadline });
      return () => get().stopPostCheckoutPolling();
    }
    set({ postCheckoutPollUntil: deadline });
    // Fire one refresh immediately so a fast webhook gets reflected
    // without waiting a full tick.
    void get().refresh();
    pollTimer = setInterval(() => {
      const state = get();
      const until = state.postCheckoutPollUntil;
      // Deadline elapsed or another call cleared the flag → stop.
      if (until == null || Date.now() >= until) {
        get().stopPostCheckoutPolling();
        return;
      }
      // Subscription is live → done. closeSubscribeModal is left to the
      // caller (the modal watches entitlement and shows the success
      // state); we just stop the loop.
      if (state.entitlement?.status === "active") {
        get().stopPostCheckoutPolling();
        return;
      }
      void get().refresh();
    }, POLL_INTERVAL_MS);
    return () => get().stopPostCheckoutPolling();
  },
  stopPostCheckoutPolling: () => {
    if (pollTimer != null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    set({ postCheckoutPollUntil: null });
  },
}));
