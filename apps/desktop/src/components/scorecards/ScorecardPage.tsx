import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useHealthStore } from "../../stores/healthStore";
import { useLocale } from "../../i18n";
import * as commands from "../../lib/tauri-commands";
import type { DashboardStatus, FleetAction } from "../../lib/tauri-commands";
import { SummaryStrip } from "./SummaryStrip";
import { ScorecardCard } from "./ScorecardCard";
import { FleetActionCards } from "./FleetActionCards";
import { FleetConnectionCard } from "./FleetConnectionCard";
import { DiskScanCard } from "./DiskScanCard";

export function ScorecardPage() {
  const { score, history, loading, error, setScore, setHistory, setLoading, setError } = useHealthStore();
  const { t } = useLocale();
  const [fleetStatus, setFleetStatus] = useState<DashboardStatus | null>(null);
  const [fleetActions, setFleetActions] = useState<FleetAction[]>([]);
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
      loadScore();
      loadHistory();
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

        {/* Fleet action cards (urgent, at top) */}
        <div className="mb-4">
          <FleetActionCards actions={fleetActions} setActions={setFleetActions} t={t} />
        </div>

        {/* Auto-heal banners */}
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
                onClick={() => setAutoHealAvailable(null)}
                className="px-3 py-1.5 text-xs font-medium text-white bg-accent-green rounded-lg hover:bg-accent-green/90 cursor-pointer"
              >
                {t("health.enableAutoHeal")}
              </button>
            </div>
          </div>
        )}

        {/* Summary strip */}
        <div className="mb-6">
          <SummaryStrip
            score={score}
            history={history}
            loading={loading}
            error={error}
            onRunCheck={handleRunCheck}
            onExport={handleExport}
            t={t}
          />
        </div>

        {/* Scorecards */}
        {hasResults ? (
          <div className="space-y-4">
            {score.categories.map((cat) => (
              <ScorecardCard key={cat.category} cat={cat} t={t} />
            ))}
          </div>
        ) : (
          /* No-data skeleton cards */
          <div className="space-y-4">
            {["Security", "Updates", "Performance", "Backups", "Network"].map((name) => (
              <div key={name} className="bg-bg-secondary border border-border-primary border-l-4 border-l-border-primary rounded-xl p-5">
                <div className="flex items-center justify-between">
                  <h3 className="text-sm font-semibold text-text-muted">{name}</h3>
                  <div className="w-16 h-5 bg-bg-tertiary rounded-md animate-pulse" />
                </div>
                <div className="mt-3 space-y-2">
                  <div className="w-3/4 h-3 bg-bg-tertiary rounded animate-pulse" />
                  <div className="w-1/2 h-3 bg-bg-tertiary rounded animate-pulse" />
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Disk scan card */}
        <div className="mt-6">
          <DiskScanCard t={t} />
        </div>

        {/* Fleet connection */}
        <div className="mt-6">
          <FleetConnectionCard fleetStatus={fleetStatus} setFleetStatus={setFleetStatus} t={t} />
        </div>

        {/* Footer */}
        <p className="text-[10px] text-text-muted text-center mt-8">{t("health.footer")}</p>
      </div>
    </div>
  );
}
