import { useCallback, useEffect, useMemo, useState } from "react";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { ArrowLeft } from "lucide-react";
import * as commands from "../lib/tauri-commands";
import { NoahIcon } from "./NoahIcon";
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

function extractToken(url: string): string | null {
  try {
    const u = new URL(url);
    return u.searchParams.get("token");
  } catch {
    // Fall back for URLs the URL constructor rejects.
    const m = url.match(/[?&]token=([^&]+)/);
    return m && m[1] ? decodeURIComponent(m[1]) : null;
  }
}

export function SignInScreen({
  onComplete,
  seedContext = null,
  onBack,
}: SignInScreenProps) {
  const { t, tArray } = useLocale();
  const [stage, setStage] = useState<Stage>("email");
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [byokKey, setByokKey] = useState("");
  const [byokSaving, setByokSaving] = useState(false);
  const taglines = tArray("setup.taglines");
  const tagline = useMemo(
    () => taglines[Math.floor(Math.random() * taglines.length)],
    [taglines],
  );

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

  // Listen for the deep link that comes back from the browser after the user
  // clicks the magic link. When we get `noah://auth?token=…`, finish sign-in.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onOpenUrl(async (urls) => {
      const url = urls.find((u) => u.startsWith("noah://auth"));
      if (!url) return;
      const token = extractToken(url);
      if (!token) {
        setError(t("signIn.errorBadLink"));
        return;
      }
      setStage("exchanging");
      try {
        await commands.consumerCompleteSignIn(token);
        onComplete();
      } catch (err) {
        setStage("sent");
        setError(
          `${t("signIn.errorVerifyFailed")}: ${
            err instanceof Error ? err.message : String(err)
          }`,
        );
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, [onComplete, t]);

  const handleSend = useCallback(async () => {
    setError(null);
    const trimmed = email.trim();
    if (!trimmed || !trimmed.includes("@")) {
      setError(t("signIn.errorInvalidEmail"));
      return;
    }
    setSubmitting(true);
    try {
      await commands.consumerRequestMagicLink(trimmed);
      // Persist the seed BEFORE showing "check inbox", so even if the
      // user follows the magic link in a different window the context
      // still rides with them.
      if (seedContext) stashPendingSeed(seedContext.seedMessage);
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
      <div className="relative w-full max-w-md">
        {onBack && (
          <button
            onClick={onBack}
            className="inline-flex items-center gap-1.5 text-xs text-text-muted hover:text-text-secondary mb-4"
          >
            <ArrowLeft size={13} strokeWidth={2} />
            {t("onboarding.backLabel")}
          </button>
        )}
        <div className="flex flex-col items-center mb-8">
          <NoahIcon className="w-16 h-16 rounded-2xl mb-4" alt="Noah" />
          <h1 className="text-xl font-semibold text-text-primary tracking-tight">
            {t("signIn.welcomeTitle")}
          </h1>
          <p className="text-sm text-text-secondary mt-2 text-center leading-relaxed">
            {tagline}
          </p>
          {seedContext && (
            <div className="mt-4 w-full px-3 py-2 rounded-lg bg-accent-green/10 border border-accent-green/25 text-xs text-text-secondary text-center">
              {t("onboarding.signinContext", { category: seedContext.label })}
            </div>
          )}
        </div>

        {stage === "email" && (
          <div className="space-y-4">
            <div>
              <label className="block text-xs text-text-muted mb-1.5">
                {t("signIn.emailLabel")}
              </label>
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
                <p className="text-xs text-accent-red mt-1.5">{error}</p>
              )}
            </div>

            <button
              onClick={handleSend}
              disabled={submitting}
              className="w-full py-2.5 rounded-xl bg-accent-green text-white text-sm font-medium hover:bg-accent-green/80 transition-colors cursor-pointer disabled:opacity-50"
            >
              {submitting ? t("signIn.sending") : t("signIn.sendLink")}
            </button>

            <p className="text-[11px] text-text-muted text-center leading-relaxed">
              {t("signIn.trialHint")}
            </p>
          </div>
        )}

        {stage === "sent" && (
          <div className="space-y-4 text-center">
            <div className="px-4 py-6 rounded-xl bg-bg-input border border-border-primary">
              <p className="text-sm text-text-primary">
                {t("signIn.checkInbox", { email })}
              </p>
              <p className="text-xs text-text-muted mt-2">
                {t("signIn.checkSpam")}
              </p>
            </div>
            <button
              onClick={() => {
                setStage("email");
                setError(null);
              }}
              className="text-xs text-text-secondary hover:text-text-primary underline"
            >
              {t("signIn.useDifferentEmail")}
            </button>
            {error && (
              <p className="text-xs text-accent-red">{error}</p>
            )}
          </div>
        )}

        {stage === "exchanging" && (
          <p className="text-sm text-text-secondary text-center">
            {t("signIn.finishing")}
          </p>
        )}

        {stage === "email" && (
          <div className="mt-8 pt-4 border-t border-border-primary/60">
            <button
              onClick={() => setShowAdvanced((v) => !v)}
              className="text-[11px] text-text-muted hover:text-text-secondary underline"
            >
              {showAdvanced
                ? t("advanced.sectionTitle")
                : t("advanced.byokTitle")}
            </button>
            {showAdvanced && (
              <div className="mt-3 space-y-2">
                <p className="text-[11px] text-text-muted leading-relaxed">
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
                  className="w-full px-4 py-2 rounded-xl bg-bg-input border border-border-primary text-sm text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors"
                />
                <button
                  onClick={handleSaveByok}
                  disabled={byokSaving}
                  className="w-full py-2 rounded-xl border border-border-primary text-sm text-text-primary hover:bg-bg-hover transition-colors disabled:opacity-50"
                >
                  {byokSaving ? t("setup.saving") : t("advanced.byokSave")}
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
