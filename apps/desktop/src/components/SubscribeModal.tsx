import { useCallback, useState } from "react";
import { Check, Sparkles } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as commands from "../lib/tauri-commands";
import { useLocale } from "../i18n";

interface SubscribeModalProps {
  /** Which trigger fired the modal — drives copy. */
  variant?: "first_fix" | "paywall" | "cap_hit";
  /** Called when user dismisses (clicks "Keep my free trial" or backdrop). */
  onDismiss: () => void;
  /** Called after a Checkout URL is opened in the browser. */
  onCheckoutOpened?: () => void;
}

type Plan = "annual" | "monthly";

/**
 * Subscribe / commitment-moment modal.
 *
 * Fires at the user's first RUN_STEP click ("Please fix it"). The fix
 * itself runs in the background regardless of how the user resolves
 * the modal — that's load-bearing, the fix-continues-note in the
 * footer says so explicitly.
 *
 * Stripe Checkout is configured with trial_period_days that matches
 * the user's remaining trial, so "Subscribe" → $0 today, card on file,
 * charge on trial-end. This is the highest-converting frame for
 * consumer subscription apps and matches what users see in the wild
 * (Calm / Headspace / CleanMyMac all use this pattern).
 */
export function SubscribeModal({
  variant = "first_fix",
  onDismiss,
  onCheckoutOpened,
}: SubscribeModalProps) {
  const { t } = useLocale();
  const [plan, setPlan] = useState<Plan>("annual");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubscribe = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const url = await commands.consumerBillingCheckoutUrl(plan);
      await openUrl(url);
      onCheckoutOpened?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [plan, onCheckoutOpened]);

  const headline =
    variant === "first_fix"
      ? t("subscribe.firstFixHeadline")
      : variant === "cap_hit"
        ? t("subscribe.capHitHeadline")
        : t("subscribe.paywallHeadline");

  const body =
    variant === "first_fix"
      ? t("subscribe.firstFixBody")
      : variant === "cap_hit"
        ? t("subscribe.capHitBody")
        : t("subscribe.paywallBody");

  // Only first_fix fires while a fix is in flight. paywall / cap_hit
  // are blocking states where the fix didn't start, so the
  // "fix continues" footnote would be a lie there.
  const showFixContinuesNote = variant === "first_fix";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onDismiss}
    >
      <div
        className="w-full max-w-[440px] mx-4 rounded-3xl bg-bg-primary border border-border-primary shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header — accent gradient strip + headline */}
        <div className="relative px-7 pt-7 pb-5">
          <div
            aria-hidden
            className="absolute inset-x-0 top-0 h-px"
            style={{
              background:
                "linear-gradient(90deg, transparent, var(--color-accent-green), transparent)",
              opacity: 0.5,
            }}
          />
          <div className="flex items-center gap-2 mb-3">
            <Sparkles
              size={14}
              strokeWidth={2}
              className="text-accent-green"
            />
            <span className="text-[11px] font-medium uppercase tracking-[0.08em] text-accent-green">
              {t("subscribe.trustBadge")}
            </span>
          </div>
          <h3 className="text-[22px] font-semibold text-text-primary leading-tight tracking-tight">
            {headline}
          </h3>
          <p className="text-[13.5px] text-text-secondary mt-2.5 leading-relaxed">
            {body}
          </p>
        </div>

        {/* Plan picker */}
        <div className="px-7 pb-2 space-y-2">
          {(["annual", "monthly"] as const).map((p) => {
            const selected = plan === p;
            const isAnnual = p === "annual";
            const savings = isAnnual ? t("subscribe.plan.annual.savingsBadge") : null;
            return (
              <label
                key={p}
                className="flex items-center gap-3 px-4 py-3.5 rounded-2xl border cursor-pointer transition-all relative"
                style={{
                  borderColor: selected
                    ? "var(--color-accent-green)"
                    : "var(--color-border-primary)",
                  borderWidth: selected ? "1.5px" : "1px",
                  backgroundColor: selected
                    ? "var(--color-accent-green-bg, rgba(52, 199, 89, 0.07))"
                    : "transparent",
                }}
              >
                {/* Custom radio with checkmark when selected */}
                <span
                  className="flex items-center justify-center w-5 h-5 rounded-full border shrink-0 transition-all"
                  style={{
                    borderColor: selected
                      ? "var(--color-accent-green)"
                      : "var(--color-border-primary)",
                    backgroundColor: selected
                      ? "var(--color-accent-green)"
                      : "transparent",
                  }}
                >
                  {selected && (
                    <Check size={12} strokeWidth={3} className="text-white" />
                  )}
                </span>
                <input
                  type="radio"
                  name="noah-plan"
                  checked={selected}
                  onChange={() => setPlan(p)}
                  className="sr-only"
                />
                <div className="flex-1 min-w-0 flex items-baseline gap-2">
                  <span className="text-sm font-semibold text-text-primary">
                    {t(`subscribe.plan.${p}.label`)}
                  </span>
                  <span className="text-sm text-text-primary">
                    {t(`subscribe.plan.${p}.price`)}
                    <span className="text-text-muted text-[12px]">
                      {t(`subscribe.plan.${p}.priceUnit`)}
                    </span>
                  </span>
                  <span className="text-[11.5px] text-text-muted ml-auto">
                    {t(`subscribe.plan.${p}.desc`)}
                  </span>
                </div>
                {savings && selected && (
                  <span
                    className="absolute -top-2 right-4 px-2 py-[1px] rounded-full text-[10px] font-semibold uppercase tracking-wider"
                    style={{
                      backgroundColor: "var(--color-accent-green)",
                      color: "var(--color-bg-primary)",
                    }}
                  >
                    {savings}
                  </span>
                )}
              </label>
            );
          })}
        </div>

        {error && (
          <p className="text-xs text-accent-red mt-2 px-7">{error}</p>
        )}

        {/* Actions */}
        <div className="px-7 pt-4 pb-3">
          <button
            onClick={handleSubscribe}
            disabled={loading}
            className="w-full py-3 rounded-2xl bg-accent-green text-white text-[14.5px] font-semibold hover:bg-accent-green/90 transition-all disabled:opacity-50 cursor-pointer shadow-sm"
            style={{
              boxShadow:
                "0 1px 0 rgba(255,255,255,0.08) inset, 0 4px 14px -4px rgba(52, 199, 89, 0.4)",
            }}
          >
            {loading ? t("subscribe.opening") : t("subscribe.subscribe")}
          </button>
          <button
            onClick={onDismiss}
            className="w-full mt-2 py-2 text-[12.5px] text-text-muted hover:text-text-secondary transition-colors cursor-pointer"
          >
            {t("subscribe.keepTrial")}
          </button>
        </div>

        {/* Footnote — explicitly tells user that dismissing doesn't
            cancel the fix. Without this the "Keep my free trial" CTA
            is ambiguous (does it pause Noah? does it stop the fix?). */}
        {showFixContinuesNote && (
          <div className="px-7 pb-5 pt-1">
            <p className="text-[11px] text-text-muted text-center leading-snug">
              {t("subscribe.fixContinuesNote")}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
