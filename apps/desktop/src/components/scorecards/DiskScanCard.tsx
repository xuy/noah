import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { formatRelativeTime } from "./shared";
import type { ScanProgressEvent } from "./shared";
import * as commands from "../../lib/tauri-commands";
import type { ScanJobRecord } from "../../lib/tauri-commands";

export function DiskScanCard({ t }: { t: (key: string) => string }) {
  const [job, setJob] = useState<ScanJobRecord | null>(null);
  const [live, setLive] = useState<ScanProgressEvent | null>(null);

  useEffect(() => {
    commands.getScanJobs().then((jobs) => {
      const disk = jobs.find((j) => j.scan_type === "disk");
      if (disk) setJob(disk);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<ScanProgressEvent>("scanner-progress", (e) => {
      if (e.payload.scan_type === "disk") {
        setLive(e.payload);
        if (e.payload.status === "completed" || e.payload.status === "failed") {
          commands.getScanJobs().then((jobs) => {
            const disk = jobs.find((j) => j.scan_type === "disk");
            if (disk) setJob(disk);
          }).catch(() => {});
        }
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const status = live?.status || job?.status || "queued";
  const pct = live?.progress_pct ?? job?.progress_pct ?? 0;
  const detail = live?.progress_detail || job?.progress_detail || "";
  const isRunning = status === "running";
  const isPaused = status === "paused";

  const statusText = isRunning ? (detail || t("diagnostics.starting"))
    : isPaused ? (detail || t("diagnostics.paused"))
    : status === "completed" ? t("diagnostics.complete")
    : status === "failed" ? t("diagnostics.failed")
    : t("diagnostics.waitingFirstScan");

  const statusClr = isRunning ? "text-accent-blue"
    : status === "completed" ? "text-accent-green"
    : status === "failed" ? "text-accent-red"
    : isPaused ? "text-accent-yellow"
    : "text-text-muted";

  const handleAction = async () => {
    if (isRunning) {
      await commands.pauseScan("disk").catch(() => {});
    } else if (isPaused) {
      await commands.resumeScan("disk").catch(() => {});
    } else {
      await commands.triggerScan("disk").catch(() => {});
    }
  };

  const btnLabel = isRunning ? t("diagnostics.pause")
    : isPaused ? t("diagnostics.resume")
    : status === "completed" ? t("diagnostics.rescan")
    : t("diagnostics.scanNow");

  const ts = job?.completed_at || job?.updated_at;

  // Active state: full card with progress
  if (isRunning || isPaused) {
    return (
      <div className="bg-bg-secondary border border-border-primary rounded-xl p-4">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-sm font-medium text-text-primary">{t("diagnostics.diskAnalysis")}</h3>
          <button
            onClick={handleAction}
            className="text-xs text-text-muted hover:text-text-secondary cursor-pointer"
          >
            {btnLabel}
          </button>
        </div>
        <div className="w-full h-1 bg-bg-tertiary rounded-full overflow-hidden mb-2">
          <div
            className={`h-full rounded-full transition-all duration-500 ${isRunning ? "bg-accent-blue" : "bg-accent-yellow"}`}
            style={{ width: `${Math.max(pct, 2)}%` }}
          />
        </div>
        <span className={`text-xs ${statusClr}`}>{statusText}</span>
      </div>
    );
  }

  // Idle state: compact inline row (matches all-passing scorecard style)
  return (
    <div className="flex items-center justify-between py-3 px-1">
      <div className="flex items-center gap-2.5">
        <span className={`text-sm ${statusClr}`}>{status === "completed" ? "\u2713" : "\u25CB"}</span>
        <span className="text-sm text-text-primary font-medium">{t("diagnostics.diskAnalysis")}</span>
        {ts && <span className="text-xs text-text-muted">{formatRelativeTime(ts)}</span>}
      </div>
      <button
        onClick={handleAction}
        className="text-xs text-text-muted hover:text-text-secondary cursor-pointer"
      >
        {btnLabel}
      </button>
    </div>
  );
}
