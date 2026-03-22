---
name: setup-openclaw/configure
description: Edit OpenClaw configuration — models, channels, sessions, automation
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# Configure OpenClaw

Help the user modify their OpenClaw configuration. This covers common
configuration tasks — for a full field reference, activate
`setup-openclaw/config-reference`.

## Key Facts

- **Config file**: `~/.openclaw/openclaw.json` (JSON5 format — comments OK)
- **NOT** YAML and **NOT** `config.yaml` — it is always `openclaw.json`
- The file is optional; safe defaults apply if missing
- Use `openclaw configure` to run an interactive wizard
- Use `openclaw config get <path>` / `openclaw config set <path> <value>` for CLI edits
- Control UI at `http://127.0.0.1:18789/` has a Config tab for visual editing
- Most changes hot-reload without restart (except gateway port/bind/TLS)

## Step 1: Understand What to Change

Ask the user what they want to configure. Common tasks:

1. **Change the AI model** → Step 2
2. **Add/modify a channel** (WhatsApp, Telegram, etc.) → Step 3
3. **Set up automation** (cron jobs, heartbeats) → Step 4
4. **Adjust access control** (who can message the bot) → Step 5
5. **Other config** → load `setup-openclaw/config-reference` and look up the field

## Step 2: Change AI Model

Read current model: `openclaw config get agents.defaults.model.primary`

Set new model:
```
openclaw config set agents.defaults.model.primary "anthropic/claude-sonnet-4-5"
```

Common model IDs:
- `anthropic/claude-sonnet-4-5` — fast, capable (recommended default)
- `anthropic/claude-opus-4-5` — most capable, slower
- `openai/gpt-4o` — OpenAI alternative
- For Chinese providers, activate `setup-openclaw/china-models`

If switching providers, may need to set the API key:
```
openclaw config set env.ANTHROPIC_API_KEY "sk-ant-..."
```
Or use secure_input to collect the key, then write it.

## Step 3: Channel Configuration

Each channel lives under `channels.<provider>` in the config.

**Check current channels**: `openclaw channels status`

**Key fields per channel:**
- `enabled` — true/false
- `dmPolicy` — "pairing" (approve first message), "allowlist", "open", "disabled"
- `allowFrom` — list of allowed sender IDs (phone numbers for WhatsApp, user IDs for Telegram)
- `groupPolicy` — "open", "allowlist", "disabled"

Example — restrict WhatsApp to specific numbers:
```json5
{
  channels: {
    whatsapp: {
      dmPolicy: "allowlist",
      allowFrom: ["+8613812345678", "+8613987654321"]
    }
  }
}
```

To re-link a channel: `openclaw channels login --channel <name>`

## Step 4: Automation (Cron & Heartbeats)

**Heartbeats** — periodic check-ins from the agent:
```
openclaw config set agents.defaults.heartbeat.every "30m"
openclaw config set agents.defaults.heartbeat.target "last"
```
Target options: "last" (last active chat), a channel name, or "none".

**Cron jobs** — scheduled tasks:
```json5
{
  cron: {
    enabled: true,
    jobs: {
      "daily-summary": {
        schedule: "0 9 * * *",
        prompt: "Send me a daily summary of pending tasks",
        target: "whatsapp:+8613812345678"
      }
    }
  }
}
```

Check cron status: `openclaw cron status`

## Step 5: Access Control

**DM policies** (per channel):
- `pairing` — first message from new sender requires approval (default)
- `allowlist` — only senders in `allowFrom` list can interact
- `open` — anyone can message (use with caution)
- `disabled` — channel won't accept DMs

**Group policies:**
- By default, groups require the bot to be mentioned
- `groupPolicy: "open"` removes mention requirement
- Configure mention patterns: `agents.list[].groupChat.mentionPatterns`

**Manage pairings**: `openclaw pairing list --channel <channel>`

## Step 6: Apply & Verify

After editing config, most changes apply automatically (hot reload).

Verify: `openclaw doctor`

If you edited gateway port/bind settings, restart:
```
openclaw gateway restart
```

## Tools referenced
- `shell_run` — openclaw CLI commands
- `ui_user_question` with options — what to configure
- `ui_user_question` with `secure_input` — API keys
- `ui_user_question` with `text_input` — phone numbers, model names
