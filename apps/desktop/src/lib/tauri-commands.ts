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
  playbook_type?: string | null;
  description?: string | null;
  emoji?: string | null;
}

// ── UI Protocol Types ──

export type AssistantActionType = "RUN_STEP" | "WAIT_FOR_USER";

export interface AssistantQuestionOption {
  label: string;
  description: string;
}

export interface AssistantTextInput {
  placeholder?: string;
  default?: string;
}

export interface AssistantSecureInput {
  placeholder?: string;
  secret_name: string;
}

export interface AssistantQuestion {
  question: string;
  header: string;
  options?: AssistantQuestionOption[];
  text_input?: AssistantTextInput;
  secure_input?: AssistantSecureInput;
  multiSelect?: boolean;
}

export interface AssistantCardAction {
  label: string;
  type: AssistantActionType;
}

export interface PlaybookProgressStep {
  number: number;
  label: string;
}

export interface PlaybookProgress {
  step: number;
  total: number;
  label: string;
  /** All steps in the playbook for stepper UI. */
  all_steps?: PlaybookProgressStep[];
  /** Playbook slug name. */
  playbook_name?: string;
  /** Emoji icon from frontmatter. */
  emoji?: string;
  /** Playbook description from frontmatter. */
  description?: string;
}

export interface AssistantUiSpa {
  kind: "spa";
  situation: string;
  plan?: string;
  action: AssistantCardAction;
  progress?: PlaybookProgress;
  qr_data?: string;
}

export interface AssistantUiUserQuestion {
  kind: "user_question";
  questions: AssistantQuestion[];
  progress?: PlaybookProgress;
}

export interface AssistantUiInfo {
  kind: "done" | "info";
  summary: string;
  progress?: PlaybookProgress;
}

export type AssistantUiPayload =
  | AssistantUiSpa
  | AssistantUiUserQuestion
  | AssistantUiInfo;

export interface SendMessageV2Result {
  text: string;
  assistant_ui?: AssistantUiPayload;
}

export type UserEventType =
  | "USER_CONFIRM"
  | "USER_SKIP_OPTIONAL"
  | "USER_ANSWER_QUESTION";

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

export async function renameSession(sessionId: string, title: string): Promise<void> {
  await invoke<void>("rename_session", { sessionId, title });
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

export interface ProxyStatus {
  status: "active" | "expired" | "not_proxy";
  reason?: string;
  invite_code?: string;
}

export async function checkProxyStatus(): Promise<ProxyStatus> {
  const json = await invoke<string>("check_proxy_status");
  return JSON.parse(json);
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

// ── Auto-Heal ──

export async function getAutoHealEnabled(): Promise<boolean> {
  return await invoke<boolean>("get_auto_heal_enabled");
}

export async function setAutoHealEnabled(enabled: boolean): Promise<void> {
  await invoke<void>("set_auto_heal_enabled", { enabled });
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

// ── Health Score ──

export interface CheckResult {
  id: string;
  category: string;
  label: string;
  status: "pass" | "warn" | "fail";
  detail: string;
}

export interface CategoryScore {
  category: string;
  score: number;
  grade: string;
  checks: CheckResult[];
}

export interface HealthScore {
  overall_score: number;
  overall_grade: string;
  categories: CategoryScore[];
  computed_at: string;
  device_id: string | null;
}

export interface HealthScoreRecord {
  id: string;
  score: number;
  grade: string;
  categories: string; // JSON
  computed_at: string;
  device_id: string | null;
}

export async function getHealthScore(): Promise<HealthScore | null> {
  const json = await invoke<string>("get_health_score");
  return JSON.parse(json);
}

export async function runHealthCheck(): Promise<HealthScore> {
  const json = await invoke<string>("run_health_check");
  return JSON.parse(json);
}

export async function openHealthFix(checkId: string): Promise<void> {
  await invoke<void>("open_health_fix", { checkId });
}

export async function getHealthHistory(limit?: number): Promise<HealthScoreRecord[]> {
  const json = await invoke<string>("get_health_history", { limit });
  return JSON.parse(json);
}

export async function exportHealthReport(): Promise<string> {
  return await invoke<string>("export_health_report");
}

// ── Dashboard Link ──

export interface DashboardStatus {
  linked: boolean;
  url?: string;
  device_id?: string;
  fleet_name?: string;
  linked_at?: string;
}

export async function linkDashboard(enrollmentUrl: string): Promise<string> {
  return await invoke<string>("link_dashboard", { enrollmentUrl });
}

export async function unlinkDashboard(): Promise<void> {
  await invoke<void>("unlink_dashboard");
}

export async function getDashboardStatus(): Promise<DashboardStatus> {
  const json = await invoke<string>("get_dashboard_status");
  return JSON.parse(json);
}

// ── Fleet Actions ──

export interface FleetAction {
  id: string;
  check_id: string;
  check_label: string;
  action_hint: string;
  created_at: string;
  action_type: "hint" | "playbook";
  playbook_slug?: string;
  playbook_content?: string;
  issue_id?: string;
}

export async function getFleetActions(): Promise<FleetAction[]> {
  const json = await invoke<string>("get_fleet_actions");
  return JSON.parse(json);
}

export async function resolveFleetAction(actionId: string, status: "completed" | "dismissed"): Promise<void> {
  await invoke<void>("resolve_fleet_action", { actionId, status });
}

export async function startFleetPlaybook(actionId: string, playbookSlug: string): Promise<string> {
  return await invoke<string>("start_fleet_playbook", { actionId, playbookSlug });
}

export async function verifyRemediation(actionId: string): Promise<string> {
  return await invoke<string>("verify_remediation", { actionId });
}

// ── Consumer (account / subscription) ──

export interface Entitlement {
  plan: string | null;
  status: "none" | "trialing" | "active" | "past_due" | "canceled" | "expired";
  trial_started_at: number | null;
  trial_ends_at: number | null;
  period_start: number | null;
  period_end: number | null;
  usage_used: number;
  usage_limit: number;
  fix_count_total: number;
}

export interface FixCompletedResult {
  fix_count_total: number;
  usage_used: number;
  entitlement: Entitlement;
}

export async function consumerHasSession(): Promise<boolean> {
  return await invoke<boolean>("consumer_has_session");
}

/**
 * Request a magic link. The server will send the emailed link for
 * future-use re-auth, but will also issue a session token immediately
 * so the user can proceed without clicking the link. Returns the fresh
 * entitlement when auto-sign-in succeeded, null if the server chose to
 * gate on the email click (fallback flow).
 */
export async function consumerRequestMagicLink(
  email: string,
): Promise<Entitlement | null> {
  return await invoke<Entitlement | null>("consumer_request_magic_link", { email });
}

export async function consumerCompleteSignIn(token: string): Promise<Entitlement> {
  return await invoke<Entitlement>("consumer_complete_sign_in", { token });
}

export async function consumerSignOut(): Promise<void> {
  await invoke<void>("consumer_sign_out");
}

export async function consumerGetEntitlement(): Promise<Entitlement | null> {
  return await invoke<Entitlement | null>("consumer_get_entitlement");
}

export async function consumerNotifyIssueStarted(): Promise<Entitlement | null> {
  return await invoke<Entitlement | null>("consumer_notify_issue_started");
}

export async function consumerNotifyFixCompleted(): Promise<FixCompletedResult | null> {
  return await invoke<FixCompletedResult | null>("consumer_notify_fix_completed");
}

export async function consumerBillingCheckoutUrl(plan: "monthly" | "annual"): Promise<string> {
  return await invoke<string>("consumer_billing_checkout_url", { plan });
}

export async function consumerBillingPortalUrl(): Promise<string> {
  return await invoke<string>("consumer_billing_portal_url");
}

// ── V2 Agent Commands ──

export async function sendMessageV2(
  sessionId: string,
  message: string,
  isConfirmation?: boolean,
): Promise<SendMessageV2Result> {
  return await invoke<SendMessageV2Result>("send_message_v2", {
    sessionId,
    message,
    isConfirmation,
  });
}

export async function sendUserEvent(
  sessionId: string,
  eventType: UserEventType,
  payload?: string,
): Promise<SendMessageV2Result> {
  return await invoke<SendMessageV2Result>("send_user_event", {
    sessionId,
    eventType,
    payload,
  });
}

export async function recordActionConfirmation(
  sessionId: string,
  message: string,
): Promise<void> {
  await invoke<void>("record_action_confirmation", { sessionId, message });
}

export async function setLocale(sessionId: string, locale: string): Promise<void> {
  return invoke("set_locale", { sessionId, locale });
}

export async function setSessionMode(sessionId: string, mode: "default" | "learn"): Promise<void> {
  return invoke("set_session_mode", { sessionId, mode });
}

export async function storeSecret(
  sessionId: string,
  secretName: string,
  secretValue: string,
): Promise<void> {
  await invoke<void>("store_secret", { sessionId, secretName, secretValue });
}
