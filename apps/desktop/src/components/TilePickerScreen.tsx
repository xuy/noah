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

  const goSignInWithSeed = useCallback((tile: Tile, message: string) => {
    setStage({ name: "signin", tile, seedMessage: message });
  }, []);

  const goSignInBlank = useCallback(() => {
    setStage({ name: "signin", tile: null, seedMessage: null });
  }, []);

  if (stage.name === "signin") {
    const label = stage.tile ? t(stage.tile.titleKey) : null;
    const seedContext =
      stage.seedMessage && label
        ? { label, seedMessage: stage.seedMessage }
        : null;
    return (
      <SignInScreen
        onComplete={onComplete}
        seedContext={seedContext}
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
        onContinue={(text) => {
          const message = composeSeedMessage(tile.id, t(tile.titleKey), text);
          goSignInWithSeed(tile, message);
        }}
      />
    );
  }

  return (
    <PickStage
      onPick={goClarify}
      onSignInClick={goSignInBlank}
    />
  );
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
      className="flex flex-col items-center justify-center min-h-screen px-6 py-10 relative overflow-hidden"
      style={{
        background:
          // Two layered radial gradients: brand teal from top-center
          // bleeding down, and a warmer amber glow from the bottom-right
          // that picks up the second logo color. Gives the canvas
          // atmosphere instead of flat gray.
          "radial-gradient(ellipse 80% 55% at 50% 0%, rgba(45, 212, 191, 0.14) 0%, transparent 70%), " +
          "radial-gradient(ellipse 50% 45% at 90% 100%, rgba(251, 191, 36, 0.06) 0%, transparent 65%), " +
          "var(--color-bg-primary)",
      }}
    >
      {/* Noise / subtle vignette to avoid banding on the gradient */}
      <div
        aria-hidden
        className="absolute inset-0 pointer-events-none opacity-[0.35]"
        style={{
          background:
            "radial-gradient(ellipse at 50% 45%, transparent 40%, rgba(0,0,0,0.25) 100%)",
        }}
      />

      <div className="relative w-full max-w-2xl">
        <div className="flex flex-col items-center mb-10">
          <div className="relative mb-5">
            {/* Soft brand glow behind the logo — same teal as the gradient,
                keeps Noah's mark feeling anchored rather than floating. */}
            <div
              aria-hidden
              className="absolute inset-0 rounded-2xl blur-2xl opacity-70"
              style={{ background: "rgba(45, 212, 191, 0.25)" }}
            />
            <NoahIcon
              className="relative w-20 h-20 rounded-2xl shadow-xl"
              alt="Noah"
            />
          </div>
          <h1 className="text-2xl font-semibold text-text-primary tracking-tight">
            {t("onboarding.greeting")}
          </h1>
          <p className="text-sm text-text-secondary mt-2 text-center leading-relaxed max-w-md">
            {tagline}
          </p>
          <p className="text-xs text-text-muted mt-3">
            {t("onboarding.subgreeting")}
          </p>
        </div>

        <div className="grid grid-cols-2 gap-3">
          {TILES.map((tile) => (
            <button
              key={tile.id}
              onClick={() => onPick(tile)}
              className="group relative flex items-start gap-3 text-left px-4 py-4 rounded-xl border border-white/[0.06] bg-bg-secondary/70 backdrop-blur-sm hover:border-accent-blue/40 hover:bg-bg-secondary transition-all duration-200 cursor-pointer hover:shadow-[0_8px_24px_-12px_rgba(45,212,191,0.25)]"
            >
              <span
                className="flex items-center justify-center w-11 h-11 rounded-lg shrink-0 transition-colors"
                style={{
                  background: "rgba(45, 212, 191, 0.09)",
                  color: "rgba(94, 234, 212, 0.9)",
                  border: "1px solid rgba(45, 212, 191, 0.12)",
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

        <div className="mt-10 text-center">
          <button
            onClick={onSignInClick}
            className="text-xs text-text-muted hover:text-text-secondary underline"
          >
            {t("onboarding.alreadyHaveAccount")}
          </button>
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
            className="flex items-center justify-center w-11 h-11 rounded-xl bg-accent-green/10 text-accent-green"
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
            className="flex-1 py-2 rounded-xl bg-accent-green text-white text-sm font-medium hover:bg-accent-green/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
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
