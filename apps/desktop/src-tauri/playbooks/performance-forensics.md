---
name: performance-forensics
description: Diagnose slowness, high CPU, memory pressure, and hangs
platform: macos
last_reviewed: 2026-03-04
author: noah-team
---

# Performance Forensics

## When to activate
User reports: computer is slow, fans are loud, spinning beach ball, apps freezing, system lagging, "everything takes forever."

## Quick check
Run `mac_system_info` and `mac_process_list` simultaneously.
Classify into one of: CPU-bound, memory pressure, disk full, or nothing obvious.

## Standard fix path (try in order)

### 1. Check for a single runaway process
Look at the top CPU consumers in `mac_process_list`.
- If one app is using >100% CPU → offer to force-quit it with `mac_kill_process`.
- This is the single most common cause of "my Mac is slow" — one app eating all the CPU.

### 2. Check memory pressure
From `mac_system_info`, check swap usage.
- **Swap > 2 GB** → system is under real memory pressure. Identify the top memory consumers from `mac_process_list` and suggest quitting apps that aren't actively needed.
- **High "Compressed Memory"** is normal — macOS compresses inactive memory to avoid swapping. This is good, not a problem.
- **High "Cached Files"** is also normal and beneficial — macOS uses free memory for file caching.

### 3. Check disk space
From `mac_disk_usage`, check free space.
- **Boot disk > 90% full** → activating the `disk-space-recovery` playbook will help. A full SSD causes severe performance degradation.

### 4. Suggest a restart
If the Mac hasn't been restarted in >7 days (check uptime from `mac_system_info`):
- macOS accumulates state (swap, caches, file descriptors, leaked memory) that a restart clears.
- This is the most underrated fix. A restart resolves many "mystery slowness" cases.

> Steps 1-4 resolve ~80% of performance complaints. #1 most common: a single runaway process.

## Caveats

**DO NOT kill these system processes — they look suspicious but are normal:**
- **`kernel_task`** — macOS thermal throttling. High CPU means the system is hot and deliberately slowing down. Fix: check ventilation, don't use Mac on a soft surface (pillow, blanket).
- **`WindowServer`** — display compositor. High CPU = too many monitors, heavy animations, or GPU issue. Try closing apps with complex UIs.
- **`mds` / `mds_stores`** — Spotlight indexing. Temporary after OS updates or restoring from backup. Resolves in 30-60 minutes. Don't kill it — it just restarts and re-indexes from scratch.
- **`trustd`** — certificate verification daemon. Brief spikes are normal.
- **`backupd`** — Time Machine backup in progress. Temporary.
- **`bird` / `cloudd`** — iCloud sync. Can be heavy with large iCloud libraries. Temporary.

## Key signals
- **"Slow after an update"** → Spotlight re-indexing (`mds`), Time Machine snapshot, or iCloud re-sync. All temporary — will resolve within hours. Explain this to the user.
- **"Fans going crazy but nothing open"** → check for `mds`, `backupd`, or `softwareupdated` in the process list. These are background tasks that spike CPU temporarily.
- **"Slow only in the morning"** → Login Items launching. System Settings → General → Login Items. Remove unnecessary items.
- **"One specific app is slow"** → not a system performance issue. The app itself may need updating, its cache may be corrupted, or it may need more memory. Consider the `app-doctor` playbook.
- **Chrome/Electron apps using lots of memory** → each tab/window is a separate process. This is by design. Reducing open tabs is the fix.

## Tools referenced
- `mac_system_info` — CPU, memory, disk, uptime overview
- `mac_process_list` — top processes by CPU and memory
- `mac_disk_usage` — disk space check
- `mac_kill_process` — force-quit a runaway process (NeedsApproval tier)

## Escalation
If performance is still poor after diagnosis:
- Run Apple Diagnostics (restart holding D key) to check for hardware issues.
- If an older Mac with HDD: upgrading to SSD is the single biggest improvement.
- If RAM is consistently maxed: need more physical RAM (if upgradeable) or fewer simultaneous apps.
