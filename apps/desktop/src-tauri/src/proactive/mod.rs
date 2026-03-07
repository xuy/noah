use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use itman_tools::Tool;
use serde::Serialize;
use tauri::Emitter;

use crate::agent::llm_client::LlmClient;
use crate::safety::journal;

/// Payload emitted to the frontend via the "proactive-suggestion" event.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestionPayload {
    pub id: String,
    pub category: String,
    pub headline: String,
    pub detail: String,
}

/// Background monitor that periodically runs read-only diagnostics,
/// sends results to Haiku for triage, and surfaces suggestions.
pub struct ProactiveMonitor {
    llm: LlmClient,
    db: Arc<Mutex<rusqlite::Connection>>,
    app_handle: tauri::AppHandle,
}

/// A diagnostic pipeline: category name + tool to run.
struct Pipeline {
    category: &'static str,
    tool: Box<dyn Tool>,
    input: serde_json::Value,
}

// ── Pure gating functions (testable without I/O) ────────────────────

/// Check if the proactive_enabled setting allows a check.
/// Default is enabled: None or "true" → true, "false" → false.
pub fn is_proactive_enabled(setting_value: Option<&str>) -> bool {
    setting_value != Some("false")
}

/// Check if enough time has elapsed since the last check.
/// Returns true if we should run (enough time passed, or no previous check).
pub fn check_interval_elapsed(last_check: Option<&str>, now: chrono::DateTime<chrono::Utc>, min_hours: i64) -> bool {
    match last_check {
        None => true,
        Some(ts_str) => {
            match chrono::DateTime::parse_from_rfc3339(ts_str) {
                Ok(ts) => {
                    let elapsed = now - ts.to_utc();
                    elapsed >= chrono::Duration::hours(min_hours)
                }
                // Unparseable timestamp → treat as stale, allow check.
                Err(_) => true,
            }
        }
    }
}

/// Check if we can show a suggestion (not shown in the last `min_hours` hours).
pub fn can_show_suggestion(last_shown: Option<&str>, now: chrono::DateTime<chrono::Utc>, min_hours: i64) -> bool {
    check_interval_elapsed(last_shown, now, min_hours)
}

impl ProactiveMonitor {
    pub fn new(
        llm: LlmClient,
        db: Arc<Mutex<rusqlite::Connection>>,
        app_handle: tauri::AppHandle,
    ) -> Self {
        Self { llm, db, app_handle }
    }

    /// Build the diagnostic pipelines for the current platform.
    fn build_pipelines() -> Vec<Pipeline> {
        let mut pipelines = Vec::new();

        #[cfg(target_os = "macos")]
        {
            use crate::platform::macos::performance::{MacDiskUsage, MacProcessList};
            use crate::platform::macos::disk_audit::DiskAudit;
            use crate::platform::macos::crash_logs::CrashLogReader;

            pipelines.push(Pipeline {
                category: "disk",
                tool: Box::new(MacDiskUsage),
                input: serde_json::json!({}),
            });
            pipelines.push(Pipeline {
                category: "disk",
                tool: Box::new(DiskAudit::new()),
                input: serde_json::json!({}),
            });
            pipelines.push(Pipeline {
                category: "performance",
                tool: Box::new(MacProcessList),
                input: serde_json::json!({"sort_by": "cpu"}),
            });
            pipelines.push(Pipeline {
                category: "crash",
                tool: Box::new(CrashLogReader),
                input: serde_json::json!({}),
            });
        }

        #[cfg(target_os = "windows")]
        {
            use crate::platform::windows::performance::{WinDiskUsage, WinProcessList};

            pipelines.push(Pipeline {
                category: "disk",
                tool: Box::new(WinDiskUsage),
                input: serde_json::json!({}),
            });
            pipelines.push(Pipeline {
                category: "performance",
                tool: Box::new(WinProcessList),
                input: serde_json::json!({"sort_by": "cpu"}),
            });
        }

        pipelines
    }

    /// Run forever: 30s initial delay, then every 6 hours.
    pub async fn run_forever(self) {
        // Initial delay to let the app finish starting up.
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        loop {
            if let Err(e) = self.run_cycle_if_due().await {
                eprintln!("[proactive] cycle error: {}", e);
            }
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    }

    /// Run a check cycle if conditions are met (enabled, auth configured, rate limit ok).
    async fn run_cycle_if_due(&self) -> anyhow::Result<()> {
        // Skip if no auth configured.
        if !self.llm.has_auth() {
            return Ok(());
        }

        let now = chrono::Utc::now();

        // Check if proactive monitoring is enabled.
        {
            let conn = self.db.lock().await;
            let enabled = journal::get_setting(&conn, "proactive_enabled")?;
            if !is_proactive_enabled(enabled.as_deref()) {
                return Ok(());
            }
        }

        // Check rate limit: don't run if last check was <5 hours ago.
        {
            let conn = self.db.lock().await;
            let last_check = journal::get_setting(&conn, "proactive_last_check")?;
            if !check_interval_elapsed(last_check.as_deref(), now, 5) {
                return Ok(());
            }
        }

        eprintln!("[proactive] starting diagnostic cycle");

        // Update last check timestamp.
        {
            let conn = self.db.lock().await;
            journal::set_setting(&conn, "proactive_last_check", &now.to_rfc3339())?;
        }

        let pipelines = Self::build_pipelines();

        // Group pipelines by category and collect tool outputs.
        let mut category_outputs: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for pipeline in &pipelines {
            match pipeline.tool.execute(&pipeline.input).await {
                Ok(result) => {
                    let entry = category_outputs
                        .entry(pipeline.category.to_string())
                        .or_default();
                    entry.push_str(&format!(
                        "--- {} ---\n{}\n\n",
                        pipeline.tool.name(),
                        result.output
                    ));
                }
                Err(e) => {
                    eprintln!(
                        "[proactive] tool {} failed: {}",
                        pipeline.tool.name(),
                        e
                    );
                }
            }
        }

        // Check if we've already shown a suggestion in the last 24h.
        let show_allowed = {
            let conn = self.db.lock().await;
            let last_shown = journal::get_setting(&conn, "proactive_last_shown")?;
            can_show_suggestion(last_shown.as_deref(), now, 24)
        };

        if !show_allowed {
            eprintln!("[proactive] rate-limited (shown <24h ago), skipping analysis");
            return Ok(());
        }

        // Send each category to Haiku for analysis.
        for (category, output) in &category_outputs {
            match self.llm.analyze_diagnostics(category, output).await {
                Ok(analysis) => {
                    if analysis.noteworthy && !analysis.headline.is_empty() {
                        eprintln!(
                            "[proactive] noteworthy finding in {}: {}",
                            category, analysis.headline
                        );

                        let id = Uuid::new_v4().to_string();

                        // Save to DB.
                        {
                            let conn = self.db.lock().await;
                            if let Err(e) = journal::insert_proactive_suggestion(
                                &conn,
                                &id,
                                category,
                                &analysis.headline,
                                &analysis.detail,
                                output,
                            ) {
                                eprintln!("[proactive] failed to save suggestion: {}", e);
                                continue;
                            }
                            // Update last shown timestamp.
                            let _ = journal::set_setting(&conn, "proactive_last_shown", &now.to_rfc3339());
                        }

                        // Emit event to frontend.
                        let payload = SuggestionPayload {
                            id,
                            category: category.clone(),
                            headline: analysis.headline,
                            detail: analysis.detail,
                        };
                        if let Err(e) = self.app_handle.emit("proactive-suggestion", &payload) {
                            eprintln!("[proactive] failed to emit event: {}", e);
                        }

                        // Only show one suggestion per cycle.
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("[proactive] analysis failed for {}: {}", category, e);
                }
            }
        }

        eprintln!("[proactive] cycle complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // ── is_proactive_enabled ─────────────────────────────────────────

    #[test]
    fn test_enabled_default_when_no_setting() {
        assert!(is_proactive_enabled(None));
    }

    #[test]
    fn test_enabled_when_set_true() {
        assert!(is_proactive_enabled(Some("true")));
    }

    #[test]
    fn test_disabled_when_set_false() {
        assert!(!is_proactive_enabled(Some("false")));
    }

    #[test]
    fn test_enabled_for_unexpected_value() {
        // Any value other than "false" is treated as enabled.
        assert!(is_proactive_enabled(Some("yes")));
        assert!(is_proactive_enabled(Some("")));
        assert!(is_proactive_enabled(Some("0")));
    }

    // ── check_interval_elapsed ───────────────────────────────────────

    #[test]
    fn test_interval_elapsed_no_previous_check() {
        let now = chrono::Utc::now();
        assert!(check_interval_elapsed(None, now, 5));
    }

    #[test]
    fn test_interval_elapsed_recent_check_blocks() {
        let now = chrono::Utc::now();
        let one_hour_ago = (now - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!check_interval_elapsed(Some(&one_hour_ago), now, 5));
    }

    #[test]
    fn test_interval_elapsed_old_check_allows() {
        let now = chrono::Utc::now();
        let six_hours_ago = (now - chrono::Duration::hours(6)).to_rfc3339();
        assert!(check_interval_elapsed(Some(&six_hours_ago), now, 5));
    }

    #[test]
    fn test_interval_elapsed_exactly_at_boundary() {
        let now = chrono::Utc::now();
        let five_hours_ago = (now - chrono::Duration::hours(5)).to_rfc3339();
        assert!(check_interval_elapsed(Some(&five_hours_ago), now, 5));
    }

    #[test]
    fn test_interval_elapsed_unparseable_timestamp_allows() {
        let now = chrono::Utc::now();
        assert!(check_interval_elapsed(Some("not-a-timestamp"), now, 5));
    }

    #[test]
    fn test_interval_elapsed_empty_string_allows() {
        let now = chrono::Utc::now();
        assert!(check_interval_elapsed(Some(""), now, 5));
    }

    // ── can_show_suggestion (24h rate limit) ─────────────────────────

    #[test]
    fn test_can_show_never_shown_before() {
        let now = chrono::Utc::now();
        assert!(can_show_suggestion(None, now, 24));
    }

    #[test]
    fn test_can_show_shown_recently_blocks() {
        let now = chrono::Utc::now();
        let twelve_hours_ago = (now - chrono::Duration::hours(12)).to_rfc3339();
        assert!(!can_show_suggestion(Some(&twelve_hours_ago), now, 24));
    }

    #[test]
    fn test_can_show_shown_long_ago_allows() {
        let now = chrono::Utc::now();
        let two_days_ago = (now - chrono::Duration::hours(48)).to_rfc3339();
        assert!(can_show_suggestion(Some(&two_days_ago), now, 24));
    }

    #[test]
    fn test_can_show_exactly_at_24h() {
        let now = chrono::Utc::now();
        let exactly_24h = (now - chrono::Duration::hours(24)).to_rfc3339();
        assert!(can_show_suggestion(Some(&exactly_24h), now, 24));
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn test_future_timestamp_blocks() {
        // If somehow the last_check is in the future, it should block
        // (negative elapsed < min_hours).
        let now = chrono::Utc::now();
        let future = (now + chrono::Duration::hours(2)).to_rfc3339();
        assert!(!check_interval_elapsed(Some(&future), now, 5));
    }

    #[test]
    fn test_zero_hour_interval_always_allows() {
        let now = chrono::Utc::now();
        let just_now = now.to_rfc3339();
        assert!(check_interval_elapsed(Some(&just_now), now, 0));
    }

    #[test]
    fn test_different_timezone_offset_handled() {
        // RFC 3339 with explicit offset should still parse correctly.
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 5, 12, 0, 0).unwrap();
        let six_hours_ago_pst = "2026-03-05T00:00:00-06:00"; // = 06:00 UTC, 6h before now
        assert!(check_interval_elapsed(Some(six_hours_ago_pst), now, 5));
    }
}
