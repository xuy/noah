import { useState, useEffect, useCallback } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTheme, type ThemePreference } from "../hooks/useTheme";
import * as commands from "../lib/tauri-commands";
import { useLocale } from "../i18n";
import { BillingSection } from "./BillingSection";

/**
 * Direction A — "Refined single page" Settings.
 *
 * Stacked single column at 760px. Three cards sharing the same rhythm:
 *   1. Billing — page hero. Status dot+pill, plan as a typed line,
 *      trial sentence as body copy, single aurora "Manage subscription"
 *      action in a soft action strip.
 *   2. Appearance — segmented control with a sliding aurora thumb.
 *   3. Help & feedback — two quiet action rows with aurora-tinted
 *      icon squares.
 *
 * Aurora is reserved strictly for the commit moment.
 */
export function SettingsPanel() {
  const [version, setVersion] = useState("");
  const [authMode, setAuthMode] = useState<"api_key" | "proxy">("proxy");

  useEffect(() => {
    commands.getAppVersion().then(setVersion).catch(() => {});
    commands.getAuthMode().then(setAuthMode).catch(() => {});
  }, []);

  const { preference: themePref, setTheme } = useTheme();
  const { t } = useLocale();

  const handleReportProblem = useCallback(async () => {
    const subject = encodeURIComponent("Noah feedback");
    const body = encodeURIComponent(
      `\n\n\n---\nNoah v${version || "?"} — please describe the issue above this line.`,
    );
    await openUrl(`mailto:support@onnoah.app?subject=${subject}&body=${body}`);
  }, [version]);

  const handleOpenHelp = useCallback(async () => {
    await openUrl("https://help.onnoah.app");
  }, []);

  return (
    <div className="flex-1 min-h-0 overflow-y-auto bg-bg-primary">
      <div className="mx-auto w-full max-w-[760px] px-6 py-10 pb-16">
        {/* Page header */}
        <header className="mb-7">
          <SectionEyebrow>{t("settings.eyebrow")}</SectionEyebrow>
          <h1 className="text-[28px] font-bold tracking-[-0.028em] text-text-primary mt-2 mb-1">
            {t("settings.title")}
          </h1>
          <p className="text-[13.5px] text-text-secondary leading-[1.55]">
            {t("settings.subtitle")}
          </p>
        </header>

        <div className="space-y-[18px]">
          {authMode === "proxy" && <BillingSection />}

          {/* ── Appearance card ──────────────────────────────────── */}
          <SettingsCard>
            <div className="px-[22px] py-5">
              <div className="flex items-center justify-between mb-3">
                <SectionEyebrow>{t("settings.appearance")}</SectionEyebrow>
                <span className="text-[11.5px] text-text-muted whitespace-nowrap">
                  {themePref === "system"
                    ? t("settings.followsOSShort")
                    : themePref === "light"
                      ? t("settings.alwaysLightShort")
                      : t("settings.alwaysDarkShort")}
                </span>
              </div>
              <AppearanceToggle value={themePref} onChange={setTheme} t={t} />
            </div>
          </SettingsCard>

          {/* ── Help & feedback card ─────────────────────────────── */}
          <SettingsCard>
            <div className="px-[14px] pt-[18px] pb-3">
              <div className="px-2 pb-2">
                <SectionEyebrow>{t("settings.helpFeedback")}</SectionEyebrow>
              </div>
              <ActionLink
                onClick={handleReportProblem}
                icon={<MailIcon />}
                label={t("settings.contactSupport")}
              />
              <ActionLink
                onClick={handleOpenHelp}
                icon={<HelpIcon />}
                label={t("settings.helpAndFaq")}
              />
            </div>
          </SettingsCard>

          {/* Footer — version line, intentionally quiet */}
          <div className="pt-6 flex items-center justify-center gap-2 text-[11.5px] text-text-muted">
            <NoahMark />
            <span>
              Noah · v{version || "…"}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Card primitive ────────────────────────────────────────────────────
function SettingsCard({ children }: { children: React.ReactNode }) {
  return (
    <section
      className="rounded-2xl bg-bg-secondary overflow-hidden"
      style={{
        border: "1px solid var(--color-surface-card-border)",
        boxShadow: "var(--shadow-card)",
      }}
    >
      {children}
    </section>
  );
}

// ── Section eyebrow with aurora hairline ─────────────────────────────
function SectionEyebrow({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-[7px] text-[10.5px] font-bold uppercase tracking-[0.14em] text-text-muted whitespace-nowrap">
      <span
        className="block w-3 h-[2px] rounded-[1px]"
        style={{ background: "var(--aurora)" }}
      />
      {children}
    </span>
  );
}

// ── Appearance segmented control with sliding aurora thumb ───────────
type Tt = (key: string, vars?: Record<string, string | number>) => string;
function AppearanceToggle({
  value,
  onChange,
  t,
}: {
  value: ThemePreference;
  onChange: (v: ThemePreference) => void;
  t: Tt;
}) {
  const opts: { id: ThemePreference; label: string; icon: React.ReactNode }[] = [
    { id: "system", label: t("settings.system"), icon: <SystemIcon /> },
    { id: "light", label: t("settings.light"), icon: <SunIcon /> },
    { id: "dark", label: t("settings.dark"), icon: <MoonIcon /> },
  ];
  const idx = Math.max(
    0,
    opts.findIndex((o) => o.id === value),
  );
  return (
    <div
      className="relative grid grid-cols-3 p-1 rounded-xl bg-bg-primary"
      style={{ border: "1px solid var(--color-surface-card-border)" }}
    >
      {/* sliding thumb — easing matches Apple's "sub-100ms feel" curve */}
      <div
        className="absolute top-1 bottom-1 rounded-[9px] pointer-events-none"
        style={{
          left: `calc(${(idx / 3) * 100}% + 4px)`,
          width: `calc(${100 / 3}% - 8px)`,
          background: "var(--color-bg-secondary)",
          border: "1px solid rgba(99, 102, 241, 0.35)",
          boxShadow:
            "0 1px 2px rgba(15,23,41,0.06), 0 0 0 3px rgba(99, 102, 241, 0.08)",
          transition: "left 220ms cubic-bezier(0.32, 0.72, 0, 1)",
        }}
      />
      {opts.map((o) => {
        const active = o.id === value;
        return (
          <button
            key={o.id}
            onClick={() => onChange(o.id)}
            className={`relative inline-flex items-center justify-center gap-[7px] py-2 px-1.5 rounded-[9px] text-[12.5px] cursor-pointer transition-colors ${
              active
                ? "font-semibold"
                : "font-medium text-text-muted hover:text-text-secondary"
            }`}
            style={
              active
                ? { color: "var(--color-accent-indigo)" }
                : undefined
            }
          >
            {o.icon}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

// ── Quiet action row — icon-in-aurora-square + label + chevron ───────
function ActionLink({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-3 w-full px-3.5 py-3 rounded-[10px] text-left cursor-pointer transition-colors hover:bg-bg-primary group"
    >
      <span
        className="inline-flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
        style={{
          background: "var(--color-accent-blue-soft)",
          color: "var(--color-accent-indigo)",
        }}
      >
        {icon}
      </span>
      <span className="flex-1 text-[13px] font-medium text-text-primary tracking-[-0.005em]">
        {label}
      </span>
      <ChevronIcon className="text-text-muted group-hover:text-text-secondary transition-colors" />
    </button>
  );
}

// ── Icons (small, inline, no external deps) ──────────────────────────
function SystemIcon() {
  // "auto" / monitor — represents OS-derived theme
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="4" width="18" height="12" rx="2" />
      <path d="M8 20h8M12 16v4" />
    </svg>
  );
}
function SunIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
    </svg>
  );
}
function MoonIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  );
}
function MailIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="m3 7 9 6 9-6" />
    </svg>
  );
}
function HelpIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
      <line x1="12" y1="17" x2="12.01" y2="17" />
    </svg>
  );
}
function ChevronIcon({ className }: { className?: string }) {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <path d="m9 18 6-6-6-6" />
    </svg>
  );
}
function NoahMark() {
  // Footer mark — tiny aurora dot. The full app icon would shout
  // here; a 14px gradient bead just whispers "Noah".
  return (
    <span
      className="inline-block w-3.5 h-3.5 rounded-[4px]"
      style={{
        background: "var(--aurora)",
        boxShadow: "0 0 0 1px rgba(99, 102, 241, 0.15)",
      }}
    />
  );
}
