---
name: email-connectivity-test
description: Test network connectivity to email servers — SMTP, IMAP, POP3, and major providers
platform: all
last_reviewed: 2026-03-17
author: noah-team
source: bundled
emoji: 📡
---

# Email Connectivity Test

Tests whether the device can reach common email infrastructure: SMTP, IMAP, POP3 ports, plus Microsoft 365 and Google Workspace endpoints. Helps isolate whether email problems are caused by local network/firewall issues vs. server-side problems.

## When to activate
User can't send or receive email, email client shows connection errors, suspected firewall or network blocking email ports, or verifying connectivity after network changes.

## Standard check path

### 1. Test DNS resolution of mail server
If the user has a specific mail server, resolve it first using a DNS lookup.
- If DNS fails, email won't work regardless of port connectivity. Activate the `network-diagnostics` playbook to fix DNS first.
- If no specific server, skip to step 2 and test common providers.

### 2. Test SMTP connectivity
Test connectivity to common SMTP ports:
- **Port 25** (SMTP relay) — often blocked by ISPs and cloud providers. Don't alarm if this fails.
- **Port 465** (SMTP over SSL) — legacy but still used by some providers.
- **Port 587** (SMTP submission with STARTTLS) — the standard port for sending email. This one matters most.

Use an HTTP check or `nc -z -w 5 <host> <port>` to test. Report which ports are reachable.

### 3. Test IMAP and POP3 connectivity
- **Port 993** (IMAP over SSL) — standard for receiving email with IMAP.
- **Port 995** (POP3 over SSL) — standard for receiving email with POP3.
- **Port 143** (IMAP, unencrypted) — rarely used now but test if 993 fails.

Most modern setups use IMAP on 993. If 993 is reachable, POP3 is less important unless the user specifically uses it.

### 4. Test Microsoft 365 / Exchange endpoints
Test connectivity to:
- `outlook.office365.com` on port 443 (Outlook web and modern clients).
- `smtp.office365.com` on port 587 (sending).
- `outlook.office365.com` on port 993 (IMAP).

If the user is on M365, all three should be reachable. Port 443 blocked is unusual and suggests a proxy or firewall issue.

### 5. Test Google Workspace endpoints
Test connectivity to:
- `smtp.gmail.com` on port 465 and 587 (sending).
- `imap.gmail.com` on port 993 (receiving).
- `mail.google.com` on port 443 (web access).

Same logic — all should be reachable for Google Workspace users.

### 6. Report results
Summarize in a table format:
- Service / Host / Port / Status (reachable or blocked).
- Flag any blocked ports that should be open.
- If SMTP ports are blocked, check if the user is on a network that blocks outbound mail (common on guest Wi-Fi, some corporate networks, and cloud VMs).
- If everything is blocked, the issue is likely a local firewall, VPN, or network-level block — not a server problem.

## Caveats
- **Port 25 is commonly blocked** by ISPs, cloud providers (AWS, GCP, Azure), and corporate networks. This is normal and expected. Modern email clients use 587 or 465.
- **VPNs** can affect connectivity — some VPNs route all traffic and may block email ports. Test with VPN disconnected to isolate.
- **Corporate firewalls** often restrict which email servers can be reached. If only company email servers are reachable, this is by design.
- **Connection success doesn't mean authentication will work** — this test only checks network reachability, not credentials.

> Steps 1-6 isolate ~85% of email connectivity problems. Most common cause: firewall or VPN blocking port 587.

## Key signals
- **"Can receive but can't send"** → SMTP ports blocked. Focus on step 2, especially port 587.
- **"Nothing works — no send, no receive"** → broader network issue. Check DNS first (step 1), then all ports.
- **"Works on phone but not laptop"** → laptop-specific firewall, VPN, or proxy. Check step 2 with VPN disconnected.
- **"Worked yesterday, broken today"** → network change (new Wi-Fi, VPN update). Compare results across networks.

## Escalation
If all ports are blocked:
- Check if the user is on a restricted network (guest Wi-Fi, hotel, airport). Try a different network.
- If on a corporate network, escalate to network admin — email ports may be intentionally restricted.
- If the problem persists across networks, check for local firewall software (Little Snitch, Windows Firewall rules).

## Tools referenced
- DNS lookup tools — resolve mail server hostnames
- HTTP check tools — test HTTPS connectivity to web endpoints
- Shell commands — test TCP connectivity to specific ports via `nc`
- Ping tools — basic reachability check
