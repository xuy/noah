---
name: setup-openclaw/install-node
description: Install Node.js 22+ for OpenClaw (sub-module)
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# Install Node.js

OpenClaw requires Node.js 22+. This module installs it.

## Step 1: Choose Installation Method

Check the platform first.

**macOS:**
- **Homebrew** (recommended if brew is available): `brew install node@22`
- **nvm**: install nvm then `nvm install 22`
- **Direct download** from nodejs.org

**Windows:**
- **nvm-windows**: `nvm install 22 && nvm use 22`
- **Direct download** from nodejs.org (LTS installer)
- **winget**: `winget install OpenJS.NodeJS.LTS`

**Linux:**
- **nvm** (recommended): install nvm then `nvm install 22`
- **Package manager**: check distro-specific instructions

## Step 2: Install

For Homebrew: `brew install node@22 && brew link node@22`.
For nvm: `curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash`
then restart shell, then `nvm install 22 && nvm use 22`.
For direct download: use WAIT_FOR_USER and guide to nodejs.org download page.

## Step 3: Verify

Run `node --version` and confirm it shows v22.x or higher.
If it shows an old version, check PATH ordering. On macOS with Homebrew,
may need `brew link --overwrite node@22`.

## Tools referenced
- `shell_run` — install and verify Node.js
- `ui_user_question` — installation method choice
- `ui_spa` with WAIT_FOR_USER — for manual download
