---
name: app-doctor
description: Fix app crashes, launch failures, and permission issues
platform: macos
---

# App Doctor

## When to activate
User reports: app crashes, app won't open, app frozen, "app is damaged" error, app unexpectedly quit, app permissions issue, app not responding.

## Protocol

### Step 1: Identify the app
Ask or determine which app is having problems.
Run `mac_app_list` to verify the app is installed and get its path.
- If app not found in list: it may be uninstalled, or installed in a non-standard location.

### Step 2: Check if app is currently running
Run `mac_process_list` and look for the app's process.
- **If running but unresponsive:** Offer to force-quit with `mac_kill_process`, then relaunch.
- **If not running:** Try to understand why it won't launch (Step 3).
- **If running and crashing repeatedly:** Check crash logs (Step 3).

### Step 3: Crash log analysis
Run `crash_log_reader` with the app name to get recent crash reports.

**Common exception types:**
- **EXC_BAD_ACCESS (SIGSEGV/SIGBUS)** — Memory access violation. The app tried to read/write invalid memory. Usually a bug in the app. Suggest: update the app, or reinstall.
- **SIGABRT** — The app deliberately aborted, usually due to a failed assertion. Often caused by corrupted preferences or cache. Try clearing app cache (Step 4).
- **EXC_GUARD** — Sandbox violation. The app tried to access a resource it's not allowed to. May need to reset permissions (Step 5).
- **EXC_CRASH (SIGKILL)** — Killed by the system. Reasons: too much memory usage, watchdog timeout (launch took too long), or thermal shutdown.
- **EXC_BAD_INSTRUCTION** — Illegal CPU instruction. On Apple Silicon, this can mean the app needs Rosetta 2.

**If no crash logs found:**
- The app may be failing silently. Check `mac_app_logs` for console output.
- Launch the app from Terminal to see error messages: `open -a "AppName"`.

### Step 4: Graduated cache/preferences reset
Try these in order, testing the app after each:

1. **Clear app cache:** Run `mac_clear_app_cache` with the app's bundle ID.
   - This removes cached data without affecting settings or user data.
   - Most apps rebuild their cache on next launch.

2. **Reset preferences (if cache clearing didn't help):**
   - App preferences live in `~/Library/Preferences/` as .plist files.
   - Identify the plist: usually `com.developer.appname.plist`.
   - Suggest renaming (not deleting) the plist to back it up, then relaunch.
   - The app will create fresh default preferences.

3. **Clear app support data (last resort before reinstall):**
   - Check `~/Library/Application Support/AppName/` for corrupted data.
   - Use `mac_app_logs` to check for specific error paths.
   - Only suggest clearing if crash logs point to data corruption.

### Step 5: Permission and security checks

**Gatekeeper / quarantine issues:**
- **"App is damaged and can't be opened"** — Usually NOT corruption. The app's quarantine flag is set and Gatekeeper is blocking it.
  - Check: The app was likely downloaded from the internet.
  - Fix: System Settings > Privacy & Security > scroll down to see blocked app > "Open Anyway".
  - Alternative: Remove quarantine attribute (with user permission).

**Privacy permissions:**
- **"App would like to access..."** dialogs not appearing:
  - Check System Settings > Privacy & Security > relevant category (Camera, Microphone, Files, etc.).
  - If the app isn't listed, it may need to be added manually.
  - If the toggle is off, the user needs to enable it.

**Full Disk Access:**
- Some apps (backup tools, security scanners, Terminal) need Full Disk Access.
- System Settings > Privacy & Security > Full Disk Access.

### Step 6: Apple Silicon / Rosetta 2 issues
For Macs with Apple Silicon (M1/M2/M3/M4):
- **App requires Rosetta 2:** If Rosetta isn't installed, Intel apps won't run.
  - Rosetta installs automatically on first use, but can fail silently.
  - Check if installed: `arch` command should show `arm64`.
  - If app was built for Intel only, it needs Rosetta translation.
- **Universal binary issues:** Some universal binaries have bugs in the ARM slice.
  - Try running under Rosetta: right-click app > Get Info > check "Open using Rosetta".

### Step 7: Reinstall as last resort
If all previous steps failed:
1. Note the app version and any important settings.
2. Suggest the user:
   - Delete the app (move to Trash from Applications).
   - Clean up: `~/Library/Caches/`, `~/Library/Preferences/`, `~/Library/Application Support/` for the app's files.
   - Re-download from the original source (App Store or developer website).
   - Reinstall.

## Common app-specific issues
- **Microsoft Office:** "Identity could not be verified" → delete Keychain entries for Microsoft.
- **Adobe Creative Cloud:** Crashes often caused by corrupted preferences. Adobe provides a Cleaner Tool.
- **Zoom:** Camera/mic permissions. Check Privacy & Security settings.
- **Slack:** High memory usage is normal for Electron apps. Restart helps.
- **Xcode:** "Unable to boot simulator" → delete derived data. "No provisioning profile" → re-sign.
