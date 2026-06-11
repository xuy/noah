import { useEffect, useRef } from "react";
import {
  scanRevealPaywallVariant,
  useConsumerStore,
  type PaywallSignals,
} from "./consumerStore";

/**
 * Orchestrator hook for the launch-arm **scan-reveal paywall** — the integration
 * point that surfaces the paywall in the onboarding flow. Mount it in the
 * onboarding component and feed it the live signals; when the scan has revealed
 * findings and a launch-arm user hasn't started a trial, it opens the
 * `scan_reveal` SubscribeModal.
 *
 * **One-shot per mount**: dismissing the paywall ("Maybe later") drops the user
 * into the after-fix path — we must NOT immediately re-pop it. The ref guards
 * that, and also guards against re-firing on every entitlement refresh.
 *
 * Design-agnostic: this is pure wiring over the tested `scanRevealPaywallVariant`
 * decision. The onboarding screens (what the scan/reveal look like) plug in
 * separately by setting `scanRevealed` once their reveal is on screen.
 */
export function useScanRevealPaywall(signals: PaywallSignals): void {
  const entitlement = useConsumerStore((s) => s.entitlement);
  const subscribeModal = useConsumerStore((s) => s.subscribeModal);
  const openSubscribeModal = useConsumerStore((s) => s.openSubscribeModal);
  const firedRef = useRef(false);

  const { scanRevealed, firstFixReached } = signals;
  useEffect(() => {
    if (firedRef.current) return; // already shown this onboarding — never re-pop
    if (subscribeModal) return; // don't stack on an already-open modal
    const variant = scanRevealPaywallVariant(entitlement, {
      scanRevealed,
      firstFixReached,
    });
    if (variant) {
      firedRef.current = true;
      openSubscribeModal(variant);
    }
  }, [entitlement, scanRevealed, firstFixReached, subscribeModal, openSubscribeModal]);
}
