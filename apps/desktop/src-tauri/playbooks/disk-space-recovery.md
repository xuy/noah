---
name: disk-space-recovery
description: Find and reclaim disk space from caches, backups, and hidden consumers
platform: macos
last_reviewed: 2026-03-04
author: noah-team
---

# Disk Space Recovery

## When to activate
User reports: disk full, low storage warning, can't install updates, can't save files, computer slowed down suddenly, "not enough space" error.

## Quick check
Run `mac_disk_usage` to confirm the disk is actually full (>85% used).
- If plenty of free space → the problem is something else (permissions, app bug).
- If disk is >90% full → proceed with fix path. SSDs degrade significantly above 90%.

## Standard fix path (try in order)

### 1. Automated scan
Run `disk_audit` to scan all known space-hog directories at once.
This returns a categorized breakdown sorted by size — present it to the user.

### 2. Safe cleanup (no user confirmation needed)
Clean these immediately with `mac_clear_caches`:
- System and app caches (`~/Library/Caches/`) — rebuilt automatically on next use.
- Browser caches — rebuilt automatically.
- macOS log files older than 30 days.
- Temporary files (`/tmp/`, `/var/folders/`).
This typically recovers 2-10 GB with zero risk.

### 3. User-confirmed cleanup (present options, let user choose)
Show findings sorted by size. For each, explain what it is and the trade-off:
- **Trash** (`~/.Trash/`) — ask "Ready to empty Trash?"
- **Downloads** (`~/Downloads/`) — "Review your Downloads folder. Want me to show the largest files?"
- **Homebrew cache** (`~/Library/Caches/Homebrew/`) — safe, re-downloads as needed.
- **npm/yarn/pip cache** — safe, re-downloads as needed.
- **Xcode DerivedData** (`~/Library/Developer/Xcode/DerivedData/`) — safe but rebuilds are slow.
- **Docker images** — ask first, may contain important containers.
- **iOS backups** (`~/Library/Application Support/MobileSync/Backup/`) — irreplaceable if not backed up to iCloud. Always ask.

### 4. Verify results
Run `mac_disk_usage` again after cleanup. Report how much was freed.

> Steps 1-3 typically recover 10-50 GB and resolve the issue.

## Caveats
- **Time Machine local snapshots** can consume 20+ GB invisibly. Run `tmutil listlocalsnapshots /` to check. macOS auto-purges these when space is critical, but it can be slow. Don't manually delete unless the user explicitly asks.
- **Purgeable space** — macOS reports some space as "purgeable" (iCloud-optimized files, caches). This space is freed automatically when needed. The real available space is free + purgeable.
- **"System Data" is large** — users see this in About This Mac. It includes Time Machine snapshots, VM swap, sleep image, and caches. Much of it is automatically managed. Don't panic.
- **`.DocumentRevisions-V100`** — macOS document versioning database. NEVER delete. Corrupting this can break file history for all apps.

## Key signals
- **"Can't install macOS update"** → updates need 15-30 GB free. After cleanup, retry the update.
- **"Disk was fine yesterday"** → check for a runaway log file. Look at disk_audit results for anything that grew suddenly. Common: app crash loops generating GBs of crash reports.
- **"I already emptied Trash"** → the big consumers are usually caches, Xcode, Docker, or iOS backups — things users don't think of.
- **"Only 2 GB on a 500 GB drive but I don't have much stuff"** → check for large hidden directories: Time Machine snapshots, Docker images, or Xcode taking 50+ GB.

## Tools referenced
- `mac_disk_usage` — overall disk stats
- `disk_audit` — scans ~12 known space-hog directories
- `mac_clear_caches` — cleans system caches (SafeAction tier)

## Escalation
If cleanup doesn't free enough space:
- The user may genuinely need a larger drive or external storage.
- Suggest iCloud Drive with "Optimize Mac Storage" to offload files.
- For developers: Xcode + simulators + Docker can easily consume 100+ GB. These are working files, not waste.
