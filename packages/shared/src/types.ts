// ── Data Types (mirroring Rust backend) ──

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

export interface PlaybookProgress {
  step: number;
  total: number;
  label: string;
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
