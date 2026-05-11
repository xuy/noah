import { useCallback, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
  AlertTriangle,
  ArrowLeft,
  BatteryLow,
  Cable,
  Gauge,
  MessageCircle,
  RefreshCcw,
  Settings2,
  Wifi,
} from "lucide-react";
import { NoahIcon } from "./NoahIcon";
import { SignInScreen } from "./SignInScreen";
import { useLocale } from "../i18n";

interface TilePickerScreenProps {
  onComplete: () => void;
}

interface Tile {
  id: string;
  Icon: LucideIcon;
  titleKey: string;
  descKey: string;
  hintKey: string;
}

const TILES: readonly Tile[] = [
  { id: "slow", Icon: Gauge, titleKey: "onboarding.tile.slow.title", descKey: "onboarding.tile.slow.desc", hintKey: "onboarding.tile.slow.hint" },
  { id: "wifi", Icon: Wifi, titleKey: "onboarding.tile.wifi.title", descKey: "onboarding.tile.wifi.desc", hintKey: "onboarding.tile.wifi.hint" },
  { id: "crash", Icon: AlertTriangle, titleKey: "onboarding.tile.crash.title", descKey: "onboarding.tile.crash.desc", hintKey: "onboarding.tile.crash.hint" },
  { id: "connect", Icon: Cable, titleKey: "onboarding.tile.connect.title", descKey: "onboarding.tile.connect.desc", hintKey: "onboarding.tile.connect.hint" },
  { id: "battery", Icon: BatteryLow, titleKey: "onboarding.tile.battery.title", descKey: "onboarding.tile.battery.desc", hintKey: "onboarding.tile.battery.hint" },
  { id: "update", Icon: RefreshCcw, titleKey: "onboarding.tile.update.title", descKey: "onboarding.tile.update.desc", hintKey: "onboarding.tile.update.hint" },
  { id: "setup", Icon: Settings2, titleKey: "onboarding.tile.setup.title", descKey: "onboarding.tile.setup.desc", hintKey: "onboarding.tile.setup.hint" },
  { id: "other", Icon: MessageCircle, titleKey: "onboarding.tile.other.title", descKey: "onboarding.tile.other.desc", hintKey: "onboarding.tile.other.hint" },
];

type Stage =
  | { name: "pick" }
  | { name: "clarify"; tile: Tile }
  | { name: "signin"; tile: Tile | null; seedMessage: string | null };

/**
 * First-run entry for users without a session. Shows a grid of eight
 * common Mac problems ("Pick One") and, once the user picks, collects
 * a short clarifier and routes into the magic-link sign-in. The
 * seed message (category + clarifier) is persisted to localStorage so
 * it survives the browser magic-link round-trip and seeds the first
 * chat turn on return.
 */
export function TilePickerScreen({ onComplete }: TilePickerScreenProps) {
  const { t } = useLocale();
  const [stage, setStage] = useState<Stage>({ name: "pick" });
  const [clarifier, setClarifier] = useState("");

  const goClarify = useCallback((tile: Tile) => {
    setClarifier("");
    setStage({ name: "clarify", tile });
  }, []);

  const goPick = useCallback(() => {
    setStage({ name: "pick" });
  }, []);

  const goSignInBlank = useCallback(() => {
    // Restore path — explicit "Already have an account? Sign in" link.
    setStage({ name: "signin", tile: null, seedMessage: null });
  }, []);

  const finishWithSeed = useCallback(
    (tile: Tile, clarifier: string) => {
      const message = composeSeedMessage(tile.id, t(tile.titleKey), clarifier);
      // Stash the seed to localStorage — ChatPanel picks it up on its
      // first-fresh-session effect and auto-sends it as the first
      // chat turn. No sign-in required; the device's anonymous trial
      // starts when the server sees /events/issue-started.
      try {
        localStorage.setItem(
          "noah.pendingSeed",
          JSON.stringify({
            message,
            expiresAt: Date.now() + 60 * 60 * 1000,
          }),
        );
      } catch {
        // localStorage disabled — the user will type manually, fine.
      }
      onComplete();
    },
    [onComplete, t],
  );

  if (stage.name === "signin") {
    return (
      <SignInScreen
        onComplete={onComplete}
        seedContext={null}
        onBack={goPick}
      />
    );
  }

  if (stage.name === "clarify") {
    const { tile } = stage;
    return (
      <ClarifyStage
        tile={tile}
        value={clarifier}
        onChange={setClarifier}
        onBack={goPick}
        onContinue={(text) => finishWithSeed(tile, text)}
      />
    );
  }

  return <PickStage onPick={goClarify} onSignInClick={goSignInBlank} />;
}

// ── Pick stage ────────────────────────────────────────────────────────────

function PickStage({
  onPick,
  onSignInClick,
}: {
  onPick: (tile: Tile) => void;
  onSignInClick: () => void;
}) {
  const { t, tArray } = useLocale();
  const taglines = tArray("setup.taglines");
  const tagline = useMemo(
    () => taglines[Math.floor(Math.random() * taglines.length)],
    [taglines],
  );

  return (
    <div
      // Layered scroll: outer fixes the aurora wash + drag region to
      // the viewport, inner scrolls so the sign-in link is reachable
      // on shorter windows (≤ ~720px tall) instead of being silently
      // clipped below the fold. At the default window size everything
      // still fits without a scrollbar.
      className="relative h-screen overflow-hidden"
      style={{
        background:
          "radial-gradient(ellipse 80% 55% at 50% 0%, rgba(99, 102, 241, 0.16) 0%, transparent 70%), " +
          "radial-gradient(ellipse 50% 45% at 90% 100%, rgba(139, 92, 246, 0.08) 0%, transparent 65%), " +
          "var(--color-bg-primary)",
      }}
    >
      {/* Window drag region — MainTitleBar (which normally owns this)
          doesn't render on unauthenticated screens, so without this
          the window becomes unmovable on macOS overlay title bars.
          Pinned to the outer (viewport-anchored) so scrolling content
          doesn't move it out of the macOS overlay region. */}
      <div
        data-tauri-drag-region=""
        className="absolute top-0 left-0 right-0 h-9 z-20"
      />

      {/* Noise / subtle vignette to avoid banding on the gradient */}
      <div
        aria-hidden
        className="absolute inset-0 pointer-events-none opacity-[0.35]"
        style={{
          background:
            "radial-gradient(ellipse at 50% 45%, transparent 40%, rgba(0,0,0,0.25) 100%)",
        }}
      />

      {/* Scrollable content layer — flex-center so it stays centered
          when it fits, scrolls naturally when it doesn't. */}
      <div className="absolute inset-0 overflow-y-auto">
        <div className="min-h-full flex flex-col items-center justify-center px-6 py-8">
          <div className="relative w-full max-w-[660px]">
            <div className="flex flex-col items-center mb-8">
              <div className="relative mb-4">
                {/* Aurora-tinted glow behind the logo — same indigo as the
                    page wash, keeps Noah's mark anchored to the launch
                    identity rather than floating in legacy teal. */}
                <div
                  aria-hidden
                  className="absolute inset-0 rounded-2xl blur-2xl opacity-70"
                  style={{ background: "rgba(99, 102, 241, 0.32)" }}
                />
                <NoahIcon
                  className="relative w-20 h-20 rounded-2xl shadow-xl"
                  alt="Noah"
                />
              </div>
              <span className="eyebrow mb-3">{t("onboarding.eyebrow")}</span>
              <h1 className="text-2xl font-semibold text-text-primary tracking-tight">
                {t("onboarding.greeting")}
              </h1>
              <p className="text-sm text-text-muted mt-2 text-center leading-relaxed max-w-md">
                {t("onboarding.subgreeting")}
              </p>
              <p className="text-xs text-text-muted mt-3">{tagline}</p>
            </div>

            <div className="grid grid-cols-2 gap-2">
              {TILES.map((tile) => (
                <button
                  key={tile.id}
                  onClick={() => onPick(tile)}
                  className="card-soft interactive aurora-focus group relative flex items-start gap-3 text-left px-4 py-4 cursor-pointer transition-all duration-200"
                >
                  <span
                    className="flex items-center justify-center w-11 h-11 rounded-lg shrink-0 transition-colors"
                    style={{
                      background: "var(--color-accent-blue-soft)",
                      color: "var(--color-accent-indigo)",
                      border: "1px solid var(--color-accent-border)",
                    }}
                    aria-hidden
                  >
                    <tile.Icon size={19} strokeWidth={1.75} />
                  </span>
                  <div className="min-w-0 flex-1 pt-0.5">
                    <div className="text-sm font-medium text-text-primary leading-snug">
                      {t(tile.titleKey)}
                    </div>
                    <div className="text-[11.5px] text-text-muted leading-relaxed mt-1">
                      {t(tile.descKey)}
                    </div>
                  </div>
                </button>
              ))}
            </div>

            <div className="mt-6 text-center">
              <button
                onClick={onSignInClick}
                className="text-xs text-text-muted hover:text-text-secondary underline cursor-pointer"
              >
                {t("onboarding.alreadyHaveAccount")}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Clarify stage ─────────────────────────────────────────────────────────

function ClarifyStage({
  tile,
  value,
  onChange,
  onBack,
  onContinue,
}: {
  tile: Tile;
  value: string;
  onChange: (v: string) => void;
  onBack: () => void;
  onContinue: (text: string) => void;
}) {
  const { t } = useLocale();
  const canContinue = value.trim().length > 0;
  const { Icon } = tile;
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-bg-primary px-6 py-10">
      <div className="w-full max-w-xl">
        <button
          onClick={onBack}
          className="inline-flex items-center gap-1.5 text-xs text-text-muted hover:text-text-secondary mb-6"
        >
          <ArrowLeft size={13} strokeWidth={2} />
          {t("onboarding.backLabel")}
        </button>

        <div className="flex items-center gap-3 mb-6">
          <span
            className="flex items-center justify-center w-11 h-11 rounded-xl"
            style={{
              background: "var(--color-accent-blue-soft)",
              color: "var(--color-accent-indigo)",
              border: "1px solid var(--color-accent-border)",
            }}
            aria-hidden
          >
            <Icon size={22} strokeWidth={1.75} />
          </span>
          <h2 className="text-lg font-semibold text-text-primary">
            {t(tile.titleKey)}
          </h2>
        </div>

        <textarea
          autoFocus
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && canContinue) {
              e.preventDefault();
              onContinue(value.trim());
            }
          }}
          placeholder={t(tile.hintKey)}
          rows={4}
          className="w-full px-4 py-3 rounded-xl bg-bg-input border border-border-primary text-base text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors resize-none"
        />

        <div className="mt-4 flex gap-2">
          <button
            onClick={onBack}
            className="px-4 py-2 rounded-xl text-sm text-text-secondary hover:text-text-primary transition-colors"
          >
            {t("onboarding.backLabel")}
          </button>
          <button
            onClick={() => onContinue(value.trim())}
            disabled={!canContinue}
            className="btn-launch flex-1 py-2 rounded-xl text-sm font-medium cursor-pointer disabled:cursor-not-allowed"
          >
            {t("onboarding.continue")}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Seed message composition ──────────────────────────────────────────────

/**
 * Combine the tile category and the user's clarifier into a single
 * message that reads naturally as the first chat turn. For the "other"
 * tile we let the user's text stand on its own.
 */
function composeSeedMessage(
  tileId: string,
  categoryTitle: string,
  clarifier: string,
): string {
  const trimmed = clarifier.trim();
  if (!trimmed) return categoryTitle;
  if (tileId === "other") return trimmed;
  return `${categoryTitle}. ${trimmed}`;
}
