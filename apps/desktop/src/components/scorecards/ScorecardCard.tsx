import { useState } from "react";
import { gradeColor, gradeBg, gradeBorder } from "./shared";
import { CheckRow } from "./CheckRow";
import type { CategoryScore } from "../../lib/tauri-commands";

export function ScorecardCard({ cat, t }: { cat: CategoryScore; t: (key: string, params?: Record<string, string | number>) => string }) {
  const failing = cat.checks.filter((c) => c.status !== "pass");
  const passing = cat.checks.filter((c) => c.status === "pass");
  const allPassing = failing.length === 0;
  const [showPassing, setShowPassing] = useState(false);

  return (
    <div className={`bg-bg-secondary border border-border-primary border-l-4 ${gradeBorder(cat.grade)} rounded-xl p-5`}>
      <div className="flex items-center justify-between mb-1">
        <h3 className="text-sm font-semibold text-text-primary capitalize">{cat.category}</h3>
        <div className={`flex items-center gap-1.5 px-2 py-0.5 rounded-md ${gradeBg(cat.grade)}`}>
          <span className={`text-sm font-bold ${gradeColor(cat.grade)}`}>{cat.grade}</span>
          <span className="text-xs text-text-muted">{cat.score}/100</span>
        </div>
      </div>

      {allPassing ? (
        <div className="flex items-center gap-2 py-2.5">
          <span className="text-accent-green">{"\u2713"}</span>
          <span className="text-sm text-accent-green font-medium">{t("health.allPassing")}</span>
        </div>
      ) : (
        <>
          <div className="divide-y divide-border-primary">
            {failing.map((check) => (
              <CheckRow key={check.id} check={check} t={t} />
            ))}
          </div>
          {passing.length > 0 && (
            <>
              <button
                onClick={() => setShowPassing(!showPassing)}
                className="text-xs text-text-muted hover:text-text-secondary mt-2 cursor-pointer"
              >
                {t("health.passingCount", { count: passing.length })} {showPassing ? "\u25B4" : "\u25BE"}
              </button>
              {showPassing && (
                <div className="divide-y divide-border-primary mt-1">
                  {passing.map((check) => (
                    <CheckRow key={check.id} check={check} t={t} />
                  ))}
                </div>
              )}
            </>
          )}
        </>
      )}
    </div>
  );
}
