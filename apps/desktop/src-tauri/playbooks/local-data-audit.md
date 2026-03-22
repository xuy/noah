---
name: local-data-audit
description: Audit a device for locally stored sensitive or company data — for offboarding or device reassignment
platform: all
last_reviewed: 2026-03-17
author: noah-team
source: bundled
emoji: 📂
---

# Local Data Audit

Scans the device for locally stored company data, personal files, and sensitive documents. Designed for employee offboarding or device reassignment — ensures nothing important is lost and no sensitive data remains on the device.

**Important**: This playbook only reads and reports. It does not delete anything without explicit user confirmation.

## When to activate
Employee offboarding, device reassignment, compliance audit, or when verifying that company data has been properly backed up before a wipe.

## Standard check path

### 1. Check user profile directories
Scan the user's home directory for document-like files:
- Count files by type in `~/Documents`, `~/Desktop`, `~/Downloads`.
- Report total size and file count for each directory.
- Flag large files (>100MB) that may need special handling.

### 2. Check cloud sync status
Determine if the user has cloud storage synced locally:
- **OneDrive**: Check for `~/OneDrive` or `~/Library/CloudStorage/OneDrive*`.
- **Google Drive**: Check for `~/Google Drive` or `~/Library/CloudStorage/GoogleDrive*`.
- **Dropbox**: Check for `~/Dropbox` or `~/.dropbox`.
- **iCloud**: Check for `~/Library/Mobile Documents`.

Report which services are synced and their local folder sizes. Synced data is generally safe (exists in the cloud), but confirm before removing.

### 3. Check for sensitive file patterns
Search for files that commonly contain sensitive data:
- Files with names containing: `password`, `credential`, `secret`, `key`, `token`, `.env`, `config`.
- Files with extensions: `.pem`, `.key`, `.pfx`, `.p12`, `.kdbx` (password databases).
- Check `~/.ssh/` for private keys.
- Check for database files (`.db`, `.sqlite`) in common locations.

Report found files with paths and sizes. Do not read or display file contents.

### 4. Check application data
Look for locally stored application data that may contain company information:
- Email client local cache/storage size (Outlook, Thunderbird, Apple Mail).
- Chat application data (Slack, Teams, Zoom).
- Browser profiles with bookmarks and history.

Report which applications have significant local data stores.

### 5. Check for code repositories
Search for git repositories that may contain company code:
- Look for `.git` directories in common locations (`~`, `~/src`, `~/code`, `~/projects`, `~/repos`, `~/dev`, `~/work`).
- Report repository names and sizes.
- Note: code repos often contain credentials in config files or `.env` — flag these.

### 6. Summarize and recommend
Present a summary table:
- Which directories have company data
- Which cloud services are synced
- Any sensitive files found
- Recommended actions (backup, transfer ownership, or safe to wipe)

Ask the user whether to proceed with any cleanup, or just generate the report for the admin.

## Caveats
- **This is a read-only audit.** Nothing is deleted unless the user explicitly requests it.
- **Encrypted volumes** (FileVault, BitLocker) are searched normally when unlocked. If the device is encrypted and locked, this playbook cannot access the data.
- **Cloud-synced folders** may contain stubs (files not fully downloaded). The reported size may differ from what's actually on disk.
- **Time-sensitive**: Run this audit before disabling the user's account, as some cloud folders may become inaccessible after account deactivation.

> Steps 1-6 cover ~85% of locally stored company data. Most commonly missed: git repositories with embedded credentials and chat application local caches.

## Key signals
- **"Employee leaving, need to check for company data"** → run all steps, focus on cloud sync status (step 2) and sensitive files (step 3).
- **"Device being wiped, need to verify backup"** → focus on steps 1-2, confirm cloud sync is current before proceeding.
- **"Compliance audit"** → run all steps, generate full report for documentation.
- **"Developer offboarding"** → prioritize step 5 (code repos) and step 3 (credentials/keys).

## Escalation
If the audit reveals:
- Large amounts of unsynced local data — pause and arrange a backup before proceeding with offboarding.
- Credentials or secrets in plaintext — notify security team. These should be rotated, not just deleted.
- Personal data mixed with company data — consult HR/legal on data handling requirements.
- If the user is uncooperative or the device may be tampered with — involve IT security for supervised data collection.

## Tools referenced
- Shell commands — file listing, directory size calculation, find/search
- Disk usage tools — checking folder sizes
