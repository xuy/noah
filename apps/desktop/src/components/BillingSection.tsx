import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as commands from "../lib/tauri-commands";
import { useConsumerStore } from "../stores/consumerStore";
import { useLocale } from "../i18n";

function formatDate(ms: number | null | undefined, locale: string): string {
  if (!ms) return "—";
  try {
    return new Date(ms).toLocaleDateString(locale);
  } catch {
    return new Date(ms).toISOString().slice(0, 10);
  }
}

export function BillingSection() {
  const { t, locale } = useLocale();
  const entitlement = useConsumerStore((s) => s.entitlement);
  const refresh = useConsumerStore((s) => s.refresh);
  const [opening, setOpening] = useState<"" | "portal" | "upgrade" | "signout">(
    "",
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleManage = useCallback(async () => {
    setOpening("portal");
    setError(null);
    try {
      const url = await commands.consumerBillingPortalUrl();
      await openUrl(url);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setOpening("");
    }
  }, []);

  const handleUpgrade = useCallback(async () => {
    setOpening("upgrade");
    setError(null);
    try {
      const url = await commands.consumerBillingCheckoutUrl("annual");
      await openUrl(url);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setOpening("");
    }
  }, []);

  const handleSignOut = useCallback(async () => {
    setOpening("signout");
    try {
      await commands.consumerSignOut();
      // Reloading the window is the simplest way to re-trigger the App.tsx gate.
      window.location.reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setOpening("");
    }
  }, []);

  if (!entitlement) return null;

  const statusLabel = ((): string => {
    switch (entitlement.status) {
      case "none":
        return t("billing.statusNone");
      case "trialing":
        return t("billing.statusTrialing");
      case "active":
        return t("billing.statusActive");
      case "past_due":
        return t("billing.statusPastDue");
      case "canceled":
        return t("billing.statusCanceled");
      case "expired":
        return t("billing.statusExpired");
    }
  })();

  const endLine =
    entitlement.status === "trialing" && entitlement.trial_ends_at
      ? t("billing.trialEndsAt", {
          date: formatDate(entitlement.trial_ends_at, locale),
        })
      : entitlement.status === "active" && entitlement.period_end
        ? t("billing.periodEndsAt", {
            date: formatDate(entitlement.period_end, locale),
          })
        : "";

  return (
    <section className="rounded-2xl border border-border-primary bg-bg-secondary p-5">
      <h2 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-3">
        {t("billing.sectionTitle")}
      </h2>
      <div className="space-y-2 text-sm">
        <div className="flex justify-between">
          <span className="text-text-muted">{t("billing.status")}</span>
          <span className="text-text-primary font-medium">{statusLabel}</span>
        </div>
        {entitlement.plan && (
          <div className="flex justify-between">
            <span className="text-text-muted">{t("billing.plan")}</span>
            <span className="text-text-primary">{entitlement.plan}</span>
          </div>
        )}
        {endLine && <p className="text-xs text-text-muted">{endLine}</p>}
        {(entitlement.status === "active" ||
          entitlement.status === "trialing") && (
          <p className="text-xs text-text-muted">
            {t("billing.usage", {
              used: entitlement.usage_used,
              limit: entitlement.usage_limit,
            })}
          </p>
        )}
      </div>

      {error && <p className="text-xs text-accent-red mt-3">{error}</p>}

      <div className="mt-4 flex flex-wrap gap-2">
        {entitlement.status === "active" ? (
          <button
            onClick={handleManage}
            disabled={opening === "portal"}
            className="px-3 py-1.5 rounded-lg bg-bg-tertiary text-xs text-text-primary hover:bg-bg-hover transition-colors disabled:opacity-50"
          >
            {opening === "portal"
              ? t("subscribe.opening")
              : t("billing.manage")}
          </button>
        ) : (
          <button
            onClick={handleUpgrade}
            disabled={opening === "upgrade"}
            className="px-3 py-1.5 rounded-lg bg-accent-green text-xs text-white font-medium hover:bg-accent-green/80 transition-colors disabled:opacity-50"
          >
            {opening === "upgrade"
              ? t("subscribe.opening")
              : t("billing.upgrade")}
          </button>
        )}
        <button
          onClick={handleSignOut}
          disabled={opening === "signout"}
          className="px-3 py-1.5 rounded-lg text-xs text-text-muted hover:text-text-primary transition-colors disabled:opacity-50"
        >
          {t("billing.signOut")}
        </button>
      </div>
    </section>
  );
}
