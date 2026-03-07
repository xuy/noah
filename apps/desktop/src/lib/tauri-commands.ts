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
  action_taken: boolean;
  action_confirmation: boolean;
}

export interface KnowledgeEntry {
  category: string;
  filename: string;
  path: string;
  title: string;
}

// ── Tauri Command Wrappers ──

export async function createSession(): Promise<SessionInfo> {
  return await invoke<SessionInfo>("create_session");
}

export async function sendMessage(
  sessionId: string,
  message: string,
  isConfirmation?: boolean,
): Promise<string> {
  return await invoke<string>("send_message", {
    sessionId,
    message,
    isConfirmation,
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

export async function redeemInviteCode(
  proxyUrl: string,
  inviteCode: string,
): Promise<void> {
  await invoke<void>("redeem_invite_code", { proxyUrl, inviteCode });
}

export async function getAuthMode(): Promise<"api_key" | "proxy"> {
  return await invoke<"api_key" | "proxy">("get_auth_mode");
}

export async function clearAuth(): Promise<void> {
  await invoke<void>("clear_auth");
}

export async function listKnowledge(
  category?: string,
): Promise<KnowledgeEntry[]> {
  return await invoke<KnowledgeEntry[]>("list_knowledge", { category });
}

export async function readKnowledgeFile(path: string): Promise<string> {
  return await invoke<string>("read_knowledge_file", { path });
}

export async function deleteKnowledgeFile(path: string): Promise<void> {
  await invoke<void>("delete_knowledge_file", { path });
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

// ── Proactive Suggestions ──

export async function getProactiveEnabled(): Promise<boolean> {
  return await invoke<boolean>("get_proactive_enabled");
}

export async function setProactiveEnabled(enabled: boolean): Promise<void> {
  await invoke<void>("set_proactive_enabled", { enabled });
}

export async function dismissProactiveSuggestion(id: string): Promise<void> {
  await invoke<void>("dismiss_proactive_suggestion", { id });
}

export async function actOnProactiveSuggestion(id: string): Promise<void> {
  await invoke<void>("act_on_proactive_suggestion", { id });
}

// ── Scanner / Diagnostics ──

export interface ScanJobRecord {
  id: string;
  scan_type: string;
  status: string;
  progress_pct: number;
  progress_detail: string | null;
  budget_secs: number | null;
  started_at: string | null;
  updated_at: string | null;
  completed_at: string | null;
  config: string | null;
}

export async function getScanJobs(): Promise<ScanJobRecord[]> {
  return await invoke<ScanJobRecord[]>("get_scan_jobs");
}

export async function triggerScan(scanType: string): Promise<string> {
  return await invoke<string>("trigger_scan", { scanType });
}

export async function pauseScan(scanType: string): Promise<void> {
  await invoke<void>("pause_scan", { scanType });
}

export async function resumeScan(scanType: string): Promise<void> {
  await invoke<void>("resume_scan", { scanType });
}
