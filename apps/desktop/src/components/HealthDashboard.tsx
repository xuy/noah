import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useHealthStore } from "../stores/healthStore";
import { useLocale } from "../i18n";
import * as commands from "../lib/tauri-commands";
import type { CategoryScore, CheckResult, HealthScore, ScanJobRecord } from "../lib/tauri-commands";

// ── Grade colors ────────────────────────────────────────────────────

function gradeColor(grade: string): string {
  switch (grade) {
    case "A": return "text-accent-green";
    case "B": return "text-accent-blue";
    case "C": return "text-accent-yellow";
    case "D": return "text-accent-orange";
    default: return "text-accent-red";
  }
}

function gradeBg(grade: string): string {
  switch (grade) {
    case "A": return "bg-accent-green/15";
    case "B": return "bg-accent-blue/15";
    case "C": return "bg-accent-yellow/15";
    case "D": return "bg-accent-orange/15";
    default: return "bg-accent-red/15";
  }
}

function gradeRing(grade: string): string {
  switch (grade) {
    case "A": return "border-accent-green";
    case "B": return "border-accent-blue";
    case "C": return "border-accent-yellow";
    case "D": return "border-accent-orange";
    default: return "border-accent-red";
  }
}

function statusIcon(status: string) {
  switch (status) {
    case "pass":
      return <span className="text-accent-green">{"\u2713"}</span>;
    case "warn":
      return <span className="text-accent-yellow">{"\u26A0"}</span>;
    default:
      return <span className="text-accent-red">{"\u2717"}</span>;
  }
}

function statusLabel(status: string, t: (key: string) => string): string {
  switch (status) {
    case "pass": return t("health.statusPass");
    case "warn": return t("health.statusWarn");
    default: return t("health.statusFail");
  }
}

// ── Time helpers ────────────────────────────────────────────────────

function timeAgo(iso: string, t: (key: string, p?: Record<string, string | number>) => string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return t("health.justNow");
  if (mins < 60) return t("health.minutesAgo", { count: mins });
  const hours = Math.floor(mins / 60);
  if (hours < 24) return t("health.hoursAgo", { count: hours });
  const days = Math.floor(hours / 24);
  return t("health.daysAgo", { count: days });
}

function formatRelativeTime(iso: string | null): string {
  if (!iso) return "";
  const diffMin = Math.floor((Date.now() - new Date(iso).getTime()) / 60000);
  if (diffMin < 1) return "Just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const h = Math.floor(diffMin / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

// ── Action hints per check ───────────────────────────────────────────

interface ActionInfo {
  hint: string;
  canOpen: boolean;
}

function actionInfo(check: CheckResult): ActionInfo | null {
  if (check.status === "pass") return null;
  switch (check.id) {
    // Security
    case "security.firewall":
      return { hint: "Turn on your firewall in Network settings", canOpen: true };
    case "security.filevault":
      return { hint: "Enable FileVault disk encryption", canOpen: true };
    case "security.sip":
      return { hint: "Requires Recovery Mode: reboot holding Cmd+R, then run csrutil enable", canOpen: false };
    case "security.gatekeeper":
      return { hint: "Re-enable Gatekeeper in Security settings", canOpen: true };
    case "security.screen_lock":
      return { hint: "Set a screen lock timeout", canOpen: true };
    case "security.xprotect":
      return { hint: "Install macOS updates to restore XProtect", canOpen: true };
    case "security.defender":
      return { hint: "Turn on Real-time protection in Windows Security", canOpen: true };
    case "security.bitlocker":
      return { hint: "Enable BitLocker drive encryption", canOpen: true };
    case "security.uac":
      return { hint: "Raise UAC level in User Account Control settings", canOpen: true };
    // Updates
    case "updates.os":
      return { hint: "Install available system updates", canOpen: true };
    case "updates.brew":
      return { hint: "Run: brew upgrade", canOpen: false };
    default:
      return null;
  }
}

// ── Check row ───────────────────────────────────────────────────────

function CheckRow({ check, t }: { check: CheckResult; t: (key: string) => string }) {
  const action = actionInfo(check);
  return (
    <div className="flex items-center gap-3 py-2.5">
      <div className="flex-shrink-0 text-base">{statusIcon(check.status)}</div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <p className="text-sm text-text-primary font-medium">{check.label}</p>
          <span className={`text-[10px] font-medium ${
            check.status === "pass" ? "text-accent-green" :
            check.status === "warn" ? "text-accent-yellow" : "text-accent-red"
          }`}>
            {statusLabel(check.status, t)}
          </span>
        </div>
        {action && (
          <p className="text-xs text-text-muted mt-0.5">{action.hint}</p>
        )}
      </div>
      {action?.canOpen && (
        <button
          onClick={() => {
            commands.openHealthFix(check.id).catch((err) => {
              console.error("Failed to open settings:", err);
            });
          }}
          className="flex-shrink-0 px-3 py-1 text-xs font-medium text-accent-blue border border-accent-blue/30 rounded-md hover:bg-accent-blue/10 transition-colors cursor-pointer"
        >
          {t("health.openSettings")}
        </button>
      )}
    </div>
  );
}

// ── Category section ────────────────────────────────────────────────

function CategorySection({ cat, t }: { cat: CategoryScore; t: (key: string) => string }) {
  return (
    <div className="bg-bg-secondary border border-border-primary rounded-xl p-5">
      <div className="flex items-center justify-between mb-1">
        <h3 className="text-sm font-semibold text-text-primary capitalize">{cat.category}</h3>
        <div className={`flex items-center gap-1.5 px-2 py-0.5 rounded-md ${gradeBg(cat.grade)}`}>
          <span className={`text-sm font-bold ${gradeColor(cat.grade)}`}>{cat.grade}</span>
          <span className="text-xs text-text-muted">{cat.score}/100</span>
        </div>
      </div>
      <div className="divide-y divide-border-primary">
        {cat.checks.map((check) => (
          <CheckRow key={check.id} check={check} t={t} />
        ))}
      </div>
    </div>
  );
}

// ── History sparkline ───────────────────────────────────────────────

function Sparkline({ history }: { history: { score: number }[] }) {
  if (history.length < 2) return null;
  const points = history.slice(0, 7).reverse();
  const w = 140;
  const h = 32;
  const step = w / (points.length - 1);
  const pathData = points
    .map((p, i) => {
      const x = i * step;
      const y = h - (p.score / 100) * h;
      return `${i === 0 ? "M" : "L"} ${x} ${y}`;
    })
    .join(" ");
  return (
    <svg width={w} height={h} className="opacity-60">
      <path d={pathData} fill="none" stroke="currentColor" strokeWidth="1.5" className="text-accent-blue" />
    </svg>
  );
}

// ── Score badge ─────────────────────────────────────────────────────

function ScoreBadge({ score }: { score: HealthScore }) {
  return (
    <div className={`flex flex-col items-center justify-center w-28 h-28 rounded-full border-4 ${gradeRing(score.overall_grade)} ${gradeBg(score.overall_grade)}`}>
      <span className={`text-3xl font-bold ${gradeColor(score.overall_grade)}`}>
        {score.overall_grade}
      </span>
      <span className="text-base text-text-secondary font-medium">{score.overall_score}</span>
    </div>
  );
}

// ── What-we-check list (shown before first scan) ────────────────────

const CHECK_PREVIEW = [
  { icon: "\uD83D\uDD25", label: "Firewall" },
  { icon: "\uD83D\uDD10", label: "Disk Encryption" },
  { icon: "\uD83D\uDEE1\uFE0F", label: "System Integrity" },
  { icon: "\uD83D\uDEAA", label: "Gatekeeper" },
  { icon: "\uD83D\uDD12", label: "Screen Lock" },
  { icon: "\uD83D\uDCE6", label: "System Updates" },
];

function CheckPreview({ t }: { t: (key: string) => string }) {
  return (
    <div className="bg-bg-secondary border border-border-primary rounded-xl p-5">
      <h3 className="text-sm font-semibold text-text-primary mb-3">{t("health.whatWeCheck")}</h3>
      <div className="grid grid-cols-2 gap-2">
        {CHECK_PREVIEW.map((item) => (
          <div key={item.label} className="flex items-center gap-2 text-xs text-text-secondary">
            <span>{item.icon}</span>
            <span>{item.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Summary bar ─────────────────────────────────────────────────────

function SummaryBar({ score, t }: { score: HealthScore; t: (key: string, p?: Record<string, string | number>) => string }) {
  const total = score.categories.reduce((n, c) => n + c.checks.length, 0);
  const passed = score.categories.reduce(
    (n, c) => n + c.checks.filter((ch) => ch.status === "pass").length, 0,
  );
  const failed = total - passed;
  return (
    <div className="flex items-center gap-4 text-xs text-text-muted">
      <span>{t("health.checksRun", { count: total })}</span>
      <span className="text-accent-green">{t("health.passed", { count: passed })}</span>
      {failed > 0 && (
        <span className="text-accent-red">{t("health.needsAttention", { count: failed })}</span>
      )}
    </div>
  );
}

// ── Disk scan section (absorbed from DiagnosticsPanel) ──────────────

interface ScanProgressEvent {
  scan_type: string;
  display_name: string;
  status: string;
  progress_pct: number;
  progress_detail: string;
}

function DiskScanCard({ t }: { t: (key: string) => string }) {
  const [job, setJob] = useState<ScanJobRecord | null>(null);
  const [live, setLive] = useState<ScanProgressEvent | null>(null);

  useEffect(() => {
    commands.getScanJobs().then((jobs) => {
      const disk = jobs.find((j) => j.scan_type === "disk");
      if (disk) setJob(disk);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<ScanProgressEvent>("scanner-progress", (e) => {
      if (e.payload.scan_type === "disk") {
        setLive(e.payload);
        if (e.payload.status === "completed" || e.payload.status === "failed") {
          commands.getScanJobs().then((jobs) => {
            const disk = jobs.find((j) => j.scan_type === "disk");
            if (disk) setJob(disk);
          }).catch(() => {});
        }
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const status = live?.status || job?.status || "queued";
  const pct = live?.progress_pct ?? job?.progress_pct ?? 0;
  const detail = live?.progress_detail || job?.progress_detail || "";
  const isRunning = status === "running";
  const isPaused = status === "paused";

  const statusText = isRunning ? (detail || t("diagnostics.starting"))
    : isPaused ? (detail || t("diagnostics.paused"))
    : status === "completed" ? t("diagnostics.complete")
    : status === "failed" ? t("diagnostics.failed")
    : t("diagnostics.waitingFirstScan");

  const statusClr = isRunning ? "text-accent-blue"
    : status === "completed" ? "text-accent-green"
    : status === "failed" ? "text-accent-red"
    : isPaused ? "text-accent-yellow"
    : "text-text-muted";

  const handleAction = async () => {
    if (isRunning) {
      await commands.pauseScan("disk").catch(() => {});
    } else if (isPaused) {
      await commands.resumeScan("disk").catch(() => {});
    } else {
      await commands.triggerScan("disk").catch(() => {});
    }
  };

  const btnLabel = isRunning ? t("diagnostics.pause")
    : isPaused ? t("diagnostics.resume")
    : status === "completed" ? t("diagnostics.rescan")
    : t("diagnostics.scanNow");

  const ts = job?.completed_at || job?.updated_at;

  return (
    <div className="bg-bg-secondary border border-border-primary rounded-xl p-5">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-text-primary">{t("diagnostics.diskAnalysis")}</h3>
        <button
          onClick={handleAction}
          className="text-xs px-2.5 py-1 rounded-md bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors cursor-pointer"
        >
          {btnLabel}
        </button>
      </div>
      {(isRunning || isPaused) && (
        <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden mb-2">
          <div
            className={`h-full rounded-full transition-all duration-500 ${isRunning ? "bg-accent-blue" : "bg-accent-yellow"}`}
            style={{ width: `${Math.max(pct, 2)}%` }}
          />
        </div>
      )}
      <div className="flex items-center justify-between text-xs">
        <span className={`truncate ${statusClr}`}>{statusText}</span>
        {ts && <span className="text-text-muted flex-shrink-0 ml-3">{formatRelativeTime(ts)}</span>}
      </div>
    </div>
  );
}

// ── Main dashboard ──────────────────────────────────────────────────

export function HealthDashboard() {
  const { score, history, loading, error, setScore, setHistory, setLoading, setError } = useHealthStore();
  const { t } = useLocale();

  const loadScore = useCallback(async () => {
    try {
      const s = await commands.getHealthScore();
      if (s && s.overall_score !== undefined) {
        setScore(s);
      }
    } catch { /* no data yet */ }
  }, [setScore]);

  const loadHistory = useCallback(async () => {
    try {
      const records = await commands.getHealthHistory(7);
      setHistory(records.map((r) => ({
        overall_score: r.score,
        overall_grade: r.grade,
        categories: JSON.parse(r.categories || "[]"),
        computed_at: r.computed_at,
        device_id: r.device_id,
      })));
    } catch { /* ok */ }
  }, [setHistory]);

  useEffect(() => {
    loadScore();
    loadHistory();
  }, [loadScore, loadHistory]);

  const handleRunCheck = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await commands.runHealthCheck();
      setScore(s);
      await loadHistory();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    setLoading(false);
  }, [setScore, setLoading, setError, loadHistory]);

  const hasResults = score !== null && score.categories.length > 0;

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-8">
        {/* Header */}
        <div className="mb-6">
          <h1 className="text-xl font-semibold text-text-primary">{t("health.title")}</h1>
          <p className="text-sm text-text-muted mt-1">{t("health.subtitle")}</p>
        </div>

        {/* Score + action area */}
        <div className="flex items-start gap-6 mb-6">
          {hasResults ? (
            <ScoreBadge score={score} />
          ) : (
            <div className="flex flex-col items-center justify-center w-28 h-28 rounded-full border-4 border-border-primary bg-bg-secondary">
              <span className="text-2xl text-text-muted">--</span>
            </div>
          )}

          <div className="flex-1 space-y-3 pt-2">
            <button
              onClick={handleRunCheck}
              disabled={loading}
              className="px-4 py-2 rounded-lg bg-accent-blue text-white text-sm font-medium hover:bg-accent-blue/90 transition-colors disabled:opacity-50 cursor-pointer"
            >
              {loading ? t("health.running") : hasResults ? t("health.runAgain") : t("health.runCheck")}
            </button>
            <p className="text-xs text-text-muted">{t("health.runCheckDesc")}</p>

            {hasResults && (
              <>
                <SummaryBar score={score} t={t} />
                <p className="text-[11px] text-text-muted">
                  {t("health.lastChecked", { time: timeAgo(score.computed_at, t) })}
                </p>
              </>
            )}

            {history.length >= 2 && (
              <div className="pt-1">
                <p className="text-[10px] text-text-muted mb-1">{t("health.recentTrend")}</p>
                <Sparkline history={history.map((h) => ({ score: h.overall_score }))} />
              </div>
            )}

            {error && <p className="text-xs text-accent-red">{error}</p>}
          </div>
        </div>

        {/* Health check results */}
        {hasResults ? (
          <div className="space-y-4">
            {score.categories.map((cat) => (
              <CategorySection key={cat.category} cat={cat} t={t} />
            ))}
          </div>
        ) : (
          <CheckPreview t={t} />
        )}

        {/* Disk scan — compact card */}
        <div className="mt-6">
          <DiskScanCard t={t} />
        </div>

        {/* Footer */}
        <p className="text-[10px] text-text-muted text-center mt-8">{t("health.footer")}</p>
      </div>
    </div>
  );
}
