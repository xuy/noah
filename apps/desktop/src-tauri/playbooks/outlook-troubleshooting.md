---
name: outlook-troubleshooting
description: Fix Outlook sync failures, crashes, calendar issues, and profile corruption
platform: all
---

# Outlook Troubleshooting

## When to activate
User reports: Outlook won't sync, email stuck in outbox, calendar not updating, Outlook crashes on launch, search not working, can't add account, "need password" loop, Outlook slow.

## Protocol

### Step 1: Identify the problem type
Ask or determine which category:
- **Sync failure** — emails not arriving/sending, calendar not updating → Step 2
- **Crash / won't launch** — app crashes immediately or during use → Step 3
- **Authentication loop** — keeps asking for password → Step 4
- **Search broken** — search returns no results or is very slow → Step 5
- **Performance** — Outlook is slow, beach ball, high CPU → Step 6

### Step 2: Sync failures
**Email stuck in outbox:**
1. Check if the message has a large attachment (>25 MB for most providers).
2. Try: switch Outlook to offline mode (Outlook menu > Work Offline), then back online.
3. If stuck: drag the message from Outbox to Drafts, then resend.

**Not receiving new email:**
1. Check internet connectivity first (can the user browse the web?).
2. Check the account status: Outlook > Preferences > Accounts — look for error icons.
3. Try: manually sync with Cmd+K (macOS) or Send/Receive button (Windows).
4. Check if emails are going to Junk/Focused Inbox instead.
5. For Exchange/Microsoft 365: the issue may be server-side. Check https://status.office.com.

**Calendar not syncing:**
1. Verify the calendar is checked/visible in the sidebar.
2. For shared calendars: permissions may have changed. Ask user to re-request access.
3. Try removing and re-adding the account (last resort — see Step 7).

### Step 3: Crashes
**On macOS:**
- Check crash logs with `crash_log_reader` for "Microsoft Outlook".
- Common fix: remove Outlook preferences — move `~/Library/Group Containers/UBF8T346G9.Office/Outlook/Outlook 15 Profiles/` to Desktop as backup, then relaunch.
- If crash mentions "identity" or "database": the Outlook profile is corrupted → Step 7.

**On Windows:**
- Start Outlook in Safe Mode: hold Ctrl while clicking Outlook icon, or run `outlook.exe /safe`.
- If Safe Mode works: a bad add-in is the cause. Disable add-ins one by one (File > Options > Add-ins > Manage COM Add-ins).
- Common culprit add-ins: antivirus email scanners, CRM plugins, old Teams add-in.

**On both platforms:**
- If Outlook crashes immediately: try creating a new profile to test (doesn't delete old data).
- Office repair: run the Microsoft repair tool (macOS: re-download from office.com, Windows: Apps & Features > Microsoft Office > Modify > Repair).

### Step 4: Authentication / password loops
**"Need password" or "Enter credentials" loop:**
1. This is usually a cached credential issue, not a real password problem.
2. **macOS fix:** Open Keychain Access, search for "Exchange" or "Microsoft" or the email address, delete those entries. Restart Outlook.
3. **Windows fix:** Control Panel > Credential Manager > Windows Credentials, remove entries for "MicrosoftOffice" or the email address. Restart Outlook.
4. If using Microsoft 365 with MFA: ensure the Authenticator app is working. Try signing in at https://outlook.office.com in a browser first.
5. **Modern Auth issue:** Some older Outlook versions don't support Modern Authentication. Update Office to the latest version.
6. For organizational accounts: check with IT if the account is locked or if Conditional Access policies changed.

### Step 5: Search not working
**macOS:**
- Outlook uses Spotlight for search. If Spotlight indexing is broken, search fails.
- Check: is Spotlight indexing? (`mdutil -s /` shows status).
- Fix: rebuild Outlook search index — Outlook menu > Preferences > Accounts > select account > Rebuild (older versions) or reindex Spotlight for the Outlook profile folder.

**Windows:**
- Fix: File > Options > Search > Indexing Options > Advanced > Rebuild.
- Alternative: run `outlook.exe /resetnavpane` to reset the navigation pane and search.

**Both:**
- If search is slow but works: the mailbox may be very large (>10 GB). Suggest archiving old mail.

### Step 6: Performance issues
**Outlook is slow:**
1. Check mailbox size — very large mailboxes (>10 GB) cause slowness.
2. Check for too many folders or rules — each rule runs on every incoming message.
3. Disable unnecessary add-ins (especially on Windows).
4. **macOS:** Check if Spotlight is re-indexing (mds process high CPU).
5. **Windows:** Try disabling hardware acceleration: File > Options > Advanced > uncheck "Disable hardware graphics acceleration" (confusing double-negative — check the box to disable GPU).

**High CPU usage:**
- On macOS: check process list for "Microsoft Outlook" CPU usage.
- If Outlook is syncing a large mailbox for the first time, high CPU is expected temporarily.
- If persistent: try repairing the profile (Step 7).

### Step 7: Profile reset (graduated approach)
Try these in order:
1. **Clear cache:** Force Outlook to re-download from server (doesn't lose local data if using Exchange/M365).
   - macOS: Outlook > Preferences > Accounts > select account > Empty Cache.
   - Windows: File > Account Settings > Account Settings > Data Files tab > examine the .ost file location.
2. **New profile:** Create a new Outlook profile to test.
   - macOS: Outlook stores profiles in `~/Library/Group Containers/UBF8T346G9.Office/Outlook/`.
   - Windows: Control Panel > Mail > Show Profiles > Add.
3. **Repair Office installation:** Use Microsoft's built-in repair.
4. **Complete reinstall:** Uninstall Office, clean up remaining files, reinstall.

## Common Outlook error messages
| Error | Meaning | Fix |
|---|---|---|
| "Cannot start Microsoft Outlook. Cannot open the Outlook window" | Corrupted profile or navigation pane | Run `outlook.exe /resetnavpane` (Windows) |
| "The operation failed" when sending | Server rejection or attachment too large | Check outbox, reduce attachment size |
| "Your mailbox is full" | Exchange quota exceeded | Archive or delete old messages |
| "Certificate error" / "Security certificate has expired" | SSL/TLS issue | Check date/time is correct, update Office |
