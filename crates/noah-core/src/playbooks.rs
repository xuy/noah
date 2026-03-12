use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use noah_tools::{SafetyTier, Tool, ToolResult};

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

// ── Playbook runtime structures ──────────────────────────────────────

/// A step parsed from the playbook markdown body.
#[derive(Debug, Clone)]
pub struct PlaybookStep {
    pub number: u32,
    pub label: String,
}

/// Runtime state for an active playbook session. Managed by the orchestrator,
/// not the LLM. Tracks progress deterministically.
#[derive(Debug, Clone)]
pub struct PlaybookState {
    pub name: String,
    pub steps: Vec<PlaybookStep>,
    pub total_steps: u32,
    /// Number of interactive ui_* turns completed so far.
    pub current_turn: u32,
}

impl PlaybookState {
    /// Create from a playbook's full markdown content.
    pub fn from_content(name: &str, content: &str) -> Self {
        let steps = parse_steps(content);
        let total = if steps.is_empty() { 1 } else { steps.len() as u32 };
        Self {
            name: name.to_string(),
            steps,
            total_steps: total,
            current_turn: 0,
        }
    }

    /// Get the current progress as a JSON value to inject into ui_* payloads.
    /// Returns None if there are no defined steps (diagnostic playbooks).
    pub fn progress_json(&self) -> Option<serde_json::Value> {
        if self.steps.is_empty() {
            return None;
        }
        let step_index = (self.current_turn as usize).min(self.steps.len().saturating_sub(1));
        let step = &self.steps[step_index];
        Some(serde_json::json!({
            "step": step.number,
            "total": self.total_steps,
            "label": step.label
        }))
    }

    /// Advance one interactive turn.
    pub fn advance(&mut self) {
        self.current_turn += 1;
    }
}

/// Parse `## Step N: Label` headers from playbook markdown.
/// Falls back to any `## ` headers with a leading number pattern.
/// This is the playbook DSL: step structure is declared by markdown headers.
fn parse_steps(content: &str) -> Vec<PlaybookStep> {
    let mut steps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // Match: ## Step 1: Check Environment
        // Match: ## 1. Check Environment
        // Match: ## Step 1 — Check Environment
        if let Some(rest) = trimmed.strip_prefix("## ") {
            let rest = rest.trim();
            // Try "Step N: Label" or "Step N — Label" or "Step N. Label"
            if let Some(after_step) = rest.strip_prefix("Step ").or_else(|| rest.strip_prefix("step ")) {
                if let Some((num_str, label)) = split_step_number(after_step) {
                    if let Ok(n) = num_str.parse::<u32>() {
                        steps.push(PlaybookStep { number: n, label: label.to_string() });
                        continue;
                    }
                }
            }
            // Try "N. Label" or "N: Label"
            if let Some((num_str, label)) = split_step_number(rest) {
                if let Ok(n) = num_str.parse::<u32>() {
                    steps.push(PlaybookStep { number: n, label: label.to_string() });
                }
            }
        }
    }
    steps
}

/// Split "3: Configure" or "3. Configure" or "3 — Configure" into ("3", "Configure").
fn split_step_number(s: &str) -> Option<(&str, &str)> {
    // Find where digits end
    let num_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if num_end == 0 { return None; }
    let num_str = &s[..num_end];
    let rest = s[num_end..].trim();
    // Strip separator: ":", ".", "—", "-", " "
    let label = rest
        .strip_prefix(':')
        .or_else(|| rest.strip_prefix('.'))
        .or_else(|| rest.strip_prefix('—'))
        .or_else(|| rest.strip_prefix('-'))
        .unwrap_or(rest)
        .trim();
    if label.is_empty() { return None; }
    Some((num_str, label))
}

/// Registry of available playbooks, loaded at startup.
pub struct PlaybookRegistry {
    pub playbooks_dir: PathBuf,
    pub metas: Vec<PlaybookMeta>,
}

// ── Bundled playbook scanning ──────────────────────────────────────────
// Playbooks are plain files on disk — not compiled into the binary.
// They ship as Tauri bundled resources and are copied to app data at init.

/// Scan a directory for flat .md playbook files, returning (filename, content) pairs.
/// Skips TEMPLATE.md.
fn scan_flat_playbooks(dir: &Path) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return results };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_file() && name.ends_with(".md") && name != "TEMPLATE.md" {
            if let Ok(content) = std::fs::read_to_string(&path) {
                results.push((name, content));
            }
        }
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Scan a directory for folder-based playbooks (one level of subdirectories),
/// returning (folder/filename, content) pairs.
fn scan_folder_playbooks(dir: &Path) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return results };
    for entry in entries.flatten() {
        let path = entry.path();
        let folder_name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if let Ok(sub_entries) = std::fs::read_dir(&path) {
                for sub in sub_entries.flatten() {
                    let sub_name = sub.file_name().to_string_lossy().to_string();
                    if sub_name.ends_with(".md") {
                        let rel = format!("{}/{}", folder_name, sub_name);
                        if let Ok(content) = std::fs::read_to_string(sub.path()) {
                            results.push((rel, content));
                        }
                    }
                }
            }
        }
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

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
        playbook_type: playbook_type.unwrap_or_else(|| "system".to_string()),
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
    /// `bundled_dir` points to the directory containing shipped playbook .md files
    /// (Tauri resource dir in production, source dir in tests).
    /// Only playbooks matching the current platform (or `platform: all`) are loaded.
    pub fn init(knowledge_dir: &Path, bundled_dir: &Path) -> Result<Self> {
        Self::init_for_platform(knowledge_dir, bundled_dir, current_platform())
    }

    /// Bootstrap and load playbooks, filtering to a specific platform + "all".
    fn init_for_platform(knowledge_dir: &Path, bundled_dir: &Path, platform: &str) -> Result<Self> {
        let playbooks_dir = knowledge_dir.join("playbooks");
        std::fs::create_dir_all(&playbooks_dir)?;

        // Scan the bundled playbooks directory for flat and folder-based playbooks.
        let flat = scan_flat_playbooks(bundled_dir);
        let folders = scan_folder_playbooks(bundled_dir);

        // Write system playbooks that match this platform (or "all") to disk.
        // Wrong-platform playbooks are removed so they don't appear in the
        // knowledge TOC or confuse the LLM.
        for (filename, content) in &flat {
            let dest = playbooks_dir.join(filename);

            // Never overwrite user-owned files.
            let is_user_owned = dest.exists() && std::fs::read_to_string(&dest)
                .ok()
                .and_then(|c| parse_frontmatter(&c))
                .map(|m| m.playbook_type == "user")
                .unwrap_or(false);
            if is_user_owned {
                continue;
            }

            // Check if this builtin matches the current platform.
            let matches_platform = parse_frontmatter(content)
                .map(|m| m.platform == "all" || m.platform == platform)
                .unwrap_or(true);

            if matches_platform {
                std::fs::write(&dest, content)?;
            } else if dest.exists() {
                // Clean up wrong-platform playbooks from previous versions.
                let _ = std::fs::remove_file(&dest);
            }
        }

        // Bootstrap folder-based playbooks.
        for (rel_path, content) in &folders {
            let dest = playbooks_dir.join(rel_path);
            // Create parent directories.
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let is_user_owned = dest.exists() && std::fs::read_to_string(&dest)
                .ok()
                .and_then(|c| parse_frontmatter(&c))
                .map(|m| m.playbook_type == "user")
                .unwrap_or(false);
            if is_user_owned { continue; }

            let matches_platform = parse_frontmatter(content)
                .map(|m| m.platform == "all" || m.platform == platform)
                .unwrap_or(true);

            if matches_platform {
                std::fs::write(&dest, content)?;
            } else if dest.exists() {
                let _ = std::fs::remove_file(&dest);
            }
        }

        // Scan directory for all .md files and parse frontmatter.
        // Only include playbooks matching current platform or "all".
        // Scans both top-level files and playbook.md inside subdirectories.
        let mut metas = Vec::new();
        let entries = std::fs::read_dir(&playbooks_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(meta) = parse_frontmatter(&content) {
                        if meta.platform == "all" || meta.platform == platform {
                            metas.push(meta);
                        }
                    }
                }
            } else if path.is_dir() {
                // Check for playbook.md inside subdirectory (folder-based playbook).
                let main_file = path.join("playbook.md");
                if main_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&main_file) {
                        if let Some(meta) = parse_frontmatter(&content) {
                            if meta.platform == "all" || meta.platform == platform {
                                metas.push(meta);
                            }
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

    /// Read a playbook by name or path.
    ///
    /// Supports:
    /// - Simple names: `"network-diagnostics"` → looks up flat files
    /// - Folder playbooks: `"setup-nanoclaw"` → reads `setup-nanoclaw/playbook.md`
    /// - Sub-modules: `"setup-nanoclaw/add-telegram"` → reads `setup-nanoclaw/add-telegram.md`
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

        // First: check if name contains "/" — it's a path-based lookup.
        if name.contains('/') {
            // Try direct path: playbooks/<name>.md
            let direct = self.playbooks_dir.join(format!("{}.md", name));
            if direct.exists() {
                if let Ok(content) = std::fs::read_to_string(&direct) {
                    return Ok(content);
                }
            }
            // Try as folder: playbooks/<name>/playbook.md
            let folder = self.playbooks_dir.join(name).join("playbook.md");
            if folder.exists() {
                if let Ok(content) = std::fs::read_to_string(&folder) {
                    return Ok(content);
                }
            }
            return Err(anyhow::anyhow!(
                "Playbook module '{}' not found. Check the available modules in the parent playbook.",
                name
            ));
        }

        // Check if it's a folder playbook: playbooks/<name>/playbook.md
        let folder_main = self.playbooks_dir.join(name).join("playbook.md");
        if folder_main.exists() {
            if let Ok(content) = std::fs::read_to_string(&folder_main) {
                let meta = parse_frontmatter(&content);
                let playbook_type = meta.as_ref()
                    .map(|m| m.playbook_type.clone())
                    .unwrap_or_else(|| "user".to_string());
                let date = meta.and_then(|m| m.last_reviewed);
                matches.push(Match { content, playbook_type, date });
            }
        }

        // Scan flat files for name match.
        let entries = std::fs::read_dir(&self.playbooks_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !path.extension().is_some_and(|ext| ext == "md") {
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
        "Load a playbook by name. Returns the full step-by-step protocol. Use 'folder/module' to load a sub-module (e.g. 'setup-openclaw/add-telegram')."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The playbook name (e.g. 'network-diagnostics') or path for sub-modules (e.g. 'setup-openclaw/add-telegram')"
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
    use std::path::PathBuf;

    /// Path to the source playbooks/ directory (available during cargo test).
    fn bundled_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("playbooks")
    }

    /// Convenience: scan bundled flat playbooks for test assertions.
    fn bundled_flat() -> Vec<(String, String)> {
        scan_flat_playbooks(&bundled_dir())
    }

    /// Convenience: scan bundled folder playbooks for test assertions.
    fn bundled_folders() -> Vec<(String, String)> {
        scan_folder_playbooks(&bundled_dir())
    }

    // ── Frontmatter parsing ────────────────────────────────────────────

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test-playbook\ndescription: A test playbook\n---\n\n# Body";
        let meta = parse_frontmatter(content).unwrap();
        assert_eq!(meta.name, "test-playbook");
        assert_eq!(meta.description, "A test playbook");
        assert_eq!(meta.platform, "all"); // default
        assert_eq!(meta.playbook_type, "system"); // default when type: absent
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
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();

        // Only matching-platform files should exist on disk.
        for (filename, content) in &bundled_flat() {
            let meta = parse_frontmatter(content).unwrap();
            let should_exist = meta.platform == "all" || meta.platform == "macos";
            let exists = tmp.path().join("playbooks").join(filename).exists();
            assert_eq!(
                exists, should_exist,
                "File {} exists={} but should_exist={} (platform={})",
                filename, exists, should_exist, meta.platform
            );
        }

        // Only macos + all playbooks loaded into metas.
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

        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();

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

        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();

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

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();

        assert!(registry.metas.iter().any(|m| m.name == "custom-test"));
    }

    #[test]
    fn test_only_matching_platform_files_written() {
        let tmp = tempfile::tempdir().unwrap();
        let _registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"windows").unwrap();

        for (filename, content) in &bundled_flat() {
            let meta = parse_frontmatter(content).unwrap();
            let should_exist = meta.platform == "all" || meta.platform == "windows";
            let exists = tmp.path().join("playbooks").join(filename).exists();
            assert_eq!(
                exists, should_exist,
                "File {} exists={} but should_exist={} (platform={})",
                filename, exists, should_exist, meta.platform
            );
        }
    }

    // ── Platform filtering ─────────────────────────────────────────────

    #[test]
    fn test_platform_filtering_macos() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();

        // Should include macos-specific and cross-platform playbooks.
        assert!(registry.metas.iter().any(|m| m.name == "network-diagnostics")); // macos
        assert!(registry.metas.iter().any(|m| m.name == "outlook-troubleshooting")); // all

        // Should NOT include windows-only playbooks.
        assert!(!registry.metas.iter().any(|m| m.platform == "windows"));
    }

    #[test]
    fn test_platform_filtering_windows() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"windows").unwrap();

        // Should include cross-platform playbook.
        assert!(registry.metas.iter().any(|m| m.name == "outlook-troubleshooting"));

        // Should include windows-specific playbook.
        assert!(registry.metas.iter().any(|m| m.name == "windows-update-troubleshooting"));

        // Should NOT include macos-only playbooks.
        assert!(!registry.metas.iter().any(|m| m.platform == "macos"));
    }

    #[test]
    fn test_platform_filtering_linux() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"linux").unwrap();

        // Should include linux-specific playbook.
        assert!(registry.metas.iter().any(|m| m.name == "setup-cuda"));

        // Should include cross-platform playbooks.
        assert!(registry.metas.iter().any(|m| m.name == "outlook-troubleshooting"));

        // Should NOT include macos-only or windows-only playbooks.
        assert!(!registry.metas.iter().any(|m| m.platform == "macos"));
        assert!(!registry.metas.iter().any(|m| m.platform == "windows"));
    }

    #[test]
    fn test_custom_windows_playbook_filtered_on_macos() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir).unwrap();

        let win_playbook = "---\nname: win-only\ndescription: Windows test\nplatform: windows\n---\n\n# Win";
        std::fs::write(playbooks_dir.join("win-only.md"), win_playbook).unwrap();

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
        assert!(!registry.metas.iter().any(|m| m.name == "win-only"));

        // But the file is on disk (written by user), and a Windows init would pick it up.
        let win_registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"windows").unwrap();
        assert!(win_registry.metas.iter().any(|m| m.name == "win-only"));
    }

    // ── Read / activate ────────────────────────────────────────────────

    #[test]
    fn test_read_playbook_found() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
        let content = registry.read_playbook("network-diagnostics").unwrap();
        assert!(content.contains("Network Diagnostics"));
    }

    #[test]
    fn test_read_playbook_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
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

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
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

        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"all").unwrap();
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

        for (filename, content) in &bundled_flat() {
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
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        let result = tool.execute(&json!({"name": "network-diagnostics"})).await.unwrap();
        assert!(result.output.contains("Network Diagnostics"));
        assert!(result.changes.is_empty()); // read-only
    }

    #[tokio::test]
    async fn test_activate_playbook_tool_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        let err = tool.execute(&json!({"name": "nonexistent"})).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_activate_playbook_tool_missing_param() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        let err = tool.execute(&json!({})).await.unwrap_err();
        assert!(err.to_string().contains("Missing required parameter"));
    }

    #[test]
    fn test_activate_playbook_tool_is_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = PlaybookRegistry::init_for_platform(tmp.path(), &bundled_dir(),"macos").unwrap();
        let tool = ActivatePlaybookTool::new(registry);

        assert_eq!(tool.safety_tier(), SafetyTier::ReadOnly);
        assert_eq!(tool.name(), "activate_playbook");
    }

    // ── Content validation ─────────────────────────────────────────────

    #[test]
    fn test_builtin_playbooks_have_substantial_content() {
        // Each built-in playbook should have real diagnostic content,
        // not just a stub. Check for minimum line count.
        for (filename, content) in &bundled_flat() {
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
        for (filename, content) in &bundled_flat() {
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

    // NOTE: test_macos_playbooks_reference_existing_tools lives in noah-desktop
    // (it needs platform::macos which is desktop-only).

    /// Verify that Windows playbooks don't reference `mac_*` tools.
    #[test]
    fn test_windows_playbooks_dont_reference_macos_tools() {
        for (filename, content) in &bundled_flat() {
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
        for (filename, content) in &bundled_flat() {
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
        for (filename, content) in &bundled_flat() {
            let meta = parse_frontmatter(content).unwrap();
            if meta.platform != "all" { continue; }
            // Procedural playbooks may mention platform tools in "Tools referenced" docs.
            if is_procedural(content) { continue; }

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

    /// Returns true if this playbook is procedural (has `## Step N:` headers).
    fn is_procedural(content: &str) -> bool {
        !parse_steps(content).is_empty()
    }

    /// Every diagnostic playbook must have an Escalation section.
    #[test]
    fn test_builtin_playbooks_have_escalation_section() {
        for (filename, content) in &bundled_flat() {
            if is_procedural(content) { continue; }
            assert!(
                content.contains("## Escalation"),
                "Playbook {} is missing '## Escalation' section. Every playbook needs a bail-out path.",
                filename
            );
        }
    }

    /// Every diagnostic playbook must have a Caveats section.
    #[test]
    fn test_builtin_playbooks_have_caveats_section() {
        for (filename, content) in &bundled_flat() {
            if is_procedural(content) { continue; }
            assert!(
                content.contains("## Caveats"),
                "Playbook {} is missing '## Caveats' section. Document when the standard path doesn't apply.",
                filename
            );
        }
    }

    /// Every diagnostic playbook must have a Key signals section.
    #[test]
    fn test_builtin_playbooks_have_key_signals_section() {
        for (filename, content) in &bundled_flat() {
            if is_procedural(content) { continue; }
            assert!(
                content.contains("## Key signals"),
                "Playbook {} is missing '## Key signals' section.",
                filename
            );
        }
    }

    /// Every diagnostic playbook should claim a success rate.
    #[test]
    fn test_builtin_playbooks_claim_success_rate() {
        for (filename, content) in &bundled_flat() {
            if is_procedural(content) { continue; }
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
        for (filename, content) in &bundled_flat() {
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
        for (filename, content) in &bundled_flat() {
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

    // ── Step parsing ─────────────────────────────────────────────────

    #[test]
    fn test_parse_steps_standard_format() {
        let content = r#"---
name: test
description: test
---
# Setup

## Step 1: Check Environment
Do stuff.

## Step 2: Install Dependencies
More stuff.

## Step 3: Configure
Final stuff.
"#;
        let steps = parse_steps(content);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].number, 1);
        assert_eq!(steps[0].label, "Check Environment");
        assert_eq!(steps[2].number, 3);
        assert_eq!(steps[2].label, "Configure");
    }

    #[test]
    fn test_parse_steps_numbered_format() {
        let content = "## 1. Check\n## 2. Install\n## 3. Done\n";
        let steps = parse_steps(content);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].label, "Check");
        assert_eq!(steps[1].label, "Install");
    }

    #[test]
    fn test_parse_steps_dash_separator() {
        let content = "## Step 1 — Check Environment\n## Step 2 — Build\n";
        let steps = parse_steps(content);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].label, "Check Environment");
    }

    #[test]
    fn test_parse_steps_diagnostic_playbook_no_steps() {
        // Diagnostic playbooks use "### 1." not "## Step 1:"
        let content = r#"## When to activate
User reports...
### 1. Check Wi-Fi
### 2. Check gateway
"#;
        let steps = parse_steps(content);
        assert!(steps.is_empty(), "Diagnostic playbooks should have no steps (### not ##)");
    }

    #[test]
    fn test_playbook_state_progress() {
        let content = "## Step 1: Check\nstuff\n## Step 2: Build\nstuff\n## Step 3: Done\n";
        let mut state = PlaybookState::from_content("test", content);
        assert_eq!(state.total_steps, 3);

        let p = state.progress_json().unwrap();
        assert_eq!(p["step"], 1);
        assert_eq!(p["total"], 3);
        assert_eq!(p["label"], "Check");

        state.advance();
        let p = state.progress_json().unwrap();
        assert_eq!(p["step"], 2);
        assert_eq!(p["label"], "Build");

        state.advance();
        let p = state.progress_json().unwrap();
        assert_eq!(p["step"], 3);
        assert_eq!(p["label"], "Done");

        // Past the end — stays at last step
        state.advance();
        let p = state.progress_json().unwrap();
        assert_eq!(p["step"], 3);
    }

    #[test]
    fn test_playbook_state_no_steps() {
        let content = "# Network Diagnostics\n## When to activate\n## Quick check\n";
        let state = PlaybookState::from_content("net", content);
        assert!(state.progress_json().is_none());
    }

    /// last_reviewed must be a valid YYYY-MM-DD date and not older than 6 months.
    #[test]
    fn test_builtin_playbooks_not_stale() {
        for (filename, content) in &bundled_flat() {
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
