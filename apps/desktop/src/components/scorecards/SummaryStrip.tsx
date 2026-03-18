import { gradeColor, gradeBg, gradeRing, timeAgo } from "./shared";
import type { HealthScore } from "../../lib/tauri-commands";

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

interface SummaryStripProps {
  score: HealthScore | null;
  history: HealthScore[];
  loading: boolean;
  error: string | null;
  onRunCheck: () => void;
  onExport: () => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

export function SummaryStrip({ score, history, loading, error, onRunCheck, onExport, t }: SummaryStripProps) {
  const hasResults = score !== null && score.categories.length > 0;

  if (!hasResults) {
    return (
      <div className="bg-bg-secondary border border-border-primary rounded-xl p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex items-center justify-center w-10 h-10 rounded-full border-2 border-border-primary bg-bg-tertiary">
              <span className="text-sm text-text-muted font-bold">--</span>
            </div>
            <div>
              <p className="text-sm text-text-primary font-medium">{t("health.title")}</p>
              <p className="text-xs text-text-muted">{t("health.runCheckDesc")}</p>
            </div>
          </div>
          <button
            onClick={onRunCheck}
            disabled={loading}
            className="px-4 py-2 rounded-lg bg-accent-blue text-white text-sm font-medium hover:bg-accent-blue/90 transition-colors disabled:opacity-50 cursor-pointer"
          >
            {loading ? t("health.running") : t("health.runCheck")}
          </button>
        </div>
        {error && <p className="text-xs text-accent-red mt-2">{error}</p>}
      </div>
    );
  }

  const total = score.categories.reduce((n, c) => n + c.checks.length, 0);
  const passed = score.categories.reduce(
    (n, c) => n + c.checks.filter((ch) => ch.status === "pass").length, 0,
  );
  const failed = total - passed;

  return (
    <div className="bg-bg-secondary border border-border-primary rounded-xl p-5">
      <div className="flex items-center gap-5">
        {/* Score + grade badge — mirrors fleet dashboard layout */}
        <div className="flex items-center gap-4 flex-shrink-0">
          <div>
            <span className="text-3xl font-bold text-text-primary">{score.overall_score}</span>
            <span className="text-base text-text-muted font-medium">/100</span>
          </div>
          <div className={`flex items-center justify-center w-10 h-10 rounded-full border-2 ${gradeRing(score.overall_grade)} ${gradeBg(score.overall_grade)}`}>
            <span className={`text-lg font-bold ${gradeColor(score.overall_grade)}`}>{score.overall_grade}</span>
          </div>
        </div>

        {/* Stats + sparkline */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-3 text-xs text-text-muted">
            <span className="text-accent-green">{t("health.passed", { count: passed })}</span>
            {failed > 0 && (
              <span className="text-accent-red">{t("health.needsAttention", { count: failed })}</span>
            )}
            <span className="text-text-muted">&middot;</span>
            <span className="text-text-muted whitespace-nowrap">
              {t("health.lastChecked", { time: timeAgo(score.computed_at, t) })}
            </span>
          </div>
          {history.length >= 2 && (
            <div className="mt-2">
              <Sparkline history={history.map((h) => ({ score: h.overall_score }))} />
            </div>
          )}
        </div>

        {/* Buttons */}
        <div className="flex items-center gap-2 flex-shrink-0">
          <button
            onClick={onRunCheck}
            disabled={loading}
            className="px-3 py-1.5 rounded-lg bg-accent-blue text-white text-xs font-medium hover:bg-accent-blue/90 transition-colors disabled:opacity-50 cursor-pointer"
          >
            {loading ? t("health.running") : t("health.runAgain")}
          </button>
          <button
            onClick={onExport}
            className="px-2.5 py-1.5 rounded-lg border border-border-primary text-text-secondary text-xs hover:bg-bg-tertiary transition-colors cursor-pointer"
            title={t("health.exportReport")}
          >
            {t("health.export")}
          </button>
        </div>
      </div>
      {error && <p className="text-xs text-accent-red mt-2">{error}</p>}
    </div>
  );
}
