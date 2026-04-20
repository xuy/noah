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

export const useConsumerStore = create<ConsumerState>((set) => ({
  entitlement: null,
  hydrated: false,
  subscribeModal: null,
  refresh: async () => {
    try {
      const ent = await commands.consumerGetEntitlement();
      set({ entitlement: ent, hydrated: true });
      return ent;
    } catch {
      set({ hydrated: true });
      return null;
    }
  },
  setEntitlement: (e) => set({ entitlement: e }),
  openSubscribeModal: (variant) => set({ subscribeModal: { variant } }),
  closeSubscribeModal: () => set({ subscribeModal: null }),
}));
