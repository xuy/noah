---
name: network-diagnostics
description: Systematic connectivity troubleshooting for Wi-Fi, DNS, and internet issues
platform: macos
---

# Network Diagnostics

## When to activate
User reports: can't connect, Wi-Fi dropping, slow internet, DNS errors, pages not loading, "no internet" warnings, VPN issues, network timeouts.

## Protocol

### Step 1: Quick connectivity check
Run `mac_ping` to `8.8.8.8` with count 3 (raw IP — tests basic connectivity without DNS).
- **If ping succeeds:** Internet is reachable. Skip to Step 3 (DNS/application layer).
- **If ping fails:** No internet connectivity. Continue to Step 2.

### Step 2: Local network check
Run `mac_network_info` to check interfaces and Wi-Fi association.

**Check Wi-Fi association:**
- If Wi-Fi shows "not associated" or no IP address → Wi-Fi is disconnected. Tell user to reconnect via Wi-Fi menu. If it keeps dropping, run `wifi_scan` to check for channel congestion.
- If Wi-Fi has a self-assigned IP (169.254.x.x) → DHCP failed. Suggest: turn Wi-Fi off and on, or renew DHCP lease via System Settings > Network.
- If Wi-Fi has a valid IP (192.168.x.x, 10.x.x.x, 172.16-31.x.x) → local network is fine. Test gateway.

**Test gateway:**
Run `mac_ping` to the gateway IP (from network info).
- If gateway ping fails → router issue. Suggest: restart router, check Ethernet cable if wired.
- If gateway ping succeeds but 8.8.8.8 fails → ISP or upstream issue. Suggest: try a different DNS (1.1.1.1), check if ISP is down.

### Step 3: DNS check
Run `mac_dns_check` for `google.com`.
- **If DNS resolves:** DNS is working. Skip to Step 4.
- **If DNS fails:**
  - Check configured DNS servers (from `mac_network_info`).
  - Try `mac_dns_check` for `google.com` — if this also fails, DNS is broken.
  - Fix: Run `mac_flush_dns` to clear cache.
  - If still failing, suggest changing DNS to 8.8.8.8 / 1.1.1.1 in System Settings > Network > Wi-Fi > DNS.
  - **VPN conflict:** If a VPN is active, DNS may be routed through VPN's DNS. Suggest disconnecting VPN to test.

### Step 4: HTTP connectivity
Run `mac_http_check` for `https://www.google.com`.
- **If HTTP works:** Full connectivity is fine. The problem may be site-specific.
  - Test the specific URL the user is having trouble with.
- **If HTTP fails but DNS works:**
  - Check for proxy settings: Run `mac_http_check` for `http://captive.apple.com` — if this redirects, user is behind a captive portal (hotel/airport Wi-Fi). Tell them to open a browser and complete the login.
  - Check for firewall blocking: `mac_http_check` different ports/sites.

### Step 5: Wi-Fi quality analysis (if drops/slowness reported)
Run `wifi_scan` to analyze the wireless environment.
- **Signal strength:** Below -70 dBm = weak signal. Suggest moving closer to router.
- **Channel congestion:** If many networks on the same channel, suggest changing router's Wi-Fi channel (1, 6, or 11 for 2.4 GHz; any non-DFS for 5 GHz).
- **PHY mode:** If connected at 802.11n instead of 802.11ac/ax, performance will be limited.
- **Noise level:** High noise (above -80 dBm) indicates interference from microwaves, Bluetooth, or other devices.

### Step 6: Known macOS issues
- **Wi-Fi drops after wake from sleep:** Known macOS bug. Fix: turn Wi-Fi off/on, or forget and re-add the network.
- **mDNSResponder high CPU:** Can cause DNS slowness. Check with process list; if high, suggest restart: `sudo killall -HUP mDNSResponder`.
- **Slow DNS with VPN:** Split-tunnel VPNs often misconfigure DNS. Check if DNS queries go through VPN tunnel unnecessarily.
- **"Wi-Fi has no IP address":** DHCP lease expired. Renew via System Settings or turn Wi-Fi off/on.

## Escalation
If all steps pass but user still has issues:
- Ask for the specific URL/service that fails.
- Check if the problem is time-dependent (certain hours = ISP congestion).
- Suggest running a speed test (fast.com or speedtest.net in browser).
- If corporate network, may need IT department involvement (802.1X auth, certificate issues).
