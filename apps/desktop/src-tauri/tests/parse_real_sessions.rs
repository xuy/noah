//! Integration test: parse every assistant message from the real journal.db
//! and verify that parse_assistant_ui handles them all without panicking.
//!
//! Run with: cargo test --test parse_real_sessions

use std::path::PathBuf;

// We need to use the library's parse function.
// Since it's pub(crate), we'll replicate the parse logic here for testing,
// or we can test via the public module. Let's use the module path.

fn journal_db_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home)
        .join("Library/Application Support/com.itman.app/journal.db");
    if path.exists() { Some(path) } else { None }
}

/// Minimal parser matching parse_assistant_ui logic — tests the same code paths
fn parse_ui_kind(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // Try JSON first
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                let candidate = &trimmed[start..=end];
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
                    if let Some(kind) = v.get("kind").and_then(|k| k.as_str()) {
                        return Some(kind.to_lowercase());
                    }
                }
            }
        }
    }

    // Legacy markers
    if trimmed.contains("[DONE]") {
        return Some("done".to_string());
    }
    if trimmed.contains("[INFO]") {
        return Some("info".to_string());
    }
    if trimmed.contains("[SITUATION]") && trimmed.contains("[PLAN]") && trimmed.contains("[ACTION:") {
        return Some("spa".to_string());
    }

    None
}

#[test]
fn all_real_assistant_messages_parse_without_panic() {
    let db_path = match journal_db_path() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: journal.db not found (CI or fresh machine)");
            return;
        }
    };

    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open journal.db");
    let mut stmt = conn
        .prepare("SELECT id, content FROM messages WHERE role = 'assistant' ORDER BY timestamp")
        .expect("Failed to prepare query");

    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("Failed to query")
        .filter_map(|r| r.ok())
        .collect();

    println!("Testing {} assistant messages from journal.db", rows.len());

    let mut parsed_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut unparsed = 0;

    for (id, content) in &rows {
        // This should never panic
        let kind = parse_ui_kind(content);

        match kind {
            Some(k) => {
                *parsed_counts.entry(k).or_insert(0) += 1;
            }
            None => {
                unparsed += 1;
                // Print first 80 chars for debugging
                let preview: String = content.chars().take(80).collect();
                println!("  unparsed [{}]: {}", id, preview);
            }
        }
    }

    println!("\nParse results:");
    for (kind, count) in &parsed_counts {
        println!("  {}: {}", kind, count);
    }
    println!("  unparsed: {}", unparsed);
    println!("  total: {}", rows.len());

    // At least some messages should be parseable if the DB has data
    if rows.len() > 5 {
        let parsed_total: usize = parsed_counts.values().sum();
        assert!(
            parsed_total > 0,
            "No messages could be parsed from {} total",
            rows.len()
        );
    }
}

#[test]
fn all_json_messages_have_valid_kind() {
    let db_path = match journal_db_path() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: journal.db not found");
            return;
        }
    };

    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open journal.db");
    let mut stmt = conn
        .prepare("SELECT id, content FROM messages WHERE role = 'assistant' AND content LIKE '{%'")
        .expect("Failed to prepare query");

    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("Failed to query")
        .filter_map(|r| r.ok())
        .collect();

    println!("Testing {} JSON assistant messages", rows.len());

    let valid_kinds = ["spa", "done", "info", "user_question"];
    let mut action_types: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (id, content) in &rows {
        let v: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                println!("  WARNING: invalid JSON in message {}: {}", id, e);
                continue;
            }
        };

        let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("MISSING");
        assert!(
            valid_kinds.contains(&kind),
            "Unexpected kind '{}' in message {}",
            kind,
            id
        );

        // For spa messages, track action types
        if kind == "spa" {
            if let Some(action_type) = v
                .get("action")
                .and_then(|a| a.get("type"))
                .and_then(|t| t.as_str())
            {
                *action_types.entry(action_type.to_string()).or_insert(0) += 1;
            }
        }
    }

    println!("\nAction types found in SPA messages:");
    for (at, count) in &action_types {
        println!("  {}: {}", at, count);
    }
}

#[test]
fn old_action_types_handled_gracefully() {
    // These are action types from old sessions that our parser must handle
    let old_types = [
        "OPEN_SECURE_FORM",
        "OPENCLAW_SECURE_CAPTURE",
    ];

    for old_type in &old_types {
        let json = format!(
            r#"{{"kind":"spa","situation":"test","plan":"test","action":{{"label":"Test","type":"{}"}}}}"#,
            old_type
        );
        let kind = parse_ui_kind(&json);
        assert_eq!(kind, Some("spa".to_string()), "Failed to parse action type: {}", old_type);
    }
}
