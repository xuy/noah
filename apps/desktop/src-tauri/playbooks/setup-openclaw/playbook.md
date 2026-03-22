---
name: setup-openclaw
description: Install and configure OpenClaw (龙虾) — an AI gateway connecting Claude to WhatsApp, Telegram, and other messaging channels
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# Set Up OpenClaw

OpenClaw (also known as "龙虾" / "longxia" in Chinese communities) is an AI
gateway that connects LLMs (Claude, GPT, etc.) to messaging channels (WhatsApp,
Telegram, Feishu/飞书, Discord, etc.). It runs locally on the user's machine
as a background service.

## When to activate
User wants to install OpenClaw, set up an AI assistant on WhatsApp/Telegram/
Feishu/Discord, configure an AI chatbot on messaging apps, or mentions
"openclaw", "龙虾", or "longxia".

## Step 1: Check Environment

Run `node --version` to check Node.js.
- Need Node.js 22+. If missing or old, activate `setup-openclaw/install-node`.
- Check if `openclaw` is already installed: `which openclaw` or `openclaw --version`.
- If already installed, skip to Step 3.

## Step 2: Install OpenClaw

Install via the official installer script:
```
curl -fsSL https://openclaw.ai/install.sh | bash
```

If the user already has Node 22+, they can also use:
```
npm install -g openclaw@latest
```

After install, verify: `openclaw --version`.

If `openclaw` command is not found, the npm global bin directory may not be in
PATH. Check with `npm prefix -g` and add `$(npm prefix -g)/bin` to PATH.

## Step 3: Run Onboarding Wizard

The wizard handles model selection, authentication, channel setup, and daemon
installation all in one flow:
```
openclaw onboard --install-daemon
```

This is interactive — use WAIT_FOR_USER. The wizard will:
1. Ask for a model provider and API key (or OAuth login)
2. Choose a workspace directory (default: `~/.openclaw/workspace`)
3. Configure the gateway (port 18789 by default)
4. Optionally set up messaging channels (WhatsApp, Telegram, etc.)
5. Install as a system daemon (launchd on macOS, systemd on Linux)

**For Chinese users:** The wizard supports several Chinese model providers.
If the user wants to use a Chinese model, activate `setup-openclaw/china-models`
for detailed guidance on Volcano Engine, Moonshot, DeepSeek, etc.

## Step 4: Verify Gateway

Check that the gateway is running:
```
openclaw gateway status
```

Expected: "Runtime: running" and "RPC probe: ok".

If not running, start it:
```
openclaw gateway
```

The Control UI is available at `http://127.0.0.1:18789/` — tell the user
they can open this in a browser to manage settings visually.

## Step 5: Set Up a Messaging Channel

If the wizard didn't set up a channel, or the user wants to add another one.
Ask which platform the user uses — common choices by region:

- **Chinese users**: Feishu (飞书) is most common for teams/work
- **International**: WhatsApp or Telegram

**Feishu (飞书)**: Two options (mutually exclusive):
- **Built-in plugin** (simpler, bot identity): activate `setup-openclaw/add-feishu`
- **Official plugin** (user identity, documents/calendar/tasks): activate `setup-openclaw/add-feishu-official`
If unsure, ask the user if they need the bot to operate Feishu documents,
calendar, or tasks. If yes → official plugin. If just chat → built-in.

**WhatsApp**:
```
openclaw channels login --channel whatsapp
```
This shows a QR code. Tell the user:
> Open WhatsApp on your phone → Settings → Linked Devices → Link a Device →
> Scan the QR code shown in the terminal.

Use WAIT_FOR_USER — the user needs to scan the QR code.

**Telegram**: Activate `setup-openclaw/add-telegram` for guided setup.

After linking, verify: `openclaw channels status --probe`

## Step 6: Test

Send a test message on the connected channel. The AI should respond.

Check logs if something doesn't work:
```
openclaw logs --follow
```

Run diagnostics:
```
openclaw doctor
```

## Step 7: Done

Show a done card summarizing:
- OpenClaw version
- Gateway port (default 18789)
- Connected channels
- Model provider in use
- How to check status: `openclaw status`
- How to view logs: `openclaw logs --follow`
- How to open Control UI: `openclaw dashboard`
- Config file location: `~/.openclaw/openclaw.json`

## Available Modules

After the core setup, the user can add optional features:

- **setup-openclaw/configure** — Edit OpenClaw configuration (models, channels,
  sessions, cron jobs, etc.)
- **setup-openclaw/add-feishu** — Add Feishu (飞书) channel (built-in plugin, bot identity)
- **setup-openclaw/add-feishu-official** — Add Feishu official plugin (user identity, documents/calendar/tasks)
- **setup-openclaw/add-telegram** — Add Telegram as a messaging channel
- **setup-openclaw/add-whatsapp** — Add or reconfigure WhatsApp
- **setup-openclaw/china-models** — Set up Chinese model providers
  (Volcano Engine, Moonshot, DeepSeek, Qwen, GLM/ZhiPu)
- **setup-openclaw/config-reference** — Comprehensive config field reference
  (load this when you need to look up specific config fields)
- **setup-openclaw/troubleshooting** — Diagnostic commands and common fixes
- **setup-openclaw/uninstall** — Completely uninstall OpenClaw (stop services,
  delete config, remove CLI and desktop app)

When setup is complete, ask the user if they'd like to configure any
of these optional modules.

## Escalation
- If install fails: check Node.js version, try `npm install -g openclaw@latest` directly
- If gateway won't start: run `openclaw doctor` and check port conflicts
- If channel won't link: run `openclaw doctor` and check logs
- If `openclaw` command not found after install: check PATH includes npm global bin

## Tools referenced
- `shell_run` — install openclaw, run CLI commands
- `ui_user_question` with options — channel choice, model provider
- `ui_user_question` with `secure_input` — API keys
- `ui_spa` with WAIT_FOR_USER — QR scanning, wizard interaction
