import { useState, useEffect, useCallback } from "react";
import { useSessionStore } from "../stores/sessionStore";
import * as commands from "../lib/tauri-commands";

export function SettingsPanel() {
  const settingsOpen = useSessionStore((s) => s.settingsOpen);
  const setSettingsOpen = useSessionStore((s) => s.setSettingsOpen);

  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [version, setVersion] = useState("");

  useEffect(() => {
    if (settingsOpen) {
      commands.getAppVersion().then(setVersion).catch(() => {});
      setApiKey("");
      setSaved(false);
      setError(null);
    }
  }, [settingsOpen]);

  const handleSaveKey = useCallback(async () => {
    const key = apiKey.trim();
    if (!key) return;
    if (!key.startsWith("sk-ant-")) {
      setError(
        "That doesn't look like an Anthropic API key. It should start with sk-ant-",
      );
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await commands.setApiKey(key);
      setSaved(true);
      setApiKey("");
      setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      setError(
        `Failed to save: ${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setSaving(false);
    }
  }, [apiKey]);

  if (!settingsOpen) return null;

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-30 bg-black/20"
        onClick={() => setSettingsOpen(false)}
      />

      {/* Slide-out panel */}
      <div className="fixed top-0 right-0 bottom-0 z-40 w-80 bg-bg-secondary border-l border-border-primary shadow-2xl flex flex-col animate-slide-in-right">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-primary">
          <h2 className="text-sm font-semibold text-text-primary">Settings</h2>
          <button
            onClick={() => setSettingsOpen(false)}
            className="w-7 h-7 rounded-md flex items-center justify-center text-text-muted hover:text-text-primary hover:bg-bg-tertiary transition-colors cursor-pointer"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 14 14"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M3 3L11 11M11 3L3 11"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-4 py-4 space-y-6">
          {/* API Key */}
          <section>
            <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-2">
              API Key
            </h3>
            <p className="text-[11px] text-text-muted mb-2">
              Enter a new Anthropic API key to replace the current one.
            </p>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSaveKey();
              }}
              placeholder="sk-ant-..."
              className="w-full px-3 py-2 rounded-lg bg-bg-input border border-border-primary text-xs text-text-primary placeholder-text-muted outline-none focus:border-border-focus transition-colors"
            />
            {error && (
              <p className="text-[11px] text-accent-red mt-1">{error}</p>
            )}
            <button
              onClick={handleSaveKey}
              disabled={!apiKey.trim() || saving}
              className="mt-2 w-full py-1.5 rounded-lg bg-accent-green text-white text-xs font-medium hover:bg-accent-green/80 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {saving ? "Saving..." : saved ? "Saved!" : "Update API Key"}
            </button>
          </section>

          {/* Links */}
          <section>
            <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-2">
              Help & Feedback
            </h3>
            <div className="space-y-1.5">
              <a
                href="https://github.com/xulea/itman/issues"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 px-3 py-2 rounded-lg text-xs text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 14 14"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                >
                  <circle
                    cx="7"
                    cy="7"
                    r="5.5"
                    stroke="currentColor"
                    strokeWidth="1.2"
                  />
                  <path
                    d="M5.5 5.5C5.5 4.67 6.17 4 7 4C7.83 4 8.5 4.67 8.5 5.5C8.5 6.33 7.83 7 7 7V8"
                    stroke="currentColor"
                    strokeWidth="1.2"
                    strokeLinecap="round"
                  />
                  <circle cx="7" cy="9.5" r="0.5" fill="currentColor" />
                </svg>
                Report a Problem
              </a>
              <a
                href="https://console.anthropic.com"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 px-3 py-2 rounded-lg text-xs text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 14 14"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                >
                  <path
                    d="M7 1.5V5.5L10.5 3.5M7 1.5L3.5 3.5V7.5L7 5.5M7 1.5L10.5 3.5M10.5 3.5V7.5L7 9.5M7 9.5L3.5 7.5M7 9.5V12.5M3.5 7.5L7 12.5M10.5 7.5L7 12.5"
                    stroke="currentColor"
                    strokeWidth="1.2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
                Anthropic Console
              </a>
            </div>
          </section>
        </div>

        {/* Footer */}
        <div className="px-4 py-3 border-t border-border-primary">
          <p className="text-[10px] text-text-muted text-center">
            Noah v{version || "..."} &middot; Powered by Claude
          </p>
        </div>
      </div>
    </>
  );
}
