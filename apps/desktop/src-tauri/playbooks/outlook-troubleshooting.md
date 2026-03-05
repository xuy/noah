---
name: outlook-troubleshooting
description: Fix Outlook sync failures, crashes, stuck email, and profile corruption
platform: all
last_reviewed: 2026-03-04
author: noah-team
---

# Outlook Troubleshooting

## When to activate
User reports: Outlook won't sync, email stuck in outbox, calendar not updating, Outlook crashes, search not working, "need password" loop, Outlook slow, OST corrupted.

## Quick check
Verify this is an Outlook-specific issue, not a network problem:
- Can the user access email at https://outlook.office.com in a browser?
- If webmail also fails → this is a server or account issue, not an Outlook client issue. Check https://status.office.com for M365 outages.
- If webmail works but Outlook doesn't → proceed with fix path below.

## Standard fix path (try in order)

### 1. Restart Outlook
Quit Outlook completely (not just close the window), then relaunch.
- macOS: Cmd+Q, or force-quit via Activity Monitor if unresponsive.
- Windows: check Task Manager for lingering OUTLOOK.EXE processes and end them.
This alone fixes transient sync hangs, stuck outbox items, and temporary auth failures.

### 2. Clear cached credentials
The #1 cause of "keeps asking for password" loops.
- macOS: Keychain Access → search "Microsoft" or "Exchange" or the user's email → delete those entries. Restart Outlook.
- Windows: Control Panel → Credential Manager → Windows Credentials → remove "MicrosoftOffice" entries and any entries matching the email address. Restart Outlook.
- If using M365 with MFA, have the user sign in at https://outlook.office.com first to confirm credentials work.

### 3. Rebuild the cache (OST/local data)
Outlook stores a local copy of the mailbox. If corrupted, sync breaks.
- macOS: Outlook → Preferences → Accounts → select account → "Empty Cache". Outlook re-downloads everything from the server.
- Windows: File → Account Settings → Account Settings → Data Files tab → note the .ost path. Close Outlook, rename the .ost file (don't delete — rename to .ost.bak), reopen Outlook. It creates a fresh OST and re-syncs.
- **This is safe for Exchange/M365 accounts** — all data lives on the server. The OST is just a cache.

### 4. Rebuild the profile
If cache rebuild didn't fix it, the profile itself may be corrupted.
- macOS: Outlook profiles live in `~/Library/Group Containers/UBF8T346G9.Office/Outlook/Outlook 15 Profiles/`. Rename the folder, relaunch Outlook — it creates a fresh profile. Re-add the email account.
- Windows: Control Panel → Mail → Show Profiles → Add a new profile. Set it as default. Open Outlook with the new profile.
- The old profile is preserved (renamed, not deleted) so nothing is lost.

### 5. Repair Office installation
If Outlook still crashes or misbehaves with a fresh profile:
- macOS: Download the latest Office installer from https://office.com and reinstall over the existing installation.
- Windows: Settings → Apps → Microsoft Office → Modify → choose "Online Repair" (not Quick Repair — Online Repair is more thorough).

> This sequence resolves ~95% of Outlook sync and crash issues.

## Caveats
- If the OST file is >10 GB, step 3 (cache rebuild) takes 30+ minutes. Warn the user and suggest doing it over lunch or end of day.
- If this is a **shared mailbox** issue (not the user's own mailbox), it's almost always a permissions problem, not a profile issue. Don't rebuild — check if the user still has delegate access.
- If Outlook crashes on launch and you can't even open it: on Windows, try `outlook.exe /safe` to start in Safe Mode with add-ins disabled. Common culprit add-ins: antivirus email scanners, CRM plugins, old Teams add-in.
- If the user says "search doesn't work" but everything else is fine, that's a search index issue:
  - macOS: Spotlight indexes Outlook data. Reindex: Outlook → Preferences → Spotlight rebuild.
  - Windows: File → Options → Search → Indexing Options → Advanced → Rebuild.

## Key signals
- **"It worked yesterday"** → likely an expired auth token. Jump to step 2 (credentials).
- **"Nobody in the office can send email"** → server-side outage, not local. Check https://status.office.com before touching anything.
- **"Only calendar won't sync"** → usually permissions on a shared calendar, not a sync issue. Have the calendar owner re-share it.
- **"Keeps crashing after macOS/Windows update"** → Office version may be incompatible with the new OS. Jump to step 5 (repair/update Office).
- **"Email stuck in outbox"** → check attachment size first (>25 MB fails for most providers). If small, toggle Work Offline on/off (Outlook menu), then back online.
- **"Your mailbox is full"** → Exchange quota hit. User needs to archive or delete old mail. This is not an Outlook bug.

## Escalation
If all 5 steps fail:
- Check if the problem is account-specific: try adding a different email account to the same Outlook. If the other account works, the issue is server-side for that specific account.
- For enterprise/M365 accounts: the IT admin may need to check Conditional Access policies, app passwords, or account lockouts in Azure AD.
- For persistent crashes: collect the crash log and Office version number for Microsoft support.
