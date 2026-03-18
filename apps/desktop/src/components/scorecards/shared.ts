import type { CheckResult } from "../../lib/tauri-commands";

// ── Grade colors ────────────────────────────────────────────────────

export function gradeColor(grade: string): string {
  switch (grade) {
    case "A": return "text-accent-green";
    case "B": return "text-accent-blue";
    case "C": return "text-accent-yellow";
    case "D": return "text-accent-orange";
    default: return "text-accent-red";
  }
}

export function gradeBg(grade: string): string {
  switch (grade) {
    case "A": return "bg-accent-green/15";
    case "B": return "bg-accent-blue/15";
    case "C": return "bg-accent-yellow/15";
    case "D": return "bg-accent-orange/15";
    default: return "bg-accent-red/15";
  }
}

export function gradeRing(grade: string): string {
  switch (grade) {
    case "A": return "border-accent-green";
    case "B": return "border-accent-blue";
    case "C": return "border-accent-yellow";
    case "D": return "border-accent-orange";
    default: return "border-accent-red";
  }
}

export function gradeBorder(grade: string): string {
  switch (grade) {
    case "A": return "border-l-accent-green";
    case "B": return "border-l-accent-blue";
    case "C": return "border-l-accent-yellow";
    case "D": return "border-l-accent-orange";
    default: return "border-l-accent-red";
  }
}

export function statusIcon(status: string) {
  switch (status) {
    case "pass":
      return "\u2713";
    case "warn":
      return "\u26A0";
    default:
      return "\u2717";
  }
}

export function statusColor(status: string): string {
  switch (status) {
    case "pass": return "text-accent-green";
    case "warn": return "text-accent-yellow";
    default: return "text-accent-red";
  }
}

export function statusLabel(status: string, t: (key: string) => string): string {
  switch (status) {
    case "pass": return t("health.statusPass");
    case "warn": return t("health.statusWarn");
    default: return t("health.statusFail");
  }
}

// ── Time helpers ────────────────────────────────────────────────────

export function timeAgo(iso: string, t: (key: string, p?: Record<string, string | number>) => string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return t("health.justNow");
  if (mins < 60) return t("health.minutesAgo", { count: mins });
  const hours = Math.floor(mins / 60);
  if (hours < 24) return t("health.hoursAgo", { count: hours });
  const days = Math.floor(hours / 24);
  return t("health.daysAgo", { count: days });
}

export function formatRelativeTime(iso: string | null): string {
  if (!iso) return "";
  const diffMin = Math.floor((Date.now() - new Date(iso).getTime()) / 60000);
  if (diffMin < 1) return "Just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const h = Math.floor(diffMin / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

// ── Action hints per check ───────────────────────────────────────────

export interface ActionInfo {
  hint: string;
  canOpen: boolean;
  /** If set, Noah can fix this directly via chat. Value is the message to send. */
  noahCanFix?: string;
}

export function actionInfo(check: CheckResult): ActionInfo | null {
  if (check.status === "pass") return null;
  switch (check.id) {
    // Security
    case "security.firewall":
      return { hint: "Turn on your firewall in Network settings", canOpen: true };
    case "security.filevault":
      return { hint: "Enable FileVault disk encryption", canOpen: true };
    case "security.sip":
      return { hint: "Requires Recovery Mode: reboot holding Cmd+R, then run csrutil enable", canOpen: false };
    case "security.gatekeeper":
      return { hint: "Re-enable Gatekeeper in Security settings", canOpen: true };
    case "security.screen_lock":
      return { hint: "Set \"Require password\" to 5 minutes or less in Lock Screen settings", canOpen: true };
    case "security.xprotect":
      return { hint: "Install macOS updates to restore XProtect", canOpen: true };
    case "security.defender":
      return { hint: "Turn on Real-time protection in Windows Security", canOpen: true };
    case "security.bitlocker":
      return { hint: "Enable BitLocker drive encryption", canOpen: true };
    case "security.uac":
      return { hint: "Raise UAC level in User Account Control settings", canOpen: true };
    // Security — Linux
    case "security.mac":
      return { hint: "Enable AppArmor or SELinux for mandatory access control", canOpen: false };
    case "security.ssh_root":
      return { hint: "Set PermitRootLogin to \"no\" in /etc/ssh/sshd_config", canOpen: false };
    case "security.auto_updates":
      return { hint: "Enable unattended-upgrades for automatic security patches", canOpen: false };
    // Updates
    case "updates.os":
      return { hint: "Install available system updates", canOpen: true };
    case "updates.brew":
      return { hint: "Outdated packages found", canOpen: false, noahCanFix: "Run brew upgrade to update my Homebrew packages" };
    // Backups
    case "backups.timemachine":
      return { hint: "Set up Time Machine in System Settings", canOpen: true };
    case "backups.timemachine_dest":
      return { hint: "Connect a backup drive or configure a network backup destination", canOpen: true };
    case "backups.filehistory":
      return { hint: "Turn on File History in Windows Settings", canOpen: true };
    case "backups.restore_points":
      return { hint: "Enable System Protection in System Properties", canOpen: true };
    // Backups — Linux
    case "backups.snapshots":
      return { hint: "Create a Timeshift snapshot to protect against system changes", canOpen: false };
    case "backups.tool":
      return { hint: "Install a backup tool like Timeshift, Borg, or Deja Dup", canOpen: false };
    // Performance
    case "performance.uptime":
      return { hint: "Restart your computer to apply pending updates and free memory", canOpen: false };
    case "performance.disk_free":
      return { hint: "Free up disk space", canOpen: false, noahCanFix: "Help me free up disk space by finding large or unused files" };
    case "performance.startup_items":
      return { hint: "Too many startup items", canOpen: false, noahCanFix: "Show me my startup items and help reduce them to speed up boot time" };
    case "performance.memory":
      return { hint: "Close unused applications to free memory", canOpen: false };
    // Network
    case "network.dns":
      return { hint: "DNS issues detected", canOpen: false, noahCanFix: "Check my DNS settings and fix any issues — try switching to 1.1.1.1 or 8.8.8.8 if needed" };
    case "network.internet":
      return { hint: "Check your internet connection and router", canOpen: false };
    case "network.gateway":
      return { hint: "Check your network adapter settings", canOpen: false };
    default:
      return null;
  }
}

// ── Types ────────────────────────────────────────────────────────────

export interface ScanProgressEvent {
  scan_type: string;
  display_name: string;
  status: string;
  progress_pct: number;
  progress_detail: string;
}
