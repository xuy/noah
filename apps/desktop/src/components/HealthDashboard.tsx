import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useHealthStore } from "../stores/healthStore";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import { useLocale } from "../i18n";
import * as commands from "../lib/tauri-commands";
import type { CategoryScore, CheckResult, DashboardStatus, FleetAction, HealthScore, ScanJobRecord } from "../lib/tauri-commands";

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
      return { hint: "Set \"Require password\" to 5 minutes or less in Lock Screen settings", canOpen: true };
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
    // Backups
    case "backups.timemachine":
      return { hint: "Set up Time Machine in System Settings", canOpen: true };
    case "backups.timemachine_dest":
      return { hint: "Connect a backup drive or configure a network backup destination", canOpen: true };
    case "backups.filehistory":
      return { hint: "Turn on File History in Windows Settings", canOpen: true };
    case "backups.restore_points":
      return { hint: "Enable System Protection in System Properties", canOpen: true };
    // Performance
    case "performance.uptime":
      return { hint: "Restart your computer to apply pending updates and free memory", canOpen: false };
    case "performance.disk_free":
      return { hint: "Free up disk space by removing unused files and apps", canOpen: false };
    case "performance.startup_items":
      return { hint: "Reduce startup items to speed up boot time", canOpen: false };
    case "performance.memory":
      return { hint: "Close unused applications to free memory", canOpen: false };
    // Network
    case "network.dns":
      return { hint: "Check your DNS settings or try switching to 1.1.1.1 or 8.8.8.8", canOpen: false };
    case "network.internet":
      return { hint: "Check your internet connection and router", canOpen: false };
    case "network.gateway":
      return { hint: "Check your network adapter settings", canOpen: false };
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

// ── Playbook preview card (for fleet-dispatched playbooks) ──────────

function PlaybookPreviewCard({ action, t, onDismiss, onRemove }: {
  action: FleetAction;
  t: (key: string, params?: Record<string, string | number>) => string;
  onDismiss: () => void;
  onRemove: () => void;
}) {
  const [starting, setStarting] = useState(false);
  const setActiveView = useSessionStore((s) => s.setActiveView);
  const setSession = useSessionStore((s) => s.setSession);
  const clearMessages = useChatStore((s) => s.clearMessages);

  // Parse playbook steps from markdown headers
  const steps = (action.playbook_content || "")
    .split("\n")
    .filter((line) => /^#{1,3}\s+\d+[\.\)]\s/.test(line))
    .map((line) => line.replace(/^#{1,3}\s+/, "").trim());

  const handleRunFix = async () => {
    if (starting) return;
    setStarting(true);
    try {
      const resultJson = await commands.startFleetPlaybook(action.id, action.playbook_slug!);
      const result = JSON.parse(resultJson);

      // Navigate to chat with the new session
      clearMessages();
      setSession(result.session_id);
      setActiveView("chat");

      // Send activate_playbook as first message after a brief delay
      setTimeout(async () => {
        try {
          await commands.sendMessageV2(result.session_id, `activate_playbook ${result.playbook_slug}`);
        } catch (err) {
          console.error("Failed to activate playbook:", err);
        }
      }, 500);

      onRemove();
    } catch (err) {
      console.error("Failed to start playbook:", err);
      setStarting(false);
    }
  };

  return (
    <div className="bg-accent-green/5 border border-accent-green/20 rounded-xl p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1">
          <p className="text-xs text-accent-green font-medium mb-0.5">{t("health.adminRequest")}</p>
          <p className="text-sm text-text-primary font-medium">{action.check_label}</p>
          <p className="text-xs text-text-muted mt-0.5">{action.action_hint}</p>
          {steps.length > 0 && (
            <div className="mt-2 space-y-0.5">
              {steps.slice(0, 5).map((step, i) => (
                <p key={i} className="text-[10px] text-text-muted">{step}</p>
              ))}
              {steps.length > 5 && (
                <p className="text-[10px] text-text-muted">+ {steps.length - 5} more steps</p>
              )}
            </div>
          )}
        </div>
        <div className="flex gap-2 flex-shrink-0">
          <button
            onClick={handleRunFix}
            disabled={starting}
            className="px-3 py-1 text-xs font-medium text-white bg-accent-green rounded-md hover:bg-accent-green/90 cursor-pointer disabled:opacity-50"
          >
            {starting ? "Starting..." : t("health.runFix")}
          </button>
          <button
            onClick={onDismiss}
            className="px-3 py-1 text-xs text-text-muted border border-border-primary rounded-md hover:bg-bg-tertiary cursor-pointer"
          >
            {t("health.dismiss")}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Fleet connection card ────────────────────────────────────────────

function FleetCard({ fleetStatus, setFleetStatus, t }: {
  fleetStatus: DashboardStatus | null;
  setFleetStatus: (s: DashboardStatus) => void;
  t: (key: string) => string;
}) {
  const [expanded, setExpanded] = useState(false);
  const [enrollUrl, setEnrollUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [linking, setLinking] = useState(false);

  const isLinked = fleetStatus?.linked === true;

  const handleLink = async () => {
    setLinking(true);
    setError(null);
    try {
      await commands.linkDashboard(enrollUrl);
      const status = await commands.getDashboardStatus();
      setFleetStatus(status);
      setEnrollUrl("");
      setExpanded(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    setLinking(false);
  };

  const handleUnlink = async () => {
    await commands.unlinkDashboard();
    setFleetStatus({ linked: false });
  };

  if (isLinked) {
    return (
      <div className="bg-bg-secondary border border-accent-green/30 rounded-xl p-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-green text-sm">{"\u2713"}</span>
          <div>
            <p className="text-sm text-text-primary font-medium">{fleetStatus?.fleet_name || t("health.fleetConnected")}</p>
            <p className="text-[10px] text-text-muted">{t("health.fleetSyncDesc")}</p>
          </div>
        </div>
        <button
          onClick={handleUnlink}
          className="text-xs text-text-muted hover:text-accent-red transition-colors cursor-pointer"
        >
          {t("health.disconnect")}
        </button>
      </div>
    );
  }

  if (!expanded) {
    return (
      <button
        onClick={() => setExpanded(true)}
        className="w-full bg-bg-secondary border border-border-primary rounded-xl p-4 text-left hover:border-accent-blue/30 transition-colors cursor-pointer"
      >
        <p className="text-sm text-text-primary font-medium">{t("health.fleetCta")}</p>
        <p className="text-xs text-text-muted mt-0.5">{t("health.fleetCtaDesc")}</p>
      </button>
    );
  }

  return (
    <div className="bg-bg-secondary border border-accent-blue/30 rounded-xl p-5 space-y-3">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm text-text-primary font-medium">{t("health.fleetConnect")}</p>
          <p className="text-xs text-text-muted mt-0.5">{t("health.fleetDataDisclosure")}</p>
        </div>
        <button onClick={() => setExpanded(false)} className="text-text-muted hover:text-text-primary text-lg leading-none cursor-pointer">&times;</button>
      </div>
      <input
        type="text"
        placeholder="https://your-dashboard.com/enroll/abc123..."
        value={enrollUrl}
        onChange={(e) => setEnrollUrl(e.target.value)}
        className="w-full px-3 py-2 text-sm bg-bg-primary border border-border-primary rounded-lg text-text-primary"
      />
      {error && <p className="text-xs text-accent-red">{error}</p>}
      <button
        onClick={handleLink}
        disabled={linking || !enrollUrl.trim()}
        className="px-4 py-2 text-sm font-medium text-white bg-accent-blue rounded-lg hover:bg-accent-blue/90 disabled:opacity-50 cursor-pointer"
      >
        {linking ? "..." : t("health.fleetLinkBtn")}
      </button>
    </div>
  );
}

// ── Main dashboard ──────────────────────────────────────────────────

export function HealthDashboard() {
  const { score, history, loading, error, setScore, setHistory, setLoading, setError } = useHealthStore();
  const { t } = useLocale();
  const [fleetStatus, setFleetStatus] = useState<DashboardStatus | null>(null);
  const [fleetActions, setFleetActions] = useState<commands.FleetAction[]>([]);
  const [autoHealActive, setAutoHealActive] = useState<{ check_id: string; playbook_slug: string } | null>(null);
  const [autoHealResult, setAutoHealResult] = useState<{ check_id: string; playbook_slug: string; success: boolean; score_before: number | null; score_after: number | null } | null>(null);
  const [autoHealAvailable, setAutoHealAvailable] = useState<{ check_id: string; playbook_slug: string; reason: string } | null>(null);

  useEffect(() => {
    commands.getDashboardStatus().then(setFleetStatus).catch(() => {});
  }, []);

  const loadFleetActions = useCallback(async () => {
    try {
      const actions = await commands.getFleetActions();
      setFleetActions(actions);
    } catch {}
  }, []);

  useEffect(() => {
    loadFleetActions();
    // Poll every 5 minutes
    const timer = setInterval(loadFleetActions, 5 * 60 * 1000);
    return () => clearInterval(timer);
  }, [loadFleetActions]);

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

  useEffect(() => {
    const unlistenStarted = listen<{ check_id: string; playbook_slug: string; reason: string }>("auto-heal-started", (e) => {
      setAutoHealActive(e.payload);
      setAutoHealResult(null);
    });
    const unlistenCompleted = listen<{ check_id: string; playbook_slug: string; success: boolean; score_before: number | null; score_after: number | null }>("auto-heal-completed", (e) => {
      setAutoHealActive(null);
      setAutoHealResult(e.payload);
      // Refresh health score after auto-heal
      loadScore();
      loadHistory();
      // Clear result after 30 seconds
      setTimeout(() => setAutoHealResult(null), 30000);
    });
    const unlistenAvailable = listen<{ check_id: string; playbook_slug: string; reason: string }>("auto-heal-available", (e) => {
      setAutoHealAvailable(e.payload);
    });
    return () => {
      unlistenStarted.then((fn) => fn());
      unlistenCompleted.then((fn) => fn());
      unlistenAvailable.then((fn) => fn());
    };
  }, [loadScore, loadHistory]);

  const handleRunCheck = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await commands.runHealthCheck();
      setScore(s);
      await loadHistory();
      loadFleetActions();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    setLoading(false);
  }, [setScore, setLoading, setError, loadHistory, loadFleetActions]);

  const handleExport = useCallback(async () => {
    try {
      const report = await commands.exportHealthReport();
      const blob = new Blob([report], { type: "text/plain" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const date = new Date().toISOString().slice(0, 10);
      a.download = `noah-health-report-${date}.txt`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("Export failed:", e);
    }
  }, []);

  const hasResults = score !== null && score.categories.length > 0;

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-8">
        {/* Header */}
        <div className="mb-6">
          <h1 className="text-xl font-semibold text-text-primary">{t("health.title")}</h1>
          <p className="text-sm text-text-muted mt-1">{t("health.subtitle")}</p>
        </div>

        {/* Auto-heal activity */}
        {autoHealActive && (
          <div className="mb-4 bg-accent-blue/10 border border-accent-blue/20 rounded-xl p-4 flex items-center gap-3">
            <div className="w-4 h-4 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
            <p className="text-sm text-text-primary">
              {t("health.autoHealInProgress", { issue: autoHealActive.playbook_slug.replace(/-/g, " ") })}
            </p>
          </div>
        )}

        {autoHealResult && (
          <div className={`mb-4 rounded-xl p-4 flex items-center gap-3 ${
            autoHealResult.success
              ? "bg-accent-green/10 border border-accent-green/20"
              : "bg-accent-red/10 border border-accent-red/20"
          }`}>
            <span className={autoHealResult.success ? "text-accent-green" : "text-accent-red"}>
              {autoHealResult.success ? "\u2713" : "\u2717"}
            </span>
            <p className="text-sm text-text-primary">
              {autoHealResult.success
                ? t("health.autoHealComplete", {
                    before: autoHealResult.score_before ?? "?",
                    after: autoHealResult.score_after ?? "?",
                  })
                : t("health.autoHealFailed")}
            </p>
          </div>
        )}

        {autoHealAvailable && (
          <div className="mb-4 bg-accent-yellow/10 border border-accent-yellow/20 rounded-xl p-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">{t("health.autoHealAvailable")}</p>
                <p className="text-xs text-text-muted mt-0.5">{autoHealAvailable.reason}</p>
              </div>
              <button
                onClick={() => {
                  // Navigate to settings to enable auto-heal
                  setAutoHealAvailable(null);
                }}
                className="px-3 py-1.5 text-xs font-medium text-white bg-accent-green rounded-lg hover:bg-accent-green/90 cursor-pointer"
              >
                {t("health.enableAutoHeal")}
              </button>
            </div>
          </div>
        )}

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
            <div className="flex items-center gap-2">
              <button
                onClick={handleRunCheck}
                disabled={loading}
                className="px-4 py-2 rounded-lg bg-accent-blue text-white text-sm font-medium hover:bg-accent-blue/90 transition-colors disabled:opacity-50 cursor-pointer"
              >
                {loading ? t("health.running") : hasResults ? t("health.runAgain") : t("health.runCheck")}
              </button>
              {hasResults && (
                <button
                  onClick={handleExport}
                  className="px-3 py-2 rounded-lg border border-border-primary text-text-secondary text-sm hover:bg-bg-tertiary transition-colors cursor-pointer"
                  title={t("health.exportReport")}
                >
                  {t("health.export")}
                </button>
              )}
            </div>
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

        {/* Fleet connection */}
        <div className="mt-6">
          <FleetCard fleetStatus={fleetStatus} setFleetStatus={setFleetStatus} t={t} />
        </div>

        {/* Fleet admin requests */}
        {fleetActions.length > 0 && (
          <div className="mt-4 space-y-2">
            {fleetActions.map((action) =>
              action.action_type === "playbook" && action.playbook_slug ? (
                <PlaybookPreviewCard
                  key={action.id}
                  action={action}
                  t={t}
                  onDismiss={async () => {
                    await commands.resolveFleetAction(action.id, "dismissed");
                    setFleetActions((prev) => prev.filter((a) => a.id !== action.id));
                  }}
                  onRemove={() => setFleetActions((prev) => prev.filter((a) => a.id !== action.id))}
                />
              ) : (
                <div key={action.id} className="bg-accent-blue/5 border border-accent-blue/20 rounded-xl p-4">
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1">
                      <p className="text-xs text-accent-blue font-medium mb-0.5">{t("health.adminRequest")}</p>
                      <p className="text-sm text-text-primary font-medium">{action.check_label}</p>
                      <p className="text-xs text-text-muted mt-0.5">{action.action_hint}</p>
                    </div>
                    <div className="flex gap-2 flex-shrink-0">
                      <button
                        onClick={async () => {
                          commands.openHealthFix(action.check_id).catch(() => {});
                          await commands.resolveFleetAction(action.id, "completed");
                          setFleetActions((prev) => prev.filter((a) => a.id !== action.id));
                        }}
                        className="px-3 py-1 text-xs font-medium text-white bg-accent-blue rounded-md hover:bg-accent-blue/90 cursor-pointer"
                      >
                        {t("health.fix")}
                      </button>
                      <button
                        onClick={async () => {
                          await commands.resolveFleetAction(action.id, "dismissed");
                          setFleetActions((prev) => prev.filter((a) => a.id !== action.id));
                        }}
                        className="px-3 py-1 text-xs text-text-muted border border-border-primary rounded-md hover:bg-bg-tertiary cursor-pointer"
                      >
                        {t("health.dismiss")}
                      </button>
                    </div>
                  </div>
                </div>
              ),
            )}
          </div>
        )}

        {/* Footer */}
        <p className="text-[10px] text-text-muted text-center mt-8">{t("health.footer")}</p>
      </div>
    </div>
  );
}
