---
name: vpn-troubleshooting
description: Diagnose VPN connection failures, drops, and split-tunnel DNS issues
platform: macos
---

# VPN Troubleshooting

## When to activate
User reports: VPN won't connect, VPN keeps disconnecting, can't access work resources, VPN connected but nothing works, slow internet with VPN, DNS not resolving with VPN.

## Protocol

### Step 1: Identify the VPN type
Ask the user or check:
- **Corporate VPN client:** GlobalProtect, Cisco AnyConnect, Pulse Secure / Ivanti, Zscaler, Cloudflare WARP
- **Built-in macOS VPN:** IKEv2, L2TP/IPSec, or WireGuard (System Settings > VPN)
- **Third-party app:** Tunnelblick (OpenVPN), WireGuard app, Tailscale, NordVPN, etc.

Run `mac_process_list` to check if the VPN client process is running.
Run `mac_network_info` to see current network interfaces — look for `utun` or `ipsec` interfaces (VPN tunnels).

### Step 2: Connection failure
**VPN won't connect at all:**
1. Check internet connectivity first — run `mac_ping` to `8.8.8.8`. VPN requires a working internet connection.
2. Check if the VPN client app is running and up to date.
3. Check if the VPN server is reachable — run `mac_ping` to the VPN server hostname/IP.
4. **Credential issues:** Has the password changed? Does MFA (Duo, Okta, Authenticator) need to be completed?
5. **Certificate issues:** Check if a client certificate is required and not expired. Keychain Access > My Certificates — look for expired certs.
6. **Port blocking:** Some networks (hotels, airports, corporate guest Wi-Fi) block VPN ports. Common ports: 443 (SSL VPN), 500/4500 (IKEv2/IPSec), 1194 (OpenVPN), 51820 (WireGuard).
   - Test: `mac_http_check` to the VPN server URL if it has a web portal.
   - Workaround: If available, try the VPN's SSL/HTTPS mode (uses port 443 which is rarely blocked).

**VPN connects then immediately disconnects:**
1. Check for VPN client version mismatch — update the client.
2. Check for conflicting VPN clients — only one should be active.
3. Check system logs: `crash_log_reader` with the VPN client name (e.g., "GlobalProtect", "AnyConnect").
4. **macOS permission:** System Settings > Privacy & Security > Network Extensions — ensure the VPN's extension is allowed.

### Step 3: VPN connected but can't access resources
**This is the most common VPN issue.** The tunnel is up but traffic isn't routing correctly.

1. **Check the tunnel interface:** Run `mac_network_info` — look for a `utun` interface with an assigned IP. If present, the tunnel is up.
2. **Check routing:** Can you reach the VPN gateway?
   - Run `mac_ping` to the VPN-assigned IP's gateway (usually visible in VPN client status).
   - Run `mac_ping` to an internal resource IP (e.g., the intranet server).
3. **DNS is the usual culprit:** Run `mac_dns_check` for an internal hostname (e.g., `intranet.company.com`).
   - If DNS fails: the VPN's DNS server isn't being used. See Step 4.
   - If DNS resolves but `mac_ping` fails: routing issue — the VPN may not be routing traffic for that subnet.

### Step 4: DNS issues with VPN (split-tunnel)
**Most VPN problems are actually DNS problems.**

Many VPNs use "split tunnel" — only corporate traffic goes through VPN, internet traffic goes direct. But DNS must be configured correctly for both.

1. Check DNS configuration: Run `mac_dns_check` for an internal hostname AND an external one (e.g., `google.com`).
2. **Internal DNS fails, external works:** VPN DNS server not in the resolver chain.
   - Check: `scutil --dns` — look for which resolver handles which domains.
   - macOS sometimes ignores VPN DNS settings. Fix: `mac_flush_dns` to clear stale cache.
   - Some VPN clients need a reconnect after the DNS config gets confused.
3. **Both DNS fail:** VPN may be capturing all DNS but its DNS server is unreachable.
   - Fix: disconnect VPN, verify DNS works, reconnect.
4. **mDNSResponder confusion:** After connecting/disconnecting VPN multiple times, macOS DNS resolver can get confused.
   - Fix: `mac_flush_dns`, then reconnect VPN.

### Step 5: Performance issues with VPN
**Internet is slow with VPN on:**
1. Is it a full-tunnel VPN? (All traffic routes through VPN.) This adds latency for everything.
   - Check: with VPN on, run `mac_http_check` for `https://www.google.com` — note the time. Compare with VPN off.
   - If full-tunnel: this is expected. The VPN server's internet connection is the bottleneck.
   - Ask IT if split-tunnel is available.
2. **MTU issues:** VPN encapsulation reduces the effective MTU. Large packets may fragment or drop.
   - Symptom: some websites load, others hang; SSH works but large file transfers stall.
   - Test: `mac_ping` with different packet sizes if needed.
3. **VPN server overloaded:** Especially during business hours. Try connecting to a different VPN gateway if available.

**VPN keeps dropping:**
1. Check Wi-Fi stability first — use `wifi_scan` to assess signal quality.
2. Weak Wi-Fi signal causes VPN to drop when packets are lost.
3. **Keep-alive settings:** Some VPNs disconnect after idle timeout. Check VPN client settings for keep-alive or idle timeout.
4. **macOS sleep:** VPN disconnects on sleep by default. Some clients have a "reconnect after wake" option.
5. **Network transitions:** Moving between Wi-Fi networks or Wi-Fi/cellular drops VPN. IKEv2 handles this better than older protocols.

### Step 6: VPN client-specific issues

**GlobalProtect (Palo Alto):**
- Portal vs Gateway confusion — user may need to enter the portal address, not the gateway.
- HIP check failures: the client checks system compliance (OS version, antivirus, etc.). If it fails, VPN is denied.
- Fix: update macOS, ensure FileVault is enabled, check if antivirus is required.

**Cisco AnyConnect:**
- "VPN agent service not available" — the AnyConnect service isn't running.
- Fix: check if `vpnagentd` process is running in process list. If not, relaunch AnyConnect.
- Keychain issues: delete AnyConnect entries in Keychain Access and reconnect.

**Built-in macOS VPN (IKEv2):**
- "Server not responding" — usually a firewall blocking ports 500/4500.
- Certificate-based auth: check Keychain for the VPN certificate.

## Escalation
If the VPN issue can't be resolved locally:
- The user should contact their IT/help desk with: VPN client name and version, error message, and whether it worked before.
- Many VPN issues are server-side (expired certificates, policy changes, server maintenance).
