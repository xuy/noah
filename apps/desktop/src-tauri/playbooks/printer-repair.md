---
name: printer-repair
description: Fix stuck print jobs, missing printers, and CUPS issues
platform: macos
---

# Printer Repair

## When to activate
User reports: can't print, print jobs stuck, printer not found, printing fails, printer offline, print quality issues, "Filter failed" errors.

## Protocol

### Step 1: Check print queue
Run `mac_print_queue` to see current jobs.
- **If jobs are stuck (status: held, paused, or stalled):**
  - Cancel all stuck jobs with `mac_cancel_print_jobs`.
  - Then check if the printer is paused (CUPS sometimes pauses a printer after errors).
- **If queue is empty:** Problem is not a stuck job. Continue to Step 2.

### Step 2: List configured printers
Run `mac_printer_list` to see all printers.
- **If no printers found:** Printer was removed or never added. Suggest: System Settings > Printers & Scanners > Add Printer.
- **If printer shows but is paused/disabled:** CUPS paused it after errors. Run `mac_restart_cups` to reset.
- **If multiple copies of same printer exist:** "Duplicate printer syndrome" — common after macOS updates. Suggest removing duplicates in System Settings > Printers & Scanners.

### Step 3: Test printer connectivity
Based on printer type:

**Network printer (IP-based):**
- Run `mac_ping` to printer's IP address.
- If ping fails → printer is offline, powered off, or IP changed. Check if printer is on and connected to same network.
- If ping succeeds → printer is reachable. Problem is likely CUPS or driver.

**AirPrint/Bonjour printer:**
- These are discovered via mDNS. If not showing up, the printer may be on a different subnet or mDNS is blocked.
- Check if printer appears in Finder sidebar (Bonjour discovery).

**USB printer:**
- Check if it appears in `mac_printer_list`. If not, try unplugging and re-plugging the USB cable.
- USB-C hubs can cause detection issues. Try connecting directly.

### Step 4: CUPS diagnosis
Run `crash_log_reader` with path `/var/log/cups/error_log` to check for CUPS errors.

**Common CUPS errors:**
- **"Filter failed"** → Driver/filter issue. The print filter (which converts documents to printer format) crashed.
  - Fix: Delete and re-add the printer. If using third-party drivers, reinstall them.
  - For AirPrint printers, try selecting "AirPrint" driver instead of manufacturer-specific.
- **"Unable to connect"** → Network connectivity issue (see Step 3).
- **"Unauthorized"** → Permissions issue. Reset CUPS with `mac_restart_cups`.
- **"Backend failed"** → Communication protocol error. Try switching between IPP, LPD, or Socket protocols.

### Step 5: Graduated reset
Try these in order, testing printing after each:

1. **Restart CUPS:** Run `mac_restart_cups`. This clears the print system state without losing configuration.
2. **Reset print system:** If CUPS restart doesn't help, suggest: System Settings > Printers & Scanners > right-click printer list > "Reset printing system". WARNING: This removes all printers; they'll need to be re-added.

### Step 6: Print dialog showing no printers
This is a known macOS UI bug where the print dialog's printer dropdown is empty even though printers are configured.
- Verify printers exist with `mac_printer_list`.
- If printers are configured but don't show in print dialog:
  - Try: close and reopen the print dialog.
  - Try: quit and relaunch the application.
  - Try: `mac_restart_cups`.

## Escalation
If all steps fail:
- Check if the printer works from another device (phone, other computer) to isolate the issue.
- For enterprise printers: may need IT to check print server or Active Directory permissions.
- For old printers: macOS may have dropped driver support. Check manufacturer's website for updated drivers.
