import { create } from "zustand";
import * as commands from "../lib/tauri-commands";
import type { Entitlement } from "../lib/tauri-commands";

export type SubscribeModalVariant = "first_fix" | "paywall" | "cap_hit";

interface ConsumerState {
  entitlement: Entitlement | null;
  /** True once we've done the initial fetch (or given up after an error). */
  hydrated: boolean;
  subscribeModal: { variant: SubscribeModalVariant } | null;
  refresh: () => Promise<Entitlement | null>;
  setEntitlement: (e: Entitlement | null) => void;
  openSubscribeModal: (variant: SubscribeModalVariant) => void;
  closeSubscribeModal: () => void;
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

export const useConsumerStore = create<ConsumerState>((set) => ({
  entitlement: null,
  hydrated: false,
  subscribeModal: null,
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
}));
