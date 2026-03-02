import { useState, useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";

export function UpdateBanner() {
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
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
    setInstalling(true);
    try {
      const update = await check();
      if (update?.available) {
        await update.downloadAndInstall();
      }
    } catch {
      setInstalling(false);
    }
  };

  return (
    <div className="flex items-center justify-between gap-3 px-4 py-2 bg-accent-blue/10 border-b border-accent-blue/20">
      <p className="text-xs text-text-primary">
        <span className="font-medium">Noah v{updateVersion}</span> is
        available.
      </p>
      <div className="flex items-center gap-2">
        <button
          onClick={() => setDismissed(true)}
          className="text-[10px] text-text-muted hover:text-text-primary transition-colors cursor-pointer"
        >
          Later
        </button>
        <button
          onClick={handleInstall}
          disabled={installing}
          className="px-3 py-1 rounded-md bg-accent-blue text-white text-[11px] font-medium hover:bg-accent-blue/80 transition-colors cursor-pointer disabled:opacity-50"
        >
          {installing ? "Installing..." : "Update now"}
        </button>
      </div>
    </div>
  );
}
