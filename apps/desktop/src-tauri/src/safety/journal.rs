use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use itman_tools::ChangeRecord;

/// Safely truncate a UTF-8 string to at most `max_chars` characters.
fn truncate_utf8(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub session_id: String,
    pub timestamp: String,
    pub tool_name: String,
    pub description: String,
    pub undo_tool: String,
    pub undo_input: Value,
    pub undone: bool,
}

/// A persisted chat message for session history replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

/// A persisted session record for the session history list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub created_at: String,
    pub ended_at: Option<String>,
    pub title: Option<String>,
    pub message_count: i32,
    pub change_count: i32,
    /// User-confirmed resolution: true = resolved, false = not resolved, None = not yet marked.
    pub resolved: Option<bool>,
}

/// Current schema version. Increment when adding migrations.
const SCHEMA_VERSION: i32 = 3;

/// Initialise the journal database, creating tables if they don't exist,
/// then run any pending migrations.
///
/// Before running migrations on an existing database, a pre-migration backup
/// is saved to `<path>.pre-migrate.bak` so data is recoverable if an update
/// introduces a broken migration.
pub fn init_db(path: &str) -> Result<Connection> {
    let conn = Connection::open(path).context("Failed to open journal database")?;

    // Check if this is an existing DB that needs migration — back it up first.
    let current_version = get_schema_version(&conn);
    if current_version > 0 && current_version < SCHEMA_VERSION {
        let bak = format!("{}.pre-migrate.bak", path);
        if let Err(e) = std::fs::copy(path, &bak) {
            eprintln!("[warn] Failed to create pre-migration backup: {}", e);
        } else {
            eprintln!(
                "[info] Migrating DB from schema v{} to v{} (backup: {})",
                current_version, SCHEMA_VERSION, bak
            );
        }
    }

    // Create base tables (idempotent via IF NOT EXISTS).
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS journal (
            id          TEXT PRIMARY KEY,
            session_id  TEXT NOT NULL,
            timestamp   TEXT NOT NULL,
            tool_name   TEXT NOT NULL,
            description TEXT NOT NULL,
            undo_tool   TEXT NOT NULL,
            undo_input  TEXT NOT NULL,
            undone      INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_journal_session ON journal(session_id);

        CREATE TABLE IF NOT EXISTS sessions (
            id            TEXT PRIMARY KEY,
            created_at    TEXT NOT NULL,
            ended_at      TEXT,
            title         TEXT,
            message_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS messages (
            id          TEXT PRIMARY KEY,
            session_id  TEXT NOT NULL,
            role        TEXT NOT NULL,
            content     TEXT NOT NULL,
            timestamp   TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);

        CREATE TABLE IF NOT EXISTS llm_traces (
            id          TEXT PRIMARY KEY,
            session_id  TEXT NOT NULL,
            timestamp   TEXT NOT NULL,
            request     TEXT NOT NULL,
            response    TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_llm_traces_session ON llm_traces(session_id);

        CREATE TABLE IF NOT EXISTS artifacts (
            id          TEXT PRIMARY KEY,
            category    TEXT NOT NULL,
            title       TEXT NOT NULL,
            content     TEXT NOT NULL,
            source      TEXT NOT NULL DEFAULT 'agent',
            session_id  TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_artifacts_category ON artifacts(category);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .context("Failed to create database tables")?;

    // Run migrations based on current schema version.
    run_migrations(&conn)?;

    Ok(conn)
}

fn get_schema_version(conn: &Connection) -> i32 {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse::<i32>().ok())
    .unwrap_or(0)
}

fn set_schema_version(conn: &Connection, version: i32) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![version.to_string()],
    )
    .context("Failed to set schema version")?;
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<()> {
    let current = get_schema_version(conn);

    if current >= SCHEMA_VERSION {
        return Ok(());
    }

    // Run each migration individually and bump the version after each step.
    // SQLite does not reliably support ALTER TABLE ADD COLUMN inside an
    // explicit transaction, so each migration runs in autocommit mode.
    apply_migrations(conn, current)?;

    Ok(())
}

/// Apply individual migrations based on the current schema version.
/// Each migration runs outside an explicit transaction (autocommit)
/// because SQLite's ALTER TABLE ADD COLUMN is unreliable within BEGIN/COMMIT.
/// The schema version is bumped after each successful migration so a crash
/// mid-sequence won't re-apply already-completed steps.
fn apply_migrations(conn: &Connection, current: i32) -> Result<()> {
    if current < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS telemetry_events (
                id          TEXT PRIMARY KEY,
                event_type  TEXT NOT NULL,
                data        TEXT NOT NULL DEFAULT '{}',
                timestamp   TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_telemetry_ts ON telemetry_events(timestamp);",
        )
        .context("Migration 1 failed")?;
        set_schema_version(conn, 1)?;
    }

    if current < 2 {
        // Migration 2: (reserved — placeholder)
        set_schema_version(conn, 2)?;
    }

    if current < 3 {
        // Migration 3: Add resolved column to sessions (NULL = not yet marked)
        // Use IF NOT EXISTS pattern: check column before altering to be idempotent.
        let has_col: bool = conn
            .prepare("SELECT resolved FROM sessions LIMIT 0")
            .is_ok();
        if !has_col {
            conn.execute_batch("ALTER TABLE sessions ADD COLUMN resolved INTEGER;")
                .context("Migration 3 failed")?;
        }
        set_schema_version(conn, 3)?;
    }

    // ── Add new migrations here ──
    // if current < 4 { ... }

    Ok(())
}

/// Record a change in the journal. Returns the generated change ID.
pub fn record_change(
    conn: &Connection,
    session_id: &str,
    tool_name: &str,
    change: &ChangeRecord,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let undo_input_str =
        serde_json::to_string(&change.undo_input).context("Failed to serialise undo_input")?;

    conn.execute(
        "INSERT INTO journal (id, session_id, timestamp, tool_name, description, undo_tool, undo_input, undone)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
        rusqlite::params![
            id,
            session_id,
            timestamp,
            tool_name,
            change.description,
            change.undo_tool,
            undo_input_str,
        ],
    )
    .context("Failed to insert journal entry")?;

    Ok(id)
}

/// Retrieve all journal entries for a given session.
pub fn get_changes(conn: &Connection, session_id: &str) -> Result<Vec<JournalEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, timestamp, tool_name, description, undo_tool, undo_input, undone
             FROM journal
             WHERE session_id = ?1
             ORDER BY timestamp ASC",
        )
        .context("Failed to prepare get_changes query")?;

    let entries = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let undo_input_str: String = row.get(6)?;
            let undone_int: i32 = row.get(7)?;
            Ok(JournalEntry {
                id: row.get(0)?,
                session_id: row.get(1)?,
                timestamp: row.get(2)?,
                tool_name: row.get(3)?,
                description: row.get(4)?,
                undo_tool: row.get(5)?,
                undo_input: serde_json::from_str(&undo_input_str).unwrap_or_default(),
                undone: undone_int != 0,
            })
        })
        .context("Failed to execute get_changes query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect journal entries")?;

    Ok(entries)
}

/// Mark a change as undone.
pub fn mark_undone(conn: &Connection, change_id: &str) -> Result<()> {
    let rows = conn
        .execute(
            "UPDATE journal SET undone = 1 WHERE id = ?1",
            rusqlite::params![change_id],
        )
        .context("Failed to mark change as undone")?;

    if rows == 0 {
        anyhow::bail!("Change ID not found: {}", change_id);
    }

    Ok(())
}

// ── Session persistence ─────────────────────────────────────────────────

/// Insert a new session record when a session is created.
pub fn create_session_record(conn: &Connection, id: &str, created_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, created_at, message_count) VALUES (?1, ?2, 0)",
        rusqlite::params![id, created_at],
    )
    .context("Failed to insert session record")?;
    Ok(())
}

/// Check if a session still needs a title (i.e. title is NULL).
pub fn session_needs_title(conn: &Connection, id: &str) -> Result<bool> {
    let title: Option<String> = conn
        .query_row(
            "SELECT title FROM sessions WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .context("Failed to check session title")?;
    Ok(title.is_none())
}

/// Set the session title (typically from the first user message).
pub fn update_session_title(conn: &Connection, id: &str, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET title = ?1 WHERE id = ?2 AND title IS NULL",
        rusqlite::params![title, id],
    )
    .context("Failed to update session title")?;
    Ok(())
}

/// Update the message count for a session.
pub fn update_session_message_count(conn: &Connection, id: &str, count: i32) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET message_count = ?1 WHERE id = ?2",
        rusqlite::params![count, id],
    )
    .context("Failed to update session message count")?;
    Ok(())
}

/// Mark a session as ended.
pub fn end_session_record(conn: &Connection, id: &str, ended_at: &str, message_count: i32) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET ended_at = ?1, message_count = ?2 WHERE id = ?3",
        rusqlite::params![ended_at, message_count, id],
    )
    .context("Failed to end session record")?;
    Ok(())
}

/// Mark a session as resolved (true) or unresolved (false) by the user.
pub fn mark_session_resolved(conn: &Connection, id: &str, resolved: bool) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET resolved = ?1 WHERE id = ?2",
        rusqlite::params![resolved as i32, id],
    )
    .context("Failed to mark session resolved")?;
    Ok(())
}

/// Delete a session and all its related data (messages, journal entries, traces).
pub fn delete_session(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM messages WHERE session_id = ?1", rusqlite::params![id])
        .context("Failed to delete messages")?;
    conn.execute("DELETE FROM journal WHERE session_id = ?1", rusqlite::params![id])
        .context("Failed to delete journal entries")?;
    conn.execute("DELETE FROM llm_traces WHERE session_id = ?1", rusqlite::params![id])
        .context("Failed to delete traces")?;
    conn.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])
        .context("Failed to delete session")?;
    Ok(())
}

// ── Message persistence ────────────────────────────────────────────────

/// Save an LLM API trace (request + response) for debugging.
pub fn save_llm_trace(
    conn: &Connection,
    session_id: &str,
    request: &str,
    response: &str,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO llm_traces (id, session_id, timestamp, request, response) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, session_id, timestamp, request, response],
    )
    .context("Failed to insert LLM trace")?;
    Ok(())
}

/// Save a display message (user or assistant text) for session history replay.
pub fn save_message(conn: &Connection, session_id: &str, role: &str, content: &str) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, session_id, role, content, timestamp],
    )
    .context("Failed to insert message")?;
    Ok(())
}

/// Retrieve all display messages for a session, in chronological order.
pub fn get_messages(conn: &Connection, session_id: &str) -> Result<Vec<MessageRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, role, content, timestamp
             FROM messages
             WHERE session_id = ?1
             ORDER BY timestamp ASC",
        )
        .context("Failed to prepare get_messages query")?;

    let records = stmt
        .query_map(rusqlite::params![session_id], |row| {
            Ok(MessageRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })
        .context("Failed to execute get_messages query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect message records")?;

    Ok(records)
}

/// List all sessions, most recent first. Includes change_count from the journal table.
pub fn list_sessions(conn: &Connection) -> Result<Vec<SessionRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.created_at, s.ended_at, s.title, s.message_count,
                    COALESCE(j.change_count, 0), s.resolved
             FROM sessions s
             LEFT JOIN (
                 SELECT session_id, COUNT(*) as change_count
                 FROM journal
                 GROUP BY session_id
             ) j ON j.session_id = s.id
             WHERE s.message_count > 0
             ORDER BY s.created_at DESC",
        )
        .context("Failed to prepare list_sessions query")?;

    let records = stmt
        .query_map([], |row| {
            let resolved_int: Option<i32> = row.get(6)?;
            Ok(SessionRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                ended_at: row.get(2)?,
                title: row.get(3)?,
                message_count: row.get(4)?,
                change_count: row.get(5)?,
                resolved: resolved_int.map(|v| v != 0),
            })
        })
        .context("Failed to execute list_sessions query")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect session records")?;

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        init_db(":memory:").expect("Failed to init in-memory DB")
    }

    #[test]
    fn test_init_creates_table() {
        let conn = test_db();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='journal'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_record_and_retrieve_change() {
        let conn = test_db();
        let change = ChangeRecord {
            description: "Set DNS to 8.8.8.8 (was 192.168.1.1)".to_string(),
            undo_tool: "mac_set_dns".to_string(),
            undo_input: serde_json::json!({"dns": "192.168.1.1"}),
        };

        let id = record_change(&conn, "session-1", "mac_flush_dns", &change).unwrap();
        assert!(!id.is_empty());

        let entries = get_changes(&conn, "session-1").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, id);
        assert_eq!(entries[0].session_id, "session-1");
        assert_eq!(entries[0].tool_name, "mac_flush_dns");
        assert_eq!(entries[0].description, "Set DNS to 8.8.8.8 (was 192.168.1.1)");
        assert_eq!(entries[0].undo_tool, "mac_set_dns");
        assert!(!entries[0].undone);
    }

    #[test]
    fn test_get_changes_empty_session() {
        let conn = test_db();
        let entries = get_changes(&conn, "nonexistent").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_mark_undone() {
        let conn = test_db();
        let change = ChangeRecord {
            description: "test".to_string(),
            undo_tool: "test_tool".to_string(),
            undo_input: serde_json::json!({}),
        };
        let id = record_change(&conn, "s1", "tool", &change).unwrap();

        mark_undone(&conn, &id).unwrap();

        let entries = get_changes(&conn, "s1").unwrap();
        assert!(entries[0].undone);
    }

    #[test]
    fn test_mark_undone_missing_id() {
        let conn = test_db();
        let result = mark_undone(&conn, "does-not-exist");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_list_sessions() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();
        create_session_record(&conn, "s2", "2026-01-02T00:00:00Z").unwrap();
        // Sessions with 0 messages are filtered out; give them messages.
        update_session_message_count(&conn, "s1", 1).unwrap();
        update_session_message_count(&conn, "s2", 2).unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 2);
        // Most recent first
        assert_eq!(sessions[0].id, "s2");
        assert_eq!(sessions[1].id, "s1");
        assert!(sessions[0].ended_at.is_none());
        assert!(sessions[0].title.is_none());
        assert_eq!(sessions[0].message_count, 2);
        assert_eq!(sessions[0].change_count, 0);
    }

    #[test]
    fn test_empty_sessions_filtered_from_list() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();
        create_session_record(&conn, "s2", "2026-01-02T00:00:00Z").unwrap();
        // Only s2 has messages; s1 should be filtered out.
        update_session_message_count(&conn, "s2", 3).unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s2");
    }

    #[test]
    fn test_session_title_and_end() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();

        update_session_title(&conn, "s1", "My internet is slow").unwrap();
        end_session_record(&conn, "s1", "2026-01-01T00:30:00Z", 5).unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions[0].title.as_deref(), Some("My internet is slow"));
        assert_eq!(
            sessions[0].ended_at.as_deref(),
            Some("2026-01-01T00:30:00Z")
        );
        assert_eq!(sessions[0].message_count, 5);
    }

    #[test]
    fn test_session_title_only_sets_once() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();
        update_session_message_count(&conn, "s1", 2).unwrap();

        update_session_title(&conn, "s1", "First message").unwrap();
        update_session_title(&conn, "s1", "Second message").unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions[0].title.as_deref(), Some("First message"));
    }

    #[test]
    fn test_session_change_count_from_journal() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();
        update_session_message_count(&conn, "s1", 1).unwrap();

        let change = ChangeRecord {
            description: "test".to_string(),
            undo_tool: "t".to_string(),
            undo_input: serde_json::json!({}),
        };
        record_change(&conn, "s1", "tool", &change).unwrap();
        record_change(&conn, "s1", "tool", &change).unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions[0].change_count, 2);
    }

    #[test]
    fn test_session_record_json_keys() {
        let rec = SessionRecord {
            id: "s1".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            ended_at: None,
            title: Some("Test".to_string()),
            message_count: 3,
            change_count: 1,
            resolved: None,
        };
        let json = serde_json::to_value(&rec).unwrap();
        let obj = json.as_object().unwrap();

        for key in [
            "id",
            "created_at",
            "ended_at",
            "title",
            "message_count",
            "change_count",
            "resolved",
        ] {
            assert!(obj.contains_key(key), "Missing expected key: {}", key);
        }
        assert_eq!(obj.len(), 7);
        // Must NOT have camelCase
        assert!(!obj.contains_key("createdAt"));
        assert!(!obj.contains_key("endedAt"));
        assert!(!obj.contains_key("messageCount"));
        assert!(!obj.contains_key("changeCount"));
    }

    #[test]
    fn test_journal_entry_serializes_with_snake_case_keys() {
        // This test ensures the JSON keys match what the TypeScript frontend expects.
        let entry = JournalEntry {
            id: "abc".to_string(),
            session_id: "s1".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            tool_name: "mac_ping".to_string(),
            description: "did a thing".to_string(),
            undo_tool: "mac_undo".to_string(),
            undo_input: serde_json::json!({}),
            undone: false,
        };
        let json = serde_json::to_value(&entry).unwrap();
        let obj = json.as_object().unwrap();

        // These are the exact keys the TS ChangeEntry interface expects
        for key in ["id", "session_id", "timestamp", "tool_name", "description", "undone"] {
            assert!(obj.contains_key(key), "Missing expected key: {}", key);
        }
        // Must NOT have camelCase variants
        assert!(!obj.contains_key("sessionId"));
        assert!(!obj.contains_key("toolName"));
        assert!(!obj.contains_key("undoTool"));
    }
}

// ── Telemetry & Settings ─────────────────────────────────────────────────

/// Record an anonymous telemetry event (stored locally).
pub fn record_telemetry_event(
    conn: &Connection,
    event_type: &str,
    data: &str,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO telemetry_events (id, event_type, data, timestamp) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, event_type, data, timestamp],
    )
    .context("Failed to record telemetry event")?;

    Ok(())
}

/// Get a setting value by key.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = ?1")
        .context("Failed to prepare get_setting")?;

    let mut rows = stmt
        .query_map(rusqlite::params![key], |row| row.get::<_, String>(0))
        .context("Failed to execute get_setting")?;

    match rows.next() {
        Some(Ok(value)) => Ok(Some(value)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// Retrieve summaries of the last N LLM traces for feedback/diagnostics.
/// Returns a list of (timestamp, truncated_request, truncated_response) tuples.
pub fn get_recent_traces(conn: &Connection, limit: usize) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT timestamp, request, response FROM llm_traces ORDER BY timestamp DESC LIMIT ?1",
        )
        .context("Failed to prepare get_recent_traces")?;

    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            let ts: String = row.get(0)?;
            let req: String = row.get(1)?;
            let resp: String = row.get(2)?;
            Ok((ts, req, resp))
        })
        .context("Failed to query traces")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to collect traces")?;

    // Truncate each field to keep the output manageable
    let truncated = rows
        .into_iter()
        .map(|(ts, req, resp)| {
            let req_short = truncate_utf8(&req, 300);
            let resp_short = truncate_utf8(&resp, 300);
            (ts, req_short, resp_short)
        })
        .collect();

    Ok(truncated)
}

/// Set a setting value by key.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )
    .context("Failed to set setting")?;

    Ok(())
}
