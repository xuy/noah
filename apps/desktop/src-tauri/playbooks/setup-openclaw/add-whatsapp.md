---
name: setup-openclaw/add-whatsapp
description: Add or reconfigure WhatsApp for OpenClaw
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# Add WhatsApp Channel

Connect WhatsApp to OpenClaw. Uses WhatsApp Web protocol (Baileys).

## Prerequisites
OpenClaw must be installed (`openclaw --version` should work).

## Step 1: Link WhatsApp Account

Run:
```
openclaw channels login --channel whatsapp
```

This displays a QR code. Tell the user:

> Open WhatsApp on your phone → Settings → Linked Devices → Link a Device →
> Scan the QR code shown in the terminal.

Use WAIT_FOR_USER — the user needs to scan the QR code on their phone.

Credentials are stored at `~/.openclaw/credentials/whatsapp/`.

**Dedicated vs personal number:** A dedicated phone number for the bot is
recommended for clearer boundaries. But personal-number mode works too —
OpenClaw has safeguards (skips self-chat read receipts, ignores self-mentions).

## Step 2: Configure Access Policy

Ask who should be allowed to message the bot:

- **Pairing** (default): First message from each new sender requires approval.
  Approval requests expire after 1 hour, max 3 pending.
- **Allowlist**: Only specific phone numbers (E.164 format: `+8613812345678`).
- **Open**: Anyone can message.
- **Disabled**: WhatsApp DMs disabled.

Set via CLI:
```
openclaw config set channels.whatsapp.dmPolicy "pairing"
```

Or for allowlist:
```json5
// In ~/.openclaw/openclaw.json
{
  channels: {
    whatsapp: {
      dmPolicy: "allowlist",
      allowFrom: ["+8613812345678"]
    }
  }
}
```

## Step 3: Group Settings (Optional)

By default, groups require the bot to be @mentioned.

To change:
```
openclaw config set channels.whatsapp.groupPolicy "open"
```

Configure mention patterns for custom trigger words:
```json5
{
  agents: {
    defaults: {
      groupChat: {
        mentionPatterns: ["@openclaw", "小助手", "hey bot"]
      }
    }
  }
}
```

## Step 4: Verify

Restart gateway if it was running:
```
openclaw gateway restart
```

Check status: `openclaw channels status --probe`

Send a test message from another phone (or from a group) to verify.

Check logs if issues: `openclaw logs --follow`

## Troubleshooting

| Issue | Fix |
|-------|-----|
| QR code expired | Re-run `openclaw channels login --channel whatsapp` |
| Disconnected/reconnect loops | Run `openclaw doctor` |
| Group messages ignored | Check `groupPolicy`, mention requirements, `groupAllowFrom` |
| No response | Verify gateway is running: `openclaw gateway status` |

## Multi-Account Setup

For multiple WhatsApp numbers, use account IDs:
```
openclaw channels login --channel whatsapp --account work
```

Configure per-account in config:
```json5
{
  channels: {
    whatsapp: {
      accounts: {
        work: {
          dmPolicy: "allowlist",
          allowFrom: ["+8613812345678"]
        }
      }
    }
  }
}
```

## Tools referenced
- `shell_run` — openclaw CLI commands
- `ui_user_question` with options — access policy choice
- `ui_user_question` with `text_input` — phone numbers
- `ui_spa` with WAIT_FOR_USER — QR code scanning
