import { useCallback, useMemo, useState } from "react";
import { NoahIcon } from "./NoahIcon";
import { SignInScreen } from "./SignInScreen";
import { useLocale } from "../i18n";

interface TilePickerScreenProps {
  onComplete: () => void;
}

interface Tile {
  id: string;
  emoji: string;
  titleKey: string;
  descKey: string;
  hintKey: string;
}

const TILES: readonly Tile[] = [
  { id: "slow", emoji: "\u{1F40C}", titleKey: "onboarding.tile.slow.title", descKey: "onboarding.tile.slow.desc", hintKey: "onboarding.tile.slow.hint" },
  { id: "wifi", emoji: "\u{1F4F6}", titleKey: "onboarding.tile.wifi.title", descKey: "onboarding.tile.wifi.desc", hintKey: "onboarding.tile.wifi.hint" },
  { id: "crash", emoji: "\u{1F4A5}", titleKey: "onboarding.tile.crash.title", descKey: "onboarding.tile.crash.desc", hintKey: "onboarding.tile.crash.hint" },
  { id: "connect", emoji: "\u{1F50C}", titleKey: "onboarding.tile.connect.title", descKey: "onboarding.tile.connect.desc", hintKey: "onboarding.tile.connect.hint" },
  { id: "battery", emoji: "\u{1F50B}", titleKey: "onboarding.tile.battery.title", descKey: "onboarding.tile.battery.desc", hintKey: "onboarding.tile.battery.hint" },
  { id: "update", emoji: "\u{1F504}", titleKey: "onboarding.tile.update.title", descKey: "onboarding.tile.update.desc", hintKey: "onboarding.tile.update.hint" },
  { id: "setup", emoji: "\u{1F6E0}\u{FE0F}", titleKey: "onboarding.tile.setup.title", descKey: "onboarding.tile.setup.desc", hintKey: "onboarding.tile.setup.hint" },
  { id: "other", emoji: "\u{1F4AC}", titleKey: "onboarding.tile.other.title", descKey: "onboarding.tile.other.desc", hintKey: "onboarding.tile.other.hint" },
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
    <div className="flex flex-col items-center justify-center min-h-screen bg-bg-primary px-6 py-10">
      <div className="w-full max-w-2xl">
        <div className="flex flex-col items-center mb-8">
          <NoahIcon className="w-16 h-16 rounded-2xl mb-4" alt="Noah" />
          <h1 className="text-xl font-semibold text-text-primary">
            {t("onboarding.greeting")}
          </h1>
          <p className="text-sm text-text-secondary mt-2 text-center leading-relaxed">
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
              className="flex items-start gap-3 text-left px-4 py-3 rounded-xl border border-border-primary hover:border-accent-green hover:bg-bg-hover transition-colors cursor-pointer"
            >
              <span className="text-2xl leading-none mt-0.5" aria-hidden>
                {tile.emoji}
              </span>
              <div className="min-w-0">
                <div className="text-sm font-medium text-text-primary">
                  {t(tile.titleKey)}
                </div>
                <div className="text-[11px] text-text-muted leading-relaxed mt-0.5">
                  {t(tile.descKey)}
                </div>
              </div>
            </button>
          ))}
        </div>

        <div className="mt-8 text-center">
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
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-bg-primary px-6 py-10">
      <div className="w-full max-w-xl">
        <button
          onClick={onBack}
          className="text-xs text-text-muted hover:text-text-secondary mb-6"
        >
          {"\u2190"} {t("onboarding.backLabel")}
        </button>

        <div className="flex items-center gap-3 mb-6">
          <span className="text-3xl leading-none" aria-hidden>
            {tile.emoji}
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
