---
name: setup-email-account
description: Add an email account to Apple Mail or Outlook on macOS
platform: macos
last_reviewed: 2026-03-07
author: noah-team
source: bundled
emoji: ✉️
---

# Set Up Email Account

## When to activate
User wants to add an email account, set up Mail.app, configure Outlook, or can't receive/send email on their Mac.

## Step 1: Choose email client
Ask which email app the user wants to use:
- Apple Mail (built-in)
- Microsoft Outlook
- Other (Thunderbird, etc.)

## Step 2: Identify the email provider
Ask the user for their email address using `text_input`. Based on the domain:
- @gmail.com → Google (OAuth sign-in)
- @outlook.com / @hotmail.com → Microsoft (OAuth sign-in)
- @icloud.com → Apple (system account)
- @company.com → likely Exchange or IMAP — need server details

## Step 3: Collect credentials
For OAuth providers (Google, Microsoft): guide user through the browser sign-in.
Use WAIT_FOR_USER — the sign-in happens in a browser window.

For IMAP/Exchange accounts:
- Ask for incoming server (e.g. `imap.company.com`) via `text_input`
- Ask for outgoing server (e.g. `smtp.company.com`) via `text_input`
- Collect password via `secure_input` (secret_name: "email_password")

## Step 4: Configure the account
For Apple Mail:
- Open System Settings → Internet Accounts → Add Account
- Guide user through the wizard
- Use WAIT_FOR_USER for interactive setup steps

For Outlook:
- Open Outlook → Preferences → Accounts → Add
- Or use the setup wizard on first launch
- Use WAIT_FOR_USER for the sign-in flow

## Step 5: Verify email works
Ask the user to send themselves a test email.
Check if the account appears in the mail client.
If issues: check server settings, ports (993 for IMAP SSL, 587 for SMTP TLS).

## Caveats
- If the user has 2FA enabled on Google, they may need an App Password instead of their regular password
- Exchange accounts may need the server URL from IT (often autodiscovered)
- iCloud accounts should use System Settings, not manual IMAP

## Tools referenced
- `ui_user_question` with `text_input` — email address, server details
- `ui_user_question` with `secure_input` — email password
- `ui_spa` with WAIT_FOR_USER — browser sign-in, app wizard steps
- `mac_run_command` — open apps, check settings
