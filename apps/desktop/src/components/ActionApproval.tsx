import { useEffect, useCallback } from "react";
import { useSessionStore } from "../stores/sessionStore";
import { useChatStore } from "../stores/chatStore";
import * as commands from "../lib/tauri-commands";
import type { ApprovalRequest } from "../lib/tauri-commands";

// Try to import Tauri event listener; in non-Tauri environments this won't work
let listenFn: typeof import("@tauri-apps/api/event").listen | null = null;
try {
  // Dynamic import handled at top-level for the listener setup
  import("@tauri-apps/api/event").then((mod) => {
    listenFn = mod.listen;
  });
} catch {
  // Not in a Tauri environment
}

export function ActionApproval() {
  const pendingApproval = useSessionStore((s) => s.pendingApproval);
  const setPendingApproval = useSessionStore((s) => s.setPendingApproval);
  const addMessage = useChatStore((s) => s.addMessage);

  // Listen for approval requests from the Tauri backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      if (!listenFn) {
        try {
          const { listen } = await import("@tauri-apps/api/event");
          listenFn = listen;
        } catch {
          return;
        }
      }

      unlisten = await listenFn<ApprovalRequest>(
        "approval-request",
        (event) => {
          setPendingApproval(event.payload);
        },
      );
    };

    setup();

    return () => {
      if (unlisten) unlisten();
    };
  }, [setPendingApproval]);

  const handleApprove = useCallback(async () => {
    if (!pendingApproval) return;
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
  }, [pendingApproval, setPendingApproval, addMessage]);

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

  // Handle Escape key to deny
  useEffect(() => {
    if (!pendingApproval) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleDeny();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [pendingApproval, handleDeny]);

  if (!pendingApproval) return null;

  const reason = pendingApproval.reason || "Noah needs your OK to continue.";

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
              Can I do this?
            </h2>
          </div>
        </div>

        {/* Reason — the main content */}
        <div className="px-6 py-4">
          <p className="text-sm text-text-primary leading-relaxed">
            {reason}
          </p>
        </div>

        {/* Actions */}
        <div className="px-6 py-4 flex items-center justify-end gap-3 border-t border-border-primary">
          <button
            onClick={handleDeny}
            className="px-5 py-2 rounded-lg text-sm text-text-secondary bg-bg-tertiary hover:bg-bg-tertiary/80 transition-colors cursor-pointer"
          >
            No thanks
          </button>
          <button
            onClick={handleApprove}
            className="px-5 py-2 rounded-lg text-sm text-white bg-accent-green hover:bg-accent-green/80 transition-colors cursor-pointer"
          >
            Go ahead
          </button>
        </div>
      </div>
    </div>
  );
}
