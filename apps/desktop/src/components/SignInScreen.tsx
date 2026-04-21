import { useCallback, useState } from "react";
import { ArrowLeft, SlidersHorizontal } from "lucide-react";
import * as commands from "../lib/tauri-commands";
import { useLocale } from "../i18n";

interface SignInScreenProps {
  onComplete: () => void;
  /**
   * Optional seed context — set when the user reached this screen via
   * the TilePicker. `label` shows as a context banner; `seedMessage`
   * is stashed in localStorage so the first chat turn auto-sends
   * after the magic-link round-trip completes.
   */
  seedContext?: { label: string; seedMessage: string } | null;
  /** Optional back button (shown when launched from the tile picker). */
  onBack?: () => void;
}

/** Storage key read by ChatPanel on a fresh session post-sign-in. */
const PENDING_SEED_KEY = "noah.pendingSeed";
const PENDING_SEED_TTL_MS = 60 * 60 * 1000;

function stashPendingSeed(seedMessage: string) {
  try {
    localStorage.setItem(
      PENDING_SEED_KEY,
      JSON.stringify({
        message: seedMessage,
        expiresAt: Date.now() + PENDING_SEED_TTL_MS,
      }),
    );
  } catch {
    // localStorage disabled — not worth blocking sign-in over.
  }
}

type Stage = "email" | "sent" | "exchanging";

export function SignInScreen({
  onComplete,
  seedContext = null,
  onBack,
}: SignInScreenProps) {
  const { t } = useLocale();
  const [stage, setStage] = useState<Stage>("email");
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [byokKey, setByokKey] = useState("");
  const [byokSaving, setByokSaving] = useState(false);
  // seedContext is still used for the localStorage stash on submit,
  // we deliberately don't render it as a banner — the user is one
  // second removed from picking the tile, a big reminder is noise.
  void seedContext;

  const handleSaveByok = useCallback(async () => {
    setError(null);
    const trimmed = byokKey.trim();
    if (!trimmed) {
      setError(t("setup.errorApiKeyEmpty"));
      return;
    }
    if (!trimmed.startsWith("sk-ant-")) {
      setError(t("setup.errorApiKeyInvalid"));
      return;
    }
    setByokSaving(true);
    try {
      await commands.setApiKey(trimmed);
      onComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setByokSaving(false);
    }
  }, [byokKey, onComplete, t]);

  // Deep-link handling moved to App.tsx's root-level listener so it
  // fires regardless of which screen is mounted (previously this
  // listener was inactive when the user was on the tile picker).

  const handleSend = useCallback(async () => {
    setError(null);
    const trimmed = email.trim();
    if (!trimmed || !trimmed.includes("@")) {
      setError(t("signIn.errorInvalidEmail"));
      return;
    }
    setSubmitting(true);
    try {
      // Stash the seed BEFORE the server call so it rides either path
      // (instant-sign-in OR fallback "check inbox" if the server ever
      // gates on email click again).
      if (seedContext) stashPendingSeed(seedContext.seedMessage);
      const ent = await commands.consumerRequestMagicLink(trimmed);
      if (ent) {
        // Happy path — server issued a session immediately, we're in.
        onComplete();
        return;
      }
      // Legacy / fallback: server didn't auto-sign-in, show the
      // "check your inbox" screen so the user knows to follow the
      // email link.
      setStage("sent");
    } catch (err) {
      setError(
        `${t("signIn.errorSendFailed")}: ${
          err instanceof Error ? err.message : String(err)
        }`,
      );
    } finally {
      setSubmitting(false);
    }
  }, [email, seedContext, t]);

  return (
    <div className="flex flex-col items-center justify-center h-screen bg-bg-primary px-6 relative">
      {/* Window drag region — see note in TilePickerScreen. */}
      <div
        data-tauri-drag-region=""
        className="absolute top-0 left-0 right-0 h-9 z-10"
      />

      {onBack && !showAdvanced && (
        <button
          onClick={onBack}
          className="absolute top-12 left-6 inline-flex items-center gap-1.5 text-xs text-text-muted hover:text-text-secondary z-20"
        >
          <ArrowLeft size={13} strokeWidth={2} />
          {t("onboarding.backLabel")}
        </button>
      )}

      {/* Advanced-options toggle: small, top-right, unlabeled. Hidden
          entry for tinkerers; invisible to average users. */}
      {stage === "email" && !showAdvanced && (
        <button
          onClick={() => setShowAdvanced(true)}
          title={t("advanced.openAdvancedTooltip")}
          aria-label={t("advanced.openAdvancedTooltip")}
          className="absolute top-12 right-6 flex items-center justify-center w-7 h-7 rounded-md text-text-muted hover:text-text-secondary hover:bg-bg-tertiary/50 transition-colors z-20"
        >
          <SlidersHorizontal size={14} strokeWidth={2} />
        </button>
      )}

      <div className="relative w-full max-w-sm">
        {stage === "email" && !showAdvanced && (
          <>
            <p className="text-lg text-text-primary text-center mb-5 tracking-tight">
              {t("signIn.prompt")}
            </p>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSend();
              }}
              placeholder={t("signIn.emailPlaceholder")}
              className="w-full px-4 py-2.5 rounded-xl bg-bg-input border border-border-primary text-sm text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors"
              autoFocus
            />
            {error && (
              <p className="text-xs text-accent-red mt-2">{error}</p>
            )}
            <button
              onClick={handleSend}
              disabled={submitting}
              className="mt-3 w-full py-2.5 rounded-xl bg-accent-green text-white text-sm font-medium hover:bg-accent-green/80 transition-colors cursor-pointer disabled:opacity-50"
            >
              {submitting ? t("signIn.sending") : t("signIn.sendLink")}
            </button>
          </>
        )}

        {stage === "email" && showAdvanced && (
          <div className="space-y-3">
            <p className="text-lg text-text-primary text-center tracking-tight">
              {t("advanced.byokTitle")}
            </p>
            <p className="text-[11.5px] text-text-muted text-center leading-relaxed">
              {t("advanced.byokDesc")}
            </p>
            <input
              type="password"
              value={byokKey}
              onChange={(e) => setByokKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSaveByok();
              }}
              placeholder={t("advanced.byokKeyPlaceholder")}
              autoFocus
              className="w-full px-4 py-2.5 rounded-xl bg-bg-input border border-border-primary text-sm text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors"
            />
            {error && (
              <p className="text-xs text-accent-red">{error}</p>
            )}
            <button
              onClick={handleSaveByok}
              disabled={byokSaving}
              className="w-full py-2.5 rounded-xl bg-accent-green text-white text-sm font-medium hover:bg-accent-green/80 transition-colors cursor-pointer disabled:opacity-50"
            >
              {byokSaving ? t("setup.saving") : t("advanced.byokSave")}
            </button>
            <button
              onClick={() => {
                setShowAdvanced(false);
                setError(null);
              }}
              className="w-full text-center text-xs text-text-muted hover:text-text-secondary transition-colors pt-1"
            >
              {t("advanced.useEmailInstead")}
            </button>
          </div>
        )}

        {stage === "sent" && (
          <div className="text-center space-y-3">
            <p className="text-sm text-text-primary leading-relaxed">
              {t("signIn.checkInbox", { email })}
            </p>
            <p className="text-xs text-text-muted">{t("signIn.checkSpam")}</p>
            <button
              onClick={() => {
                setStage("email");
                setError(null);
              }}
              className="text-xs text-text-secondary hover:text-text-primary underline"
            >
              {t("signIn.useDifferentEmail")}
            </button>
            {error && <p className="text-xs text-accent-red">{error}</p>}
          </div>
        )}

        {stage === "exchanging" && (
          <p className="text-sm text-text-secondary text-center">
            {t("signIn.finishing")}
          </p>
        )}
      </div>
    </div>
  );
}
