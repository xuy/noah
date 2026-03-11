import { useState, useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useLocale } from "../i18n";

export function UpdateBanner() {
  const { t } = useLocale();
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [status, setStatus] = useState<"idle" | "downloading" | "installing" | "done" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      try {
        const update = await check();
        if (!cancelled && update?.available) {
          setUpdateVersion(update.version);
        }
      } catch {
        // Silently ignore update check failures (offline, no endpoint, etc.)
      }
    }

    // Check on mount
    checkForUpdate();

    // Check every 6 hours
    const interval = setInterval(checkForUpdate, 6 * 60 * 60 * 1000);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  if (!updateVersion || dismissed) return null;

  const handleInstall = async () => {
    setStatus("downloading");
    setErrorMsg(null);
    try {
      const update = await check();
      if (update?.available) {
        await update.downloadAndInstall((event) => {
          if (event.event === "Finished") {
            setStatus("installing");
          }
        });
        setStatus("done");
        // Relaunch the app to apply the update
        await relaunch();
      }
    } catch (err) {
      console.error("Update failed:", err);
      const message = err instanceof Error ? err.message : String(err);
      setErrorMsg(message);
      setStatus("error");
    }
  };

  const handleRetry = () => {
    setStatus("idle");
    setErrorMsg(null);
    handleInstall();
  };

  const buttonLabel = {
    idle: t("update.updateNow"),
    downloading: t("update.downloading"),
    installing: t("update.restarting"),
    done: t("update.restarting"),
    error: t("update.retry"),
  }[status];

  return (
    <div>
      <div className="flex items-center justify-between gap-3 px-4 py-2 bg-accent-blue/10 border-b border-accent-blue/20">
        <p className="text-xs text-text-primary">
          {t("update.available", { version: updateVersion })}
        </p>
        <div className="flex items-center gap-2">
          {status === "idle" && (
            <button
              onClick={() => setDismissed(true)}
              className="text-[10px] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
            >
              {t("update.later")}
            </button>
          )}
          <button
            onClick={status === "error" ? handleRetry : handleInstall}
            disabled={status === "downloading" || status === "installing" || status === "done"}
            className={`px-3 py-1 rounded-md text-[11px] font-medium transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed ${
              status === "error"
                ? "bg-accent-red text-white hover:bg-accent-red/80"
                : "bg-accent-blue text-white hover:bg-accent-blue/80"
            }`}
          >
            {buttonLabel}
          </button>
        </div>
      </div>
      {status === "error" && errorMsg && (
        <div className="px-4 py-1.5 bg-accent-red/5 border-b border-accent-red/10">
          <p className="text-[10px] text-accent-red">
            {t("update.errorDetail", { error: errorMsg })}
          </p>
        </div>
      )}
    </div>
  );
}
