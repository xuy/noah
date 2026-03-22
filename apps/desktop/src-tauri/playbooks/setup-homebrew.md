---
name: setup-homebrew
description: Install and configure Homebrew package manager on macOS
platform: macos
last_reviewed: 2026-03-07
author: noah-team
source: bundled
emoji: 🍺
---

# Set Up Homebrew

## When to activate
User wants to install Homebrew, set up a package manager, or says they need to install software on their Mac and doesn't have Homebrew yet.

## Step 1: Check if Homebrew is already installed
Run `mac_run_command` with `which brew` or `brew --version` to see if it exists.
- If Homebrew is already installed, skip to Step 4 (install packages).
- If not found, continue with Step 2.

## Step 2: Install Homebrew
Tell the user to open Terminal and paste the official install command:
```
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```
Use WAIT_FOR_USER — the user needs to run this in Terminal themselves and follow any prompts (password entry, Xcode CLT installation). This can take 5–15 minutes.

## Step 3: Add Homebrew to PATH
After installation, Homebrew often needs to be added to the shell profile.
Check if `brew` is in PATH by running `which brew`.
If not found, tell the user to run:
```
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
eval "$(/opt/homebrew/bin/brew shellenv)"
```
Then verify with `brew --version`.

## Step 4: Install requested packages
If the user had specific software in mind, help them install it via `brew install <package>` or `brew install --cask <app>`.
Common requests: Chrome (`google-chrome`), VS Code (`visual-studio-code`), Slack (`slack`), Zoom (`zoom`).

## Tools referenced
- `mac_run_command` — run shell commands to check/install
- `ui_spa` with WAIT_FOR_USER — for Terminal steps the user must do themselves
- `ui_user_question` — ask what packages they want

## Escalation
If Xcode CLT installation fails, the user may need to download it manually from developer.apple.com. If Homebrew install script fails, check proxy/firewall settings.
