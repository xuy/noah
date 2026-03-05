---
name: printer-repair
description: Fix stuck print jobs, missing printers, and CUPS issues
platform: macos
last_reviewed: 2026-03-04
author: noah-team
---

# Printer Repair

## When to activate
User reports: can't print, print jobs stuck, printer not found, printing fails, printer offline.

## Quick check
Run `mac_print_queue` to see current jobs and `mac_printer_list` to see configured printers.
- If no printers configured → printer was never added or was removed. Jump to escalation.
- If printers exist → proceed with fix path.

## Standard fix path (try in order)

### 1. Clear stuck jobs
Run `mac_cancel_print_jobs` to clear all pending jobs.
A single stuck job blocks everything behind it. Clearing the queue and reprinting is the fastest fix.

### 2. Restart CUPS
Run `mac_restart_cups` to restart the macOS print system.
This clears the print spooler state, re-enables paused printers, and re-establishes connections.
CUPS often auto-pauses a printer after a few failed jobs — restarting fixes this.

### 3. Check printer connectivity
Based on printer type (visible in `mac_printer_list`):
- **Network printer:** Run `mac_ping` to the printer's IP. If unreachable, the printer is off, disconnected from Wi-Fi, or its IP changed.
- **USB printer:** If not in the printer list, try unplugging and re-plugging the cable. USB-C hubs can cause detection issues — try connecting directly.
- **AirPrint:** These are discovered via Bonjour/mDNS. If not appearing, the printer may be on a different subnet.

### 4. Delete and re-add the printer
If steps 1-3 didn't fix it, remove the printer in System Settings → Printers & Scanners, then add it again.
- For AirPrint printers, select the "AirPrint" driver instead of manufacturer-specific. AirPrint drivers are more reliable on modern macOS.
- This forces a fresh connection and clears any corrupted driver state.

> Steps 1-2 fix ~80% of print issues. The most common cause is a stuck job that paused the queue.

## Caveats
- **"Filter failed" error** in CUPS logs → the print filter (document converter) crashed. This is almost always a driver issue. Delete the printer and re-add with the AirPrint or generic PostScript driver.
- **Duplicate printers** after macOS update — macOS sometimes creates copies (e.g., "HP LaserJet" and "HP LaserJet (2)"). Remove the duplicates in System Settings → Printers & Scanners.
- **Print dialog shows no printers** even though `mac_printer_list` finds them — this is a known macOS UI bug. Quit and relaunch the app, or try printing from a different app.

## Key signals
- **"It printed fine yesterday"** → stuck job. Steps 1-2 will fix it.
- **"Printer shows 'offline'"** → check if printer is powered on and connected to the same network. Run `mac_ping` to its IP.
- **"Prints from my phone but not my Mac"** → CUPS or driver issue on the Mac. Steps 2 and 4 will fix it.
- **"Everything prints garbled/blank"** → driver mismatch. Re-add printer with the correct or generic driver (step 4).

## Tools referenced
- `mac_printer_list` — list configured printers
- `mac_print_queue` — check pending jobs
- `mac_cancel_print_jobs` — clear all stuck jobs
- `mac_restart_cups` — restart the print system
- `mac_ping` — test network printer connectivity
- `crash_log_reader` — read CUPS error log at `/var/log/cups/error_log`

## Escalation
If all steps fail:
- Test if the printer works from another device (phone, other computer) to isolate the issue.
- For enterprise printers: may need IT to check print server or AD permissions.
- For older printers: macOS may have dropped driver support. Check manufacturer's website.
