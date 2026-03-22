---
name: setup-openclaw/uninstall
description: Completely uninstall OpenClaw — stop services, remove config, uninstall CLI
platform: all
last_reviewed: 2026-03-10
author: noah-team
source: bundled
emoji: 🦞
---

# Uninstall OpenClaw

Completely remove OpenClaw from the user's system: stop the gateway service,
remove service definitions, delete configuration and state, uninstall the CLI,
and clean up any leftover profiles. This is destructive and irreversible.

## When to activate
User wants to uninstall OpenClaw, remove OpenClaw, mentions "openclaw killer",
or wants to completely clean up / get rid of OpenClaw / 龙虾.

## Step 1: Confirm Uninstall

Ask the user to confirm they want to completely remove OpenClaw. Explain:
- This will stop the gateway service
- Delete all configuration and state (`~/.openclaw/`)
- Uninstall the CLI tool
- Remove the macOS desktop app (if present)
- This cannot be undone

Use `ui_user_question` with options: **Yes, uninstall everything** / **Cancel**.
If the user cancels, use `ui_done` and stop.

## Step 2: Remove Everything

Execute all cleanup operations in this single step. Show progress via `ui_spa`
with `action_type: "RUN_STEP"` as you work through each sub-task.

### 2a. Stop and remove gateway service

First check if the `openclaw` CLI is available:
Run `shell_run` with `which openclaw`.

**If CLI is available (easy path):**
Run `shell_run` with `openclaw gateway stop`. Ignore errors (may already be stopped).
Run `shell_run` with `openclaw gateway uninstall`. Ignore errors.

**If CLI is NOT available (manual path):**

On macOS:
- Check for service: `launchctl list | grep openclaw`
- Stop it: `launchctl bootout gui/$(id -u)/ai.openclaw.gateway` (ignore errors)
- Remove plist: `rm -f ~/Library/LaunchAgents/ai.openclaw.gateway.plist`
- Also clean legacy: `rm -f ~/Library/LaunchAgents/com.openclaw.gateway.plist`

On Linux:
- Stop service: `systemctl --user disable --now openclaw-gateway.service` (ignore errors)
- Remove unit file: `rm -f ~/.config/systemd/user/openclaw-gateway.service`
- Reload: `systemctl --user daemon-reload`

### 2b. Delete configuration and state

The main state directory is `~/.openclaw` (or `$OPENCLAW_STATE_DIR` if set).

Run `shell_run` with `rm -rf ~/.openclaw`.
This removes config, state, workspace, and all stored data.

Also check for multi-profile directories (legacy feature):
Run `shell_run` with `ls -d ~/.openclaw-* 2>/dev/null || echo "none"`.
If any exist, delete each one: `rm -rf ~/.openclaw-*`.

### 2c. Uninstall CLI

Try each package manager in order (only one will have it installed):

1. npm: `npm list -g openclaw 2>/dev/null && npm rm -g openclaw`
2. pnpm: `pnpm list -g openclaw 2>/dev/null && pnpm remove -g openclaw`
3. bun: `bun remove -g openclaw 2>/dev/null`

If none succeed, warn the user the CLI may need manual removal.

Verify removal: `which openclaw` should return "not found".

### 2d. Remove desktop app (macOS only)

On macOS, check for the desktop app:
Run `shell_run` with `ls /Applications/OpenClaw.app 2>/dev/null || echo "not found"`.
If it exists: `rm -rf /Applications/OpenClaw.app`.

Skip on Linux (no desktop app).

## Step 3: Done

Show a done card summarizing what was removed:
- Gateway service: stopped and removed
- Configuration: `~/.openclaw/` deleted
- Multi-profile directories: cleaned (if any existed)
- CLI: uninstalled
- Desktop app: removed (if applicable)
- Reminder: if the user wants to reinstall later, activate `setup-openclaw`

## Escalation
- If gateway service won't stop: `kill $(pgrep -f openclaw)` as a last resort
- If files can't be deleted: check permissions, may need `sudo`
- If CLI uninstall fails: `rm -f $(which openclaw)` to remove the binary directly

## Tools referenced
- `shell_run` — stop services, delete files, uninstall packages
- `ui_user_question` with options — confirm uninstall
- `ui_spa` with RUN_STEP — show progress during automated cleanup
- `ui_done` — completion summary
