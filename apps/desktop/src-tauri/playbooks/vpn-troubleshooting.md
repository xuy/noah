---
name: vpn-troubleshooting
description: Diagnose VPN connection failures, drops, and split-tunnel DNS issues
platform: macos
last_reviewed: 2026-03-04
author: noah-team
source: bundled
emoji: 🔒
---

# VPN Troubleshooting

## When to activate
User reports: VPN won't connect, VPN keeps disconnecting, can't access work resources on VPN, slow internet with VPN, DNS not resolving with VPN on.

## Quick check
**First: check knowledge for VPN context.**
Use `search_knowledge` for "VPN" — look for saved VPN client name, server address, auth method.
If no knowledge found, ask the user: "Which VPN app do you use?" (GlobalProtect, Cisco AnyConnect, WireGuard, built-in macOS VPN, etc.) and save the answer with `write_knowledge` to category `network` for next time.

Then run `mac_network_info` — look for a `utun` or `ipsec` interface.
- If VPN interface exists with an IP → tunnel is up. Problem is likely DNS. Jump to step 3.
- If no VPN interface → VPN is not connected. Start at step 1.

## Standard fix path (try in order)

### 1. Verify internet connectivity
Run `mac_ping` to `8.8.8.8`. VPN requires a working internet connection first.
- If ping fails → fix internet first. Activate `network-diagnostics` playbook.
- If ping works → continue.

### 2. Disconnect, flush, reconnect
This three-step reset resolves most transient VPN failures:
1. Disconnect the VPN completely.
2. Run `mac_flush_dns` to clear stale DNS entries.
3. Reconnect the VPN.

macOS DNS resolver gets confused after multiple VPN connect/disconnect cycles. A clean flush before reconnecting prevents this.

### 3. Fix DNS (the #1 VPN issue)
Most "VPN connected but can't access anything" problems are DNS.
Run `mac_dns_check` for an internal hostname AND `google.com`.

- **Internal fails, external works** → VPN DNS server not in resolver chain. Flush DNS (`mac_flush_dns`), reconnect VPN. If still failing, the VPN client may need reconfiguration — see client-specific notes below.
- **Both fail** → VPN is capturing all DNS but its server is unreachable. Disconnect VPN, verify DNS works without it, reconnect.
- **Both work** → DNS is fine. Problem is routing — specific subnets may not route through the tunnel. This is a VPN server config issue, not fixable locally.

### 4. Check VPN client process
Run `mac_process_list` — look for the VPN client process.
- **Not running** → VPN service crashed or didn't start. Relaunch the VPN app.
- **Running but VPN won't connect** → the client may have a stale state. Quit the VPN app completely, then relaunch.
- If the VPN system extension was disabled after a macOS update: System Settings → Privacy & Security → Network Extensions — re-enable it.

> Steps 1-4 resolve ~75% of VPN issues. Success rate improves significantly with client-specific knowledge (see below).

## Client-specific notes
**These are most useful when grounded with the user's actual VPN setup from knowledge.**

**GlobalProtect (Palo Alto):**
- Portal vs Gateway confusion — user may need to enter the portal URL, not the gateway.
- "HIP check failed" → compliance check failing. Update macOS, ensure FileVault is enabled, check if required antivirus is running.
- Reconnect: quit GlobalProtect from menu bar → relaunch from Applications.

**Cisco AnyConnect:**
- "VPN agent service not available" → `vpnagentd` process not running. Relaunch AnyConnect.
- Credential issues: clear AnyConnect entries in Keychain Access, reconnect.
- If AnyConnect connects but DNS breaks: AnyConnect aggressively overrides DNS settings. `mac_flush_dns` after connect usually fixes it.

**WireGuard:**
- DNS is configured per-tunnel in the WireGuard config file. If DNS breaks with WireGuard, the `DNS =` line in the config may be wrong.
- WireGuard doesn't "reconnect" — it's always on. Toggle the tunnel off/on.

**Built-in macOS VPN (IKEv2/L2TP):**
- "Server not responding" → usually a firewall blocking ports 500/4500 (IKEv2) or 1701 (L2TP).
- Certificate-based auth: check Keychain for expired VPN certificates.

## Caveats
- **Port blocking:** Hotel/airport Wi-Fi often blocks VPN ports. If the VPN supports SSL mode (port 443), try that — it's rarely blocked. Alternative: use a mobile hotspot to bypass the network.
- **macOS sleep disconnects VPN** — this is default macOS behavior, not a bug. Some VPN clients have a "reconnect after wake" option.
- **Full-tunnel = slow internet** — if ALL traffic routes through VPN, the VPN server's connection is the bottleneck. This is expected. Ask IT if split-tunnel is available.

## Key signals
- **"Works from home but not this hotel"** → port blocking. Try SSL mode.
- **"Can't access [internal site] but internet works"** → DNS issue. Step 3.
- **"VPN worked until I updated macOS"** → system extension got disabled. Step 4.
- **"Keeps asking for credentials"** → clear Keychain entries for the VPN and re-enter.
- **"Nobody at the office can connect"** → VPN server is down. Nothing to fix locally — contact IT.

## Knowledge to save
After resolving a VPN issue, save these details with `write_knowledge` to `network/vpn-config`:
- VPN client name and version
- VPN server/portal URL
- Auth method (password, MFA app, certificate)
- Known working DNS servers
- Split-tunnel vs full-tunnel
This makes future VPN issues much faster to resolve.

## Tools referenced
- `mac_ping` — basic connectivity test
- `mac_network_info` — check for VPN tunnel interface
- `mac_dns_check` — test internal vs external DNS resolution
- `mac_flush_dns` — clear DNS cache
- `mac_process_list` — check if VPN client is running
- `search_knowledge` — check for saved VPN configuration
- `write_knowledge` — save VPN config for future sessions

## Escalation
If local troubleshooting fails:
- Provide IT with: VPN client name/version, exact error message, whether it worked before, and what network the user is on.
- Many VPN issues are server-side (expired certs, policy changes, maintenance).
