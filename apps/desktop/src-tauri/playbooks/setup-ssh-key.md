---
name: setup-ssh-key
description: Generate an SSH key pair and add it to GitHub or another service
platform: all
last_reviewed: 2026-03-07
author: noah-team
source: bundled
emoji: 🔑
---

# Set Up SSH Key

## When to activate
User wants to set up SSH keys, connect to GitHub via SSH, push to git without password, or says "permission denied (publickey)".

## Step 1: Check for existing SSH keys
Run a command to list `~/.ssh/` and look for `id_ed25519.pub` or `id_rsa.pub`.
- If a key exists, ask the user if they want to use the existing one or generate a new one.
- If no keys found, proceed to Step 2.

## Step 2: Collect email address
Ask the user for their email address (used as the SSH key comment). This is non-sensitive — use `text_input`.

## Step 3: Generate the SSH key
Run: `ssh-keygen -t ed25519 -C "<email>"` with an empty passphrase for simplicity, or ask if they want a passphrase.
Show the generated public key to the user.

## Step 4: Copy public key
Run `cat ~/.ssh/id_ed25519.pub` and display the key. Tell the user to copy it.
Use WAIT_FOR_USER — the user needs to go to GitHub (Settings → SSH Keys → New SSH Key) and paste it.

Provide a direct link: https://github.com/settings/ssh/new

## Step 5: Test the connection
Run: `ssh -T git@github.com` to verify.
Expected success output: "Hi username! You've successfully authenticated"
If it fails, check `~/.ssh/config` and suggest adding:
```
Host github.com
  IdentityFile ~/.ssh/id_ed25519
  AddKeysToAgent yes
```

## Tools referenced
- `mac_run_command` / `win_run_command` — run ssh-keygen and test commands
- `ui_user_question` with `text_input` — collect email
- `ui_spa` with WAIT_FOR_USER — GitHub key paste step
