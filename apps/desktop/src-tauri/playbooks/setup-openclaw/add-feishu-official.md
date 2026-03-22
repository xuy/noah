---
name: setup-openclaw/add-feishu-official
description: Add Feishu official plugin (user-identity OAuth — documents, calendar, tasks)
platform: all
last_reviewed: 2026-03-09
author: noah-team
source: bundled
emoji: 🦞
---

# Add Feishu Official Plugin

Install the Feishu official OpenClaw plugin (`feishu-openclaw-plugin`),
developed by the Feishu team. Unlike the built-in plugin (bot identity),
this operates as **the user's own identity** via OAuth — it can create/edit
documents, manage calendars, tasks, and spreadsheets.

**The built-in and official plugins are mutually exclusive.** Installing the
official plugin automatically disables the built-in one.

## When to activate
User wants the Feishu official plugin, needs document/calendar/task capabilities
via Feishu, or wants AI to operate as their identity (not as a bot).

## Prerequisites
- OpenClaw installed and running (`openclaw --version` ≥ 2026.2)
- A Feishu enterprise app with bot capability (if you don't have one yet,
  do Steps 1-3 of `setup-openclaw/add-feishu` first — the app is shared
  between both plugins)

## Step 1: Install the Official Plugin

Run `shell_run` with:
```
openclaw plugins install @larksuiteoapi/feishu-openclaw-plugin
```

This will automatically disable the built-in Feishu plugin if it was active.

Verify: run `shell_run` with `openclaw plugins list`.
Expected: `feishu-openclaw-plugin` shows "loaded", `feishu` shows "disabled".

If you see `duplicate plugin id` error:
Run `shell_run` with `rm -rf ~/.openclaw/extensions/feishu && openclaw gateway restart`.

## Step 2: Configure Credentials

If the user already has an App ID and App Secret from a previous setup,
collect them. Otherwise, they need to create a Feishu app first (activate
`setup-openclaw/add-feishu` Steps 1-5, then come back here).

Collect the App ID via `text_input` (placeholder: "cli_xxxxxxxxx").
Collect the App Secret via `secure_input` (secret_name: "feishu_app_secret").

Run `shell_run` with:
```
openclaw config set channels.feishu.accounts.main.appId "<app_id>"
```

Use `write_secret` to set the App Secret:
Run `shell_run` with `openclaw config set channels.feishu.accounts.main.appSecret "{{value}}"`,
substituting secret_name "feishu_app_secret".

Restart gateway:
Run `shell_run` with `openclaw gateway restart`.

## Step 3: Configure Event Subscription

**The gateway must be running before this step.**

Tell the user:

> 1. Go to [飞书开放平台](https://open.feishu.cn/app) → your app
> 2. Go to **事件与回调** (Events & Callbacks) > **事件配置** (Event Config)
> 3. Select **使用长连接接收事件** (Use long connection) — not Webhook
> 4. Add event: `im.message.receive_v1` (接收消息)
> 5. Save, then go to **版本管理与发布** → **创建版本** → publish

Use WAIT_FOR_USER — the user does this in the Feishu developer console.

## Step 4: Pair and Authorize

Tell the user:

> 1. In Feishu, find your bot and send it a message
> 2. You'll receive a pairing code — copy it

If pairing code received, run `shell_run` with:
```
openclaw pairing approve feishu <code>
```

After approval, tell the user:

> You should see an **authorization card** (授权卡片) from the bot.
> Click it to complete OAuth authorization — this lets OpenClaw operate
> Feishu as your identity (create documents, manage calendar, etc.).

Use WAIT_FOR_USER — the user clicks the OAuth authorization card.

Alternatively, the user can authorize later by typing `/feishu auth`
in the chat with the bot.

## Step 5: Verify

Run `shell_run` with `openclaw plugins list`.
Expected: `feishu-openclaw-plugin` loaded.

Tell the user to type `/feishu start` in the Feishu chat.
Expected: bot returns a version number.

Run `shell_run` with `openclaw channels status --probe`.
Expected: Feishu channel connected.

## Step 6: Done

Show a done card summarizing:
- Feishu official plugin installed and authorized
- The bot operates as the user's identity (OAuth)
- Capabilities: messages, documents, spreadsheets, calendar, tasks
- How to check: `openclaw plugins list`, `/feishu start` in chat
- How to re-authorize: `/feishu auth` in chat
- How to update: `openclaw plugins update feishu-openclaw-plugin`
- How to check health: `/feishu doctor` in chat
- To switch back to built-in plugin: uninstall official plugin and restart gateway

## Tools referenced
- `shell_run` — plugin install, config, gateway restart, pairing
- `ui_user_question` with `text_input` — App ID
- `ui_user_question` with `secure_input` — App Secret
- `ui_spa` with WAIT_FOR_USER — Feishu console setup, OAuth authorization
- `write_secret` — write App Secret to config
- `ui_done` — completion summary
