import { useState } from "react";
import { useSessionStore } from "../../stores/sessionStore";
import { useChatStore } from "../../stores/chatStore";
import * as commands from "../../lib/tauri-commands";
import type { FleetAction } from "../../lib/tauri-commands";

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

      clearMessages();
      setSession(result.session_id);
      setActiveView("chat");

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

function HintActionCard({ action, t, onDismiss }: {
  action: FleetAction;
  t: (key: string) => string;
  onDismiss: () => void;
}) {
  return (
    <div className="bg-accent-blue/5 border border-accent-blue/20 rounded-xl p-4">
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
              onDismiss();
            }}
            className="px-3 py-1 text-xs font-medium text-white bg-accent-blue rounded-md hover:bg-accent-blue/90 cursor-pointer"
          >
            {t("health.fix")}
          </button>
          <button
            onClick={async () => {
              await commands.resolveFleetAction(action.id, "dismissed");
              onDismiss();
            }}
            className="px-3 py-1 text-xs text-text-muted border border-border-primary rounded-md hover:bg-bg-tertiary cursor-pointer"
          >
            {t("health.dismiss")}
          </button>
        </div>
      </div>
    </div>
  );
}

interface FleetActionCardsProps {
  actions: FleetAction[];
  setActions: React.Dispatch<React.SetStateAction<FleetAction[]>>;
  t: (key: string, params?: Record<string, string | number>) => string;
}

export function FleetActionCards({ actions, setActions, t }: FleetActionCardsProps) {
  if (actions.length === 0) return null;

  return (
    <div className="space-y-2">
      {actions.map((action) =>
        action.action_type === "playbook" && action.playbook_slug ? (
          <PlaybookPreviewCard
            key={action.id}
            action={action}
            t={t}
            onDismiss={async () => {
              await commands.resolveFleetAction(action.id, "dismissed");
              setActions((prev) => prev.filter((a) => a.id !== action.id));
            }}
            onRemove={() => setActions((prev) => prev.filter((a) => a.id !== action.id))}
          />
        ) : (
          <HintActionCard
            key={action.id}
            action={action}
            t={t}
            onDismiss={() => setActions((prev) => prev.filter((a) => a.id !== action.id))}
          />
        ),
      )}
    </div>
  );
}
