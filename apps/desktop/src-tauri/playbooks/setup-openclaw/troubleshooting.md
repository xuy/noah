---
name: setup-openclaw/troubleshooting
description: Diagnose and fix common OpenClaw issues
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# OpenClaw Troubleshooting

Load this module when the user has a problem with an existing OpenClaw
installation.

## Step 1: Run Diagnostics

Start with the standard diagnostic ladder:
```
openclaw status
openclaw gateway status
openclaw doctor
openclaw channels status --probe
```

Check logs for errors:
```
openclaw logs --follow
```

## Common Issues

### `openclaw` command not found
- npm global bin not in PATH
- Fix: add `$(npm prefix -g)/bin` to shell PATH
- Verify: `npm prefix -g` shows the global prefix

### Gateway won't start
- **Port conflict**: another process on port 18789
  - Check: `lsof -i :18789` (macOS/Linux) or `netstat -ano | findstr :18789` (Windows)
  - Fix: stop the other process or change port in config
- **Auth required**: non-loopback bind without auth configured
  - Error: `"refusing to bind gateway ... without auth"`
  - Fix: `openclaw config set gateway.auth.token "some-secret"`
- **Config error**: invalid JSON5
  - Fix: `openclaw doctor --fix` or manually fix `~/.openclaw/openclaw.json`

### No response to messages
1. Check gateway is running: `openclaw gateway status`
2. Check channel is connected: `openclaw channels status --probe`
3. Check access policy: `openclaw config get channels.<channel>.dmPolicy`
4. Check pairings (if using pairing mode): `openclaw pairing list --channel <channel>`
5. Check model is accessible: `openclaw models status`
6. Check logs for errors: `openclaw logs --follow`

### WhatsApp disconnected / reconnect loops
- Run `openclaw doctor`
- Re-link: `openclaw channels login --channel whatsapp`
- May need to remove old credentials and re-scan QR

### Rate limiting (429 errors)
- Error: `"HTTP 429: rate_limit_error"`
- Common with Anthropic free-tier or high usage
- Fix: upgrade to paid plan, or switch to a different model/provider
- Add fallback models: `agents.defaults.model.fallbacks`

### Messages ignored in groups
- Check mention requirement: groups require @mention by default
- Check `groupPolicy` setting
- Check `groupAllowFrom` (doesn't inherit from DM `allowFrom`)
- Verify bot has group privacy disabled (Telegram only)

### Cron jobs not running
- Check: `openclaw cron status`
- If disabled: `openclaw config set cron.enabled true`
- Check job history: `openclaw cron runs --id <jobId> --limit 20`

### Config file issues
- **Wrong file**: config is at `~/.openclaw/openclaw.json` (NOT config.yaml)
- **Format**: JSON5 (supports comments and trailing commas)
- **Validation**: `openclaw doctor` checks config validity
- **Hot reload**: most changes apply without restart
- **Restart needed for**: gateway port, bind address, TLS settings

### After upgrade issues
- Check: `openclaw config get gateway.mode`
- Old `gateway.token` key → now `gateway.auth.token`
- Force service reinstall: `openclaw gateway install --force && openclaw gateway restart`
- Clear stale state: `openclaw devices list` and `openclaw pairing list`

## Environment Variables

Key env vars that affect OpenClaw behavior:
- `OPENCLAW_HOME` — home directory for path resolution
- `OPENCLAW_STATE_DIR` — state directory override
- `OPENCLAW_CONFIG_PATH` — config file path override

## Tools referenced
- `shell_run` — diagnostic commands
- `ui_info` — explain findings to user
