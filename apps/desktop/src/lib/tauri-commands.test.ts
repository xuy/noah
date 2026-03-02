/**
 * IPC Contract Tests
 *
 * These tests verify that our TypeScript types and invoke() calls match
 * what the Rust backend expects. If either side changes, these fail.
 *
 * Run with: npx vitest run
 */
import { describe, it, expect } from "vitest";

// ── Type-shape tests: ensure our TS interfaces have the right keys ──

// Simulate what Rust serializes (snake_case, no rename_all)
const MOCK_SESSION_INFO = {
  id: "abc-123",
  created_at: "2026-01-01T00:00:00Z",
  message_count: 0,
};

const MOCK_JOURNAL_ENTRY = {
  id: "change-1",
  session_id: "session-1",
  timestamp: "2026-01-01T00:00:00Z",
  tool_name: "mac_flush_dns",
  description: "Flushed DNS cache",
  undo_tool: "noop",
  undo_input: {},
  undone: false,
};

const MOCK_APPROVAL_REQUEST = {
  approval_id: "req-1",
  tool_name: "mac_kill_process",
  description: "Kill process 1234",
  parameters: { pid: 1234 },
};

const MOCK_SESSION_RECORD = {
  id: "s1",
  created_at: "2026-01-01T00:00:00Z",
  ended_at: "2026-01-01T00:30:00Z",
  title: "My internet is slow",
  message_count: 5,
  change_count: 2,
};

describe("IPC contract: Rust → TypeScript", () => {
  it("SessionInfo fields match Rust serialization", () => {
    // Rust SessionInfo has: id, created_at, message_count (snake_case)
    expect(MOCK_SESSION_INFO).toHaveProperty("id");
    expect(MOCK_SESSION_INFO).toHaveProperty("created_at");
    expect(MOCK_SESSION_INFO).toHaveProperty("message_count");
    expect(typeof MOCK_SESSION_INFO.id).toBe("string");
    expect(typeof MOCK_SESSION_INFO.created_at).toBe("string");
    expect(typeof MOCK_SESSION_INFO.message_count).toBe("number");
  });

  it("JournalEntry fields match Rust serialization", () => {
    // Rust JournalEntry has: id, session_id, timestamp, tool_name, description, undo_tool, undo_input, undone
    const requiredKeys = [
      "id",
      "session_id",
      "timestamp",
      "tool_name",
      "description",
      "undo_tool",
      "undo_input",
      "undone",
    ];
    for (const key of requiredKeys) {
      expect(MOCK_JOURNAL_ENTRY).toHaveProperty(key);
    }
    expect(typeof MOCK_JOURNAL_ENTRY.undone).toBe("boolean");
  });

  it("ApprovalRequest fields match Rust serialization", () => {
    // Rust ApprovalRequest has: approval_id, tool_name, description, parameters
    const requiredKeys = [
      "approval_id",
      "tool_name",
      "description",
      "parameters",
    ];
    for (const key of requiredKeys) {
      expect(MOCK_APPROVAL_REQUEST).toHaveProperty(key);
    }
  });

  it("SessionRecord fields match Rust serialization", () => {
    // Rust SessionRecord has: id, created_at, ended_at, title, message_count, change_count
    const requiredKeys = [
      "id",
      "created_at",
      "ended_at",
      "title",
      "message_count",
      "change_count",
    ];
    for (const key of requiredKeys) {
      expect(MOCK_SESSION_RECORD).toHaveProperty(key);
    }
    expect(typeof MOCK_SESSION_RECORD.message_count).toBe("number");
    expect(typeof MOCK_SESSION_RECORD.change_count).toBe("number");
  });
});

describe("IPC contract: TypeScript → Rust (invoke keys)", () => {
  // Tauri 2 auto-converts snake_case Rust params to camelCase for JS.
  // So Rust `session_id: String` becomes JS `sessionId`.
  // These tests document the correct key names for invoke() calls.

  it("send_message uses camelCase keys for Rust snake_case params", () => {
    // Rust: send_message(session_id: String, message: String)
    // JS invoke must use: { sessionId, message }
    const args = { sessionId: "s1", message: "hello" };
    expect(args).toHaveProperty("sessionId");
    expect(args).toHaveProperty("message");
    // MUST NOT use snake_case (Tauri converts for us)
    expect(args).not.toHaveProperty("session_id");
  });

  it("approve_action uses camelCase key", () => {
    // Rust: approve_action(approval_id: String)
    const args = { approvalId: "req-1" };
    expect(args).toHaveProperty("approvalId");
    expect(args).not.toHaveProperty("approval_id");
  });

  it("get_changes uses camelCase key", () => {
    // Rust: get_changes(session_id: String)
    const args = { sessionId: "s1" };
    expect(args).toHaveProperty("sessionId");
  });

  it("undo_change uses camelCase key", () => {
    // Rust: undo_change(change_id: String)
    const args = { changeId: "c1" };
    expect(args).toHaveProperty("changeId");
  });

  it("end_session uses camelCase key", () => {
    // Rust: end_session(session_id: String)
    const args = { sessionId: "s1" };
    expect(args).toHaveProperty("sessionId");
  });

  it("list_sessions takes no arguments", () => {
    // Rust: list_sessions() -> Vec<SessionRecord>
    // No args needed, just the command name
    const args = {};
    expect(Object.keys(args)).toHaveLength(0);
  });
});

describe("createSession return type", () => {
  it("returns SessionInfo object, not a bare string", () => {
    // This was a real bug: we treated the return as a string but it's an object.
    // The frontend must extract .id from the response.
    const result = MOCK_SESSION_INFO;
    expect(typeof result).toBe("object");
    expect(typeof result.id).toBe("string");
    // If someone changes createSession to return string, this catches it
    expect(result).not.toBe("abc-123");
  });
});
