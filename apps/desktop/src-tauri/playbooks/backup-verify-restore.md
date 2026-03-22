---
name: backup-verify-restore
description: Verify backup integrity by checking status, timestamps, and testing a real file restore
platform: all
last_reviewed: 2026-03-17
author: noah-team
source: bundled
emoji: 💾
---

# Backup Verify & Restore Test

Verifies that the device's backup system is working correctly by checking the backup tool status, confirming the last backup timestamp, and performing a test restore of a known file. This proves backups are not just running but actually recoverable.

## When to activate
Periodic backup verification, after setting up a new backup, compliance audit, or when the admin wants to confirm RPO/RTO for a device.

## Standard check path

### 1. Identify the backup tool
Detect which backup system is in use:
- **macOS**: Check Time Machine status via `tmutil status` and `tmutil latestbackup`.
- **Windows**: Check File History via `fhmanagew.exe -status`, or Windows Backup settings.
- **Linux**: Check for Timeshift (`timeshift --list`), Borg (`borg list`), Restic (`restic snapshots`), or Deja Dup.

If no backup tool is detected, report this immediately and recommend setting one up.

### 2. Check last backup timestamp
For the detected backup tool:
- **Time Machine**: Parse the date from `tmutil latestbackup` output.
- **File History**: Check the last backup timestamp from the File History log.
- **Timeshift**: Parse the most recent snapshot date from `timeshift --list`.
- **Borg/Restic**: Parse the most recent archive/snapshot timestamp.

Calculate hours since last backup. Report:
- < 24h: Good — backup is recent.
- 24h-7d: Warning — backup may be stale.
- > 7d: Critical — backup is significantly out of date.

### 3. Check backup destination health
Verify the backup destination is accessible:
- **Time Machine**: Check if the backup volume is mounted (`tmutil destinationinfo`).
- **Windows File History**: Verify the backup drive is connected.
- **Network backups**: Test connectivity to the backup server/NAS.
- **Cloud backups**: Test connectivity to the cloud endpoint.

Report: destination type (local drive, network, cloud), available space, and connectivity status.

### 4. Test restore of a known file
Perform an actual restore test:
- Create a small test file in a known location (e.g., `~/Desktop/noah-backup-test.txt` with a timestamp).
- Wait briefly, then verify it's included in the backup (or use the latest backup).
- Attempt to restore a known file from the most recent backup to a temporary location.
  - **Time Machine**: `tmutil restore <backup_path> /tmp/restore-test/`
  - **Borg**: `borg extract --path <file> <repo>::<archive>`
  - **Restic**: `restic restore <snapshot> --target /tmp/restore-test/ --include <file>`
- Verify the restored file matches the original (compare size and content).

Report: restore succeeded/failed, time taken, file integrity check result.

### 5. Document RPO/RTO
Based on the findings, calculate and report:
- **RPO (Recovery Point Objective)**: Maximum data loss = time since last backup.
- **RTO (Recovery Time Objective)**: Estimated restore time based on backup size and destination speed.
- **Backup frequency**: How often backups run (continuous, hourly, daily).
- **Retention**: How far back can you restore (if detectable).

Present this as a summary the admin can use for compliance documentation.

### 6. Clean up
Remove any test files created during the verification:
- Delete `~/Desktop/noah-backup-test.txt` if created.
- Delete the temporary restore directory (`/tmp/restore-test/`).

## Caveats
- **Restore test writes to /tmp** — this is safe and doesn't affect user data.
- **Time Machine restore requires the backup disk to be connected.** If it's a network backup, ensure the network is available.
- **Full system restore cannot be tested this way.** This only verifies file-level restore. Full bare-metal recovery requires booting from recovery media.
- **Encrypted backups** may require a password to restore. If the password is unknown, the backup is effectively unusable — flag this to the admin.

> Steps 1-5 cover ~90% of backup verification needs. Most commonly missed: verifying the backup destination has enough free space for continued backups.

## Key signals
- **"When was the last backup?"** → run steps 1-2 only. Quick check.
- **"Can we actually restore from this backup?"** → run steps 1-4. Full verification.
- **"Compliance audit needs backup documentation"** → run all steps, focus on step 5 (RPO/RTO documentation).
- **"Setting up a new backup, want to verify it works"** → run all steps after the first backup completes.

## Escalation
If verification reveals:
- No backup tool installed → recommend setting one up immediately. Use the `setup-backup` playbook.
- Backup destination is full or inaccessible → the admin needs to provision more storage or fix the network path.
- Restore test fails → the backup may be corrupted. Check backup logs, try restoring from an older snapshot, or reconfigure the backup.
- Backup is encrypted and password is unknown → this is a critical issue. The backup is unrecoverable. Document and escalate to the admin.

## Tools referenced
- Shell commands — backup tool queries, file creation, restore commands
- Disk usage tools — checking backup destination space
