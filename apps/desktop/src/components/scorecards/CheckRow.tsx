import { statusIcon, statusColor, actionInfo } from "./shared";
import * as commands from "../../lib/tauri-commands";
import type { CheckResult } from "../../lib/tauri-commands";

export function CheckRow({ check, t, onAskNoah }: {
  check: CheckResult;
  t: (key: string) => string;
  onAskNoah?: (message: string) => void;
}) {
  const action = actionInfo(check);
  return (
    <div className="flex items-center gap-3 py-2.5">
      <span className={`flex-shrink-0 text-sm ${statusColor(check.status)}`}>{statusIcon(check.status)}</span>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-text-primary">{check.label}</p>
        {action && (
          <p className="text-xs text-text-muted mt-0.5">{action.hint}</p>
        )}
      </div>
      {action?.noahCanFix && onAskNoah ? (
        <button
          onClick={() => onAskNoah(action.noahCanFix!)}
          className="flex-shrink-0 px-2.5 py-1 text-xs font-medium text-accent-blue hover:text-accent-blue/80 transition-colors cursor-pointer"
        >
          {t("health.fix")}
        </button>
      ) : action?.canOpen ? (
        <button
          onClick={() => {
            commands.openHealthFix(check.id).catch((err) => {
              console.error("Failed to open settings:", err);
            });
          }}
          className="flex-shrink-0 px-2.5 py-1 text-xs text-accent-blue hover:text-accent-blue/80 transition-colors cursor-pointer"
        >
          {t("health.openSettings")}
        </button>
      ) : null}
    </div>
  );
}
