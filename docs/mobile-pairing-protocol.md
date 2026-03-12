# Desktop-Mobile Pairing Protocol

**Ticket:** ON-14 / NOA-5
**Status:** Design
**Date:** 2025-03-11

## Overview

Noah mobile connects to the Noah desktop app over the local network. The desktop runs a lightweight HTTP/WebSocket server; the mobile app discovers it by scanning a QR code displayed on the desktop UI. Once paired, the mobile app can submit photos for triage, receive proactive notifications, and approve pending actions remotely.

This document covers the v1 protocol (LAN-only, no cloud relay).

---

## 1. Discovery

### Mechanism

The desktop app displays a QR code in Settings > Mobile Pairing. The QR code encodes a JSON payload:

```json
{
  "v": 1,
  "host": "192.168.1.42",
  "port": 7892,
  "token": "a3f8c1...64-char-hex"
}
```

| Field   | Description |
|---------|-------------|
| `v`     | Protocol version. Mobile rejects unknown versions with upgrade prompt. |
| `host`  | Desktop's local IP (IPv4). Determined via `local_ip_address` crate or equivalent. Multi-NIC machines show a dropdown to pick the right interface. |
| `port`  | Embedded server port. Default `7892`, configurable in settings if conflicts arise. |
| `token` | One-time pairing token. 32 random bytes, hex-encoded (64 chars). Expires after 5 minutes or first successful use. |

### QR Code Lifecycle

1. User clicks "Pair Mobile Device" on desktop.
2. Desktop generates token, stores `(token, created_at)` in memory (not DB).
3. QR code renders on screen. A countdown shows remaining validity.
4. After 5 minutes or successful pairing, the token is invalidated and the QR disappears.
5. If the user closes the dialog, the token is immediately invalidated.

### Why QR (not mDNS, not Bluetooth)

- Works on every network, no mDNS/Bonjour dependency.
- No Bluetooth pairing UX complexity.
- Physically proves the user has access to the desktop (security boundary).
- Zero configuration. Scan and go.

---

## 2. Embedded Server

### Stack

An axum server bound to `0.0.0.0:{port}` runs inside the Tauri backend. It is started lazily when the user opens the pairing screen (or immediately on startup if an active pairing exists).

```
apps/desktop/src-tauri/src/mobile_server.rs   — server setup, routes
apps/desktop/src-tauri/src/mobile_auth.rs     — token + shared secret logic
apps/desktop/src-tauri/src/mobile_handlers.rs — request handlers
```

### Why axum

- Already in the Rust ecosystem (tokio-based, like the rest of the backend).
- Lightweight. No need for a full framework.
- Native WebSocket and SSE support.
- `axum` is a dependency of `tauri` transitively, so no new dependency tree.

### Server Lifecycle

- **Start:** On app launch if `paired_device` exists in settings, or when user opens pairing screen.
- **Stop:** On app quit. Optionally stoppable from settings ("Disable mobile access").
- **Port conflict:** If `7892` is taken, try `7893..7899`, then fail with a user-visible error and let them pick a port.

---

## 3. Pairing Handshake

### Step-by-step

```
Mobile                              Desktop
  |                                    |
  |--- POST /pair {token} ----------->|
  |                                    | validate token (exists, not expired)
  |                                    | generate shared_secret (32 bytes)
  |                                    | store device record
  |<-- 200 {device_id, secret} -------|
  |                                    |
  |  (all subsequent requests include  |
  |   Authorization: Bearer <secret>)  |
```

### Token vs. Shared Secret

The **token** is one-time and short-lived. It proves the mobile user physically saw the desktop screen. Once consumed, it is deleted.

The **shared secret** is persistent. It lives until the user explicitly unpairs. It is stored:

- **Desktop:** in the SQLite `settings` table, row `paired_device`, value is JSON with `{device_id, secret_hash, paired_at, device_name}`. We store `BLAKE3(secret)`, not the raw secret.
- **Mobile:** in the OS keychain (iOS Keychain / Android Keystore). Raw secret bytes.

### Re-pairing

If the mobile app loses its secret (reinstall, new phone), the user repeats the QR flow. The old device record is replaced. Only one mobile device paired at a time in v1.

---

## 4. API Surface

All endpoints except `POST /pair` require `Authorization: Bearer <shared_secret>`.

### 4.1 `POST /pair`

Establish pairing. Called once.

**Request:**
```json
{
  "token": "a3f8c1...64-char-hex",
  "device_name": "iPhone 15",
  "device_os": "iOS 18.2"
}
```

**Response (200):**
```json
{
  "device_id": "d_8a7b3c",
  "secret": "e4f2a1...64-char-hex",
  "desktop_name": "MacBook Pro",
  "noah_version": "0.7.2"
}
```

**Errors:**
| Code | Meaning |
|------|---------|
| 401  | Token invalid or expired |
| 409  | Another device is already paired (v1 limit) |
| 410  | Token already consumed |

### 4.2 `POST /triage`

Submit a photo for diagnostic triage. Desktop creates a new session and runs the appropriate playbook.

**Request:**
```
Content-Type: multipart/form-data

Fields:
  photo: <binary JPEG/PNG, max 10 MB>
  caption: "Weird noise from the printer"  (optional text)
  context: "home_office"                    (optional location/tag)
```

**Response (202):**
```json
{
  "session_id": "s_abc123",
  "status": "queued",
  "message": "Photo received. Starting analysis."
}
```

The desktop processes the image asynchronously. The mobile app monitors progress via the SSE stream (section 4.3).

**Processing flow on desktop:**
1. Save image to temp dir.
2. Call vision-capable LLM with the image + caption to identify the device/issue.
3. Select appropriate playbook or start freeform diagnostic.
4. Session appears in the desktop UI as a new active session.

**Errors:**
| Code | Meaning |
|------|---------|
| 413  | Image too large (>10 MB) |
| 429  | Another triage is already in progress |
| 503  | Desktop is busy (active user session) |

### 4.3 `GET /notifications`

Server-Sent Events stream. Long-lived connection. Delivers proactive suggestions and session updates.

**Event types:**

```
event: proactive_suggestion
data: {"id":"ps_1","title":"macOS update available","severity":"low","summary":"macOS 15.3 is available. Contains security fixes.","suggested_action":"run_update","created_at":"2025-03-11T10:00:00Z"}

event: session_update
data: {"session_id":"s_abc123","step":2,"total_steps":5,"status":"running","summary":"Checking printer driver version..."}

event: session_complete
data: {"session_id":"s_abc123","result":"resolved","summary":"Reinstalled printer driver. Test page printed successfully."}

event: approval_needed
data: {"id":"ap_7","session_id":"s_abc123","action":"restart_print_spooler","description":"Restart the print spooler service to apply driver changes","risk":"low","timeout_seconds":300}

event: heartbeat
data: {"t":1710150000}
```

**Heartbeat:** Sent every 30 seconds. If the mobile app receives no heartbeat for 90 seconds, it should show a "Desktop unreachable" indicator and attempt reconnection.

**Reconnection:** Mobile sends `Last-Event-ID` header on reconnect. Desktop replays missed events from an in-memory ring buffer (last 100 events, last 10 minutes).

### 4.4 `POST /approve/{id}`

Approve or reject a pending action.

**Request:**
```json
{
  "decision": "approve",
  "comment": ""
}
```

`decision` is one of: `"approve"`, `"reject"`, `"defer"`.

**Response (200):**
```json
{
  "id": "ap_7",
  "decision": "approve",
  "executed": true
}
```

**Errors:**
| Code | Meaning |
|------|---------|
| 404  | Approval ID not found or already resolved |
| 408  | Approval timed out (past `timeout_seconds`) |
| 409  | Already decided (race with desktop user) |

### 4.5 `GET /status`

Health check and desktop state summary. Useful for the mobile app's home screen.

**Response (200):**
```json
{
  "desktop_online": true,
  "active_sessions": 1,
  "pending_approvals": 0,
  "pending_suggestions": 3,
  "uptime_seconds": 86400,
  "noah_version": "0.7.2"
}
```

---

## 5. Security

### Threat Model

- **Attacker on same LAN** can see traffic, attempt to connect.
- **Physical access** to desktop screen is the trust anchor (QR scan).
- **Stolen shared secret** allows full mobile API access until unpaired.

### Mitigations

| Threat | Mitigation |
|--------|------------|
| Token brute-force | 256-bit token space. Rate limit `POST /pair` to 3 attempts/minute. Lock out after 10 failures for 1 hour. |
| Eavesdropping | TLS with self-signed cert. Desktop generates a CA + cert on first run, pins it in the QR payload (add `cert_fingerprint` field). Mobile validates fingerprint on connect. |
| Replay attacks | Each request includes `X-Timestamp` header. Desktop rejects requests older than 60 seconds. |
| Secret exfiltration | Secret stored in OS keychain, never in plaintext files. Desktop stores only `BLAKE3(secret)`. |
| Rogue desktop | Mobile shows the desktop name and cert fingerprint for user verification. |

### TLS Detail

The QR payload in v1.1 adds:

```json
{
  "v": 1,
  "host": "192.168.1.42",
  "port": 7892,
  "token": "a3f8c1...",
  "cert_fp": "sha256:b94d27b9934d3e08..."
}
```

The mobile app pins this fingerprint and rejects connections to any other cert. This gives us encryption + authentication without a public CA.

For v1.0, TLS is optional (adds complexity to the first implementation). The shared secret over plaintext HTTP on a local network is an acceptable starting risk for a beta.

---

## 6. Error Handling

### Desktop Goes Offline

| Scenario | Mobile behavior |
|----------|-----------------|
| Desktop app quits | SSE stream drops. Mobile shows "Desktop offline" after 90s heartbeat timeout. Queues any pending actions locally. |
| Desktop sleeps | Same as quit. macOS `caffeinate` is NOT used; we don't fight the OS. |
| Network change (desktop gets new IP) | SSE drops. Mobile retries old IP for 30s, then shows "Desktop moved — re-scan QR code." |
| Desktop crashes | Same as quit. On restart, server resumes, mobile reconnects automatically if IP unchanged. |

### Mobile Goes Offline

| Scenario | Desktop behavior |
|----------|-----------------|
| Mobile disconnects | SSE connection drops. Desktop continues normally. Events buffer in ring buffer. |
| Mobile reconnects | Replays missed events via `Last-Event-ID`. |
| Approval timeout | If mobile doesn't respond within `timeout_seconds`, desktop shows approval prompt in its own UI as fallback. |

### Token Errors

| Scenario | Behavior |
|----------|----------|
| Token expired (>5 min) | `POST /pair` returns 401. Mobile shows "Token expired — generate a new QR code on desktop." |
| Token already used | `POST /pair` returns 410. Mobile shows "Token already used." |
| Wrong token | `POST /pair` returns 401. After 10 failures, desktop locks pairing for 1 hour. |

### Secret Invalidation

If the desktop user unpairs from desktop settings:
1. Desktop deletes the device record.
2. Next SSE heartbeat includes `"unpaired": true` (or connection is closed with a specific close code).
3. Mobile clears its stored secret and shows "Unpaired by desktop."

---

## 7. Data Flow Examples

### Example: Photo Triage End-to-End

```
1. User takes photo of broken printer on mobile.
2. Mobile → POST /triage (photo + "paper jam error")
3. Desktop → 202 Accepted, session s_abc123 created
4. Desktop runs vision analysis → identifies HP LaserJet, paper jam
5. Desktop → SSE: session_update {step 1, "Identifying device..."}
6. Desktop activates printer-diagnostics playbook
7. Desktop → SSE: session_update {step 2, "Checking print queue..."}
8. Desktop finds stuck job, needs approval to clear it
9. Desktop → SSE: approval_needed {id: ap_7, "Clear print queue"}
10. User taps Approve on mobile
11. Mobile → POST /approve/ap_7 {decision: "approve"}
12. Desktop clears queue, resumes playbook
13. Desktop → SSE: session_complete {result: "resolved"}
14. Mobile shows "Issue resolved" card
```

### Example: Proactive Suggestion

```
1. Desktop background monitor detects outdated macOS
2. Desktop → SSE: proactive_suggestion {id: ps_1, "macOS update available"}
3. Mobile shows notification card
4. User taps "Run Update" on mobile
5. Mobile → POST /approve/ps_1 {decision: "approve"}
6. Desktop starts update session
```

---

## 8. Implementation Plan

### Phase 1: Bare bones (v0.8)

- [ ] Embedded axum server in Tauri backend, starts on demand
- [ ] `POST /pair` + `GET /status` endpoints
- [ ] QR code generation (desktop UI) using `qrcode` crate + display in React
- [ ] Shared secret exchange, stored in settings DB
- [ ] Mobile: React Native or Swift prototype — scan QR, call `/pair`, show `/status`

### Phase 2: Notifications (v0.9)

- [ ] SSE `/notifications` endpoint
- [ ] Proactive suggestions forwarded to mobile
- [ ] Heartbeat + reconnection logic
- [ ] Mobile: notification cards, offline indicator

### Phase 3: Photo triage (v1.0)

- [ ] `POST /triage` with multipart image upload
- [ ] Vision LLM integration for photo analysis
- [ ] Session creation from mobile-submitted photos
- [ ] Session progress streaming over SSE

### Phase 4: Remote approval (v1.0)

- [ ] `POST /approve/{id}` endpoint
- [ ] Approval timeout + desktop fallback
- [ ] Mobile: approval cards with approve/reject buttons

### Phase 5: Security hardening (v1.1)

- [ ] Self-signed TLS with cert pinning
- [ ] Timestamp-based replay protection
- [ ] Rate limiting middleware

---

## 9. Future Considerations

### Cloud Relay

When mobile and desktop are not on the same network (e.g., user is away from home), a cloud relay is needed.

**Approach:** Lightweight relay server (Cloudflare Worker or small VPS) that both sides connect to via WebSocket. Messages are end-to-end encrypted with the shared secret — the relay sees only ciphertext.

```
Mobile ←→ wss://relay.onnoah.app/channel/{device_id} ←→ Desktop
```

The relay holds no state. It forwards encrypted blobs between the two connected parties. If neither is connected, messages are dropped (no persistence on relay).

**Discovery change:** Desktop registers with relay on startup, keyed by `device_id`. Mobile tries LAN first (stored IP), falls back to relay if unreachable after 5 seconds.

### Push Notifications

SSE requires an open connection. When the mobile app is backgrounded, the OS kills the connection. For real-time alerts:

- **iOS:** APNs via the cloud relay. Desktop sends encrypted payload → relay → APNs → mobile wakes, decrypts, shows notification.
- **Android:** FCM, same pattern.

This requires the cloud relay (Phase 5+), so it is not in scope for v1.

### Multi-Device Pairing

v1 supports one mobile device. Future versions could support multiple devices with per-device secrets and a device management UI on desktop.

### Desktop-to-Desktop

The same protocol could pair two desktops (e.g., "manage my home server from my laptop"). No design changes needed — just remove the photo-specific endpoints or make them optional.

---

## 10. Open Questions

1. **Port allocation:** Fixed default (7892) vs. random port encoded in QR? Fixed is simpler for firewall rules. Random avoids conflicts.
2. **Image processing:** Process on desktop (requires vision model API call) or send to cloud? Desktop-only keeps data local but needs API key with vision support.
3. **Mobile tech stack:** React Native (code sharing with potential future mobile app) vs. native Swift/Kotlin (better OS integration for notifications)?
4. **Approval conflicts:** If both desktop user and mobile user act on the same approval simultaneously, who wins? Current design: first write wins, second gets 409.
