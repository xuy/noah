import { statusIcon, statusColor, statusLabel, actionInfo } from "./shared";
import * as commands from "../../lib/tauri-commands";
import type { CheckResult } from "../../lib/tauri-commands";

export function CheckRow({ check, t }: { check: CheckResult; t: (key: string) => string }) {
  const action = actionInfo(check);
  return (
    <div className="flex items-center gap-3 py-2.5">
      <div className="flex-shrink-0 text-base">
        <span className={statusColor(check.status)}>{statusIcon(check.status)}</span>
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <p className="text-sm text-text-primary font-medium">{check.label}</p>
          <span className={`text-[10px] font-medium ${statusColor(check.status)}`}>
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
