---
name: windows-update-troubleshooting
description: Fix stuck Windows Updates, failed installations, and update service errors
platform: windows
last_reviewed: 2026-03-04
author: noah-team
source: bundled
emoji: 🔄
---

# Windows Update Troubleshooting

## When to activate
User reports: Windows Update stuck, update won't install, update error code, system slow after update, "something went wrong" during update, pending restart that won't complete.

## Quick check
Run `shell_run` with `powershell -Command "Get-WindowsUpdate -ErrorAction SilentlyContinue; Get-HotFix | Sort-Object InstalledOn -Descending | Select-Object -First 5"` to see recent updates.
- If recent updates installed fine → problem may be a specific failed update. Ask for the error code.
- If no recent updates → update service may be stuck. Proceed with fix path.

Also check `win_disk_usage` — Windows Update needs 10-20 GB free.

## Standard fix path (try in order)

### 1. Check for pending reboot
Run `shell_run` with `powershell -Command "Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired'"`.
- If `True` → a previous update is waiting for reboot. Reboot first, then check if the problem is resolved.
- This is the #1 cause of "updates won't install" — a pending reboot blocks new updates.

### 2. Restart Windows Update services
Run `win_restart_service` for `wuauserv` (Windows Update).
Also restart these related services via `shell_run`:
```
net stop bits && net start bits
net stop cryptSvc && net start cryptSvc
```
- `wuauserv` — the update engine
- `bits` — Background Intelligent Transfer Service (downloads updates)
- `cryptSvc` — Cryptographic Services (verifies update signatures)

Restarting these three services fixes most transient update failures.

### 3. Clear the update cache
If restarting services didn't help, clear the cached update files:
Run `shell_run` with:
```
net stop wuauserv && net stop bits
ren C:\Windows\SoftwareDistribution SoftwareDistribution.old
ren C:\Windows\System32\catroot2 catroot2.old
net start wuauserv && net start bits
```
This forces Windows to re-download updates from scratch. The old folders can be deleted after updates succeed.

### 4. Run the Windows Update troubleshooter
Run `shell_run` with `powershell -Command "Get-TroubleshootingPack -Path 'C:\Windows\diagnostics\system\WindowsUpdate' | Invoke-TroubleshootingPack -Unattended"`.
Microsoft's built-in troubleshooter resets update components and fixes common issues automatically.

### 5. DISM and SFC repair
If updates still fail, the system image may be corrupted:
Run `shell_run` with:
```
DISM /Online /Cleanup-Image /RestoreHealth
sfc /scannow
```
- DISM repairs the Windows component store (downloads clean copies from Windows Update).
- SFC repairs protected system files using the component store.
- Run DISM first, then SFC. This order matters.
- DISM can take 15-30 minutes. Warn the user.

> Steps 1-3 resolve ~80% of Windows Update issues. #1 cause: pending reboot blocking new updates.

## Caveats
- **Error code 0x80070057** — invalid parameter. Usually caused by corrupted update cache. Step 3 fixes it.
- **Error code 0x800f081f** — source files not found. DISM can't download repair files. Try: `DISM /Online /Cleanup-Image /RestoreHealth /Source:C:\path\to\mounted\iso\sources\install.wim` with a mounted Windows ISO.
- **"Updates are managed by your organization"** — Group Policy or MDM is controlling updates. Nothing to fix locally — contact IT admin.
- **Metered connection blocking updates** — Windows won't download large updates on metered connections. Settings → Network & Internet → check if current connection is set to metered.
- **Update loops (install → reboot → install again)** — a broken update is being retried. Hide the specific update: `powershell -Command "Hide-WindowsUpdate -KBArticleID 'KBXXXXXXX'"` or uninstall it from Settings → Update History → Uninstall updates.

## Key signals
- **"Stuck at a percentage for hours"** → if actively downloading/installing, wait up to 2 hours. If truly stuck, force-reboot and retry. Step 3 to clear cache.
- **"Blue screen after update"** → boot to Safe Mode (hold Shift + click Restart), uninstall the problematic update from Settings → Recovery → Advanced startup.
- **"Not enough space"** → run `win_disk_usage`. Clear temp files with `win_clear_caches`. Windows Update needs 10-20 GB free.
- **"Updates disabled by admin"** → check `win_service_list` for `wuauserv`. If disabled, it's likely a policy decision — contact IT.
- **Specific KB error** → search the error code. Microsoft documents most update errors with specific fixes.

## Tools referenced
- `shell_run` — run PowerShell and cmd commands for update management
- `win_restart_service` — restart Windows Update and related services
- `win_disk_usage` — check free space
- `win_service_list` — check Windows Update service status
- `win_clear_caches` — clear temp files to free space
- `win_system_info` — check Windows version and build

## Escalation
If all steps fail:
- Download the update manually from the Microsoft Update Catalog (https://catalog.update.microsoft.com) and install with `wusa.exe`.
- For feature updates (e.g., 23H2 → 24H2): download the Update Assistant or Media Creation Tool from Microsoft.
- For managed PCs: WSUS or SCCM may be blocking updates. Contact IT admin.
- Persistent BSOD after updates: may need system restore or Windows repair install.
