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
    /// Target platform: "macos", "windows", or "all".
    pub platform: String,
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

    for line in yaml_block.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("description:") {
            description = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("platform:") {
            platform = Some(val.trim().to_string());
        }
    }

    Some(PlaybookMeta {
        name: name?,
        description: description?,
        platform: platform.unwrap_or_else(|| "all".to_string()),
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
    /// Bootstrap playbooks directory and load metadata from all `.md` files.
    /// Only playbooks matching the current platform (or `platform: all`) are loaded.
    pub fn init(app_dir: &Path) -> Result<Self> {
        Self::init_for_platform(app_dir, current_platform())
    }

    /// Bootstrap and load playbooks, filtering to a specific platform + "all".
    fn init_for_platform(app_dir: &Path, platform: &str) -> Result<Self> {
        let playbooks_dir = app_dir.join("playbooks");
        std::fs::create_dir_all(&playbooks_dir)?;

        // Write built-in playbooks if they don't already exist (preserves user edits).
        for (filename, content) in BUILTIN_PLAYBOOKS {
            let dest = playbooks_dir.join(filename);
            if !dest.exists() {
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

    /// Render the compact playbook listing for the system prompt.
    pub fn prompt_section(&self) -> String {
        if self.metas.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();
        lines.push("## Playbooks".to_string());
        lines.push("You have expert diagnostic playbooks for complex problems. When a user describes a non-trivial issue that matches a playbook, activate it to get a step-by-step protocol.".to_string());
        lines.push(String::new());
        lines.push("Available playbooks:".to_string());
        for meta in &self.metas {
            lines.push(format!("- {}: {}", meta.name, meta.description));
        }
        lines.push(String::new());
        lines.push(
            "Use `activate_playbook` with the playbook name to load the full protocol.".to_string(),
        );

        lines.join("\n")
    }

    /// Read the full content of a playbook by name.
    fn read_playbook(&self, name: &str) -> Result<String> {
        // Scan the playbooks directory for a matching file.
        let entries = std::fs::read_dir(&self.playbooks_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(meta) = parse_frontmatter(&content) {
                        if meta.name == name {
                            return Ok(content);
                        }
                    }
                }
            }
        }

        // Not found — return an error listing available names.
        let available: Vec<&str> = self.metas.iter().map(|m| m.name.as_str()).collect();
        anyhow::bail!(
            "Playbook '{}' not found. Available playbooks: {}",
            name,
            available.join(", ")
        )
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

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test-playbook\ndescription: A test playbook\n---\n\n# Body";
        let meta = parse_frontmatter(content).unwrap();
        assert_eq!(meta.name, "test-playbook");
        assert_eq!(meta.description, "A test playbook");
        assert_eq!(meta.platform, "all"); // default
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

    #[test]
    fn test_bootstrap_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        // All 8 built-in files should exist on disk.
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
    fn test_bootstrap_preserves_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        // Write a modified version of a built-in playbook.
        let custom_content =
            "---\nname: network-diagnostics\ndescription: Custom version\nplatform: macos\n---\n\n# Custom";
        std::fs::write(
            playbooks_dir.join("network-diagnostics.md"),
            custom_content,
        )
        .unwrap();

        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();

        // The custom version should be preserved.
        let content =
            std::fs::read_to_string(playbooks_dir.join("network-diagnostics.md")).unwrap();
        assert!(content.contains("Custom version"));
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

        // Should NOT include macos-only playbooks.
        assert!(!registry.metas.iter().any(|m| m.platform == "macos"));
    }

    #[test]
    fn test_prompt_section_contains_names() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), "macos").unwrap();
        let section = registry.prompt_section();

        assert!(section.contains("network-diagnostics"));
        assert!(section.contains("outlook-troubleshooting"));
        assert!(section.contains("activate_playbook"));
    }

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
        assert!(err.to_string().contains("network-diagnostics"));
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
}
