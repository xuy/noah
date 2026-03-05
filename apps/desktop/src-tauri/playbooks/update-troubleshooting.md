---
name: update-troubleshooting
description: Fix stuck macOS updates, failed installations, and software update errors
platform: macos
---

# Update Troubleshooting

## When to activate
User reports: macOS update stuck, update won't install, "unable to check for updates" error, update failed, Mac won't restart after update, app updates failing, "not enough space" for update.

## Protocol

### Step 1: Identify what's being updated
- **macOS system update** (e.g., Sonoma 14.5 → 14.6, or major upgrade to Sequoia) → Step 2
- **App Store app updates** → Step 5
- **Third-party app updates** (Chrome, Office, etc.) → Step 6

### Step 2: Check macOS update status
Run `shell_run` with `softwareupdate --list` to see available updates.

**Common outcomes:**
- **Updates found:** Shows available updates with labels and sizes. Proceed to installation issues (Step 3).
- **"No new software available":** System is up to date, or the update catalog is cached/stale.
  - Fix: `shell_run` with `softwareupdate --clear-catalog` to reset the catalog cache.
  - Then re-check with `softwareupdate --list`.
- **"Cannot check for updates":** Network issue or Apple's servers are down.
  - Check connectivity: `mac_ping` to `8.8.8.8`.
  - Check Apple's update servers: `mac_http_check` for `https://swscan.apple.com`.
  - Check https://www.apple.com/support/systemstatus/ for Apple server outages.

### Step 3: Update installation failures
**"Not enough space" error:**
1. Run `mac_disk_usage` to check free space.
2. macOS updates typically need 15-30 GB of free space (major upgrades need more).
3. If space is low, activate the `disk-space-recovery` playbook.
4. After freeing space, retry the update.

**Update downloads but won't install:**
1. Check if a restart is pending — the update may be waiting for a restart.
2. **Interrupt recovery:** If a previous update attempt was interrupted (power loss, force shutdown):
   - The update installer may be partially applied.
   - Try: restart in Safe Mode (hold Shift during boot on Intel, hold power button on Apple Silicon then select Safe Mode).
   - In Safe Mode, retry the update.
3. **Install from Recovery:** If the update keeps failing:
   - Restart into Recovery Mode (Cmd+R on Intel, hold power button on Apple Silicon).
   - Use "Reinstall macOS" — this installs the latest version without erasing data.

**Update stuck at a percentage / progress bar:**
1. **Don't panic.** Major updates can take 30-90 minutes. The progress bar is often inaccurate.
2. Check if the Mac is still working: is the fan running? Is the hard drive light blinking?
3. If truly stuck for >2 hours:
   - Force restart (hold power button 10 seconds).
   - It should resume or restart the update.
   - If it boots to a progress bar again, let it complete.
4. If it boots to a blank screen or recovery: see Step 4.

### Step 4: Post-update boot issues
**Mac won't start after update:**
1. **Apple Silicon Mac:**
   - Hold power button until "Loading startup options" appears.
   - Select Options > Recovery to enter Recovery Mode.
   - Try "Reinstall macOS" to complete the update.
2. **Intel Mac:**
   - Hold Cmd+R during boot to enter Recovery Mode.
   - Try "Reinstall macOS".
3. **Reset NVRAM/PRAM (Intel only):** Hold Option+Cmd+P+R during boot for 20 seconds.
4. **Safe Mode boot:** Hold Shift (Intel) to boot with minimal extensions. If Safe Mode works, a third-party extension is blocking normal boot.

**Boot into the wrong macOS volume:**
- After a major upgrade, Startup Disk may have changed.
- System Settings > General > Startup Disk — select the correct volume.

### Step 5: App Store update issues
**App updates stuck / won't download:**
1. Check internet connectivity.
2. Sign out and back into the App Store: Store menu > Sign Out, then Sign In.
3. Check if the app is shown in Launchpad with a loading indicator — it may be downloading in the background.
4. Fix: delete the partial download and retry.
   - Go to `/private/var/folders/` — partial App Store downloads live here. But it's easier to just restart and retry.
5. **"Update unavailable for this Apple ID":** The app was purchased/downloaded with a different Apple ID.

**"This update requires macOS X.Y":**
- The app requires a newer macOS version than what's installed.
- User needs to update macOS first, or stay on the older app version.

### Step 6: Third-party app updates

**Chrome:**
- Chrome updates itself. If stuck: chrome://settings/help shows update status.
- Fix: re-download from google.com/chrome.

**Microsoft Office:**
- Use Microsoft AutoUpdate (MAU): open any Office app > Help > Check for Updates.
- If MAU is broken: download the latest Office from office.com.
- Enterprise-managed Macs may have updates controlled by IT.

**Homebrew:**
- `brew update` updates Homebrew itself.
- `brew upgrade` upgrades installed packages.
- If broken: `brew doctor` shows common issues.

## Preventive advice
- Keep at least 20 GB free at all times for updates.
- Don't force-shutdown during updates unless truly stuck (>2 hours).
- Back up with Time Machine before major macOS upgrades.
- Check compatibility before major upgrades (older apps may not work on the new version).
