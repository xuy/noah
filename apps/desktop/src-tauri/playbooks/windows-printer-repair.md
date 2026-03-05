---
name: windows-printer-repair
description: Fix stuck print jobs, offline printers, and spooler issues on Windows
platform: windows
last_reviewed: 2026-03-04
author: noah-team
---

# Windows Printer Repair

## When to activate
User reports: can't print, print jobs stuck, printer offline, printer not found, printing fails, "driver is unavailable."

## Quick check
Run `win_print_queue` to see pending jobs and `win_printer_list` to see configured printers.
- If no printers configured → printer was never added or was removed. Jump to escalation.
- If printers exist → proceed with fix path.

## Standard fix path (try in order)

### 1. Clear stuck jobs
Run `win_cancel_print_jobs` to clear all pending jobs.
A single stuck job blocks everything behind it. Clearing the queue and reprinting is the fastest fix.

### 2. Restart the Print Spooler
Run `win_restart_spooler` to restart the Windows Print Spooler service.
This clears the spooler state and re-establishes connections. The Print Spooler often gets stuck after a few failed jobs — restarting fixes this.

If `win_restart_spooler` fails, try manually via `shell_run`:
```
net stop spooler
del /Q /F %systemroot%\System32\spool\PRINTERS\*
net start spooler
```
This also clears any corrupted spool files.

### 3. Check printer connectivity
Based on printer type (visible in `win_printer_list`):
- **Network printer:** Run `win_ping` to the printer's IP. If unreachable, the printer is off or disconnected from the network.
- **USB printer:** Try unplugging and re-plugging. If using a USB hub, try connecting directly.
- **Shared printer (SMB):** Run `shell_run` with `net view \\printserver` to check if the print server is reachable.

### 4. Update or reinstall the printer driver
If the printer shows "Driver is unavailable":
- Open Settings → Bluetooth & devices → Printers & scanners → select the printer → Remove.
- Re-add: Settings → Add a printer → let Windows search, or add manually by IP.
- Windows Update often has the right driver. If not, download from the manufacturer's website.

> Steps 1-2 fix ~80% of Windows print issues. The most common cause is a stuck job that crashed the spooler.

## Caveats
- **"Printer offline" but it's on** → right-click the printer in Settings → Printers & scanners → open print queue → Printer menu → uncheck "Use Printer Offline." Windows sometimes flips this flag after a failed job.
- **Duplicate printers after Windows Update** → Windows sometimes creates copies. Remove the duplicates in Settings → Printers & scanners.
- **"Access denied" printing to a shared printer** → credentials may have changed. Run `shell_run` with `net use \\printserver\sharename /delete` then reconnect.
- **Print to PDF missing** → run `shell_run` with `powershell -Command "Enable-WindowsOptionalFeature -Online -FeatureName 'Printing-PrintToPDFServices-Features'"`.

## Key signals
- **"It printed fine yesterday"** → stuck job. Steps 1-2 will fix it.
- **"Printer shows offline"** → check power and network, then uncheck "Use Printer Offline" (see caveats).
- **"Prints from other computers but not mine"** → driver issue on this PC. Step 4.
- **"Everything prints garbled or blank"** → driver mismatch. Remove and re-add with correct driver (step 4).
- **"Nobody can print"** → print server or network printer is down. Nothing to fix locally.

## Tools referenced
- `win_printer_list` — list configured printers
- `win_print_queue` — check pending jobs
- `win_cancel_print_jobs` — clear all stuck jobs
- `win_restart_spooler` — restart the Print Spooler service
- `win_ping` — test network printer connectivity
- `shell_run` — manual spooler cleanup commands

## Escalation
If all steps fail:
- Test if the printer works from another PC to isolate the issue.
- For enterprise printers: may need IT to check print server or AD permissions.
- For older printers: Windows may have dropped driver support. Check manufacturer's website.
