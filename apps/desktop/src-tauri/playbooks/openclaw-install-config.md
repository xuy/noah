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
- Direct user to OpenClaw's secure token input flow (wizard/secure prompts) so secrets are written to local config without appearing in conversation history.
- Confirm that Noah cannot read back the secret values after save.
- When asking about providers, use human-readable names ("OpenAI", "Anthropic", "OpenRouter"), not code-style shorthand lists.
- After install, your next action should explicitly direct the user to secure capture (not app wizard-only handoff).
- Do not run interactive TUI commands like `openclaw config` / `openclaw configure` via `shell_run` as if Noah can operate arrow-key prompts. Instead, tell the user exactly what to run and what to select, then wait for confirmation.
- If a command is blocked as interactive, do not pretend it launched. Treat that as a hard stop for that command path and switch to explicit user-guided steps.

### 5. Verify configuration end-to-end
Run validation checks after each major step:
- Runtime/env check (Node/runtime prerequisites)
- OpenClaw install check (`openclaw --version`)
- Provider token configured
- Chat channel token configured
- Ready for first chat test
- Do NOT mark setup complete after install only.
- If you cannot run token checks directly, keep the session in setup mode and guide the user through secure capture + verification checkpoints.

### 5a. User handoff stages (`await_user_step` pattern)
When the user must do something outside Noah (provider console, Telegram BotFather, Discord portal), use a clear handoff style in chat:
- Say exactly this is a user step and Noah is waiting.
- Provide clickable links and a short checklist (3-5 bullets max).
- Keep instructions plain-language; avoid command-heavy blocks for non-technical users.
- For optional channel setup, always offer: "skip for now and finish basic setup".

Preferred references:
- Telegram bot token: https://core.telegram.org/bots/tutorial
- BotFather entry point: https://t.me/BotFather
- OpenClaw installer docs: https://docs.openclaw.ai/install/installer

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
- "I don't see the wizard" -> do not pretend it started. Ask the user to run the exact command/app and confirm visible output before continuing.

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

## FSM
```json
{
  "version": 1,
  "machine": "openclaw-install-config",
  "initial_state": "INSTALL_CHECK",
  "states": {
    "INSTALL_CHECK": {"summary": "Confirm OpenClaw is installed and runnable."},
    "INSTALL": {"summary": "Install OpenClaw and verify version output."},
    "PROVIDER_CAPTURE": {"summary": "Capture provider secret via secure form."},
    "PROVIDER_VERIFY": {"summary": "Verify provider configuration works."},
    "CHANNEL_CAPTURE": {"summary": "Optional channel token capture via secure form."},
    "DONE": {"summary": "Basic setup complete with optional channel status recorded."}
  },
  "events": {
    "install_verified": {"source": "llm_or_runtime"},
    "install_missing": {"source": "llm_or_runtime"},
    "provider_captured": {"source": "llm_or_runtime"},
    "provider_verified": {"source": "llm_or_runtime"},
    "channel_skipped": {"source": "user_event"},
    "channel_captured": {"source": "llm_or_runtime"}
  },
  "transitions": [
    {
      "from": "INSTALL_CHECK",
      "to": "PROVIDER_CAPTURE",
      "goal": "OpenClaw install is confirmed.",
      "acceptance": ["Version check succeeds or user confirms OpenClaw is already installed."],
      "triggers": ["install_verified"]
    },
    {
      "from": "INSTALL_CHECK",
      "to": "INSTALL",
      "goal": "OpenClaw is missing and needs install.",
      "acceptance": ["Install check fails."],
      "triggers": ["install_missing"]
    },
    {
      "from": "INSTALL",
      "to": "PROVIDER_CAPTURE",
      "goal": "Install completed and verified.",
      "acceptance": ["Version check succeeds after install."],
      "triggers": ["install_verified"]
    },
    {
      "from": "PROVIDER_CAPTURE",
      "to": "PROVIDER_VERIFY",
      "goal": "Provider credential captured securely.",
      "acceptance": ["Secure form submission recorded."],
      "triggers": ["provider_captured", "secure_form_submitted"]
    },
    {
      "from": "PROVIDER_VERIFY",
      "to": "CHANNEL_CAPTURE",
      "goal": "Provider is verified and optional channel decision remains.",
      "acceptance": ["Provider check passes."],
      "triggers": ["provider_verified"]
    },
    {
      "from": "CHANNEL_CAPTURE",
      "to": "DONE",
      "goal": "Basic setup is complete whether channel is configured or skipped.",
      "acceptance": ["Channel captured or explicitly skipped."],
      "triggers": ["channel_captured", "channel_skipped", "user_skip_optional"]
    }
  ],
  "terminal": {
    "states": ["DONE"],
    "goal": "Install + provider configured; channel optional."
  },
  "guards": {
    "blocked_commands": {
      "*": ["openclaw configure"],
      "PROVIDER_VERIFY": ["openclaw doctor --fix", "openclaw channels add"],
      "CHANNEL_CAPTURE": ["openclaw channels add"]
    }
  }
}
```
