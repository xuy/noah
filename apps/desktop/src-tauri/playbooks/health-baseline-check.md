---
name: health-baseline-check
description: Comprehensive device health check — disk, memory, uptime, updates, firewall, backup, and network
platform: all
last_reviewed: 2026-03-17
author: noah-team
type: system
emoji: 🩺
---

# Health Baseline Check

Runs a comprehensive health check across the device and produces a baseline summary. Covers disk, memory, uptime, OS updates, firewall, backup status, and network connectivity. Useful for onboarding, periodic audits, or establishing a reference point before changes.

## When to activate
New device setup, periodic health audit, pre-migration baseline, user reports general slowness, or "just check everything is OK."

## Standard check path

### 1. Check disk space
Check overall disk stats.
- **Green**: <80% used.
- **Yellow**: 80-90% used. Mention the `disk-space-recovery` playbook.
- **Red**: >90% used. SSDs degrade above 90%. Flag for immediate cleanup.

### 2. Check memory usage
Check RAM usage using system memory info.
- Report total RAM, used, and available.
- **Green**: >20% available.
- **Yellow**: 10-20% available. Check what's consuming memory.
- **Red**: <10% available or heavy swap usage. Identify top consumers.

### 3. Check system uptime
Run `uptime` to check how long since last reboot.
- **Green**: <14 days.
- **Yellow**: 14-30 days. Suggest a restart — many updates and fixes require a reboot.
- **Red**: >30 days. Strongly recommend a restart.

### 4. Check OS version and update status
- Report the current OS version.
- Run `softwareupdate --list` (macOS) to check for pending updates.
- **Green**: up to date.
- **Yellow**: non-security updates pending.
- **Red**: security updates pending.

### 5. Check firewall status
- **macOS**: Check `defaults read /Library/Preferences/com.apple.alf globalstate`.
- **Green**: firewall enabled (value 1 or 2).
- **Red**: firewall disabled (value 0). Recommend enabling it.

### 6. Check backup status
- **macOS**: Check Time Machine status via `tmutil status` and `tmutil latestbackup`.
- Report when the last backup completed.
- **Green**: backup within the last 24 hours.
- **Yellow**: backup is 1-7 days old.
- **Red**: no backup configured, or last backup is older than 7 days.

### 7. Check network connectivity
Run the quick connectivity checks from the `network-diagnostics` playbook:
- Ping `8.8.8.8` — basic internet.
- DNS check for `google.com` — DNS working.
- HTTP check for `https://www.google.com` — full connectivity.
- **Green**: all pass.
- **Yellow**: partial (e.g., ping works but DNS fails).
- **Red**: no connectivity.

### 8. Summarize health baseline
Present a summary with a status for each category:

| Check | Status | Detail |
|-------|--------|--------|
| Disk | Green/Yellow/Red | X% used, Y GB free |
| Memory | Green/Yellow/Red | X GB available of Y GB |
| Uptime | Green/Yellow/Red | X days |
| OS Updates | Green/Yellow/Red | Up to date / N updates pending |
| Firewall | Green/Red | Enabled / Disabled |
| Backup | Green/Yellow/Red | Last backup: date |
| Network | Green/Yellow/Red | All checks pass / Issues |

Give an overall assessment: healthy, needs attention, or needs immediate action. List specific recommendations in priority order.

## Caveats
- This is a point-in-time snapshot. Conditions change — memory usage fluctuates, network can be intermittent.
- **"Purgeable" disk space** on macOS is technically available. Don't flag disk as critical if most of the used space is purgeable.
- **High memory usage isn't always bad.** macOS aggressively caches files in RAM. "Memory pressure" is a better indicator than raw usage. Check for swap usage as the real signal.
- **No Time Machine** isn't necessarily a problem if the user uses another backup solution (Backblaze, CrashPlan, iCloud). Ask before flagging.

> The full baseline covers ~90% of common device health issues. Most frequent finding: pending OS updates and stale backups.

## Key signals
- **"My computer feels slow"** → focus on steps 2 (memory) and 3 (uptime). A reboot often helps.
- **"Just got a new laptop"** → run all steps to establish a clean baseline.
- **"Preparing for a big project"** → ensure disk space is healthy and backups are current.
- **"Something feels off"** → run all steps — the summary table makes issues obvious.

## Escalation
If multiple categories are red:
- Prioritize disk space (>90%) and missing backups — these risk data loss.
- If OS updates have been pending for weeks, check for MDM or policy blocks preventing updates.
- If the device is consistently unhealthy, it may need a fresh OS install or hardware evaluation.

## Tools referenced
- Disk usage tools — disk space stats
- Memory info tools — RAM usage
- Shell commands — uptime, firewall check, softwareupdate
- DNS lookup tools — DNS connectivity
- HTTP check tools — HTTP connectivity
- Ping tools — basic network check
