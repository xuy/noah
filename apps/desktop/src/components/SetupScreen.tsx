import { useState, useCallback } from "react";
import * as commands from "../lib/tauri-commands";
import { NoahIcon } from "./NoahIcon";

const PROXY_URL = "https://noah-proxy.fly.dev";

interface SetupScreenProps {
  onComplete: () => void;
}

type AuthPath = "invite" | "api_key";

export function SetupScreen({ onComplete }: SetupScreenProps) {
  const [authPath, setAuthPath] = useState<AuthPath>("invite");
  const [inviteCode, setInviteCode] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const handleSave = useCallback(async () => {
    setError(null);
    setSaving(true);

    try {
      if (authPath === "invite") {
        const code = inviteCode.trim().toUpperCase();
        if (!code) {
          setError("Please enter your invite code.");
          return;
        }
        await commands.redeemInviteCode(PROXY_URL, code);
      } else {
        const key = apiKey.trim();
        if (!key) {
          setError("Please paste your API key.");
          return;
        }
        if (!key.startsWith("sk-ant-")) {
          setError(
            "That doesn't look like an Anthropic API key. It should start with sk-ant-",
          );
          return;
        }
        await commands.setApiKey(key);
      }
      onComplete();
    } catch (err) {
      setError(
        `Failed: ${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setSaving(false);
    }
  }, [authPath, inviteCode, apiKey, onComplete]);

  return (
    <div className="flex flex-col items-center justify-center h-screen bg-bg-primary px-6">
      <div className="w-full max-w-md">
        {/* Logo */}
        <div className="flex flex-col items-center mb-8">
          <NoahIcon className="w-16 h-16 rounded-2xl mb-4" alt="Noah" />
          <h1 className="text-xl font-semibold text-text-primary">
            Welcome to Noah
          </h1>
          <p className="text-sm text-text-secondary mt-2 text-center leading-relaxed">
            Noah uses Claude by Anthropic to help fix your computer.
          </p>
        </div>

        {/* Auth path toggle */}
        <div className="space-y-4">
          <div className="space-y-2">
            <label
              className="flex items-center gap-3 px-4 py-3 rounded-xl border cursor-pointer transition-colors"
              style={{
                borderColor:
                  authPath === "invite"
                    ? "var(--color-accent-green)"
                    : "var(--color-border-primary)",
                backgroundColor:
                  authPath === "invite"
                    ? "var(--color-accent-green-bg, rgba(52, 199, 89, 0.08))"
                    : "transparent",
              }}
            >
              <input
                type="radio"
                name="auth-path"
                checked={authPath === "invite"}
                onChange={() => {
                  setAuthPath("invite");
                  setError(null);
                }}
                className="accent-[var(--color-accent-green)]"
              />
              <div>
                <div className="text-sm font-medium text-text-primary">
                  I have an invite code
                </div>
                <div className="text-[11px] text-text-muted">
                  From a friend or the Noah team
                </div>
              </div>
            </label>

            <label
              className="flex items-center gap-3 px-4 py-3 rounded-xl border cursor-pointer transition-colors"
              style={{
                borderColor:
                  authPath === "api_key"
                    ? "var(--color-accent-green)"
                    : "var(--color-border-primary)",
                backgroundColor:
                  authPath === "api_key"
                    ? "var(--color-accent-green-bg, rgba(52, 199, 89, 0.08))"
                    : "transparent",
              }}
            >
              <input
                type="radio"
                name="auth-path"
                checked={authPath === "api_key"}
                onChange={() => {
                  setAuthPath("api_key");
                  setError(null);
                }}
                className="accent-[var(--color-accent-green)]"
              />
              <div>
                <div className="text-sm font-medium text-text-primary">
                  I have an Anthropic API key
                </div>
                <div className="text-[11px] text-text-muted">
                  Use your own key directly
                </div>
              </div>
            </label>
          </div>

          {/* Input field */}
          <div>
            {authPath === "invite" ? (
              <input
                type="text"
                value={inviteCode}
                onChange={(e) => setInviteCode(e.target.value.toUpperCase())}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSave();
                }}
                placeholder="NOAH-XXXX-XXXX"
                className="w-full px-4 py-2.5 rounded-xl bg-bg-input border border-border-primary text-sm text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors tracking-widest font-mono"
                autoFocus
              />
            ) : (
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSave();
                }}
                placeholder="sk-ant-..."
                className="w-full px-4 py-2.5 rounded-xl bg-bg-input border border-border-primary text-sm text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors"
                autoFocus
              />
            )}
            {error && (
              <p className="text-xs text-accent-red mt-1.5">{error}</p>
            )}
          </div>

          <button
            onClick={handleSave}
            disabled={saving}
            className="w-full py-2.5 rounded-xl bg-accent-green text-white text-sm font-medium hover:bg-accent-green/80 transition-colors cursor-pointer disabled:opacity-50"
          >
            {saving
              ? authPath === "invite"
                ? "Connecting..."
                : "Saving..."
              : authPath === "invite"
                ? "Connect"
                : "Save & Start"}
          </button>

          {authPath === "api_key" && (
            <p className="text-[11px] text-text-muted text-center leading-relaxed">
              Don't have a key?{" "}
              <a
                href="https://console.anthropic.com"
                target="_blank"
                rel="noopener noreferrer"
                className="text-accent-green hover:underline"
              >
                Get one from Anthropic
              </a>
              .
              <br />
              Your key is saved locally and never shared.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
