---
name: performance-forensics
description: Diagnose slowness, high CPU, memory pressure, and hangs
platform: macos
---

# Performance Forensics

## When to activate
User reports: computer is slow, fans are loud, beach ball spinning, apps freezing, high CPU usage, running out of memory, system lagging.

## Protocol

### Step 1: Quick triage (30 seconds)
Run `mac_system_info` to get CPU, memory, and disk overview.
Run `mac_process_list` to see top processes by CPU and memory.

**Classify the situation:**
- **CPU-bound:** One or more processes using >80% CPU. Go to Step 2.
- **Memory pressure:** Memory pressure is "critical" or swap usage is high (>2 GB). Go to Step 3.
- **Disk full:** Boot disk >90% full. Activate `disk-space-recovery` playbook instead.
- **Nothing obvious:** System looks healthy by the numbers. Go to Step 5 (background agents).

### Step 2: CPU analysis
Identify the top CPU consumer from `mac_process_list`.

**Known high-CPU processes — DO NOT KILL these:**
- **`kernel_task`** — This is macOS thermal throttling. The system is deliberately slowing down to prevent overheating. Not a bug. Suggest: check ventilation, close demanding apps, use on a hard surface (not a pillow/blanket).
- **`WindowServer`** — The display compositor. High CPU usually means GPU/display issues: too many monitors, high resolution scaling, or a misbehaving app with heavy animations. Try closing apps with complex UIs.
- **`mds` / `mds_stores`** — Spotlight indexing. Temporary after OS updates, new file additions, or restoring from backup. Usually resolves in 30-60 minutes. To check: `sudo mdutil -s /` shows indexing status.
- **`trustd`** — Certificate verification. Can spike when certificate cache is corrupted. Fix: `sudo rm -rf /var/folders/*/com.apple.trustd/` (requires restart, suggest only if persistent).

**User-killable processes:**
- If a regular app (Chrome, Slack, Zoom, etc.) is using excessive CPU:
  - Present the process name, PID, and CPU% to the user.
  - Suggest force-quitting via `mac_kill_process` (with user confirmation).
  - For Chrome specifically: check chrome://memory for per-tab memory usage.

### Step 3: Memory analysis
Check memory breakdown from `mac_system_info`:
- **App Memory:** Memory actively used by applications.
- **Wired Memory:** Kernel and system memory, cannot be freed.
- **Compressed:** macOS is compressing inactive memory to avoid swapping — this is normal.
- **Swap Used:** If >2 GB, system is under real memory pressure.

**Memory hogs from process list:**
- Sort by memory usage and identify top consumers.
- Common offenders: Chrome/Electron apps (each tab/window is a process), Docker, VMs, Xcode, Adobe apps.
- Suggest quitting apps that aren't actively needed.

**If no single app is the culprit:**
- Many small processes adding up? Check Login Items (Step 4).
- High "Cached Files" is normal and good — macOS uses free memory for file caching.

### Step 4: Login Items check
Run `mac_process_list` and look for processes that shouldn't be running:
- Helper apps, agents, updaters running in background.
- **`loginwindow` high CPU** = too many Login Items launching at startup.
- Suggest: System Settings > General > Login Items — review and remove unnecessary items.

### Step 5: Background agents
Look for background processes that might cause intermittent slowness:
- **Time Machine backups:** `backupd` or `tmutil` running = backup in progress. Temporary.
- **Software updates:** `softwareupdated` — checking for or installing updates.
- **iCloud sync:** `bird` or `cloudd` — syncing files. Can be heavy with large iCloud libraries.
- **Antivirus:** Third-party AV (CrowdStrike, Norton, etc.) can cause significant overhead. Identify and inform user.

### Step 6: Disk I/O check
If system feels slow but CPU/memory look normal:
- Check disk usage with `mac_disk_usage` — a nearly full disk causes severe slowness.
- SSDs slow down significantly when >90% full (no space for wear leveling).
- HDDs (older Macs) are inherently slower and degrade with fragmentation.

## Quick fixes to suggest
1. **Restart:** If the system hasn't been restarted in >7 days, suggest a restart. macOS accumulates state that a restart clears.
2. **Close unused apps:** Each open app consumes memory even when idle.
3. **Reduce browser tabs:** Each tab is a separate process consuming memory and sometimes CPU.
4. **Check for macOS updates:** Performance bugs are often fixed in point releases.

## Escalation
If performance is still poor after diagnosis:
- Suggest running Apple Diagnostics (restart holding D key) to check for hardware issues.
- If an older Mac with HDD: upgrading to SSD is the single biggest performance improvement.
- If RAM is consistently maxed: may need more physical RAM (if upgradeable) or need to use fewer apps simultaneously.
