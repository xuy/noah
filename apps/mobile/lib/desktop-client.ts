import * as SecureStore from "expo-secure-store";

const PAIRING_KEY = "noah_desktop_pairing";

export interface PairingInfo {
  host: string;
  port: number;
  deviceId: string;
  secret: string;
  desktopName: string;
}

export interface QrPayload {
  version: number;
  host: string;
  port: number;
  token: string;
}

interface PairResponse {
  device_id: string;
  secret: string;
  desktop_name: string;
}

export interface TriageResponse {
  session_id: string;
  status: string;
}

export interface DesktopStatus {
  online: boolean;
  paired: boolean;
  pending_approvals: number;
  version: string;
}

function baseUrl(pairing: PairingInfo): string {
  return `http://${pairing.host}:${pairing.port}`;
}

function authHeaders(pairing: PairingInfo): Record<string, string> {
  return {
    Authorization: `Bearer ${pairing.secret}`,
    "Content-Type": "application/json",
  };
}

export function parseQrPayload(data: string): QrPayload | null {
  try {
    const obj = JSON.parse(data);
    if (obj.version === 1 && obj.host && obj.port && obj.token) {
      return obj as QrPayload;
    }
    return null;
  } catch {
    return null;
  }
}

export async function pairWithDesktop(
  qr: QrPayload,
  deviceName: string,
): Promise<PairingInfo> {
  const url = `http://${qr.host}:${qr.port}/pair`;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      token: qr.token,
      device_name: deviceName,
    }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Pairing failed (${res.status}): ${body}`);
  }

  const data = (await res.json()) as PairResponse;
  const pairing: PairingInfo = {
    host: qr.host,
    port: qr.port,
    deviceId: data.device_id,
    secret: data.secret,
    desktopName: data.desktop_name,
  };

  await savePairing(pairing);
  return pairing;
}

export async function getDesktopStatus(
  pairing: PairingInfo,
): Promise<DesktopStatus> {
  const res = await fetch(`${baseUrl(pairing)}/status`, {
    headers: authHeaders(pairing),
  });
  if (!res.ok) throw new Error(`Status check failed: ${res.status}`);
  return (await res.json()) as DesktopStatus;
}

export async function submitTriage(
  pairing: PairingInfo,
  analysis: string,
  caption?: string,
): Promise<TriageResponse> {
  const res = await fetch(`${baseUrl(pairing)}/triage`, {
    method: "POST",
    headers: authHeaders(pairing),
    body: JSON.stringify({ analysis, caption }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Triage failed (${res.status}): ${body}`);
  }

  return (await res.json()) as TriageResponse;
}

export async function approveAction(
  pairing: PairingInfo,
  id: string,
  approve: boolean,
): Promise<void> {
  const res = await fetch(`${baseUrl(pairing)}/approve/${id}`, {
    method: "POST",
    headers: authHeaders(pairing),
    body: JSON.stringify({ approve }),
  });
  if (!res.ok) throw new Error(`Approve failed: ${res.status}`);
}

// ── Persistence ──

export async function savePairing(pairing: PairingInfo): Promise<void> {
  await SecureStore.setItemAsync(PAIRING_KEY, JSON.stringify(pairing));
}

export async function loadPairing(): Promise<PairingInfo | null> {
  try {
    const raw = await SecureStore.getItemAsync(PAIRING_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as PairingInfo;
  } catch {
    return null;
  }
}

export async function clearPairing(): Promise<void> {
  await SecureStore.deleteItemAsync(PAIRING_KEY);
}
