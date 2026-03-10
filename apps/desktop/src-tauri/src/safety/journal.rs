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
    /// True if the user confirmed this action card (assistant messages only).
    pub action_taken: bool,
    /// True if this is a user confirmation message (e.g. "Go ahead").
    pub action_confirmation: bool,
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
const SCHEMA_VERSION: i32 = 7;

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

    // Ensure critical tables exist even if schema_version was advanced manually
    // or a prior migration only partially applied.
    ensure_schema_invariants(&conn)?;

    Ok(conn)
}

fn ensure_schema_invariants(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS system_scan_results (
            id          INTEGER PRIMARY KEY,
            scan_type   TEXT NOT NULL,
            category    TEXT,
            path        TEXT,
            key         TEXT,
            value_num   REAL,
            value_text  TEXT,
            metadata    TEXT,
            stale       INTEGER NOT NULL DEFAULT 0,
            scanned_at  TEXT NOT NULL,
            generation  INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_ssr_type ON system_scan_results(scan_type);
        CREATE INDEX IF NOT EXISTS idx_ssr_type_cat ON system_scan_results(scan_type, category);

        CREATE TABLE IF NOT EXISTS scan_jobs (
            id              TEXT PRIMARY KEY,
            scan_type       TEXT NOT NULL,
            status          TEXT NOT NULL,
            progress_pct    INTEGER NOT NULL DEFAULT 0,
            progress_detail TEXT,
            budget_secs     INTEGER,
            started_at      TEXT,
            updated_at      TEXT,
            completed_at    TEXT,
            config          TEXT
        );",
    )
    .context("Failed to ensure scanner schema invariants")?;

    Ok(())
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

    if current < 4 {
        // Migration 4: Artifacts → knowledge files.
        // The actual file migration runs in run_file_migrations() after init_db(),
        // because it needs the knowledge_dir path. This step just bumps the version.
        set_schema_version(conn, 4)?;
    }

    if current < 5 {
        // Migration 5: Add action_taken and action_confirmation columns to messages.
        let has_action_taken: bool = conn
            .prepare("SELECT action_taken FROM messages LIMIT 0")
            .is_ok();
        if !has_action_taken {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN action_taken INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE messages ADD COLUMN action_confirmation INTEGER NOT NULL DEFAULT 0;",
            )
            .context("Migration 5 failed")?;
        }
        set_schema_version(conn, 5)?;
    }

    if current < 6 {
        // Migration 6: Proactive suggestions table.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS proactive_suggestions (
                id         TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                category   TEXT NOT NULL,
                headline   TEXT NOT NULL,
                detail     TEXT NOT NULL,
                raw_data   TEXT NOT NULL,
                dismissed  INTEGER NOT NULL DEFAULT 0,
                acted_on   INTEGER NOT NULL DEFAULT 0
            );",
        )
        .context("Migration 6 failed")?;
        set_schema_version(conn, 6)?;
    }

    if current < 7 {
        // Migration 7: Background scanner tables.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS system_scan_results (
                id          INTEGER PRIMARY KEY,
                scan_type   TEXT NOT NULL,
                category    TEXT,
                path        TEXT,
                key         TEXT,
                value_num   REAL,
                value_text  TEXT,
                metadata    TEXT,
                stale       INTEGER NOT NULL DEFAULT 0,
                scanned_at  TEXT NOT NULL,
                generation  INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_ssr_type ON system_scan_results(scan_type);
            CREATE INDEX IF NOT EXISTS idx_ssr_type_cat ON system_scan_results(scan_type, category);

            CREATE TABLE IF NOT EXISTS scan_jobs (
                id              TEXT PRIMARY KEY,
                scan_type       TEXT NOT NULL,
                status          TEXT NOT NULL,
                progress_pct    INTEGER NOT NULL DEFAULT 0,
                progress_detail TEXT,
                budget_secs     INTEGER,
                started_at      TEXT,
                updated_at      TEXT,
                completed_at    TEXT,
                config          TEXT
            );",
        )
        .context("Migration 7 failed")?;
        set_schema_version(conn, 7)?;
    }

    // ── Add new migrations here ──
    // if current < 8 { ... }

    Ok(())
}

/// Run file-based migrations that require paths outside the DB.
/// Called from lib.rs after init_db() and init_knowledge_dir().
pub fn run_file_migrations(conn: &Connection, knowledge_dir: &std::path::Path) -> Result<()> {
    // Check if artifacts have already been migrated by looking for a sentinel.
    let migrated = get_setting(conn, "artifacts_migrated_to_files")?;
    if migrated.is_some() {
        return Ok(());
    }

    // Migrate artifacts to knowledge files.
    crate::knowledge::migrate_artifacts_to_files(conn, knowledge_dir)?;

    // Set sentinel so we don't re-run.
    set_setting(conn, "artifacts_migrated_to_files", "true")?;

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

/// Set the session title (only if currently NULL — for auto-titling from first message).
pub fn update_session_title(conn: &Connection, id: &str, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET title = ?1 WHERE id = ?2 AND title IS NULL",
        rusqlite::params![title, id],
    )
    .context("Failed to update session title")?;
    Ok(())
}

/// Rename a session (unconditional — overwrites any existing title).
pub fn rename_session_title(conn: &Connection, id: &str, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET title = ?1 WHERE id = ?2",
        rusqlite::params![title, id],
    )
    .context("Failed to rename session")?;
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
    save_message_with_flags(conn, session_id, role, content, false, false)
}

/// Save a display message with action flags.
pub fn save_message_with_flags(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    action_taken: bool,
    action_confirmation: bool,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, content, timestamp, action_taken, action_confirmation) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, session_id, role, content, timestamp, action_taken as i32, action_confirmation as i32],
    )
    .context("Failed to insert message")?;
    Ok(())
}

/// Mark the most recent assistant message in a session as action_taken.
pub fn mark_last_action_taken(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE messages SET action_taken = 1
         WHERE id = (
             SELECT id FROM messages
             WHERE session_id = ?1 AND role = 'assistant'
             ORDER BY timestamp DESC LIMIT 1
         )",
        rusqlite::params![session_id],
    )
    .context("Failed to mark last action taken")?;
    Ok(())
}

/// Retrieve all display messages for a session, in chronological order.
pub fn get_messages(conn: &Connection, session_id: &str) -> Result<Vec<MessageRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, role, content, timestamp, action_taken, action_confirmation
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
                action_taken: row.get::<_, i32>(5).unwrap_or(0) != 0,
                action_confirmation: row.get::<_, i32>(6).unwrap_or(0) != 0,
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

/// Get a single session by ID.
pub fn get_session(conn: &Connection, session_id: &str) -> Result<Option<SessionRecord>> {
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
             WHERE s.id = ?1",
        )
        .context("Failed to prepare get_session query")?;

    let result = stmt
        .query_row(rusqlite::params![session_id], |row| {
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
        });

    match result {
        Ok(record) => Ok(Some(record)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::Error::from(e).context("Failed to get session")),
    }
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

    // ── Proactive suggestion tests ──────────────────────────────────────

    #[test]
    fn test_migration_6_creates_proactive_suggestions_table() {
        let conn = test_db();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='proactive_suggestions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_schema_version_is_7() {
        let conn = test_db();
        let version = get_schema_version(&conn);
        assert_eq!(version, 7);
    }

    #[test]
    fn test_insert_and_query_proactive_suggestion() {
        let conn = test_db();
        insert_proactive_suggestion(
            &conn,
            "sug-1",
            "disk",
            "Disk almost full",
            "Your main drive is 95% full.",
            "df -h output here",
        )
        .unwrap();

        // Verify it was inserted.
        let (headline, dismissed, acted_on): (String, i32, i32) = conn
            .query_row(
                "SELECT headline, dismissed, acted_on FROM proactive_suggestions WHERE id = ?1",
                rusqlite::params!["sug-1"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(headline, "Disk almost full");
        assert_eq!(dismissed, 0);
        assert_eq!(acted_on, 0);
    }

    #[test]
    fn test_dismiss_proactive_suggestion() {
        let conn = test_db();
        insert_proactive_suggestion(&conn, "sug-2", "perf", "High CPU", "Details", "raw")
            .unwrap();

        dismiss_proactive_suggestion(&conn, "sug-2").unwrap();

        let dismissed: i32 = conn
            .query_row(
                "SELECT dismissed FROM proactive_suggestions WHERE id = ?1",
                rusqlite::params!["sug-2"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dismissed, 1);
    }

    #[test]
    fn test_mark_suggestion_acted_on() {
        let conn = test_db();
        insert_proactive_suggestion(&conn, "sug-3", "crash", "App crashed", "Details", "raw")
            .unwrap();

        mark_suggestion_acted_on(&conn, "sug-3").unwrap();

        let acted_on: i32 = conn
            .query_row(
                "SELECT acted_on FROM proactive_suggestions WHERE id = ?1",
                rusqlite::params!["sug-3"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(acted_on, 1);
    }

    #[test]
    fn test_dismiss_nonexistent_suggestion_is_ok() {
        // Dismiss on a missing ID should succeed (0 rows affected, not an error).
        let conn = test_db();
        let result = dismiss_proactive_suggestion(&conn, "does-not-exist");
        assert!(result.is_ok());
    }

    #[test]
    fn test_duplicate_suggestion_id_fails() {
        let conn = test_db();
        insert_proactive_suggestion(&conn, "dup-1", "disk", "A", "B", "C").unwrap();
        let result = insert_proactive_suggestion(&conn, "dup-1", "disk", "D", "E", "F");
        assert!(result.is_err(), "Duplicate primary key should fail");
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

    #[test]
    fn test_get_session_returns_existing_session() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();
        update_session_message_count(&conn, "s1", 5).unwrap();

        let session = get_session(&conn, "s1").unwrap();
        assert!(session.is_some());
        let session = session.unwrap();
        assert_eq!(session.id, "s1");
        assert_eq!(session.created_at, "2026-01-01T00:00:00Z");
        assert_eq!(session.message_count, 5);
        assert_eq!(session.change_count, 0);
    }

    #[test]
    fn test_get_session_returns_none_for_nonexistent() {
        let conn = test_db();
        let session = get_session(&conn, "nonexistent").unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn test_get_session_includes_change_count() {
        let conn = test_db();
        create_session_record(&conn, "s1", "2026-01-01T00:00:00Z").unwrap();
        update_session_message_count(&conn, "s1", 3).unwrap();

        let change = ChangeRecord {
            description: "test change".to_string(),
            undo_tool: "test_tool".to_string(),
            undo_input: serde_json::json!({}),
        };
        record_change(&conn, "s1", "tool", &change).unwrap();
        record_change(&conn, "s1", "tool", &change).unwrap();

        let session = get_session(&conn, "s1").unwrap().unwrap();
        assert_eq!(session.change_count, 2);
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

// ── Proactive suggestions ────────────────────────────────────────────

/// Insert a proactive suggestion into the database.
pub fn insert_proactive_suggestion(
    conn: &Connection,
    id: &str,
    category: &str,
    headline: &str,
    detail: &str,
    raw_data: &str,
) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO proactive_suggestions (id, created_at, category, headline, detail, raw_data)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, created_at, category, headline, detail, raw_data],
    )
    .context("Failed to insert proactive suggestion")?;
    Ok(())
}

/// Mark a proactive suggestion as dismissed.
pub fn dismiss_proactive_suggestion(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE proactive_suggestions SET dismissed = 1 WHERE id = ?1",
        rusqlite::params![id],
    )
    .context("Failed to dismiss proactive suggestion")?;
    Ok(())
}

/// Mark a proactive suggestion as acted on.
pub fn mark_suggestion_acted_on(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE proactive_suggestions SET acted_on = 1 WHERE id = ?1",
        rusqlite::params![id],
    )
    .context("Failed to mark suggestion acted on")?;
    Ok(())
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

// ── Scan Jobs ────────────────────────────────────────────────────────

/// A scan job record from the scan_jobs table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanJobRecord {
    pub id: String,
    pub scan_type: String,
    pub status: String,
    pub progress_pct: i32,
    pub progress_detail: Option<String>,
    pub budget_secs: Option<i32>,
    pub started_at: Option<String>,
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
    pub config: Option<String>,
}

/// Upsert a scan job (insert or update by id).
pub fn upsert_scan_job(conn: &Connection, job: &ScanJobRecord) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO scan_jobs (id, scan_type, status, progress_pct, progress_detail, budget_secs, started_at, updated_at, completed_at, config)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            job.id,
            job.scan_type,
            job.status,
            job.progress_pct,
            job.progress_detail,
            job.budget_secs,
            job.started_at,
            job.updated_at,
            job.completed_at,
            job.config,
        ],
    )
    .context("Failed to upsert scan job")?;
    Ok(())
}

/// Get the most recent scan job for a given scan_type.
pub fn get_latest_scan_job(conn: &Connection, scan_type: &str) -> Result<Option<ScanJobRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, status, progress_pct, progress_detail, budget_secs, started_at, updated_at, completed_at, config
         FROM scan_jobs WHERE scan_type = ?1
         ORDER BY COALESCE(updated_at, started_at) DESC LIMIT 1",
    )?;

    let mut rows = stmt.query_map(rusqlite::params![scan_type], |row| {
        Ok(ScanJobRecord {
            id: row.get(0)?,
            scan_type: row.get(1)?,
            status: row.get(2)?,
            progress_pct: row.get(3)?,
            progress_detail: row.get(4)?,
            budget_secs: row.get(5)?,
            started_at: row.get(6)?,
            updated_at: row.get(7)?,
            completed_at: row.get(8)?,
            config: row.get(9)?,
        })
    })?;

    match rows.next() {
        Some(Ok(job)) => Ok(Some(job)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// Get all scan jobs (for the Diagnostics UI).
pub fn list_scan_jobs(conn: &Connection) -> Result<Vec<ScanJobRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, status, progress_pct, progress_detail, budget_secs, started_at, updated_at, completed_at, config
         FROM scan_jobs ORDER BY COALESCE(updated_at, started_at) DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ScanJobRecord {
            id: row.get(0)?,
            scan_type: row.get(1)?,
            status: row.get(2)?,
            progress_pct: row.get(3)?,
            progress_detail: row.get(4)?,
            budget_secs: row.get(5)?,
            started_at: row.get(6)?,
            updated_at: row.get(7)?,
            completed_at: row.get(8)?,
            config: row.get(9)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

// ── System Scan Results ──────────────────────────────────────────────

/// A row from the system_scan_results table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub id: i64,
    pub scan_type: String,
    pub category: Option<String>,
    pub path: Option<String>,
    pub key: Option<String>,
    pub value_num: Option<f64>,
    pub value_text: Option<String>,
    pub metadata: Option<String>,
    pub stale: bool,
    pub scanned_at: String,
    pub generation: i64,
}

/// Insert a batch of scan results, replacing any existing rows for the same paths within a scan_type.
pub fn upsert_scan_results(conn: &Connection, scan_type: &str, results: &[(String, Option<String>, Option<String>, Option<f64>, Option<String>, Option<String>, bool, i64)]) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO system_scan_results (scan_type, category, path, key, value_num, value_text, metadata, stale, scanned_at, generation)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;

    for (path, category, key, value_num, value_text, metadata, stale, generation) in results {
        // Delete existing row for this path+scan_type first so we don't accumulate.
        conn.execute(
            "DELETE FROM system_scan_results WHERE scan_type = ?1 AND path = ?2",
            rusqlite::params![scan_type, path],
        )?;
        stmt.execute(rusqlite::params![
            scan_type,
            category,
            path,
            key,
            value_num,
            value_text,
            metadata,
            *stale as i32,
            now,
            generation,
        ])?;
    }

    Ok(())
}

/// Query scan results for a scan_type, optionally filtered by category, min value, and path prefix.
pub fn query_scan_results(
    conn: &Connection,
    scan_type: &str,
    category: Option<&str>,
    min_value: Option<f64>,
    path_prefix: Option<&str>,
    limit: usize,
) -> Result<Vec<ScanResult>> {
    let mut sql = String::from(
        "SELECT id, scan_type, category, path, key, value_num, value_text, metadata, stale, scanned_at, generation
         FROM system_scan_results WHERE scan_type = ?1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(scan_type.to_string())];
    let mut idx = 2;

    if let Some(cat) = category {
        sql.push_str(&format!(" AND category = ?{}", idx));
        params.push(Box::new(cat.to_string()));
        idx += 1;
    }

    if let Some(min) = min_value {
        sql.push_str(&format!(" AND value_num >= ?{}", idx));
        params.push(Box::new(min));
        idx += 1;
    }

    if let Some(prefix) = path_prefix {
        sql.push_str(&format!(" AND path LIKE ?{}", idx));
        params.push(Box::new(format!("{}%", prefix)));
        // idx += 1;
    }

    sql.push_str(" ORDER BY value_num DESC");
    sql.push_str(&format!(" LIMIT {}", limit));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(ScanResult {
            id: row.get(0)?,
            scan_type: row.get(1)?,
            category: row.get(2)?,
            path: row.get(3)?,
            key: row.get(4)?,
            value_num: row.get(5)?,
            value_text: row.get(6)?,
            metadata: row.get(7)?,
            stale: row.get::<_, i32>(8)? != 0,
            scanned_at: row.get(9)?,
            generation: row.get(10)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

/// Get the timestamp of the most recent scan result for a scan_type.
pub fn latest_scan_timestamp(conn: &Connection, scan_type: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT MAX(scanned_at) FROM system_scan_results WHERE scan_type = ?1",
    )?;
    let mut rows = stmt.query_map(rusqlite::params![scan_type], |row| row.get::<_, Option<String>>(0))?;
    match rows.next() {
        Some(Ok(ts)) => Ok(ts),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}
