---
name: identity-provider-test
description: Test connectivity to identity providers — Microsoft Entra ID, Google Workspace, and SSO endpoints
platform: all
last_reviewed: 2026-03-17
author: noah-team
source: bundled
emoji: 🪪
---

# Identity Provider Test

Tests connectivity to common identity providers (Microsoft Entra ID, Google Workspace) and checks for conditions that cause authentication failures: DNS issues, network blocks, clock skew, and proxy interference.

## When to activate
User can't sign in to corporate services, SSO failures, "session expired" loops, MFA not working, or diagnosing whether the problem is network vs. account vs. IdP outage.

## Standard check path

### 1. Test Microsoft Entra ID (M365) DNS
Resolve `login.microsoftonline.com` using a DNS lookup.
- If resolution fails, DNS is broken — activate `network-diagnostics` first.
- If it resolves to an unexpected IP or a local IP, a proxy or DNS filter may be intercepting authentication traffic.

### 2. Test Google Workspace DNS
Resolve `accounts.google.com` using a DNS lookup.
- Same logic as above. Both should resolve to public IPs owned by Microsoft and Google respectively.

### 3. Test HTTPS connectivity
Test HTTPS (port 443) to each endpoint:
- `https://login.microsoftonline.com` — should return a 200 or redirect.
- `https://accounts.google.com` — should return a 200 or redirect.
- `https://login.windows.net` — alternate Microsoft endpoint.

Use an HTTP check for each. If DNS resolves but HTTPS fails, a firewall or proxy is likely blocking the connection.

### 4. Check time synchronization
Kerberos and SAML authentication fail if the device clock is more than 5 minutes off from the server.
- **macOS**: Run `sntp -d time.apple.com` or check System Settings → Date & Time for "Set time and date automatically."
- **Windows**: Check `w32tm /query /status`.
- **Linux**: Check `timedatectl` for NTP sync status.

Flag if automatic time sync is disabled or the clock is skewed by more than 2 minutes.

### 5. Check for proxy/VPN interference
- Check if a system proxy is configured: **macOS** — System Settings → Network → Wi-Fi → Proxies, or `scutil --proxy`.
- Check if a VPN is active: look for VPN interfaces in network info or active VPN processes.
- Check if a PAC file is configured (can selectively route auth traffic through a proxy).

If a proxy or VPN is active, it may be intercepting or blocking identity provider traffic. Test with it disabled to isolate.

### 6. Try fetching the login page
Use an HTTP check to fetch the actual login pages:
- `https://login.microsoftonline.com/common/oauth2/authorize` — Microsoft's OAuth endpoint.
- `https://accounts.google.com/ServiceLogin` — Google's login page.

If these return valid HTML/redirects, the IdP is reachable and the problem is likely account-level (wrong password, MFA issue, conditional access policy) rather than network-level.

Report results: which providers are reachable, any issues found (clock skew, proxy, DNS), and whether the problem appears to be network or account-related.

## Caveats
- **Conditional Access policies** (Microsoft) can block sign-ins from certain networks, devices, or locations even when connectivity is fine. If the IdP is reachable but login fails, check with the IT admin for policy blocks.
- **Certificate inspection proxies** (Zscaler, etc.) can break SSO by intercepting TLS. Symptoms: certificate errors on login pages. Check the certificate chain in the browser.
- **Multiple Microsoft endpoints**: Some organizations use custom tenant URLs or ADFS (`sts.company.com`). Ask the user if they have a specific login URL.
- **Google may challenge from new locations** with extra verification even when connectivity is fine.

> Steps 1-6 isolate ~80% of SSO/authentication failures. Most common cause: clock skew or VPN interference.

## Key signals
- **"Login page won't load"** → DNS or firewall issue. Start at step 1.
- **"Login page loads but sign-in fails"** → account-level issue (wrong password, MFA, conditional access). Steps 1-3 will be green.
- **"Session expired loop"** → often clock skew (step 4) or cookie/proxy issue (step 5).
- **"Works on phone but not laptop"** → laptop-specific proxy, VPN, or certificate issue. Focus on step 5.

## Escalation
If IdP endpoints are reachable but authentication still fails:
- Check with IT admin for conditional access policies or account lockouts.
- If certificate errors appear on login pages, a TLS-intercepting proxy may be breaking SSO — escalate to network team.
- If the IdP itself is down (returns 5xx), check the provider's status page and wait.

## Tools referenced
- DNS lookup tools — resolve IdP hostnames
- HTTP check tools — test HTTPS connectivity to login endpoints
- Network info tools — check for VPN interfaces
- Shell commands — check proxy settings, time sync, VPN status
