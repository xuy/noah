import { useState } from "react";
import * as commands from "../../lib/tauri-commands";
import type { DashboardStatus } from "../../lib/tauri-commands";

interface FleetConnectionCardProps {
  fleetStatus: DashboardStatus | null;
  setFleetStatus: (s: DashboardStatus) => void;
  t: (key: string) => string;
}

export function FleetConnectionCard({ fleetStatus, setFleetStatus, t }: FleetConnectionCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [enrollUrl, setEnrollUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [linking, setLinking] = useState(false);

  const isLinked = fleetStatus?.linked === true;

  const handleLink = async () => {
    setLinking(true);
    setError(null);
    try {
      await commands.linkDashboard(enrollUrl);
      const status = await commands.getDashboardStatus();
      setFleetStatus(status);
      setEnrollUrl("");
      setExpanded(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    setLinking(false);
  };

  if (isLinked) {
    return (
      <div className="bg-bg-secondary border border-accent-green/30 rounded-xl p-4 flex items-center gap-2">
        <span className="text-accent-green text-sm">{"\u2713"}</span>
        <div>
          <p className="text-sm text-text-primary font-medium">{fleetStatus?.fleet_name || t("health.fleetConnected")}</p>
          <p className="text-[10px] text-text-muted">{t("health.fleetSyncDesc")}</p>
        </div>
      </div>
    );
  }

  if (!expanded) {
    return (
      <button
        onClick={() => setExpanded(true)}
        className="w-full bg-bg-secondary border border-border-primary rounded-xl p-4 text-left hover:border-accent-blue/30 transition-colors cursor-pointer"
      >
        <p className="text-sm text-text-primary font-medium">{t("health.fleetCta")}</p>
        <p className="text-xs text-text-muted mt-0.5">{t("health.fleetCtaDesc")}</p>
      </button>
    );
  }

  return (
    <div className="bg-bg-secondary border border-accent-blue/30 rounded-xl p-5 space-y-3">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm text-text-primary font-medium">{t("health.fleetConnect")}</p>
          <p className="text-xs text-text-muted mt-0.5">{t("health.fleetDataDisclosure")}</p>
        </div>
        <button onClick={() => setExpanded(false)} className="text-text-muted hover:text-text-primary text-lg leading-none cursor-pointer">&times;</button>
      </div>
      <input
        type="text"
        placeholder="https://your-dashboard.com/enroll/abc123..."
        value={enrollUrl}
        onChange={(e) => setEnrollUrl(e.target.value)}
        className="w-full px-3 py-2 text-sm bg-bg-primary border border-border-primary rounded-lg text-text-primary"
      />
      {error && <p className="text-xs text-accent-red">{error}</p>}
      <button
        onClick={handleLink}
        disabled={linking || !enrollUrl.trim()}
        className="px-4 py-2 text-sm font-medium text-white bg-accent-blue rounded-lg hover:bg-accent-blue/90 disabled:opacity-50 cursor-pointer"
      >
        {linking ? "..." : t("health.fleetLinkBtn")}
      </button>
    </div>
  );
}
