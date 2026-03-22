---
name: disk-space-recovery
description: Find and reclaim disk space from caches, backups, and hidden consumers
platform: macos
last_reviewed: 2026-03-07
author: noah-team
source: bundled
emoji: 💾
---

# Disk Space Recovery

## When to activate
User reports: disk full, low storage warning, can't install updates, can't save files, computer slowed down suddenly, "not enough space" error.

## Quick check
Run `mac_disk_usage` to confirm the disk is actually full (>85% used).
- If plenty of free space → the problem is something else (permissions, app bug).
- If disk is >90% full → proceed with fix path. SSDs degrade significantly above 90%.

## Standard fix path (try in order)

### 1. Smart overview
Run `disk_audit` to get a categorized breakdown sorted by size.
- If a background scan has completed recently, this returns **comprehensive** results covering the entire home directory — not just known locations.
- If no background data is available yet, it scans ~17 known space-hog directories (caches, Xcode, Docker, iOS backups, simulators, package managers, etc.).
Present the results to the user.

### 2. Drill down (if needed)
If the largest entry is vague or very large (e.g. "~/Library — 45 GB"), drill deeper:
- Run `disk_audit` with `{"target": "~/Library"}` to break it down by subdirectory.
- Use `{"min_size_mb": 500}` to focus on the biggest consumers only.

This lets you explore the filesystem interactively without scanning the whole disk each time.

### 3. Safe cleanup (no user confirmation needed)
Clean these immediately with `mac_clear_caches`:
- System and app caches (`~/Library/Caches/`) — rebuilt automatically on next use.
- Browser caches — rebuilt automatically.
- macOS log files older than 30 days.
- Temporary files (`/tmp/`, `/var/folders/`).
This typically recovers 2-10 GB with zero risk.

### 4. User-confirmed cleanup (present options, let user choose)
Show findings sorted by size. For each, explain what it is and the trade-off:
- **Trash** (`~/.Trash/`) — ask "Ready to empty Trash?"
- **Downloads** (`~/Downloads/`) — "Review your Downloads folder. Want me to show the largest files?"
- **Homebrew cache** (`~/Library/Caches/Homebrew/`) — safe, re-downloads as needed.
- **npm/yarn/pip/Cargo cache** — safe, re-downloads as needed.
- **Xcode DerivedData** (`~/Library/Developer/Xcode/DerivedData/`) — safe but rebuilds are slow.
- **Xcode iOS Device Support** — old device support files, 1-5 GB each, safe if you don't need to debug that iOS version.
- **iOS Simulator Runtimes** (`~/Library/Developer/CoreSimulator/Devices/`) — can be 10-30 GB. Ask first.
- **Docker images** — ask first, may contain important containers.
- **iOS backups** (`~/Library/Application Support/MobileSync/Backup/`) — irreplaceable if not backed up to iCloud. Always ask.
- **Stale build artifacts** — old `node_modules/`, `.venv/`, `target/` directories in projects not accessed in 90+ days. The background scanner flags these automatically. Safe to delete; rebuilds on next use.

### 5. Verify results
Run `mac_disk_usage` again after cleanup. Report how much was freed.

> Steps 1-4 typically recover 10-50 GB and resolve the issue.

## Background scanner
A background disk scanner runs automatically when the computer is idle (every ~6 hours, 30-60 second budget per cycle). It builds a comprehensive map of space usage across the home directory over time. The user can see scanner status and trigger on-demand scans from the **Diagnostics** section in the sidebar.

When background scan data is available, `disk_audit` uses it for instant, comprehensive results instead of scanning known directories one by one.

## Caveats
- **Time Machine local snapshots** can consume 20+ GB invisibly. Run `tmutil listlocalsnapshots /` to check. macOS auto-purges these when space is critical, but it can be slow. Don't manually delete unless the user explicitly asks.
- **Purgeable space** — macOS reports some space as "purgeable" (iCloud-optimized files, caches). This space is freed automatically when needed. The real available space is free + purgeable.
- **"System Data" is large** — users see this in About This Mac. It includes Time Machine snapshots, VM swap, sleep image, and caches. Much of it is automatically managed. Don't panic.
- **`.DocumentRevisions-V100`** — macOS document versioning database. NEVER delete. Corrupting this can break file history for all apps.

## Key signals
- **"Can't install macOS update"** → updates need 15-30 GB free. After cleanup, retry the update.
- **"Disk was fine yesterday"** → check for a runaway log file. Run `disk_audit` with `{"target": "~/Library/Logs"}` or check crash_logs. Common: app crash loops generating GBs of crash reports.
- **"I already emptied Trash"** → the big consumers are usually caches, Xcode, Docker, or iOS backups — things users don't think of.
- **"Only 2 GB on a 500 GB drive but I don't have much stuff"** → check for large hidden directories: Time Machine snapshots, Docker images, or Xcode taking 50+ GB. Run `disk_audit` with `{"target": "~"}` for a full top-level breakdown.

## Tools referenced
- `mac_disk_usage` — overall disk stats
- `disk_audit` — categorized space breakdown (uses background scan data when available; supports `target` and `min_size_mb` params for drill-down)
- `mac_clear_caches` — cleans system caches (SafeAction tier)

## Escalation
If cleanup doesn't free enough space:
- The user may genuinely need a larger drive or external storage.
- Suggest iCloud Drive with "Optimize Mac Storage" to offload files.
- For developers: Xcode + simulators + Docker can easily consume 100+ GB. These are working files, not waste.
