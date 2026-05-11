import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as commands from "../lib/tauri-commands";
import { useConsumerStore } from "../stores/consumerStore";
import { useLocale } from "../i18n";

/**
 * Friendly billing-context date: "Mon, May 11" if same calendar year,
 * "Mon, May 11, 2027" if it's a future year. Day-of-week makes it
 * parseable at a glance, year is suppressed unless ambiguous.
 */
function friendlyDate(ms: number | null | undefined, locale: string): string {
  if (!ms) return "—";
  const d = new Date(ms);
  const sameYear = d.getFullYear() === new Date().getFullYear();
  try {
    return d.toLocaleDateString(locale, {
      weekday: "short",
      month: "short",
      day: "numeric",
      ...(sameYear ? {} : { year: "numeric" }),
    });
  } catch {
    return d.toISOString().slice(0, 10);
  }
}

const PRICE_BY_PLAN: Record<string, string> = {
  monthly: "$4.99/month",
  annual: "$50/year",
};

const PLAN_LABEL: Record<string, string> = {
  monthly: "Monthly plan",
  annual: "Annual plan",
};

/** Status dot + label — calm "we're here, everything's fine" signal. */
function StatusPill({
  color,
  label,
}: {
  color: "green" | "blue" | "amber" | "muted";
  label: string;
}) {
  // Map semantic intent → CSS var. Soft halo via box-shadow keeps the
  // dot from looking pasted-on; it sits in a translucent same-color
  // glow that softens against any card background.
  const colorVar = {
    green: "var(--color-accent-green)",
    blue: "var(--color-accent-blue)",
    amber: "var(--color-accent-amber)",
    muted: "var(--color-text-muted)",
  }[color];
  return (
    <span className="inline-flex items-center gap-2 whitespace-nowrap">
      <span
        className="inline-block w-[7px] h-[7px] rounded-full"
        style={{
          background: colorVar,
          boxShadow: `0 0 0 3px ${colorVar}22`,
        }}
      />
      <span className="text-[13px] font-semibold text-text-primary">
        {label}
      </span>
    </span>
  );
}

/** Small typographic eyebrow with the aurora hairline. Repeats across
    Settings cards so they share a visual rhythm. */
function SectionEyebrow({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-[7px] text-[10.5px] font-bold uppercase tracking-[0.14em] text-text-muted whitespace-nowrap">
      <span
        className="block w-3 h-[2px] rounded-[1px]"
        style={{ background: "var(--aurora)" }}
      />
      {children}
    </span>
  );
}

export function BillingSection() {
  const { t, locale } = useLocale();
  const entitlement = useConsumerStore((s) => s.entitlement);
  const refresh = useConsumerStore((s) => s.refresh);
  const [opening, setOpening] = useState<"" | "portal" | "upgrade" | "signout">(
    "",
  );
  const [error, setError] = useState<string | null>(null);

  // ── "Already a subscriber? Sign in" sub-flow ─────────────────────
  // Reinstall recovery: device forgets the user paid, lands them in
  // trial. Hidden behind a small text link so the primary subscribe
  // path stays the loudest action.
  const [signInOpen, setSignInOpen] = useState(false);
  const [signInEmail, setSignInEmail] = useState("");
  const [signInSubmitting, setSignInSubmitting] = useState(false);
  const [signInError, setSignInError] = useState<string | null>(null);
  const [linkSentTo, setLinkSentTo] = useState<string | null>(null);

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
      window.location.reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setOpening("");
    }
  }, []);

  const handleRequestMagicLink = useCallback(async () => {
    const trimmed = signInEmail.trim();
    if (!trimmed) return;
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(trimmed)) {
      setSignInError(t("billing.signInInvalidEmail"));
      return;
    }
    setSignInSubmitting(true);
    setSignInError(null);
    try {
      await commands.consumerRequestMagicLink(trimmed);
      setLinkSentTo(trimmed);
      setSignInOpen(false);
    } catch (err) {
      setSignInError(err instanceof Error ? err.message : String(err));
    } finally {
      setSignInSubmitting(false);
    }
  }, [signInEmail, t]);

  if (!entitlement) return null;

  // Status dot color/label — picks the right semantic for each state.
  const statusDot = ((): { color: "green" | "blue" | "amber" | "muted"; label: string } => {
    switch (entitlement.status) {
      case "active":
        return { color: "green", label: t("billing.statusActive") };
      case "trialing":
        return { color: "blue", label: t("billing.statusTrialing") };
      case "past_due":
        return { color: "amber", label: t("billing.statusPastDue") };
      case "canceled":
        return { color: "muted", label: t("billing.statusCanceled") };
      case "expired":
        return { color: "muted", label: t("billing.statusExpired") };
      case "none":
      default:
        return { color: "muted", label: t("billing.statusNone") };
    }
  })();

  // Heuristic: still in the first billing period (Stripe trial) after
  // checkout? Within 2 days = yes; past that = real renewal cycle.
  const isInPaidTrial = (() => {
    if (entitlement.status !== "active") return false;
    if (!entitlement.period_end || !entitlement.trial_ends_at) return false;
    const diffDays =
      Math.abs(entitlement.period_end - entitlement.trial_ends_at) /
      (1000 * 60 * 60 * 24);
    return diffDays < 2;
  })();

  const planKey = entitlement.plan ?? "";
  const planLabel = PLAN_LABEL[planKey] ?? "";
  const planPrice = PRICE_BY_PLAN[planKey] ?? "";

  // Page-hero sentence — the one human line that explains where you
  // are in the billing arc.
  const billingLine = ((): string => {
    if (entitlement.status === "trialing" && entitlement.trial_ends_at) {
      return t("billing.lineTrial", {
        date: friendlyDate(entitlement.trial_ends_at, locale),
      });
    }
    if (entitlement.status === "active") {
      if (isInPaidTrial && entitlement.period_end && planPrice) {
        return t("billing.linePaidTrial", {
          date: friendlyDate(entitlement.period_end, locale),
          price: planPrice,
        });
      }
      if (entitlement.period_end) {
        return t("billing.lineActive", {
          date: friendlyDate(entitlement.period_end, locale),
        });
      }
    }
    if (entitlement.status === "canceled" && entitlement.period_end) {
      return t("billing.lineCanceled", {
        date: friendlyDate(entitlement.period_end, locale),
      });
    }
    if (entitlement.status === "past_due") {
      return t("billing.linePastDue");
    }
    return "";
  })();

  const isActive = entitlement.status === "active";
  // Manage-vs-Upgrade split: anything where the user has a live Stripe
  // customer (active, canceled-with-period-left, past_due) goes through
  // the portal. Anything before payment (none, trialing) gets the
  // Upgrade CTA. Keeps the aurora moment aimed at the right action.
  const showsManageCta =
    entitlement.status === "active" ||
    entitlement.status === "past_due" ||
    entitlement.status === "canceled";
  const ctaHint = ((): string => {
    if (entitlement.status === "active") return t("billing.manageHint");
    if (entitlement.status === "past_due") return t("billing.pastDueHint");
    if (entitlement.status === "canceled") return t("billing.canceledHint");
    return t("billing.upgradeHint");
  })();
  // Reinstall-recovery affordance: only when *not* active. Active users
  // wouldn't need it and showing it would confuse them.
  const showSignInAffordance = !isActive;

  return (
    <section
      className="rounded-2xl bg-bg-secondary overflow-hidden"
      style={{
        border: "1px solid var(--color-surface-card-border)",
        boxShadow: "var(--shadow-card)",
      }}
    >
      {/* ── Top content area ──────────────────────────────────────── */}
      <div className="px-5 pt-5 pb-4">
        <div className="flex items-center justify-between mb-3.5">
          <SectionEyebrow>{t("billing.sectionTitle")}</SectionEyebrow>
          <StatusPill color={statusDot.color} label={statusDot.label} />
        </div>

        {/* Plan headline — large name + small price, tabular nums so
            the $ aligns nicely if the user opens this twice in a row. */}
        {planLabel ? (
          <div className="flex items-baseline gap-2.5 mb-0.5">
            <div className="text-[22px] font-bold text-text-primary tracking-[-0.022em]">
              {planLabel}
            </div>
            {planPrice && (
              <div
                className="text-[13.5px] font-medium text-text-muted"
                style={{ fontVariantNumeric: "tabular-nums" }}
              >
                {planPrice}
              </div>
            )}
          </div>
        ) : (
          // Trial / no-plan state: still give the card a typographic
          // anchor so it doesn't read as empty. Reuses the same
          // hierarchy slot the plan name would occupy.
          <div className="text-[22px] font-bold text-text-primary tracking-[-0.022em] mb-0.5">
            {entitlement.status === "trialing"
              ? t("billing.trialHeadline")
              : t("billing.noPlanHeadline")}
          </div>
        )}

        {billingLine && (
          <p className="text-[13px] text-text-secondary leading-[1.55] max-w-[480px] mt-0.5">
            {billingLine}
          </p>
        )}

        {error && (
          <p className="text-xs text-accent-red mt-3">{error}</p>
        )}
      </div>

      {/* ── Action strip — single aurora commit moment ────────────── */}
      <div
        className="flex items-center justify-between gap-3 px-5 py-3"
        style={{
          borderTop: "1px solid var(--color-surface-card-border)",
          background: "var(--aurora-soft)",
        }}
      >
        <span className="text-[11.5px] text-text-muted whitespace-nowrap overflow-hidden text-ellipsis">
          {ctaHint}
        </span>
        <div className="flex items-center gap-1.5">
          <button
            onClick={handleSignOut}
            disabled={opening === "signout"}
            className="px-2.5 py-1.5 rounded-lg text-[12px] font-medium text-text-muted hover:text-text-secondary transition-colors disabled:opacity-50 cursor-pointer whitespace-nowrap"
          >
            {t("billing.signOut")}
          </button>
          {showsManageCta ? (
            <button
              onClick={handleManage}
              disabled={opening === "portal"}
              className="btn-commit px-3.5 py-2 rounded-[10px] text-[12.5px] font-semibold disabled:opacity-50 cursor-pointer whitespace-nowrap"
            >
              {opening === "portal"
                ? t("subscribe.opening")
                : t("billing.manage")}
            </button>
          ) : (
            <button
              onClick={handleUpgrade}
              disabled={opening === "upgrade"}
              className="btn-commit px-3.5 py-2 rounded-[10px] text-[12.5px] font-semibold disabled:opacity-50 cursor-pointer whitespace-nowrap"
            >
              {opening === "upgrade"
                ? t("subscribe.opening")
                : t("billing.upgrade")}
            </button>
          )}
        </div>
      </div>

      {/* ── Reinstall sign-in recovery (collapsed by default) ─────── */}
      {showSignInAffordance && (
        <div
          className="px-5 py-4"
          style={{ borderTop: "1px solid var(--color-surface-card-border)" }}
        >
          {linkSentTo ? (
            <div className="space-y-2">
              <p className="text-[13px] text-text-primary">
                {t("billing.signInLinkSent", { email: linkSentTo })}
              </p>
              <p className="text-[12.5px] text-text-muted leading-[1.55]">
                {t("billing.signInLinkSentHint")}
              </p>
              <button
                onClick={() => {
                  setLinkSentTo(null);
                  setSignInEmail("");
                  setSignInOpen(true);
                }}
                className="text-[12.5px] text-accent-blue hover:underline cursor-pointer"
              >
                {t("billing.signInUseDifferentEmail")}
              </button>
            </div>
          ) : signInOpen ? (
            <div className="space-y-2">
              <p className="text-[13px] font-medium text-text-primary">
                {t("billing.signInPrompt")}
              </p>
              <p className="text-[12.5px] text-text-muted leading-[1.55]">
                {t("billing.signInHelp")}
              </p>
              <div className="flex items-stretch gap-2 mt-2">
                <input
                  type="email"
                  value={signInEmail}
                  onChange={(e) => {
                    setSignInEmail(e.target.value);
                    setSignInError(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleRequestMagicLink();
                  }}
                  placeholder={t("billing.signInEmailPlaceholder")}
                  disabled={signInSubmitting}
                  autoFocus
                  className="flex-1 min-w-0 px-3 py-2 rounded-xl bg-bg-input border border-border-primary text-[13px] text-text-primary placeholder:text-text-muted aurora-focus disabled:opacity-50"
                />
                <button
                  onClick={handleRequestMagicLink}
                  disabled={signInSubmitting || !signInEmail.trim()}
                  className="btn-action px-3 py-2 rounded-xl text-[12.5px] font-semibold disabled:opacity-50 whitespace-nowrap cursor-pointer"
                >
                  {signInSubmitting
                    ? t("billing.signInSending")
                    : t("billing.signInSendLink")}
                </button>
              </div>
              {signInError && (
                <p className="text-[11.5px] text-accent-red">{signInError}</p>
              )}
              <button
                onClick={() => {
                  setSignInOpen(false);
                  setSignInError(null);
                }}
                className="text-[12px] text-text-muted hover:text-text-secondary mt-1 cursor-pointer"
              >
                {t("billing.signInCancel")}
              </button>
            </div>
          ) : (
            <button
              onClick={() => setSignInOpen(true)}
              className="text-[13px] text-accent-blue hover:underline cursor-pointer"
            >
              {t("billing.signInLink")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
