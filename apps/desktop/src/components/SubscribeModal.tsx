import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as commands from "../lib/tauri-commands";
import { useLocale } from "../i18n";
import { useConsumerStore } from "../stores/consumerStore";
import { formatTrialEndDate } from "../lib/trial-format";

interface SubscribeModalProps {
  /** Which trigger fired the modal — drives copy. `scan_reveal` is the
   *  launch-arm paywall shown right after the onboarding scan reveals
   *  findings; it reuses the proof-oriented first_fix copy ("here's what we
   *  found — start your free trial to fix it"). */
  variant?: "first_fix" | "second_issue" | "paywall" | "cap_hit" | "scan_reveal";
  /** Called when user dismisses (clicks "Keep my free trial" or backdrop). */
  onDismiss: () => void;
  /** Called after a Checkout URL is opened in the browser. */
  onCheckoutOpened?: () => void;
}

type Plan = "annual" | "monthly";

/**
 * Subscribe modal. Three customer-visible surfaces:
 *
 *  - second_issue: "you're on a trial, plan options". Customer never
 *    sees countdown timers or hour counts — only a date.
 *  - paywall: trial ended.
 *  - cap_hit: hidden 10-issue cap tripped during trial. Headline
 *    "you've hit your trial quota" — no specifics on what the cap was.
 *
 * The legacy first_fix variant is preserved for back-compat but no
 * longer fires from any trigger; the first issue runs uninterrupted.
 */
export function SubscribeModal({
  variant = "second_issue",
  onDismiss,
  onCheckoutOpened,
}: SubscribeModalProps) {
  const { t } = useLocale();
  const ent = useConsumerStore((s) => s.entitlement);
  // Truthy only when the user has clicked Subscribe within the last 15
  // minutes and the post-checkout poll loop hasn't yet seen status
  // flip to "active". Drives the quiet reassurance footnote — we
  // *don't* surface this as a progress bar or "verifying…" spinner
  // because that broadcasts internal state and raises anxiety. The
  // line only shows for users who'd be wondering "did my payment
  // register?", and stays silent for everyone else.
  const isPostCheckoutPolling = useConsumerStore(
    (s) => s.postCheckoutPollUntil != null,
  );
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

  // i18n strings carry a {device} placeholder ("Keep Noah on your {device}")
  // so the same key works on Mac and Windows builds. Substitute here based
  // on the current platform — `navigator.userAgent` is reliable inside a
  // Tauri webview because Tauri preserves the host platform's UA string.
  const device = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent || "")
    ? "PC"
    : "Mac";
  const fillDevice = (s: string) => s.replace(/\{device\}/g, device);
  const headline = fillDevice(
    variant === "first_fix" || variant === "scan_reveal"
      ? t("subscribe.firstFixHeadline")
      : variant === "second_issue"
        ? t("subscribe.secondIssueHeadline")
        : variant === "cap_hit"
          ? t("subscribe.capHitHeadline")
          : t("subscribe.paywallHeadline")
  );

  const dateLabel = formatTrialEndDate(
    ent?.trial_ends_at ?? null,
    ent?.tz_offset_minutes ?? null,
  );

  const body =
    variant === "first_fix" || variant === "scan_reveal"
      ? t("subscribe.firstFixBody")
      : variant === "second_issue"
        ? t("subscribe.secondIssueBody").replace("{date}", dateLabel)
        : variant === "cap_hit"
          ? t("subscribe.capHitBody")
          : t("subscribe.paywallBody");

  // Only first_fix fires while a fix is in flight. paywall / cap_hit
  // are blocking states where nothing's running, so the "fix continues"
  // footnote would be a lie there. second_issue (the new default) also
  // doesn't apply because no fix is currently in flight at session-start.
  const showFixContinuesNote = variant === "first_fix";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onDismiss}
    >
      <div
        className="w-full max-w-[470px] mx-4 rounded-3xl bg-bg-primary border border-border-primary shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header — value-led. Trust pill at the top frames the moment
            ("trial · no card"); headline + body follow. The detailed
            footnote still appears below the CTA for reassurance. */}
        <div className="px-7 pt-7 pb-5">
          <span
            className="inline-flex items-center px-[13px] py-[6px] rounded-full text-[11px] font-semibold uppercase tracking-[0.08em] whitespace-nowrap mb-4"
            style={{
              background: "var(--aurora-soft)",
              border: "1px solid rgba(99, 102, 241, 0.28)",
              color: "var(--color-accent-indigo)",
            }}
          >
            {t("subscribe.trustPill")}
          </span>
          <h3 className="text-[24px] font-semibold text-text-primary leading-[1.15] tracking-tight">
            {headline}
          </h3>
          <p className="text-[14.5px] text-text-secondary mt-3 leading-relaxed">
            {body}
          </p>
        </div>

        {/* Plan picker — segmented control. One tap to compare prices.
            Annual segment carries a "SAVE X%" badge anchored to its top-right
            corner (anchored to the segment, not absolute on the modal). */}
        <div className="px-7 pb-2">
          <div
            role="radiogroup"
            aria-label={t("subscribe.planAriaLabel")}
            className="relative inline-flex w-full p-1 rounded-2xl"
            style={{
              background: "var(--color-bg-secondary)",
              border: "1px solid var(--color-surface-card-border)",
            }}
          >
            {(["annual", "monthly"] as const).map((p) => {
              const selected = plan === p;
              const isAnnual = p === "annual";
              const savings = isAnnual ? t("subscribe.plan.annual.savingsBadge") : null;
              return (
                <button
                  key={p}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  aria-label={t(`subscribe.plan.${p}.label`)}
                  onClick={() => setPlan(p)}
                  className="relative flex-1 px-4 py-3 rounded-xl cursor-pointer transition-all flex flex-col items-center"
                  style={{
                    background: selected ? "var(--aurora)" : "transparent",
                    color: selected ? "white" : "var(--color-text-secondary)",
                    boxShadow: selected
                      ? "0 6px 16px -4px rgba(99, 102, 241, 0.5)"
                      : "none",
                  }}
                >
                  <span
                    className="text-[11px] font-semibold uppercase tracking-[0.08em]"
                    style={{ opacity: selected ? 0.85 : 0.7 }}
                  >
                    {t(`subscribe.plan.${p}.label`)}
                  </span>
                  <span className="mt-1 text-[15px] font-semibold leading-tight">
                    {t(`subscribe.plan.${p}.price`)}
                    <span className="text-[12px] font-medium opacity-80">
                      {t(`subscribe.plan.${p}.priceUnit`)}
                    </span>
                  </span>
                  {savings && (
                    <span
                      className="absolute -top-2 right-3 px-2 py-[1px] rounded-full text-[10px] font-semibold uppercase tracking-wider whitespace-nowrap transition-all"
                      style={{
                        background: selected
                          ? "var(--color-accent-green)"
                          : "var(--color-bg-tertiary)",
                        color: selected ? "white" : "var(--color-text-muted)",
                        boxShadow: selected
                          ? "0 4px 10px -2px rgba(20, 184, 166, 0.5)"
                          : "none",
                      }}
                    >
                      {savings}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {error && (
          <p className="text-xs text-accent-red mt-2 px-7">{error}</p>
        )}

        {/* Actions */}
        <div className="px-7 pt-5 pb-3">
          <button
            onClick={handleSubscribe}
            disabled={loading}
            className="w-full py-3.5 rounded-2xl text-[15.5px] font-semibold cursor-pointer disabled:opacity-50 btn-commit"
          >
            {loading ? t("subscribe.opening") : t("subscribe.subscribe")}
          </button>
          {/* Trust footnote — lives BELOW the CTA, not above it. The
              user has already seen the headline and the price; this
              is just reassurance, not the lead.
              When the user has just opened checkout (poll loop active),
              the line quietly swaps to a "you might already be paid"
              reassurance so a returning-from-browser user doesn't feel
              like the app forgot they paid. */}
          <p className="mt-2.5 text-[11.5px] text-text-muted text-center">
            {isPostCheckoutPolling
              ? t("subscribe.alreadyPaidNote")
              : t("subscribe.footnote")}
          </p>

          <button
            onClick={onDismiss}
            className="w-full mt-2 py-2 text-[12.5px] text-text-muted hover:text-text-secondary transition-colors cursor-pointer"
          >
            {t("subscribe.keepTrial")}
          </button>
        </div>

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

