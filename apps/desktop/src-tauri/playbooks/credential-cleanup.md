---
name: credential-cleanup
description: Audit and clean up stored credentials on a device — for offboarding or post-incident response
platform: all
last_reviewed: 2026-03-17
author: noah-team
type: system
emoji: 🔑
---

# Credential Cleanup

Audits stored credentials on a device: browser passwords, Wi-Fi keys, cached tokens, SSH keys, and keychain entries. Shows what exists and guides the user through selective removal. Designed for employee offboarding, device reprovisioning, or post-security-incident cleanup.

**Important**: This playbook shows what exists but always asks for confirmation before removing anything.

## When to activate
Employee offboarding, device being reassigned, compromised credential response, or user wants to clean up old credentials.

## Standard check path

### 1. Check browser saved passwords
Count saved passwords in each installed browser (don't dump the actual passwords):
- **Chrome**: Check size of `~/Library/Application Support/Google/Chrome/Default/Login Data`.
- **Firefox**: Check `~/Library/Application Support/Firefox/Profiles/*/logins.json` for entry count.
- **Edge**: Check `~/Library/Application Support/Microsoft Edge/Default/Login Data`.

Report: "Chrome has ~N saved passwords" etc. If any are found, note they should be cleared for offboarding.

### 2. Check saved Wi-Fi passwords
- **macOS**: Wi-Fi passwords are stored in the system keychain. Run `security find-generic-password -D "AirPort network password"` to list known networks (names only, not passwords).
- **Windows**: Run `netsh wlan show profiles` to list saved networks.
- **Linux**: Check `/etc/NetworkManager/system-connections/`.

Report the count and network names. Corporate Wi-Fi credentials are especially important to remove during offboarding.

### 3. Check for cached authentication tokens
- **Kerberos**: Run `klist` to check for cached Kerberos tickets (Active Directory environments).
- **macOS Keychain**: Check for entries related to corporate services — look for entries matching the company domain in `security dump-keychain` (metadata only).
- **Windows**: Check Credential Manager for cached Windows/domain credentials.

Report what's cached. Kerberos tickets expire on their own but can be cleared immediately with `kdestroy`.

### 4. Check for SSH keys
Check `~/.ssh/` for key files:
- List key files (id_rsa, id_ed25519, etc.) and their ages.
- Check `~/.ssh/authorized_keys` for keys that grant access to this machine.
- Check `~/.ssh/known_hosts` for hosts this machine has connected to.

Report count and types. SSH keys that grant access to company servers should be revoked server-side (not just deleted locally) during offboarding.

### 5. Check keychain / credential store
- **macOS**: Summarize keychain contents by category using `security dump-keychain` (metadata only — kind, service name, account, not passwords).
- **Windows**: Summarize Credential Manager entries.

Focus on entries related to corporate services, VPN credentials, and API tokens.

### 6. Guide selective removal
Present all findings and ask the user which credential stores to clear. For each:
- **Browser passwords**: Guide to browser settings → passwords → clear all. Or delete the Login Data file directly.
- **Wi-Fi passwords**: Remove specific networks via System Settings → Network → Wi-Fi → known networks, or `security delete-generic-password`.
- **Kerberos tickets**: Run `kdestroy`.
- **SSH keys**: Delete specific key files from `~/.ssh/`. Remind user to also revoke the public key on any servers it was added to.
- **Keychain entries**: Remove specific entries via Keychain Access or `security delete-generic-password` / `security delete-internet-password`.

Always confirm before each deletion action.

## Caveats
- **Deleting local credentials doesn't revoke access.** For offboarding, credentials must also be revoked server-side (disable the AD account, remove SSH public keys from servers, revoke OAuth tokens, etc.).
- **macOS Keychain** may require the user's login password to view or modify entries.
- **Some credentials regenerate automatically.** iCloud Keychain syncs back, Chrome syncs passwords if signed in. Sign out of sync services before clearing.
- **Don't delete the SSH directory itself** — just the key files. The directory and `config` file structure should remain.

> Steps 1-6 cover ~90% of locally stored credentials. Most commonly missed: cached Kerberos tickets and browser-synced passwords.

## Key signals
- **"Employee is leaving the company"** → full offboarding. Run all steps and remind about server-side revocation.
- **"Device is being reassigned"** → same as offboarding, plus check for personal accounts (iCloud, Google) still signed in.
- **"Credential may have been compromised"** → focus on steps 3 and 5. Change passwords server-side first, then clean locally.
- **"Just want to clean up old stuff"** → run steps 1-5 as an audit, let user decide what to remove in step 6.

## Escalation
If the cleanup is for a security incident:
- Do not delete credentials until they've been documented — they may be needed for forensics.
- Ensure server-side revocation happens in parallel (disable AD account, revoke OAuth tokens, remove SSH keys from servers).
- If the device may be compromised, a full wipe may be more appropriate than selective cleanup. Consult IT security.

## Tools referenced
- Shell commands — keychain queries, klist, SSH key listing
- User confirmation prompts — confirmation before each removal action
