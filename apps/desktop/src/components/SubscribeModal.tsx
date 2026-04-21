import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as commands from "../lib/tauri-commands";
import { useLocale } from "../i18n";

interface SubscribeModalProps {
  /** Title copy to display — defaults to post-first-fix variant. */
  variant?: "first_fix" | "paywall" | "cap_hit";
  onDismiss: () => void;
  /** Called after a Checkout URL is opened in the browser. */
  onCheckoutOpened?: () => void;
}

export function SubscribeModal({
  variant = "first_fix",
  onDismiss,
  onCheckoutOpened,
}: SubscribeModalProps) {
  const { t } = useLocale();
  const [plan, setPlan] = useState<"annual" | "monthly">("annual");
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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md mx-4 rounded-2xl bg-bg-primary border border-border-primary shadow-xl p-6">
        <h3 className="text-lg font-semibold text-text-primary tracking-tight">{headline}</h3>
        <p className="text-sm text-text-secondary mt-2 leading-relaxed">
          {body}
        </p>

        <div className="mt-4 space-y-2">
          {(["annual", "monthly"] as const).map((p) => {
            const selected = plan === p;
            return (
              <label
                key={p}
                className="flex items-center gap-3 px-4 py-3 rounded-xl border cursor-pointer transition-colors"
                style={{
                  borderColor: selected
                    ? "var(--color-accent-green)"
                    : "var(--color-border-primary)",
                  backgroundColor: selected
                    ? "var(--color-accent-green-bg, rgba(52, 199, 89, 0.08))"
                    : "transparent",
                }}
              >
                <input
                  type="radio"
                  name="noah-plan"
                  checked={selected}
                  onChange={() => setPlan(p)}
                  className="accent-[var(--color-accent-green)]"
                />
                <div className="flex-1">
                  <div className="text-sm font-medium text-text-primary">
                    {t(`subscribe.plan.${p}.label`)}
                  </div>
                  <div className="text-[11px] text-text-muted">
                    {t(`subscribe.plan.${p}.desc`)}
                  </div>
                </div>
              </label>
            );
          })}
        </div>

        {error && <p className="text-xs text-accent-red mt-3">{error}</p>}

        <div className="mt-5 flex gap-2">
          <button
            onClick={onDismiss}
            className="px-4 py-2 rounded-xl text-sm text-text-secondary hover:text-text-primary transition-colors"
          >
            {t("subscribe.notNow")}
          </button>
          <button
            onClick={handleSubscribe}
            disabled={loading}
            className="flex-1 py-2 rounded-xl bg-accent-green text-white text-sm font-medium hover:bg-accent-green/80 transition-colors disabled:opacity-50"
          >
            {loading ? t("subscribe.opening") : t("subscribe.subscribe")}
          </button>
        </div>
      </div>
    </div>
  );
}
