---
name: setup-openclaw/config-reference
description: Comprehensive OpenClaw configuration field reference
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# OpenClaw Configuration Reference

**Config file**: `~/.openclaw/openclaw.json` (JSON5 format)
**NOT** YAML, **NOT** `config.yaml` — always `openclaw.json`.

Use `openclaw configure` for interactive wizard, or edit the file directly.
Use `openclaw config get <path>` / `openclaw config set <path> <value>` for CLI.

## Agent & Model Settings

```json5
{
  agents: {
    defaults: {
      workspace: "~/.openclaw/workspace",  // agent working directory
      model: {
        primary: "anthropic/claude-sonnet-4-5",
        fallbacks: ["openai/gpt-4o"]       // failover models
      },
      models: { /* model catalog/allowlist */ },
      imageMaxDimensionPx: 1200,           // vision token optimization
      heartbeat: {
        every: "30m",                       // check-in interval
        target: "last"                      // "last", channel name, or "none"
      },
      sandbox: {
        mode: "non-main",                   // "off" | "non-main" | "all"
        scope: "session"                    // "session" | "agent" | "shared"
      },
      groupChat: {
        mentionPatterns: ["@openclaw"]      // regex patterns for group activation
      }
    },
    list: [
      // Multiple agent definitions with per-agent overrides
    ]
  }
}
```

## Channel Settings

All channels share the same field patterns:

```json5
{
  channels: {
    whatsapp: {
      enabled: true,
      dmPolicy: "pairing",        // "pairing" | "allowlist" | "open" | "disabled"
      allowFrom: ["+8613812345678"],
      groupPolicy: "open",        // "open" | "allowlist" | "disabled"
      groupAllowFrom: [],          // falls back to allowFrom if unset
      sendReadReceipts: true,
      mediaMaxMb: 50,
      accounts: { /* multi-account overrides */ }
    },
    telegram: {
      botToken: "${TELEGRAM_BOT_TOKEN}",
      enabled: true,
      dmPolicy: "pairing",
      streaming: "partial"        // "off" | "partial" | "block"
    },
    discord: { /* same pattern */ },
    signal: { /* same pattern */ },
    slack: { /* same pattern */ }
  }
}
```

**DM policies:**
- `pairing` — new senders need approval (expires 1h, max 3 pending)
- `allowlist` — only `allowFrom` senders allowed
- `open` — anyone can message
- `disabled` — channel off

## Gateway Settings

```json5
{
  gateway: {
    port: 18789,                   // default port
    reload: {
      mode: "hybrid",              // "hybrid" | "hot" | "restart" | "off"
      debounceMs: 300
    },
    auth: {
      token: "your-secret-token"   // access control for Control UI
    }
  }
}
```

**Port/bind changes require restart.** All other settings hot-reload.

## Session Management

```json5
{
  session: {
    dmScope: "main",               // "main" | "per-peer" | "per-channel-peer"
    threadBindings: {
      enabled: false,
      idleHours: 24
    },
    reset: {
      mode: "daily",
      atHour: 4,                   // UTC hour
      idleMinutes: 120
    }
  }
}
```

## Automation

```json5
{
  cron: {
    enabled: true,
    maxConcurrentRuns: 2,
    sessionRetention: "7d",
    jobs: {
      "job-name": {
        schedule: "0 9 * * *",     // cron expression
        prompt: "What to do",
        target: "whatsapp:+861381234"
      }
    }
  },
  hooks: {
    enabled: true,
    token: "webhook-secret",
    path: "/hooks",
    mappings: { /* route definitions */ }
  }
}
```

## Environment Variables

```json5
{
  env: {
    ANTHROPIC_API_KEY: "sk-ant-...",
    TELEGRAM_BOT_TOKEN: "123456:ABC...",
    // Reference in other fields as "${ANTHROPIC_API_KEY}"
    shellEnv: {
      enabled: false,              // auto-import from login shell
      timeoutMs: 5000
    }
  }
}
```

Variable substitution: `"${VAR_NAME}"` in any string value.
Only uppercase `[A-Z_][A-Z0-9_]*`. Missing vars cause load errors.

## Custom Model Providers

```json5
{
  models: {
    providers: {
      "my-provider": {
        baseUrl: "https://api.example.com/v1",
        apiKey: "${MY_API_KEY}",
        api: "openai",             // "openai" | "anthropic" | "auto"
        models: [
          { id: "model-name" }
        ]
      }
    }
  }
}
```

Then reference as: `"my-provider/model-name"`

## File Inclusion

Split large configs across files:
```json5
{
  agents: { $include: "./agents.json5" },
  channels: { $include: "./channels.json5" }
}
```

Supports recursive merge, relative paths, up to 10 nesting levels.

## CLI Commands Quick Reference

```
openclaw configure                 # interactive wizard
openclaw config get <path>         # read a field
openclaw config set <path> <value> # set a field
openclaw config unset <path>       # remove a field
openclaw doctor                    # validate config + diagnose issues
openclaw doctor --fix              # auto-repair common issues
openclaw gateway status            # check gateway health
openclaw gateway restart           # restart after config changes
openclaw channels status --probe   # check channel connections
openclaw models status             # check model availability
openclaw logs --follow             # real-time logs
openclaw status                    # overall system status
openclaw dashboard                 # open Control UI in browser
```
