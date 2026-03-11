import { useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";
import type { ApprovalRequest } from "../lib/tauri-commands";
import { useLocale } from "../i18n";

export function ActionApproval() {
  const { t } = useLocale();
  const pendingApproval = useSessionStore((s) => s.pendingApproval);
  const setPendingApproval = useSessionStore((s) => s.setPendingApproval);
  const autoConfirm = useSessionStore((s) => s.autoConfirm);
  const setAutoConfirm = useSessionStore((s) => s.setAutoConfirm);
  const addMessage = useChatStore((s) => s.addMessage);

  // Listen for approval requests from the Tauri backend
  useEffect(() => {
    const unlisten = listen<ApprovalRequest>(
      "approval-request",
      (event) => {
        setPendingApproval(event.payload);
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setPendingApproval]);

  const handleApprove = useCallback(async (dontAskAgain?: boolean) => {
    if (!pendingApproval) return;
    if (dontAskAgain) {
      setAutoConfirm(true);
    }
    try {
      await commands.approveAction(pendingApproval.approval_id);
      addMessage({
        role: "system",
        content: `Approved: ${pendingApproval.reason || "Action approved"}`,
      });
    } catch (err) {
      console.error("Failed to approve action:", err);
    } finally {
      setPendingApproval(null);
    }
  }, [pendingApproval, setPendingApproval, setAutoConfirm, addMessage]);

  const handleDeny = useCallback(async () => {
    if (!pendingApproval) return;
    try {
      await commands.denyAction(pendingApproval.approval_id);
      addMessage({
        role: "system",
        content: `Skipped: ${pendingApproval.reason || "Action skipped"}`,
      });
    } catch (err) {
      console.error("Failed to deny action:", err);
    } finally {
      setPendingApproval(null);
    }
  }, [pendingApproval, setPendingApproval, addMessage]);

  // Auto-approve when "don't ask again" is active.
  useEffect(() => {
    if (pendingApproval && autoConfirm) {
      handleApprove();
    }
  }, [pendingApproval, autoConfirm, handleApprove]);

  // Handle Escape key to deny
  useEffect(() => {
    if (!pendingApproval) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleDeny();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [pendingApproval, handleDeny]);

  // Don't show modal when auto-approving
  if (!pendingApproval || autoConfirm) return null;

  const reason = pendingApproval.reason || t("approval.defaultReason");

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay backdrop-blur-sm animate-fade-in">
      <div className="bg-bg-secondary border border-border-primary rounded-2xl shadow-2xl w-full max-w-sm mx-4 overflow-hidden">
        {/* Header */}
        <div className="px-6 pt-5 pb-2">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-accent-amber/15 flex items-center justify-center flex-shrink-0">
              <svg
                width="20"
                height="20"
                viewBox="0 0 20 20"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M10 2L1 18H19L10 2Z"
                  stroke="#f59e0b"
                  strokeWidth="1.5"
                  fill="none"
                />
                <path d="M9 8H11V12H9V8ZM9 14H11V16H9V14Z" fill="#f59e0b" />
              </svg>
            </div>
            <h2 className="text-sm font-semibold text-text-primary">
              {t("approval.title")}
            </h2>
          </div>
        </div>

        {/* Reason — the main content */}
        <div className="px-6 py-4">
          <p className="text-sm text-text-primary leading-relaxed">
            {reason}
          </p>
        </div>

        {/* Actions — three buttons */}
        <div className="px-6 py-4 flex items-center justify-end gap-2 border-t border-border-primary">
          <button
            onClick={handleDeny}
            className="px-4 py-2 rounded-lg text-sm text-text-secondary bg-bg-tertiary hover:bg-bg-tertiary/80 transition-colors cursor-pointer"
          >
            {t("approval.deny")}
          </button>
          <button
            onClick={() => handleApprove()}
            className="px-4 py-2 rounded-lg text-sm text-white bg-accent-green hover:bg-accent-green/80 transition-colors cursor-pointer"
          >
            {t("approval.approve")}
          </button>
          <button
            onClick={() => handleApprove(true)}
            className="px-4 py-2 rounded-lg text-sm text-accent-green border border-accent-green/40 hover:bg-accent-green/10 transition-colors cursor-pointer"
          >
            {t("approval.approveAll")}
          </button>
        </div>
      </div>
    </div>
  );
}
