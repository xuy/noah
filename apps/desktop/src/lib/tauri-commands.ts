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
  resolved: boolean | null;
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

export async function deleteSession(sessionId: string): Promise<void> {
  await invoke<void>("delete_session", { sessionId });
}

export async function listSessions(): Promise<SessionRecord[]> {
  return await invoke<SessionRecord[]>("list_sessions");
}

export async function getSessionMessages(
  sessionId: string,
): Promise<MessageRecord[]> {
  return await invoke<MessageRecord[]>("get_session_messages", { sessionId });
}

export async function markResolved(
  sessionId: string,
  resolved: boolean,
): Promise<void> {
  await invoke<void>("mark_resolved", { sessionId, resolved });
}

export async function exportSession(sessionId: string): Promise<string> {
  return await invoke<string>("export_session", { sessionId });
}

export async function getSessionSummary(sessionId: string): Promise<string> {
  return await invoke<string>("get_session_summary", { sessionId });
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

export interface Suggestion {
  label: string;
  description: string;
}

export async function getContextualSuggestions(): Promise<Suggestion[]> {
  return await invoke<Suggestion[]>("get_contextual_suggestions");
}

export async function getAppVersion(): Promise<string> {
  return await invoke<string>("get_app_version");
}

export async function cancelProcessing(): Promise<void> {
  await invoke<void>("cancel_processing");
}

export async function getTelemetryConsent(): Promise<boolean> {
  return await invoke<boolean>("get_telemetry_consent");
}

export async function setTelemetryConsent(enabled: boolean): Promise<void> {
  await invoke<void>("set_telemetry_consent", { enabled });
}

export async function trackEvent(
  eventType: string,
  data: string = "{}",
): Promise<void> {
  await invoke<void>("track_event", { eventType, data });
}

export interface TraceSummary {
  timestamp: string;
  request: string;
  response: string;
}

export interface FeedbackContext {
  version: string;
  os: string;
  traces: TraceSummary[];
}

export async function getFeedbackContext(): Promise<FeedbackContext> {
  return await invoke<FeedbackContext>("get_feedback_context");
}
