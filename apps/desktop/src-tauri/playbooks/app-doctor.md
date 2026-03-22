---
name: app-doctor
description: Fix app crashes, launch failures, and permission issues
platform: macos
last_reviewed: 2026-03-04
author: noah-team
source: bundled
emoji: 🩺
---

# App Doctor

## When to activate
User reports: app crashes, app won't open, app frozen, "app is damaged" error, app unexpectedly quit.

## Quick check
Run `mac_process_list` — is the app currently running?
- If running but unresponsive → offer to force-quit with `mac_kill_process`, then relaunch.
- If not running → proceed with fix path.

## Standard fix path (try in order)

### 1. Force-quit and relaunch
Ensure the app is fully quit (not just the window closed). Check for lingering processes in `mac_process_list`. Kill any remaining instances, then relaunch.
Most "app won't open" cases are actually a zombie process holding a lock.

### 2. Clear app cache
Run `mac_clear_app_cache` with the app's bundle ID.
This removes cached data without affecting settings or user data. Most apps rebuild their cache on next launch.
- Fixes: corrupted cache causing crashes, slow startup, stale data.

### 3. Check crash logs
Run `crash_log_reader` with the app name to get recent crash reports.
The exception type tells you what's wrong:
- **EXC_BAD_ACCESS (SIGSEGV)** — memory bug in the app. Update the app to latest version.
- **SIGABRT** — failed assertion, often from corrupted preferences. Go to step 4.
- **EXC_CRASH (SIGKILL)** — killed by the system (too much memory, launch timeout). Check if the system is under memory pressure.
- **EXC_BAD_INSTRUCTION** — on Apple Silicon, may need Rosetta 2. Go to step 5.

### 4. Reset preferences
App preferences live in `~/Library/Preferences/` as `.plist` files (usually `com.developer.appname.plist`).
Rename the plist to `.plist.bak` (don't delete — back it up), then relaunch. The app creates fresh default preferences.
- This fixes crashes caused by corrupted or incompatible settings (common after app updates).

### 5. Check Gatekeeper and permissions
**"App is damaged and can't be opened"** — this is almost never actual corruption. It's the quarantine flag.
- Fix: System Settings → Privacy & Security → scroll down → "Open Anyway" for the blocked app.
- The app was downloaded from the internet and macOS is being cautious.

**App needs Rosetta 2 (Apple Silicon Macs):**
- If the app was built for Intel only, it needs Rosetta translation.
- Rosetta installs automatically on first use but can fail silently.
- If the app crashes with EXC_BAD_INSTRUCTION: try right-click app → Get Info → check "Open using Rosetta."

### 6. Reinstall the app
Last resort. Delete the app, clean up its support files, reinstall from the original source.
- Delete: drag app from Applications to Trash.
- Clean up: remove related files in `~/Library/Caches/`, `~/Library/Preferences/`, `~/Library/Application Support/` for that app.
- Reinstall from App Store or developer website.

> Steps 1-4 resolve ~90% of app issues. Most common: corrupted cache (step 2) or preferences (step 4).

## Caveats
- **Don't reinstall as the first step.** It's the most disruptive and often doesn't fix the underlying issue if corrupted preferences or cache are the cause — those survive reinstallation.
- **"App is damaged"** = quarantine flag, not corruption. Don't suggest reinstalling for this — just clear the Gatekeeper block (step 5).
- **Crashes only after OS update** → app may need updating for the new OS version. Check the App Store or developer website for a compatible update before clearing caches.

## Key signals
- **"It worked until I updated macOS"** → app incompatibility. Check for app update first, then try Rosetta (step 5).
- **"Crashes immediately on launch"** → usually corrupted preferences (step 4) or a lingering zombie process (step 1).
- **"Works for a while then crashes"** → memory leak in the app or corrupted cache. Step 2 then step 4.
- **"Says I don't have permission"** → check Privacy & Security settings. The app may need Camera, Microphone, Files, or Full Disk Access permissions.

## Tools referenced
- `mac_process_list` — check for running/zombie processes
- `mac_kill_process` — force-quit (NeedsApproval tier)
- `mac_clear_app_cache` — clear app cache
- `crash_log_reader` — read and summarize crash reports
- `mac_app_list` — verify app is installed

## Escalation
If all steps fail:
- Collect crash logs and app version for the developer's support team.
- Check the app's support forum or release notes for known issues.
- For enterprise apps: contact the vendor or internal IT.
