---
name: update-troubleshooting
description: Fix stuck macOS updates, failed installations, and software update errors
platform: macos
last_reviewed: 2026-03-04
author: noah-team
source: bundled
emoji: 🔄
---

# Update Troubleshooting

## When to activate
User reports: macOS update stuck, update won't install, "unable to check for updates" error, update failed, "not enough space" for update, app updates failing.

## Quick check
Run `shell_run` with `softwareupdate --list` to see available updates.
- **Updates listed** → updates are available. Proceed with fix path.
- **"No new software available"** → system is up to date, or catalog is stale. Try `softwareupdate --clear-catalog` and re-check.
- **Error checking** → network issue. Run `mac_ping` to `8.8.8.8` and `mac_http_check` for `https://swscan.apple.com`.

## Standard fix path (try in order)

### 1. Free disk space
Run `mac_disk_usage`. macOS updates need 15-30 GB of free space (major upgrades need more).
- If space is low → activate the `disk-space-recovery` playbook first, then retry the update.
- This is the #1 cause of failed updates.

### 2. Run the update again
Most update failures are transient (network timeout, server load). Simply retrying often works.
- System Settings → General → Software Update → Download and Install.
- Alternative: `softwareupdate --install --all` via `shell_run`.

### 3. Reset the update catalog
If the update won't download or check for updates fails:
- Run `shell_run` with `softwareupdate --clear-catalog`.
- This clears the cached update catalog and forces a fresh check.
- Wait a few minutes, then check for updates again.

### 4. Safe Mode update
If the update downloads but fails to install:
- Restart in Safe Mode: hold Shift during boot (Intel) or hold power button → select Safe Mode (Apple Silicon).
- In Safe Mode, retry the update. Safe Mode disables third-party extensions that can interfere.

> Steps 1-3 resolve ~85% of update issues. #1 cause: insufficient disk space.

## Caveats
- **Don't force-shutdown during an update** unless truly stuck for >2 hours. The progress bar is often inaccurate — macOS updates can legitimately take 30-90 minutes.
- **"Update stuck at a percentage"** — if the Mac is still working (fans running, drive activity), let it continue. Force-shutting down mid-install can leave the system in a partially updated state.
- **Major OS upgrades (e.g., Ventura → Sonoma)** are riskier than point updates. Suggest backing up with Time Machine first. If the upgrade fails, Recovery Mode (Cmd+R on Intel, hold power on Apple Silicon) can reinstall.

## Key signals
- **"Not enough space"** → disk space issue. Step 1.
- **"Update downloaded but won't install"** → often a third-party kernel extension blocking. Step 4 (Safe Mode).
- **"Mac won't start after update"** → boot into Recovery Mode. Use "Reinstall macOS" — this installs the latest version without erasing data.
- **"It's been updating for 3 hours"** → if the progress bar hasn't moved at all, force-restart (hold power 10 seconds). It should resume or restart the update.
- **App Store updates stuck** → sign out (Store → Sign Out) and back in. If an app is stuck downloading, delete the partial download and retry.
- **Homebrew issues** → run `brew update && brew upgrade`. If broken, `brew doctor` shows common issues.

## Tools referenced
- `mac_disk_usage` — check free space
- `shell_run` — run `softwareupdate` commands
- `mac_ping` — verify internet connectivity
- `mac_http_check` — verify Apple update servers are reachable

## Escalation
If updates consistently fail:
- Try downloading the full macOS installer from the App Store (search for "macOS Sequoia" or current version) and running it manually.
- For managed Macs: MDM software may be blocking or deferring updates. Contact IT.
- Persistent boot issues after update: Apple Diagnostics (hold D during boot) to check for hardware problems.
