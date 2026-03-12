import * as SecureStore from "expo-secure-store";

// Auth configuration
const PROXY_URL = "https://noah-proxy.fly.dev";
const AUTH_TOKEN_KEY = "noah_auth_token";
const AUTH_USER_KEY = "noah_auth_user";

export interface AuthUser {
  email: string;
  name: string | null;
  subscription_tier: string;
  expires_at: string | null;
}

export interface AuthState {
  token: string | null;
  user: AuthUser | null;
}

export function getProxyUrl(): string {
  return PROXY_URL;
}

export async function getAuthState(): Promise<AuthState> {
  try {
    const token = await SecureStore.getItemAsync(AUTH_TOKEN_KEY);
    const userRaw = await SecureStore.getItemAsync(AUTH_USER_KEY);
    const user = userRaw ? (JSON.parse(userRaw) as AuthUser) : null;
    return { token, user };
  } catch {
    return { token: null, user: null };
  }
}

export async function saveAuth(token: string, user: AuthUser): Promise<void> {
  await SecureStore.setItemAsync(AUTH_TOKEN_KEY, token);
  await SecureStore.setItemAsync(AUTH_USER_KEY, JSON.stringify(user));
}

export async function clearAuth(): Promise<void> {
  await SecureStore.deleteItemAsync(AUTH_TOKEN_KEY);
  await SecureStore.deleteItemAsync(AUTH_USER_KEY);
}

export async function signInWithGoogle(idToken: string): Promise<AuthState> {
  const res = await fetch(`${PROXY_URL}/auth/google`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ id_token: idToken }),
  });

  if (!res.ok) {
    const body = await res.text();
    if (res.status === 403) {
      throw new Error("Access denied. You may need an invite code first.");
    }
    throw new Error(`Sign-in failed (${res.status}): ${body.slice(0, 200)}`);
  }

  const data = (await res.json()) as { token: string; user: AuthUser };
  await saveAuth(data.token, data.user);
  return { token: data.token, user: data.user };
}

export async function redeemInviteCode(code: string): Promise<AuthState> {
  const res = await fetch(`${PROXY_URL}/auth/redeem`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ invite_code: code }),
  });

  if (!res.ok) {
    const body = await res.text();
    if (res.status === 404) {
      throw new Error("Invalid invite code. Please check and try again.");
    }
    if (res.status === 409) {
      throw new Error("This invite code has already been used.");
    }
    throw new Error(`Redeem failed (${res.status}): ${body.slice(0, 200)}`);
  }

  const data = (await res.json()) as { token: string; user?: AuthUser };
  const user = data.user ?? {
    email: "",
    name: null,
    subscription_tier: "trial",
    expires_at: null,
  };
  await saveAuth(data.token, user);
  return { token: data.token, user };
}

export async function getProxyHeaders(): Promise<Record<string, string>> {
  const token = await SecureStore.getItemAsync(AUTH_TOKEN_KEY);
  if (!token) return {};
  return { Authorization: `Bearer ${token}` };
}

export async function isAuthenticated(): Promise<boolean> {
  const token = await SecureStore.getItemAsync(AUTH_TOKEN_KEY);
  return !!token;
}
