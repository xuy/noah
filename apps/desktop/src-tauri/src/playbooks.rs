use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use itman_tools::{SafetyTier, Tool, ToolResult};

/// Metadata parsed from a playbook's YAML frontmatter.
#[derive(Debug, Clone)]
pub struct PlaybookMeta {
    pub name: String,
    pub description: String,
    /// Target platform: "macos", "windows", "linux", or "all".
    pub platform: String,
    /// Date of last review (YYYY-MM-DD). Used to flag stale playbooks.
    pub last_reviewed: Option<String>,
    /// Author or last reviewer.
    pub author: Option<String>,
    /// "system" for built-in playbooks (always refreshed), "user" for user-created ones.
    pub playbook_type: String,
}

/// Registry of available playbooks, loaded at startup.
pub struct PlaybookRegistry {
    pub playbooks_dir: PathBuf,
    pub metas: Vec<PlaybookMeta>,
}

// ── Built-in playbooks embedded at compile time ────────────────────────

const BUILTIN_PLAYBOOKS: &[(&str, &str)] = &[
    (
        "network-diagnostics.md",
        include_str!("../playbooks/network-diagnostics.md"),
    ),
    (
        "printer-repair.md",
        include_str!("../playbooks/printer-repair.md"),
    ),
    (
        "performance-forensics.md",
        include_str!("../playbooks/performance-forensics.md"),
    ),
    (
        "disk-space-recovery.md",
        include_str!("../playbooks/disk-space-recovery.md"),
    ),
    (
        "app-doctor.md",
        include_str!("../playbooks/app-doctor.md"),
    ),
    (
        "outlook-troubleshooting.md",
        include_str!("../playbooks/outlook-troubleshooting.md"),
    ),
    (
        "vpn-troubleshooting.md",
        include_str!("../playbooks/vpn-troubleshooting.md"),
    ),
    (
        "update-troubleshooting.md",
        include_str!("../playbooks/update-troubleshooting.md"),
    ),
    (
        "windows-update-troubleshooting.md",
        include_str!("../playbooks/windows-update-troubleshooting.md"),
    ),
    (
        "windows-printer-repair.md",
        include_str!("../playbooks/windows-printer-repair.md"),
    ),
];

// ── Frontmatter parser ─────────────────────────────────────────────────

/// Parse YAML frontmatter from a playbook markdown string.
/// Expects `---\n...\n---\n` at the start of the file.
fn parse_frontmatter(content: &str) -> Option<PlaybookMeta> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing `---`
    let after_first = &trimmed[3..];
    let end = after_first.find("\n---")?;
    let yaml_block = &after_first[..end];

    let mut name = None;
    let mut description = None;
    let mut platform = None;
    let mut last_reviewed = None;
    let mut author = None;
    let mut playbook_type = None;

    for line in yaml_block.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("description:") {
            description = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("platform:") {
            platform = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("last_reviewed:") {
            last_reviewed = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("author:") {
            author = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("type:") {
            playbook_type = Some(val.trim().to_string());
        }
    }

    Some(PlaybookMeta {
        name: name?,
        description: description?,
        platform: platform.unwrap_or_else(|| "all".to_string()),
        last_reviewed,
        author,
        playbook_type: playbook_type.unwrap_or_else(|| "user".to_string()),
    })
}

// ── Bootstrap & registry ───────────────────────────────────────────────

/// Return the current platform identifier used for playbook filtering.
fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

impl PlaybookRegistry {
    /// Bootstrap the `playbooks/` subdirectory inside `knowledge_dir` and load metadata.
    /// Only playbooks matching the current platform (or `platform: all`) are loaded.
    pub fn init(knowledge_dir: &Path) -> Result<Self> {
        Self::init_for_platform(knowledge_dir, current_platform())
    }

    /// Bootstrap and load playbooks, filtering to a specific platform + "all".
    fn init_for_platform(knowledge_dir: &Path, platform: &str) -> Result<Self> {
        let playbooks_dir = knowledge_dir.join("playbooks");
        std::fs::create_dir_all(&playbooks_dir)?;

        // Always overwrite system playbooks from embedded content so they stay current.
        // User playbooks (type: user or no frontmatter) are never touched here.
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let dest = playbooks_dir.join(filename);
            // Only skip if the on-disk file is explicitly user-owned.
            let is_user_owned = dest.exists() && std::fs::read_to_string(&dest)
                .ok()
                .and_then(|c| parse_frontmatter(&c))
                .map(|m| m.playbook_type == "user")
                .unwrap_or(false);
            if !is_user_owned {
                std::fs::write(&dest, content)?;
            }
        }

        // Scan directory for all .md files and parse frontmatter.
        // Only include playbooks matching current platform or "all".
        let mut metas = Vec::new();
        let entries = std::fs::read_dir(&playbooks_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(meta) = parse_frontmatter(&content) {
                        if meta.platform == "all" || meta.platform == platform {
                            metas.push(meta);
                        }
                    }
                }
            }
        }

        // Sort by name for deterministic ordering.
        metas.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self {
            playbooks_dir,
            metas,
        })
    }

    /// Read a playbook by name or filename stem, merging system + user tracks when both exist.
    ///
    /// Matching: frontmatter `name:` field first, then filename stem for user-written entries.
    ///
    /// If both a `type: system` and a `type: user` version match the same name, both are
    /// returned concatenated with origin/date annotations so the LLM can draw from both.
    fn read_playbook(&self, name: &str) -> Result<String> {
        struct Match {
            content: String,
            playbook_type: String,
            date: Option<String>,
        }

        let mut matches: Vec<Match> = Vec::new();

        let entries = std::fs::read_dir(&self.playbooks_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|ext| ext == "md") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };

            let name_matches = parse_frontmatter(&content)
                .map(|m| m.name == name)
                .unwrap_or(false)
                || path.file_stem().map(|s| s.to_string_lossy() == name).unwrap_or(false);

            if name_matches {
                let meta = parse_frontmatter(&content);
                let playbook_type = meta.as_ref()
                    .map(|m| m.playbook_type.clone())
                    .unwrap_or_else(|| "user".to_string());
                let date = meta.and_then(|m| m.last_reviewed);
                matches.push(Match { content, playbook_type, date });
            }
        }

        if matches.is_empty() {
            anyhow::bail!(
                "Playbook '{}' not found. Use `list_knowledge` with category 'playbooks' to see what's available.",
                name
            );
        }

        if matches.len() == 1 {
            return Ok(matches.remove(0).content);
        }

        // Multiple matches (system + user): concatenate with origin annotations.
        // System first, then user.
        matches.sort_by(|a, b| a.playbook_type.cmp(&b.playbook_type).reverse()); // "user" < "system"
        let mut parts: Vec<String> = Vec::new();
        for m in &matches {
            let date_str = m.date.as_deref().unwrap_or("unknown date");
            parts.push(format!(
                "<!-- [origin: {}, last updated: {}] -->\n{}",
                m.playbook_type, date_str, m.content
            ));
        }
        Ok(parts.join("\n\n---\n\n"))
    }
}

// ── ActivatePlaybookTool ───────────────────────────────────────────────

pub struct ActivatePlaybookTool {
    registry: PlaybookRegistry,
}

impl ActivatePlaybookTool {
    pub fn new(registry: PlaybookRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for ActivatePlaybookTool {
    fn name(&self) -> &str {
        "activate_playbook"
    }

    fn description(&self) -> &str {
        "Load a diagnostic playbook by name. Returns the full step-by-step protocol."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The playbook name (e.g. 'network-diagnostics')"
                }
            },
            "required": ["name"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let name = input["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let content = self.registry.read_playbook(name)?;

        Ok(ToolResult::read_only(
            content.clone(),
            json!({ "playbook": name, "loaded": true }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Frontmatter parsing ────────────────────────────────────────────

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test-playbook\ndescription: A test playbook\n---\n\n# Body";
        let meta = parse_frontmatter(content).unwrap();
        assert_eq!(meta.name, "test-playbook");
        assert_eq!(meta.description, "A test playbook");
        assert_eq!(meta.platform, "all"); // default
        assert_eq!(meta.playbook_type, "user"); // default when type: absent
    }

    #[test]
    fn test_parse_frontmatter_system_type() {
        let content = "---\nname: net\ndescription: Net diag\nplatform: macos\ntype: system\n---\n\n# Body";
        let meta = parse_frontmatter(content).unwrap();
        assert_eq!(meta.playbook_type, "system");
    }

    #[test]
    fn test_parse_frontmatter_with_platform() {
        let content =
            "---\nname: test\ndescription: Test\nplatform: macos\n---\n\n# Body";
        let meta = parse_frontmatter(content).unwrap();
        assert_eq!(meta.platform, "macos");
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just a markdown file\nNo frontmatter here.";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_missing_name() {
        let content = "---\ndescription: No name field\n---\n\n# Body";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_missing_description() {
        let content = "---\nname: no-desc\n---\n\n# Body";
        assert!(parse_frontmatter(content).is_none());
    }

    // ── Bootstrap & registry ───────────────────────────────────────────

    #[test]
    fn test_bootstrap_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        // All built-in files should exist on disk.
        for (filename, _) in BUILTIN_PLAYBOOKS {
            assert!(
                tmp.path().join("playbooks").join(filename).exists(),
                "Missing: {}",
                filename
            );
        }

        // But only macos + all playbooks loaded into metas.
        assert!(registry.metas.iter().all(|m| m.platform == "macos" || m.platform == "all"));
    }

    #[test]
    fn test_bootstrap_always_refreshes_system_playbooks() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        // Pre-write a stale/modified system playbook (type: system on disk).
        let stale = "---\nname: network-diagnostics\ndescription: Stale version\nplatform: macos\ntype: system\n---\n\n# Stale";
        std::fs::write(playbooks_dir.join("network-diagnostics.md"), stale).unwrap();

        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        // System file should be overwritten with the current embedded content.
        let content = std::fs::read_to_string(playbooks_dir.join("network-diagnostics.md")).unwrap();
        assert!(!content.contains("Stale version"), "System playbook was not refreshed");
        assert!(content.contains("type: system"));
    }

    #[test]
    fn test_bootstrap_preserves_user_owned_file() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        // A file with the same name as a built-in but marked type: user — must not be overwritten.
        let user_version = "---\nname: network-diagnostics\ndescription: My custom version\nplatform: macos\ntype: user\n---\n\n# My custom";
        std::fs::write(playbooks_dir.join("network-diagnostics.md"), user_version).unwrap();

        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        let content = std::fs::read_to_string(playbooks_dir.join("network-diagnostics.md")).unwrap();
        assert!(content.contains("My custom version"), "User-owned file was overwritten");
    }

    #[test]
    fn test_custom_playbook_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        // Add a custom playbook (platform: all by default).
        let custom = "---\nname: custom-test\ndescription: A custom playbook\n---\n\n# Custom";
        std::fs::write(playbooks_dir.join("custom-test.md"), custom).unwrap();

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        assert!(registry.metas.iter().any(|m| m.name == "custom-test"));
    }

    #[test]
    fn test_all_builtin_files_written_regardless_of_platform() {
        let tmp = tempfile::tempdir().unwrap();
        // Even when filtering for Windows, all built-in files should be written to disk
        // (so switching platforms doesn't lose playbooks).
        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), "windows").unwrap();

        for (filename, _) in BUILTIN_PLAYBOOKS {
            assert!(
                tmp.path().join("playbooks").join(filename).exists(),
                "Missing: {}",
                filename
            );
        }
    }

    // ── Platform filtering ─────────────────────────────────────────────

    #[test]
    fn test_platform_filtering_macos() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        // Should include macos-specific and cross-platform playbooks.
        assert!(registry.metas.iter().any(|m| m.name == "network-diagnostics")); // macos
        assert!(registry.metas.iter().any(|m| m.name == "outlook-troubleshooting")); // all

        // Should NOT include windows-only playbooks.
        assert!(!registry.metas.iter().any(|m| m.platform == "windows"));
    }

    #[test]
    fn test_platform_filtering_windows() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "windows").unwrap();

        // Should include cross-platform playbook.
        assert!(registry.metas.iter().any(|m| m.name == "outlook-troubleshooting"));

        // Should include windows-specific playbook.
        assert!(registry.metas.iter().any(|m| m.name == "windows-update-troubleshooting"));

        // Should NOT include macos-only playbooks.
        assert!(!registry.metas.iter().any(|m| m.platform == "macos"));
    }

    #[test]
    fn test_custom_windows_playbook_filtered_on_macos() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        let win_playbook = "---\nname: win-only\ndescription: Windows test\nplatform: windows\n---\n\n# Win";
        std::fs::write(playbooks_dir.join("win-only.md"), win_playbook).unwrap();

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        assert!(!registry.metas.iter().any(|m| m.name == "win-only"));

        // But the file is on disk (written by user), and a Windows init would pick it up.
        let win_registry = PlaybookRegistry::init_for_platform(tmp.path(), "windows").unwrap();
        assert!(win_registry.metas.iter().any(|m| m.name == "win-only"));
    }

    // ── Read / activate ────────────────────────────────────────────────

    #[test]
    fn test_read_playbook_found() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let content = registry.read_playbook("network-diagnostics").unwrap();
        assert!(content.contains("Network Diagnostics"));
    }

    #[test]
    fn test_read_playbook_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let err = registry.read_playbook("nonexistent").unwrap_err();
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("list_knowledge"));
    }

    #[test]
    fn test_read_playbook_by_filename_stem() {
        // User-written playbooks have no frontmatter — should match by filename stem.
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        let user_playbook = "# Corrupted App Repair\n\nSteps to repair a corrupted macOS app.";
        std::fs::write(playbooks_dir.join("corrupted-app-repair.md"), user_playbook).unwrap();

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let content = registry.read_playbook("corrupted-app-repair").unwrap();
        assert!(content.contains("Corrupted App Repair"));
    }

    #[test]
    fn test_read_playbook_dual_track_concatenation() {
        // Both a system and a user version of the same playbook → both returned, annotated.
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        let system_pb = "---\nname: wifi-fix\ndescription: System wifi fix\nplatform: all\nlast_reviewed: 2026-01-01\nauthor: noah-team\ntype: system\n---\n\n# System steps";
        let user_pb   = "---\nname: wifi-fix\ndescription: My notes\nplatform: all\ntype: user\n---\n\n# My extra steps";
        std::fs::write(playbooks_dir.join("wifi-fix.md"), system_pb).unwrap();
        std::fs::write(playbooks_dir.join("wifi-fix-user.md"), user_pb).unwrap();

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "all").unwrap();
        let content = registry.read_playbook("wifi-fix").unwrap();

        assert!(content.contains("System steps"), "System content missing");
        assert!(content.contains("My extra steps"), "User content missing");
        assert!(content.contains("origin: system"));
        assert!(content.contains("origin: user"));
        assert!(content.contains("2026-01-01"), "Date annotation missing");
    }

    #[test]
    fn test_every_builtin_playbook_individually_loadable() {
        let tmp = tempfile::tempdir().unwrap();
        // Use "all" as platform so every playbook passes the filter.
        // (We can't use init_for_platform("macos") because that filters out
        // hypothetical windows-only builtins. Instead, just test all files
        // have valid frontmatter and are readable.)
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        for (filename, content) in BUILTIN_PLAYBOOKS {
            std::fs::write(playbooks_dir.join(filename), content).unwrap();

            // Every built-in must have valid frontmatter.
            let meta = parse_frontmatter(content);
            assert!(
                meta.is_some(),
                "Built-in playbook {} has invalid frontmatter",
                filename
            );

            let meta = meta.unwrap();

            // Platform must be a known value.
            assert!(
                ["macos", "windows", "linux", "all"].contains(&meta.platform.as_str()),
                "Playbook {} has invalid platform: {}",
                filename,
                meta.platform
            );

            // All built-ins must declare type: system.
            assert_eq!(
                meta.playbook_type, "system",
                "Built-in playbook {} is missing 'type: system' in frontmatter",
                filename
            );
        }
    }

    #[tokio::test]
    async fn test_activate_playbook_tool_success() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        let result = tool.execute(&json!({"name": "network-diagnostics"})).await.unwrap();
        assert!(result.output.contains("Network Diagnostics"));
        assert!(result.changes.is_empty()); // read-only
    }

    #[tokio::test]
    async fn test_activate_playbook_tool_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        let err = tool.execute(&json!({"name": "nonexistent"})).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_activate_playbook_tool_missing_param() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        let err = tool.execute(&json!({})).await.unwrap_err();
        assert!(err.to_string().contains("Missing required parameter"));
    }

    #[test]
    fn test_activate_playbook_tool_is_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        assert_eq!(tool.safety_tier(), SafetyTier::ReadOnly);
        assert_eq!(tool.name(), "activate_playbook");
    }

    // ── Content validation ─────────────────────────────────────────────

    #[test]
    fn test_builtin_playbooks_have_substantial_content() {
        // Each built-in playbook should have real diagnostic content,
        // not just a stub. Check for minimum line count.
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let line_count = content.lines().count();
            assert!(
                line_count >= 30,
                "Playbook {} has only {} lines — too short for a real protocol",
                filename,
                line_count
            );
        }
    }

    #[test]
    fn test_builtin_playbooks_have_unique_names() {
        let mut names: Vec<&str> = Vec::new();
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).expect(&format!(
                "{} has invalid frontmatter",
                filename
            ));
            assert!(
                !names.contains(&meta.name.as_str()),
                "Duplicate playbook name: {}",
                meta.name
            );
            names.push(Box::leak(meta.name.into_boxed_str()));
        }
    }

    /// Verify that macOS playbooks only reference tool names that actually exist.
    /// This catches typos like `mac_dns_flush` instead of `mac_flush_dns`.
    #[test]
    fn test_macos_playbooks_reference_existing_tools() {
        use crate::agent::tool_router::ToolRouter;

        // Build the real tool router to get all registered tool names.
        let mut router = ToolRouter::new();
        crate::platform::macos::register_tools(&mut router);
        let tool_defs = router.tool_definitions();
        let tool_names: Vec<&str> = tool_defs.iter().map(|d| d.name.as_str()).collect();

        // Also accept tools registered outside platform (knowledge, playbooks).
        let extra_tools = [
            "write_knowledge",
            "search_knowledge",
            "read_knowledge",
            "list_knowledge",
            "activate_playbook",
        ];

        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).unwrap();
            if meta.platform != "macos" {
                continue; // Only check macOS playbooks for mac_* tool refs.
            }

            // Find backtick-quoted tool references in the playbook body.
            for cap in content.split('`') {
                let word = cap.trim();
                // Only check words that look like tool names (contain underscore,
                // start with mac_ or known prefixes).
                if (word.starts_with("mac_")
                    || word.starts_with("wifi_")
                    || word.starts_with("disk_")
                    || word.starts_with("crash_")
                    || word.starts_with("shell_"))
                    && word.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    assert!(
                        tool_names.contains(&word) || extra_tools.contains(&word),
                        "Playbook {} references tool `{}` which is not registered",
                        filename,
                        word
                    );
                }
            }
        }
    }

    /// Verify that Windows playbooks don't reference `mac_*` tools.
    #[test]
    fn test_windows_playbooks_dont_reference_macos_tools() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).unwrap();
            if meta.platform != "windows" {
                continue;
            }

            for cap in content.split('`') {
                let word = cap.trim();
                if word.starts_with("mac_")
                    && word.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    panic!(
                        "Windows playbook {} references macOS tool `{}`",
                        filename, word
                    );
                }
            }
        }
    }

    /// Verify that Linux playbooks don't reference `mac_*` or `win_*` tools.
    #[test]
    fn test_linux_playbooks_dont_reference_other_platform_tools() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).unwrap();
            if meta.platform != "linux" {
                continue;
            }

            for cap in content.split('`') {
                let word = cap.trim();
                if (word.starts_with("mac_") || word.starts_with("win_"))
                    && word.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    panic!(
                        "Linux playbook {} references other-platform tool `{}`",
                        filename, word
                    );
                }
            }
        }
    }

    /// Cross-platform playbooks (platform: all) should NOT reference
    /// platform-prefixed tool names like `mac_*` or `win_*`, since they
    /// need to work on both platforms.
    #[test]
    fn test_cross_platform_playbooks_avoid_platform_tool_names() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).unwrap();
            if meta.platform != "all" {
                continue;
            }

            // Check backtick-quoted words for platform-prefixed tool names.
            let mut in_backtick = false;
            for part in content.split('`') {
                if in_backtick {
                    let word = part.trim();
                    assert!(
                        !word.starts_with("mac_") && !word.starts_with("win_"),
                        "Cross-platform playbook {} references platform-specific tool `{}`. \
                         Use generic instructions instead.",
                        filename,
                        word
                    );
                }
                in_backtick = !in_backtick;
            }
        }
    }

    // ── Quality guardrails ────────────────────────────────────────────

    /// Every playbook must have an Escalation section — a bail-out path
    /// so Noah doesn't endlessly retry when the problem is beyond local fixes.
    #[test]
    fn test_builtin_playbooks_have_escalation_section() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            assert!(
                content.contains("## Escalation"),
                "Playbook {} is missing '## Escalation' section. Every playbook needs a bail-out path.",
                filename
            );
        }
    }

    /// Every playbook must have a Caveats section — conditions that change
    /// the standard fix path. Forces authors to think about edge cases.
    #[test]
    fn test_builtin_playbooks_have_caveats_section() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            assert!(
                content.contains("## Caveats"),
                "Playbook {} is missing '## Caveats' section. Document when the standard path doesn't apply.",
                filename
            );
        }
    }

    /// Every playbook must have a Key signals section — pattern matching
    /// for common user phrases that redirect the diagnosis.
    #[test]
    fn test_builtin_playbooks_have_key_signals_section() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            assert!(
                content.contains("## Key signals"),
                "Playbook {} is missing '## Key signals' section.",
                filename
            );
        }
    }

    /// Every playbook should claim a success rate (e.g. "~80%") somewhere.
    /// This forces the author to think about confidence and tells the LLM
    /// how aggressively to follow the standard path.
    #[test]
    fn test_builtin_playbooks_claim_success_rate() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            assert!(
                content.contains('%'),
                "Playbook {} never mentions a success rate (e.g. '~80%'). \
                 State how often the standard fix path resolves the issue.",
                filename
            );
        }
    }

    /// Playbooks should stay under 120 lines. They load into the LLM context
    /// when activated — shorter = cheaper, more likely followed precisely.
    #[test]
    fn test_builtin_playbooks_not_too_long() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let line_count = content.lines().count();
            assert!(
                line_count <= 120,
                "Playbook {} has {} lines (max 120). Trim it — long playbooks get skimmed.",
                filename,
                line_count
            );
        }
    }

    /// Every built-in playbook must have last_reviewed and author in frontmatter.
    #[test]
    fn test_builtin_playbooks_have_review_metadata() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).unwrap();
            assert!(
                meta.last_reviewed.is_some(),
                "Playbook {} is missing 'last_reviewed: YYYY-MM-DD' in frontmatter.",
                filename
            );
            assert!(
                meta.author.is_some(),
                "Playbook {} is missing 'author:' in frontmatter.",
                filename
            );
        }
    }

    /// last_reviewed must be a valid YYYY-MM-DD date and not older than 6 months.
    #[test]
    fn test_builtin_playbooks_not_stale() {
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let meta = parse_frontmatter(content).unwrap();
            if let Some(ref date_str) = meta.last_reviewed {
                // Validate format: YYYY-MM-DD
                let parts: Vec<&str> = date_str.split('-').collect();
                assert!(
                    parts.len() == 3
                        && parts[0].len() == 4
                        && parts[1].len() == 2
                        && parts[2].len() == 2
                        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())),
                    "Playbook {} has invalid last_reviewed date '{}'. Use YYYY-MM-DD format.",
                    filename,
                    date_str
                );

                // Check not older than 6 months (approximate: 183 days).
                let year: i32 = parts[0].parse().unwrap();
                let month: u32 = parts[1].parse().unwrap();
                let day: u32 = parts[2].parse().unwrap();

                // Simple staleness check: convert to a day count for comparison.
                let reviewed_days = year as i64 * 365 + month as i64 * 30 + day as i64;
                // Use compile-time approximate date (tests run at build time).
                // This will naturally fail when playbooks go 6 months without review.
                let now = {
                    use chrono::Datelike;
                    let today = chrono::Utc::now().date_naive();
                    today.year() as i64 * 365 + today.month() as i64 * 30 + today.day() as i64
                };

                let age_days = now - reviewed_days;
                assert!(
                    age_days <= 183,
                    "Playbook {} was last reviewed on {} ({} days ago). \
                     Review it and update the last_reviewed date.",
                    filename,
                    date_str,
                    age_days
                );
            }
        }
    }
}
