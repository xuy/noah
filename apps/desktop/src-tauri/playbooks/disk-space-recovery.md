---
name: disk-space-recovery
description: Find and reclaim disk space from caches, backups, and hidden consumers
platform: macos
---

# Disk Space Recovery

## When to activate
User reports: disk full, low storage warning, "not enough space" errors, can't install updates, can't download files, startup disk almost full.

## Protocol

### Step 1: Assess the situation
Run `mac_disk_usage` to get current disk usage.
- Note total space, used space, and free space.
- **Purgeable space:** macOS reports some space as "purgeable" — it's technically used but can be freed automatically when needed (e.g., Time Machine snapshots, optimized iCloud files). The actual available space is free + purgeable.

### Step 2: Automated scan
Run `disk_audit` to scan known space-hogging directories.
This returns a categorized breakdown sorted by size.

### Step 3: Categorize findings by risk

**SAFE to clean (no user confirmation needed):**
- System caches (`~/Library/Caches/`) — rebuilt automatically
- Browser caches — rebuilt automatically
- macOS log files older than 30 days
- Temporary files (`/tmp/`, `/var/folders/`)

**NEEDS USER CONFIRMATION (suggest but ask first):**
- Downloads folder — may contain files the user wants
- Trash (`~/.Trash/`) — user may have accidentally deleted something
- Homebrew cache (`~/Library/Caches/Homebrew/`) — will re-download if needed
- npm/yarn cache (`~/.npm/`, `~/.yarn/cache/`)
- pip cache (`~/Library/Caches/pip/`)
- Xcode DerivedData (`~/Library/Developer/Xcode/DerivedData/`) — rebuilds needed but slow
- Docker images/volumes — may contain important data
- iOS device backups (`~/Library/Application Support/MobileSync/Backup/`) — irreplaceable if not on iCloud

**DO NOT TOUCH (never clean without explicit instruction):**
- `.DocumentRevisions-V100` — macOS document versioning. Deleting can corrupt files.
- `/.Spotlight-V100` — Spotlight index. Deleting forces full re-index (hours of CPU).
- `/System/` — SIP-protected. Cannot and should not be modified.
- Time Machine backup drive — managed by the system.
- Any directory inside user's Documents, Desktop, or home folder.

### Step 4: Present recovery options
Present findings to user sorted by size (largest first).
For each item:
- Show the directory/category and size
- Indicate risk level (safe / needs confirmation / do not touch)
- Explain what it is in plain language

Example format:
```
Found 47 GB that could be recovered:
- Xcode DerivedData: 15 GB (build caches, safe to delete but rebuilds take time)
- Docker images: 12 GB (unused images can be pruned)
- Downloads folder: 8 GB (review before deleting)
- System caches: 6 GB (safe, rebuilds automatically)
- Trash: 4 GB (empty to reclaim)
- npm cache: 2 GB (safe, re-downloads as needed)
```

### Step 5: Clean with verification
After user confirms which categories to clean:
- Use `mac_clear_caches` for system caches.
- For other categories, explain the specific command needed and get confirmation.
- After cleaning, run `mac_disk_usage` again to verify space was recovered.
- Report how much space was freed.

### Step 6: Time Machine snapshots (hidden space hog)
Time Machine creates local snapshots that can consume significant space.
- These are not visible in Finder and don't show up in normal disk usage tools.
- macOS automatically deletes them when space is needed, but this can be slow.
- To check: `tmutil listlocalsnapshots /`
- The system will automatically purge these when disk space is critically low.
- DO NOT manually delete snapshots unless the user explicitly requests it and understands the implications.

### Step 7: iCloud optimization
If the user has iCloud Drive enabled:
- "Optimize Mac Storage" may be downloading files locally that should be kept in the cloud.
- Check System Settings > Apple ID > iCloud > iCloud Drive.
- Files showing in Finder but stored in iCloud don't use local space (cloud icon in Finder).

## Common large directories
| Directory | What it is | Safe to clean? |
|---|---|---|
| `~/Library/Caches/` | App caches | Yes |
| `~/Library/Developer/Xcode/DerivedData/` | Xcode build artifacts | Yes, but rebuilds needed |
| `~/Library/Application Support/MobileSync/Backup/` | iOS backups | Only if backed up elsewhere |
| `~/Library/Containers/com.docker.docker/` | Docker data | Check what images/volumes exist first |
| `~/Library/Caches/Homebrew/` | Downloaded packages | Yes |
| `~/.npm/` or `~/.yarn/cache/` | Package manager caches | Yes |
| `/Library/Updates/` | macOS update downloads | Yes, will re-download |
| `~/Downloads/` | User downloads | Ask user first |
