import { gradeColor, timeAgo } from "./shared";
import type { HealthScore } from "../../lib/tauri-commands";

type CheckState = "idle" | "checking" | "done";

interface SummaryStripProps {
  score: HealthScore | null;
  history: HealthScore[];
  checkState: CheckState;
  error: string | null;
  onRunCheck: () => void;
  onExport: () => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

export function SummaryStrip({ score, checkState, error, onRunCheck, onExport, t }: SummaryStripProps) {
  const hasResults = score !== null && score.categories.length > 0;

  if (!hasResults) {
    return (
      <div className="py-6">
        <span className="text-4xl font-bold text-text-muted tracking-tight">--</span>
        <p className="text-sm text-text-muted mt-1 mb-5">{t("health.runCheckDesc")}</p>
        <button
          onClick={onRunCheck}
          disabled={checkState === "checking"}
          className="px-4 py-2 rounded-lg bg-accent-blue text-white text-sm font-medium hover:bg-accent-blue/90 transition-colors disabled:opacity-50 cursor-pointer"
        >
          {checkState === "checking" ? t("health.running") : t("health.runCheck")}
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

  const buttonLabel = checkState === "checking" ? t("health.running")
    : checkState === "done" ? t("health.done")
    : t("health.runAgain");

  return (
    <div className="py-4">
      {/* Score + actions */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-baseline gap-2">
          <span className={`text-4xl font-bold tracking-tight transition-colors duration-500 ${checkState === "done" ? "text-accent-blue" : "text-text-primary"}`}>
            {score.overall_score}
          </span>
          <span className={`text-2xl font-bold ${gradeColor(score.overall_grade)}`}>{score.overall_grade}</span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={onRunCheck}
            disabled={checkState === "checking"}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium border transition-all active:scale-95 cursor-pointer disabled:cursor-default ${
              checkState === "done"
                ? "border-accent-green/30 text-accent-green"
                : "border-border-primary text-text-secondary hover:bg-bg-tertiary disabled:opacity-50"
            }`}
          >
            {checkState === "checking" && (
              <div className="w-3 h-3 border-[1.5px] border-text-muted border-t-transparent rounded-full animate-spin" />
            )}
            {checkState === "done" && <span className="text-sm leading-none">{"\u2713"}</span>}
            {buttonLabel}
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

      {/* Stats */}
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
