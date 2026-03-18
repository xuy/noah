import { gradeColor, timeAgo } from "./shared";
import type { HealthScore } from "../../lib/tauri-commands";

interface SummaryStripProps {
  score: HealthScore | null;
  history: HealthScore[];
  loading: boolean;
  error: string | null;
  onRunCheck: () => void;
  onExport: () => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

export function SummaryStrip({ score, loading, error, onRunCheck, onExport, t }: SummaryStripProps) {
  const hasResults = score !== null && score.categories.length > 0;

  if (!hasResults) {
    return (
      <div className="py-6">
        <div className="flex items-baseline gap-3 mb-1">
          <span className="text-4xl font-bold text-text-muted tracking-tight">--</span>
        </div>
        <p className="text-sm text-text-muted mb-5">{t("health.runCheckDesc")}</p>
        <button
          onClick={onRunCheck}
          disabled={loading}
          className="px-4 py-2 rounded-lg bg-accent-blue text-white text-sm font-medium hover:bg-accent-blue/90 transition-colors disabled:opacity-50 cursor-pointer"
        >
          {loading ? t("health.running") : t("health.runCheck")}
        </button>
        {error && <p className="text-xs text-accent-red mt-3">{error}</p>}
      </div>
    );
  }

  const total = score.categories.reduce((n, c) => n + c.checks.length, 0);
  const passed = score.categories.reduce(
    (n, c) => n + c.checks.filter((ch) => ch.status === "pass").length, 0,
  );
  const failed = total - passed;

  return (
    <div className="py-4">
      {/* Hero: score + grade */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-baseline gap-2">
          <span className="text-4xl font-bold text-text-primary tracking-tight">{score.overall_score}</span>
          <span className={`text-2xl font-bold ${gradeColor(score.overall_grade)}`}>{score.overall_grade}</span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={onRunCheck}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-text-secondary text-xs font-medium border border-border-primary hover:bg-bg-tertiary active:bg-bg-tertiary active:scale-95 transition-all disabled:opacity-50 cursor-pointer"
          >
            {loading && (
              <div className="w-3 h-3 border-[1.5px] border-text-muted border-t-transparent rounded-full animate-spin" />
            )}
            {loading ? t("health.running") : t("health.runAgain")}
          </button>
          <button
            onClick={onExport}
            className="px-3 py-1.5 rounded-lg text-text-muted text-xs border border-border-primary hover:bg-bg-tertiary hover:text-text-secondary active:scale-95 transition-all cursor-pointer"
            title={t("health.exportReport")}
          >
            {t("health.export")}
          </button>
        </div>
      </div>

      {/* Subtitle: stats */}
      <p className="text-xs text-text-muted">
        <span className="text-accent-green">{t("health.passed", { count: passed })}</span>
        {failed > 0 && (
          <span className="text-accent-red"> &middot; {t("health.needsAttention", { count: failed })}</span>
        )}
        <span> &middot; {t("health.lastChecked", { time: timeAgo(score.computed_at, t) })}</span>
      </p>
      {error && <p className="text-xs text-accent-red mt-2">{error}</p>}
    </div>
  );
}
