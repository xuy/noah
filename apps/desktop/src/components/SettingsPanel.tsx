import { useState, useEffect, useCallback } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useSessionStore } from "../stores/sessionStore";
import { useTheme, type ThemePreference } from "../hooks/useTheme";
import * as commands from "../lib/tauri-commands";

export function SettingsPanel() {
  const settingsOpen = useSessionStore((s) => s.settingsOpen);
  const setSettingsOpen = useSessionStore((s) => s.setSettingsOpen);

  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [version, setVersion] = useState("");
  const [authMode, setAuthMode] = useState<"api_key" | "proxy">("api_key");

  useEffect(() => {
    if (settingsOpen) {
      commands.getAppVersion().then(setVersion).catch(() => {});
      commands.getAuthMode().then(setAuthMode).catch(() => {});
      setApiKey("");
      setSaved(false);
      setError(null);
    }
  }, [settingsOpen]);

  const [proactiveEnabled, setProactiveEnabled] = useState(true);

  useEffect(() => {
    if (settingsOpen) {
      commands.getProactiveEnabled().then(setProactiveEnabled).catch(() => {});
    }
  }, [settingsOpen]);

  const handleToggleProactive = useCallback(async () => {
    const next = !proactiveEnabled;
    setProactiveEnabled(next);
    try {
      await commands.setProactiveEnabled(next);
    } catch (err) {
      console.error("Failed to save proactive setting:", err);
      setProactiveEnabled(!next); // revert on error
    }
  }, [proactiveEnabled]);

  const { preference: themePref, setTheme } = useTheme();

  const [reportingBug, setReportingBug] = useState(false);

  const handleReportProblem = useCallback(async () => {
    setReportingBug(true);
    try {
      const ctx = await commands.getFeedbackContext();

      // Build diagnostic section
      let diag = `\n\n---\n**Diagnostics (auto-attached)**\n`;
      diag += `- Noah v${ctx.version}\n`;
      diag += `- OS: ${ctx.os}\n`;

      if (ctx.traces.length > 0) {
        diag += `\n<details><summary>Last ${ctx.traces.length} LLM trace(s)</summary>\n\n`;
        for (const t of ctx.traces) {
          diag += `**${t.timestamp}**\n`;
          diag += `Request: \`${t.request.replace(/`/g, "'")}\`\n`;
          diag += `Response: \`${t.response.replace(/`/g, "'")}\`\n\n`;
        }
        diag += `</details>\n`;
      }

      const body = encodeURIComponent(
        `**What happened?**\n\n(Describe what you expected vs what actually happened)\n\n**Steps to reproduce**\n\n1. \n2. \n3. \n${diag}`,
      );
      const title = encodeURIComponent("Bug report from Noah app");
      const url = `https://github.com/xuy/noah/issues/new?title=${title}&body=${body}&labels=bug`;

      await openUrl(url);
    } catch (err) {
      console.error("Failed to gather feedback context:", err);
      // Fallback: open issues page without context
      await openUrl("https://github.com/xuy/noah/issues/new");
    } finally {
      setReportingBug(false);
    }
  }, []);

  const handleShareFeedback = useCallback(async () => {
    const body = encodeURIComponent(
      `**What are you trying to do?**\n\n(Describe the workflow you want help with)\n\n**What's hard today?**\n\n(Where Noah falls short or feels confusing)\n\n**What would make this 10x better?**\n\n(Feature ideas, integrations, UX suggestions)\n\n**Would you be open to a short follow-up chat?**\n\n- [ ] Yes\n- [ ] No\n\nIf yes, share the best way to reach you (email, WhatsApp, Discord, etc).`,
    );
    const title = encodeURIComponent("Product feedback from Noah app");
    const url = `https://github.com/xuy/noah/issues/new?title=${title}&body=${body}&labels=feedback`;
    await openUrl(url);
  }, []);

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
      setAuthMode("api_key");
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
          {/* Auth */}
          <section>
            <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-2">
              {authMode === "proxy" ? "Connection" : "API Key"}
            </h3>
            {authMode === "proxy" ? (
              <>
                <p className="text-[11px] text-text-muted mb-2">
                  Connected via Noah Beta (invite code).
                </p>
                <p className="text-[11px] text-text-muted mb-2">
                  Want to use your own Anthropic API key instead?
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
                  {saving
                    ? "Saving..."
                    : saved
                      ? "Saved!"
                      : "Switch to Own API Key"}
                </button>
              </>
            ) : (
              <>
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
              </>
            )}
          </section>

          {/* Proactive Suggestions */}
          <section>
            <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-2">
              Proactive Suggestions
            </h3>
            <div className="flex items-center justify-between">
              <div className="flex-1 min-w-0 mr-3">
                <p className="text-xs text-text-secondary">
                  Notify me of potential issues
                </p>
                <p className="text-[10px] text-text-muted mt-0.5">
                  Noah will periodically check your system and alert you if
                  something needs attention. At most once per day.
                </p>
              </div>
              <button
                onClick={handleToggleProactive}
                className={`relative w-9 h-5 rounded-full transition-colors cursor-pointer shrink-0 ${
                  proactiveEnabled ? "bg-accent-green" : "bg-bg-tertiary"
                }`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                    proactiveEnabled ? "translate-x-4" : "translate-x-0"
                  }`}
                />
              </button>
            </div>
          </section>

          {/* Appearance */}
          <section>
            <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-2">
              Appearance
            </h3>
            <div className="flex rounded-lg border border-border-primary overflow-hidden">
              {(["system", "light", "dark"] as ThemePreference[]).map((opt) => (
                <button
                  key={opt}
                  onClick={() => setTheme(opt)}
                  className={`flex-1 py-1.5 text-xs font-medium transition-colors cursor-pointer ${
                    themePref === opt
                      ? "bg-accent-blue/15 text-accent-blue"
                      : "text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/50"
                  }`}
                >
                  {opt === "system" ? "System" : opt === "light" ? "Light" : "Dark"}
                </button>
              ))}
            </div>
            <p className="text-[10px] text-text-muted mt-1.5">
              {themePref === "system"
                ? "Follows your operating system setting."
                : themePref === "light"
                  ? "Always use light mode."
                  : "Always use dark mode."}
            </p>
          </section>

          {/* Links */}
          <section>
            <h3 className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-2">
              Help & Feedback
            </h3>
            <div className="space-y-1.5">
              <button
                onClick={handleReportProblem}
                disabled={reportingBug}
                className="flex items-center gap-2 w-full px-3 py-2 rounded-lg text-xs text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors cursor-pointer disabled:opacity-50"
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
                {reportingBug ? "Gathering info..." : "Report a Problem"}
              </button>
              <button
                onClick={handleShareFeedback}
                className="flex items-center gap-2 w-full px-3 py-2 rounded-lg text-xs text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors cursor-pointer"
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
                Share Product Feedback
              </button>
              <a
                href="https://github.com/xuy/noah/discussions"
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
                    d="M2 3.75C2 2.7835 2.7835 2 3.75 2H10.25C11.2165 2 12 2.7835 12 3.75V8.25C12 9.2165 11.2165 10 10.25 10H6L3.25 12V10H3.75C2.7835 10 2 9.2165 2 8.25V3.75Z"
                    stroke="currentColor"
                    strokeWidth="1.2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
                Join Early Customer Discussions
              </a>
            </div>
            <p className="text-[10px] text-text-muted mt-1.5">
              Use discussions for open conversation with other early users, and
              feedback reports for specific product requests.
            </p>
          </section>
        </div>

        {/* Footer */}
        <div className="px-4 py-3 border-t border-border-primary">
          <p className="text-[10px] text-text-muted text-center">
            Noah v{version || "..."}
          </p>
        </div>
      </div>
    </>
  );
}
