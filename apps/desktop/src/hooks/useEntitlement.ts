import { useCallback, useEffect, useState } from "react";
import * as commands from "../lib/tauri-commands";
import type { Entitlement } from "../lib/tauri-commands";

export interface EntitlementState {
  entitlement: Entitlement | null;
  loading: boolean;
  refresh: () => Promise<void>;
  trialDaysLeft: number | null;
  usageRemaining: number | null;
  isPaywalled: boolean;
  isTrialing: boolean;
  isActive: boolean;
}

function deriveFields(ent: Entitlement | null) {
  if (!ent) {
    return {
      trialDaysLeft: null,
      usageRemaining: null,
      isPaywalled: false,
      isTrialing: false,
      isActive: false,
    };
  }
  const now = Date.now();
  const trialDaysLeft =
    ent.trial_ends_at != null
      ? Math.max(0, Math.ceil((ent.trial_ends_at - now) / (24 * 60 * 60 * 1000)))
      : null;
  const usageRemaining = Math.max(0, ent.usage_limit - ent.usage_used);
  const isActive = ent.status === "active";
  const isTrialing = ent.status === "trialing";
  const isPaywalled =
    ent.status === "expired" ||
    ent.status === "canceled" ||
    ent.status === "past_due" ||
    (ent.status === "active" && usageRemaining <= 0);
  return { trialDaysLeft, usageRemaining, isPaywalled, isTrialing, isActive };
}

/**
 * Hook for the current entitlement. Fetches on mount and whenever `refresh()`
 * is called. Listens for an `entitlement-changed` Tauri event so other parts
 * of the app (e.g. issue-started, fix-completed) can push updates.
 */
export function useEntitlement(): EntitlementState {
  const [entitlement, setEntitlement] = useState<Entitlement | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const ent = await commands.consumerGetEntitlement();
      setEntitlement(ent);
    } catch {
      setEntitlement(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return {
    entitlement,
    loading,
    refresh,
    ...deriveFields(entitlement),
  };
}
