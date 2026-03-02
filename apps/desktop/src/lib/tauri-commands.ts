import { invoke } from "@tauri-apps/api/core";

// ── Types mirroring Rust backend ──

export interface ChangeEntry {
  id: string;
  session_id: string;
  description: string;
  tool_name: string;
  timestamp: number;
  undone: boolean;
}

export interface ApprovalRequest {
  approval_id: string;
  tool_name: string;
  description: string;
  parameters: Record<string, unknown>;
  reason: string;
}

export interface SessionInfo {
  id: string;
  created_at: string;
  message_count: number;
}

export interface SessionRecord {
  id: string;
  created_at: string;
  ended_at: string | null;
  title: string | null;
  message_count: number;
  change_count: number;
}

export interface MessageRecord {
  id: string;
  session_id: string;
  role: string;
  content: string;
  timestamp: string;
}

export interface Artifact {
  id: string;
  category: string;
  title: string;
  content: string;
  source: string;
  session_id: string | null;
  created_at: string;
  updated_at: string;
}

// ── Tauri Command Wrappers ──

export async function createSession(): Promise<SessionInfo> {
  return await invoke<SessionInfo>("create_session");
}

export async function sendMessage(
  sessionId: string,
  message: string,
): Promise<string> {
  return await invoke<string>("send_message", {
    sessionId,
    message,
  });
}

export async function approveAction(approvalId: string): Promise<void> {
  await invoke<void>("approve_action", { approvalId });
}

export async function denyAction(approvalId: string): Promise<void> {
  await invoke<void>("deny_action", { approvalId });
}

export async function getChanges(sessionId: string): Promise<ChangeEntry[]> {
  return await invoke<ChangeEntry[]>("get_changes", { sessionId });
}

export async function undoChange(changeId: string): Promise<void> {
  await invoke<void>("undo_change", { changeId });
}

export async function endSession(sessionId: string): Promise<void> {
  await invoke<void>("end_session", { sessionId });
}

export async function listSessions(): Promise<SessionRecord[]> {
  return await invoke<SessionRecord[]>("list_sessions");
}

export async function getSessionMessages(
  sessionId: string,
): Promise<MessageRecord[]> {
  return await invoke<MessageRecord[]>("get_session_messages", { sessionId });
}

export async function hasApiKey(): Promise<boolean> {
  return await invoke<boolean>("has_api_key");
}

export async function setApiKey(apiKey: string): Promise<void> {
  await invoke<void>("set_api_key", { apiKey });
}

export async function listArtifacts(
  search?: string,
  category?: string,
): Promise<Artifact[]> {
  return await invoke<Artifact[]>("list_artifacts", { search, category });
}

export async function deleteArtifact(artifactId: string): Promise<void> {
  await invoke<void>("delete_artifact", { artifactId });
}

export async function getAppVersion(): Promise<string> {
  return await invoke<string>("get_app_version");
}
