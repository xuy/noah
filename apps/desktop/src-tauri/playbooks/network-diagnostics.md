---
name: network-diagnostics
description: Systematic connectivity troubleshooting for Wi-Fi, DNS, and internet issues
platform: macos
last_reviewed: 2026-03-04
author: noah-team
source: bundled
emoji: 🌐
---

# Network Diagnostics

## When to activate
User reports: can't connect, Wi-Fi dropping, slow internet, DNS errors, pages not loading, "no internet" warning.

## Quick check
Run `mac_ping` to `8.8.8.8` with count 3.
- If ping succeeds → internet works. Problem is DNS or application-level. Jump to step 3.
- If ping fails → no internet connectivity. Start at step 1.

## Standard fix path (try in order)

### 1. Check Wi-Fi association
Run `mac_network_info` to check interfaces and IP address.
- **No Wi-Fi interface or "not associated"** → Wi-Fi is off or disconnected. Tell user to reconnect via Wi-Fi menu bar.
- **Self-assigned IP (169.254.x.x)** → DHCP failed. Turn Wi-Fi off and back on. If repeats, try: forget network, re-join with password.
- **Valid local IP (192.168.x.x, 10.x.x.x)** → Wi-Fi is connected. Continue.

### 2. Check gateway and upstream
Run `mac_ping` to the gateway IP (shown in `mac_network_info`).
- **Gateway ping fails** → router issue. Suggest: power-cycle the router (unplug 10 seconds, plug back in). This fixes most home/office router hangs.
- **Gateway works, 8.8.8.8 fails** → ISP or upstream issue. Try a different DNS: `mac_ping` to `1.1.1.1`. If that also fails, it's an ISP outage — nothing Noah can fix locally.

### 3. Check DNS
Run `mac_dns_check` for `google.com`.
- **DNS resolves** → DNS is fine. Jump to step 4.
- **DNS fails** → flush DNS cache with `mac_flush_dns`. Re-test.
  - If still failing: suggest changing DNS to 8.8.8.8 / 1.1.1.1 in System Settings → Network → Wi-Fi → Details → DNS.

### 4. Check HTTP
Run `mac_http_check` for `https://www.google.com`.
- **Works** → full connectivity is fine. Test the specific site/service the user is having trouble with.
- **Fails** → check for captive portal: `mac_http_check` for `http://captive.apple.com`. If it redirects, user is on hotel/airport Wi-Fi and needs to open a browser to complete login.

> Steps 1-4 resolve ~85% of connectivity issues. Most common fix: power-cycling the router.

## Caveats
- If a **VPN is active**, DNS often breaks because VPN configures its own DNS servers. Try disconnecting VPN to test. If DNS works without VPN → activate the `vpn-troubleshooting` playbook instead.
- **Wi-Fi drops after sleep** is a known macOS bug. Fix: turn Wi-Fi off/on, or forget and re-add the network. Persistent cases may need a new network location (System Settings → Network → Locations).
- **`mDNSResponder` high CPU** can cause DNS slowness. If you see it in process list, it's usually resolving itself. Restarting it helps: `mac_flush_dns` triggers a restart.

## Key signals
- **"It was working 5 minutes ago"** → most likely a router hiccup. Power-cycle first.
- **"Only one website doesn't work"** → DNS is fine, the site is down. Check with `mac_http_check` for that URL.
- **"Works on my phone but not my Mac"** → Mac-specific DNS or proxy issue. Check for proxy settings in System Settings → Network → Wi-Fi → Proxies.
- **"Slow but connected"** → run `wifi_scan` to check signal strength and channel congestion. Below -70 dBm = weak signal. Many networks on the same channel = congestion.

## Tools referenced
- `mac_ping` — basic connectivity test
- `mac_network_info` — interfaces, IP, gateway, DNS config
- `mac_dns_check` — DNS resolution test
- `mac_http_check` — HTTP connectivity and timing
- `mac_flush_dns` — clear DNS cache
- `wifi_scan` — signal quality and channel analysis

## Escalation
If all steps pass but the problem persists:
- Ask for the specific URL/service that fails — it may be a firewall or proxy issue.
- If on a corporate network, may need IT involvement (802.1X auth, certificate issues).
- Suggest a speed test (fast.com) to quantify slowness.
