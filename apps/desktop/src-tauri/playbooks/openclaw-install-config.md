---
name: openclaw-install-config
description: Install and configure OpenClaw with safe token handling and verification
platform: all
last_reviewed: 2026-03-07
author: noah-team
type: system
---

# OpenClaw Install & Config

## When to activate
User asks to install, set up, configure, or re-install OpenClaw. Trigger on phrases like "set up OpenClaw", "install OpenClaw", "fresh OpenClaw setup", or "make OpenClaw work again."

## Quick check
Run `shell_run` with `openclaw --version`.
- If command succeeds, OpenClaw is installed; continue to configuration and verification.
- If command fails, start at install step 1.

## Standard fix path (try in order)

### 1. Set context clearly
First explain what OpenClaw is in plain language: an AI assistant that can execute workflows, connect model providers, and use chat channels like Telegram/Discord.
- State that setup has two parts: install + configure tokens.

### 2. Install OpenClaw
Run `shell_run` with the platform-appropriate installer command (Homebrew/script/package manager based on environment).
- Verify immediately with `shell_run` `openclaw --version`.
- If missing after install, check PATH and shell profile before repeating install.

### 3. Gather config requirements
Tell the user exactly what is needed:
- Model provider credential (provider API key/token)
- Chat channel token (Telegram/Discord/etc.) when they want chat integrations
- Optional gateway/auth token depending on provider mode

### 4. Capture secrets safely
Never ask users to paste secrets in chat text if secure field capture is available.
- Direct user to the secure token input flow (Settings secure capture) so secrets are written to local config without appearing in conversation history.
- Confirm that Noah cannot read back the secret values after save.
- After install, your next action should explicitly direct the user to secure capture (not app wizard-only handoff).

### 5. Verify configuration end-to-end
Run validation checks after each major step:
- Runtime/env check (Node/runtime prerequisites)
- OpenClaw install check (`openclaw --version`)
- Provider token configured
- Chat channel token configured
- Ready for first chat test
- Do NOT mark setup complete after install only.
- If you cannot run token checks directly, keep the session in setup mode and guide the user through secure capture + verification checkpoints.

### 6. Save non-secret profile for future repair
After successful setup, save an OpenClaw profile to knowledge so future troubleshooting can use the user's actual setup.
- Include version, platform, enabled channels, and last verification time.
- Never store raw token values in knowledge.

> This path resolves ~90% of OpenClaw setup requests with one guided pass.

## Caveats
- If user previously removed OpenClaw, stale shell/profile references can remain. Verify command path after install.
- If Node/runtime is below requirement, install/update runtime first; OpenClaw install may appear successful but fail later.
- If Telegram/Discord token is invalid, install can still pass while channel setup fails. Treat channel verification as a separate checkpoint.
- If user is non-technical, keep instructions one action at a time and avoid dense CLI blocks.

## Key signals
- "I just want a clean install" -> do install + minimal provider setup first, then optional channels.
- "I don't want tokens in chat" -> force secure capture path and avoid any text-based token request.
- "It used to cause high CPU" -> include immediate post-install sanity check and confirm no runaway process.
- "Help me connect Telegram" -> guide bot creation step, then secure token capture, then channel verification.
- "The app opened a wizard" -> do not hand off and stop. Continue as Noah: explain the exact next checkpoint and stay in guided setup.

## Tools referenced
- `activate_playbook` — load this protocol for setup consistency
- `shell_run` — install checks and version verification
- `search_knowledge` — detect prior OpenClaw incidents/config context
- `write_knowledge` — save non-secret OpenClaw setup profile
- `list_knowledge` — verify profile was saved and available for future repair

## Escalation
If install and token checks pass but OpenClaw still cannot complete first chat:
- Ask user to share the non-secret error output from validation/doctor commands.
- Recommend upgrading runtime + reinstalling OpenClaw once.
- If still failing, escalate with exact environment details (OS, runtime version, OpenClaw version, enabled channel) and avoid repeated blind reinstalls.

## Completion criteria
Only return a final setup [DONE] when ALL are true:
- OpenClaw is installed and version check succeeds.
- User explicitly confirms provider token is configured via secure capture (or equivalent non-chat method).
- User explicitly confirms at least one intended chat channel token is configured when channel usage is requested.
- User has a concrete next action for first chat verification (or verification already passed).

Install-only success is not setup success.
Wizard-only handoff is not setup success.
