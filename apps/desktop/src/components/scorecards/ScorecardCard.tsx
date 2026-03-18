import { useState } from "react";
import { gradeColor } from "./shared";
import { CheckRow } from "./CheckRow";
import type { CategoryScore } from "../../lib/tauri-commands";

export function ScorecardCard({ cat, t, onAskNoah }: { cat: CategoryScore; t: (key: string, params?: Record<string, string | number>) => string; onAskNoah?: (message: string) => void }) {
  const failing = cat.checks.filter((c) => c.status !== "pass");
  const passing = cat.checks.filter((c) => c.status === "pass");
  const allPassing = failing.length === 0;
  const [showPassing, setShowPassing] = useState(false);

  // All-passing: compact inline row, no card chrome
  if (allPassing) {
    return (
      <div className="flex items-center justify-between py-3 px-1">
        <div className="flex items-center gap-2.5">
          <span className="text-accent-green text-sm">{"\u2713"}</span>
          <span className="text-sm text-text-primary font-medium capitalize">{cat.category}</span>
        </div>
        <span className={`text-sm font-semibold ${gradeColor(cat.grade)}`}>{cat.grade}</span>
      </div>
    );
  }

  // Has failures: full card treatment
  return (
    <div className="bg-bg-secondary border border-border-primary rounded-xl p-4">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-text-primary capitalize">{cat.category}</h3>
        <span className={`text-sm font-bold ${gradeColor(cat.grade)}`}>{cat.grade}</span>
      </div>

      <div className="divide-y divide-border-primary">
        {failing.map((check) => (
          <CheckRow key={check.id} check={check} t={t} onAskNoah={onAskNoah} />
        ))}
      </div>

      {passing.length > 0 && (
        <button
          onClick={() => setShowPassing(!showPassing)}
          className="text-xs text-text-muted hover:text-text-secondary mt-3 cursor-pointer"
        >
          {t("health.passingCount", { count: passing.length })} {showPassing ? "\u25B4" : "\u25BE"}
        </button>
      )}
      {showPassing && (
        <div className="divide-y divide-border-primary mt-1">
          {passing.map((check) => (
            <CheckRow key={check.id} check={check} t={t} onAskNoah={onAskNoah} />
          ))}
        </div>
      )}
    </div>
  );
}
