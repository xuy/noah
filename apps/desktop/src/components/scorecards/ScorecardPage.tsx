import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useHealthStore } from "../../stores/healthStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useChatStore } from "../../stores/chatStore";
import { useLocale } from "../../i18n";
import * as commands from "../../lib/tauri-commands";
import type { DashboardStatus, FleetAction } from "../../lib/tauri-commands";
import { currentLocale } from "../../i18n";
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

  const setActiveView = useSessionStore((s) => s.setActiveView);
  const setSession = useSessionStore((s) => s.setSession);
  const prependSession = useSessionStore((s) => s.prependSession);
  const setProcessingSession = useSessionStore((s) => s.setProcessingSession);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const addMessage = useChatStore((s) => s.addMessage);

  const handleAskNoah = useCallback(async (message: string) => {
    try {
      // Create a fresh session (mirrors useSession.createSession logic)
      const session = await commands.createSession();
      clearMessages();
      setSession(session.id);
      prependSession({
        id: session.id,
        created_at: session.created_at,
        ended_at: null,
        title: null,
        message_count: 0,
        change_count: 0,
        resolved: null,
      });
      commands.setLocale(session.id, currentLocale()).catch(() => {});

      // Add greeting + user message to chat
      addMessage({ role: "system", content: "Hey! I'm Noah, your computer helper. Just tell me what's going on and I'll take care of it." });
      addMessage({ role: "user", content: message });

      // Switch to chat view
      setActiveView("chat");
      setProcessingSession(session.id);

      // Send the message
      try {
        const result = await commands.sendMessageV2(session.id, message);
        addMessage({
          role: "assistant",
          content: result.text,
          assistantUi: result.assistant_ui,
        });
      } catch (err) {
        console.error("Failed to send message:", err);
        addMessage({ role: "system", content: "Something went wrong. Please try again." });
      }
      setProcessingSession(null);
    } catch (err) {
      console.error("Failed to start session:", err);
    }
  }, [clearMessages, setSession, setActiveView, prependSession, setProcessingSession, addMessage]);

  const hasResults = score !== null && score.categories.length > 0;

  // Split categories: failing cards get full treatment, passing ones are compact rows
  const failingCats = hasResults ? score.categories.filter((c) => c.checks.some((ch) => ch.status !== "pass")) : [];
  const passingCats = hasResults ? score.categories.filter((c) => c.checks.every((ch) => ch.status === "pass")) : [];

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-8">

        {/* Fleet action cards (urgent, at top) */}
        {fleetActions.length > 0 && (
          <div className="mb-6">
            <FleetActionCards actions={fleetActions} setActions={setFleetActions} t={t} />
          </div>
        )}

        {/* Auto-heal banners */}
        {autoHealActive && (
          <div className="mb-4 bg-accent-blue/8 border border-accent-blue/15 rounded-xl p-4 flex items-center gap-3">
            <div className="w-3.5 h-3.5 border-2 border-accent-blue border-t-transparent rounded-full animate-spin" />
            <p className="text-sm text-text-primary">
              {t("health.autoHealInProgress", { issue: autoHealActive.playbook_slug.replace(/-/g, " ") })}
            </p>
          </div>
        )}

        {autoHealResult && (
          <div className={`mb-4 rounded-xl p-4 flex items-center gap-3 ${
            autoHealResult.success
              ? "bg-accent-green/8 border border-accent-green/15"
              : "bg-accent-red/8 border border-accent-red/15"
          }`}>
            <span className={autoHealResult.success ? "text-accent-green text-sm" : "text-accent-red text-sm"}>
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
          <div className="mb-4 bg-accent-yellow/8 border border-accent-yellow/15 rounded-xl p-4">
            <div className="flex items-start gap-3">
              <div className="flex-1 min-w-0">
                <p className="text-sm text-text-primary font-medium">{t("health.autoHealAvailable")}</p>
                <p className="text-xs text-text-muted mt-0.5 line-clamp-2">{autoHealAvailable.reason}</p>
              </div>
              <button
                onClick={() => setAutoHealAvailable(null)}
                className="flex-shrink-0 px-3 py-1.5 text-xs font-medium text-white bg-accent-green rounded-lg hover:bg-accent-green/90 cursor-pointer whitespace-nowrap"
              >
                {t("health.enableAutoHeal")}
              </button>
            </div>
          </div>
        )}

        {/* Score summary */}
        <SummaryStrip
          score={score}
          history={history}
          loading={loading}
          error={error}
          onRunCheck={handleRunCheck}
          onExport={handleExport}
          t={t}
        />

        {/* Divider */}
        <div className="border-t border-border-primary mb-4" />

        {hasResults ? (
          <>
            {/* Failing categories: full card treatment */}
            {failingCats.length > 0 && (
              <div className="space-y-3 mb-2">
                {failingCats.map((cat) => (
                  <ScorecardCard key={cat.category} cat={cat} t={t} onAskNoah={handleAskNoah} />
                ))}
              </div>
            )}

            {/* Passing categories: compact rows */}
            {passingCats.length > 0 && (
              <div className="divide-y divide-border-primary/50">
                {passingCats.map((cat) => (
                  <ScorecardCard key={cat.category} cat={cat} t={t} onAskNoah={handleAskNoah} />
                ))}
              </div>
            )}

            {/* Disk scan — matches compact row style */}
            <div className="border-t border-border-primary/50">
              <DiskScanCard t={t} />
            </div>
          </>
        ) : (
          /* No-data skeleton */
          <div className="space-y-3">
            {["Security", "Updates", "Performance", "Backups", "Network"].map((name) => (
              <div key={name} className="flex items-center justify-between py-3 px-1">
                <span className="text-sm text-text-muted font-medium">{name}</span>
                <div className="w-8 h-4 bg-bg-tertiary rounded animate-pulse" />
              </div>
            ))}
          </div>
        )}

        {/* Fleet connection */}
        <div className="mt-8">
          <FleetConnectionCard fleetStatus={fleetStatus} setFleetStatus={setFleetStatus} t={t} />
        </div>

        {/* Footer */}
        <p className="text-[10px] text-text-muted text-center mt-8">{t("health.footer")}</p>
      </div>
    </div>
  );
}
