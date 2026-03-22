---
name: setup-backup
description: Set up Time Machine backup on macOS or configure basic backup strategy
platform: macos
last_reviewed: 2026-03-07
author: noah-team
source: bundled
emoji: 🛡️
---

# Set Up Backup

## When to activate
User wants to set up backups, configure Time Machine, asks about data protection, or says they're worried about losing files.

## Step 1: Check current backup status
Run `tmutil destinationinfo` to check if Time Machine is already configured.
- If configured and running: show status and ask if they want to change settings.
- If not configured: continue to Step 2.

## Step 2: Identify backup destination
Ask the user what they want to back up to:
- External hard drive (USB)
- Network drive (NAS/Time Capsule)
- They don't have a drive yet (recommend one)

## Step 3: Set up Time Machine
For USB drives:
1. Check connected drives with `diskutil list`
2. Identify the right volume
3. Enable Time Machine: `tmutil setdestination /Volumes/<DriveName>`
4. Start first backup: `tmutil startbackup`

For network drives: ask for the network path using `text_input`, then mount and configure.

Use WAIT_FOR_USER if the user needs to plug in a drive first.

## Step 4: Configure exclusions (optional)
Ask if the user wants to exclude any folders (large folders, VMs, etc.).
Common exclusions: ~/Downloads, ~/Library/Caches, node_modules.
Use `tmutil addexclusion <path>` for each.

## Step 5: Verify backup is running
Check with `tmutil status` and `tmutil latestbackup`.
Confirm the first backup has started and show estimated time.

## Tools referenced
- `mac_run_command` — tmutil commands
- `ui_user_question` — backup destination choice, exclusion paths
- `ui_spa` with WAIT_FOR_USER — plugging in drives
