use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use noah_tools::{SafetyTier, Tool, ToolResult};

// ── Constants ───────────────────────────────────────────────────────────

const DEFAULT_CATEGORIES: &[&str] = &[
    "devices",
    "issues",
    "network",
    "playbooks",
    "preferences",
    "software",
];

// ── Init ────────────────────────────────────────────────────────────────

/// Create the `knowledge/` directory tree inside the app data dir.
pub fn init_knowledge_dir(app_dir: &Path) -> Result<PathBuf> {
    let knowledge_dir = app_dir.join("knowledge");
    for cat in DEFAULT_CATEGORIES {
        std::fs::create_dir_all(knowledge_dir.join(cat))
            .with_context(|| format!("Failed to create knowledge/{}", cat))?;
    }
    Ok(knowledge_dir)
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Turn a title/filename into a URL-safe slug.
pub fn slugify(input: &str) -> String {
    let lower = input.to_lowercase();
    let slug: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse runs of dashes, trim leading/trailing dashes.
    let mut result = String::new();
    let mut prev_dash = true; // treat start as dash to trim leading
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(ch);
            prev_dash = false;
        }
    }
    // Trim trailing dash
    if result.ends_with('-') {
        result.pop();
    }
    if result.is_empty() {
        "untitled".to_string()
    } else {
        result
    }
}

/// Resolve a relative path inside `knowledge_dir`, rejecting traversal.
pub fn safe_resolve(knowledge_dir: &Path, relative: &str) -> Result<PathBuf> {
    let joined = knowledge_dir.join(relative);
    let canonical_base = knowledge_dir
        .canonicalize()
        .with_context(|| format!("Knowledge dir not found: {}", knowledge_dir.display()))?;

    // The file might not exist yet (for writes), so canonicalize the parent.
    let parent = joined.parent().unwrap_or(&joined);
    // Ensure parent exists for canonicalization.
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create parent: {}", parent.display()))?;

    let canonical_parent = parent
        .canonicalize()
        .with_context(|| format!("Cannot resolve: {}", parent.display()))?;

    if !canonical_parent.starts_with(&canonical_base) {
        anyhow::bail!("Path traversal rejected: {}", relative);
    }

    // Reconstruct the final path using the canonical parent + filename.
    let filename = joined.file_name().context("Missing filename")?;
    Ok(canonical_parent.join(filename))
}

// ── Knowledge entry ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub category: String,
    pub filename: String,
    pub path: String,
    pub title: String,
    pub playbook_type: Option<String>,
    /// Description from frontmatter (playbooks only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Emoji icon from frontmatter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
}

/// Parsed frontmatter fields from a knowledge/playbook markdown file.
struct Frontmatter {
    description: Option<String>,
    playbook_type: Option<String>,
    emoji: Option<String>,
}

fn extract_frontmatter(content: &str) -> Frontmatter {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Frontmatter { description: None, playbook_type: None, emoji: None };
    }

    let after_first = &trimmed[3..];
    let Some(end) = after_first.find("\n---") else {
        return Frontmatter { description: None, playbook_type: None, emoji: None };
    };
    let yaml_block = &after_first[..end];

    let mut description = None;
    let mut playbook_type = None;
    let mut emoji = None;

    for line in yaml_block.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("type:") {
            let kind = value.trim();
            if !kind.is_empty() {
                playbook_type = Some(kind.to_string());
            }
        } else if let Some(value) = line.strip_prefix("description:") {
            let desc = value.trim();
            if !desc.is_empty() {
                description = Some(desc.to_string());
            }
        } else if let Some(value) = line.strip_prefix("emoji:") {
            let e = value.trim();
            if !e.is_empty() {
                emoji = Some(e.to_string());
            }
        }
    }

    Frontmatter { description, playbook_type, emoji }
}

/// Extract the title from the first `# ` heading line, or derive from filename.
fn extract_title(content: &str, filename: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let title = heading.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    // Derive from filename: replace dashes with spaces, title-case first letter.
    let derived = filename.trim_end_matches(".md").replace('-', " ");
    let mut chars = derived.chars();
    match chars.next() {
        None => "Untitled".to_string(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// List all knowledge files, optionally filtered by category.
/// Recurses one level into subdirectories (for folder playbooks like setup-openclaw/).
pub fn list_knowledge_tree(
    knowledge_dir: &Path,
    category: Option<&str>,
) -> Result<Vec<KnowledgeEntry>> {
    let mut entries = Vec::new();

    let dirs_to_scan: Vec<PathBuf> = if let Some(cat) = category {
        let cat_dir = knowledge_dir.join(cat);
        if cat_dir.is_dir() {
            vec![cat_dir]
        } else {
            return Ok(entries);
        }
    } else {
        // Scan all subdirectories.
        let mut dirs = Vec::new();
        if let Ok(read_dir) = std::fs::read_dir(knowledge_dir) {
            for entry in read_dir.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    dirs.push(entry.path());
                }
            }
        }
        dirs.sort();
        dirs
    };

    for dir in dirs_to_scan {
        let cat_name = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if let Ok(read_dir) = std::fs::read_dir(&dir) {
            let mut dir_entries: Vec<_> = read_dir.flatten().collect();
            dir_entries.sort_by_key(|e| e.file_name());

            for file_entry in dir_entries {
                let fname = file_entry.file_name().to_string_lossy().to_string();

                if file_entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    // Recurse one level into subdirectories (folder playbooks).
                    scan_subdir(&mut entries, &cat_name, &file_entry.path(), &fname);
                    continue;
                }

                if !fname.ends_with(".md") {
                    continue;
                }
                let rel_path = format!("{}/{}", cat_name, fname);
                let content = std::fs::read_to_string(file_entry.path()).unwrap_or_default();
                let title = extract_title(&content, &fname);
                let fm = extract_frontmatter(&content);
                let playbook_type = if cat_name == "playbooks" {
                    fm.playbook_type.or_else(|| Some("system".to_string()))
                } else {
                    None
                };
                entries.push(KnowledgeEntry {
                    category: cat_name.clone(),
                    filename: fname,
                    path: rel_path,
                    title,
                    playbook_type,
                    description: fm.description,
                    emoji: fm.emoji,
                });
            }
        }
    }

    Ok(entries)
}

/// Scan a subdirectory within a category (e.g. playbooks/setup-openclaw/).
fn scan_subdir(entries: &mut Vec<KnowledgeEntry>, cat_name: &str, dir: &Path, folder_name: &str) {
    let Ok(read_dir) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<_> = read_dir.flatten().collect();
    files.sort_by_key(|e| e.file_name());

    for file_entry in files {
        let fname = file_entry.file_name().to_string_lossy().to_string();
        if !fname.ends_with(".md") {
            continue;
        }
        let rel_path = format!("{}/{}/{}", cat_name, folder_name, fname);
        let content = std::fs::read_to_string(file_entry.path()).unwrap_or_default();
        let title = extract_title(&content, &fname);
        let fm = extract_frontmatter(&content);
        let playbook_type = if cat_name == "playbooks" {
            fm.playbook_type.or_else(|| Some("system".to_string()))
        } else {
            None
        };
        entries.push(KnowledgeEntry {
            category: cat_name.to_string(),
            filename: fname,
            path: rel_path,
            title,
            playbook_type,
            description: fm.description,
            emoji: fm.emoji,
        });
    }
}

/// Build a compact table-of-contents string for the system prompt.
///
/// Playbook entries show "slug — description" format.
/// Non-playbook categories show compact slug lists.
/// Folder playbooks (e.g. setup-openclaw/) are shown as a single entry using their
/// main playbook.md description, not expanded into individual sub-modules.
pub fn knowledge_toc(knowledge_dir: &Path) -> Result<String> {
    let entries = list_knowledge_tree(knowledge_dir, None)?;
    if entries.is_empty() {
        return Ok(String::new());
    }

    // Group entries by category.
    let mut cats: std::collections::BTreeMap<String, Vec<&KnowledgeEntry>> =
        std::collections::BTreeMap::new();
    for entry in &entries {
        cats.entry(entry.category.clone()).or_default().push(entry);
    }

    let mut lines = vec![
        "## Knowledge Base".to_string(),
        "Use knowledge_search to find files, knowledge_read to read them.".to_string(),
    ];

    for (cat, cat_entries) in &cats {
        if cat == "playbooks" {
            // Show playbooks with descriptions, deduplicating folder playbooks.
            lines.push(String::new());
            lines.push("playbooks:".to_string());
            let mut seen_folders: std::collections::HashSet<String> = std::collections::HashSet::new();
            for entry in cat_entries {
                // Check if this is a folder playbook sub-module (path has 3 segments).
                let segments: Vec<&str> = entry.path.split('/').collect();
                if segments.len() == 3 {
                    // Folder playbook: show once using the folder name.
                    let folder = segments[1].to_string();
                    if !seen_folders.insert(folder.clone()) {
                        continue; // Already shown this folder.
                    }
                    // Find the playbook.md entry for this folder to get its description.
                    let desc = cat_entries.iter()
                        .find(|e| e.path == format!("playbooks/{}/playbook.md", folder))
                        .and_then(|e| e.description.as_deref());
                    if let Some(d) = desc {
                        lines.push(format!("- {} — {}", folder, d));
                    } else {
                        lines.push(format!("- {}", folder));
                    }
                } else {
                    // Flat playbook.
                    let slug = entry.filename.trim_end_matches(".md");
                    if let Some(d) = &entry.description {
                        lines.push(format!("- {} — {}", slug, d));
                    } else {
                        lines.push(format!("- {}", slug));
                    }
                }
            }
        } else {
            // Non-playbook: compact slug list.
            let slugs: Vec<String> = cat_entries
                .iter()
                .map(|e| e.filename.trim_end_matches(".md").to_string())
                .collect();
            lines.push(format!("{}: {}", cat, slugs.join(", ")));
        }
    }

    Ok(lines.join("\n"))
}

// ── LLM Tools ───────────────────────────────────────────────────────────

// -- WriteKnowledge --

pub struct WriteKnowledgeTool {
    knowledge_dir: PathBuf,
}

impl WriteKnowledgeTool {
    pub fn new(knowledge_dir: PathBuf) -> Self {
        Self { knowledge_dir }
    }
}

#[async_trait]
impl Tool for WriteKnowledgeTool {
    fn name(&self) -> &str {
        "write_knowledge"
    }

    fn description(&self) -> &str {
        "Create or update a markdown knowledge file. Use to remember device details, resolved issues, user preferences, or system configuration. The file is stored in a category folder."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "Folder name: devices, issues, network, playbooks, preferences, software, or a new category name."
                },
                "filename": {
                    "type": "string",
                    "description": "Slug for the file (without .md). E.g. 'hp-laserjet-pro-m404n'."
                },
                "content": {
                    "type": "string",
                    "description": "Full markdown content. Start with '# Title'."
                }
            },
            "required": ["category", "filename", "content"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .context("Missing 'category'")?;
        let filename = input
            .get("filename")
            .and_then(|v| v.as_str())
            .context("Missing 'filename'")?;
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .context("Missing 'content'")?;

        let slug = slugify(filename);
        let rel_path = format!("{}/{}.md", slugify(category), slug);
        let full_path = safe_resolve(&self.knowledge_dir, &rel_path)?;

        std::fs::write(&full_path, content)
            .with_context(|| format!("Failed to write {}", rel_path))?;

        Ok(ToolResult::read_only(
            format!("Saved knowledge file: {}", rel_path),
            json!({ "path": rel_path }),
        ))
    }
}

// -- KnowledgeSearch --

pub struct KnowledgeSearchTool {
    knowledge_dir: PathBuf,
}

impl KnowledgeSearchTool {
    pub fn new(knowledge_dir: PathBuf) -> Self {
        Self { knowledge_dir }
    }
}

/// Collect all .md files under a directory recursively (max 2 levels deep).
/// Returns (relative_path, absolute_path) pairs.
fn collect_files_under(base: &Path, rel_prefix: &str) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(base) else { return results };
    let mut dir_entries: Vec<_> = read_dir.flatten().collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in dir_entries {
        let fname = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();

        if path.is_dir() {
            // Recurse one level.
            let sub_prefix = if rel_prefix.is_empty() {
                fname.clone()
            } else {
                format!("{}/{}", rel_prefix, fname)
            };
            let Ok(sub_dir) = std::fs::read_dir(&path) else { continue };
            let mut sub_entries: Vec<_> = sub_dir.flatten().collect();
            sub_entries.sort_by_key(|e| e.file_name());
            for sub in sub_entries {
                let sub_name = sub.file_name().to_string_lossy().to_string();
                if sub_name.ends_with(".md") {
                    let rel = format!("{}/{}", sub_prefix, sub_name);
                    results.push((rel, sub.path()));
                }
            }
        } else if fname.ends_with(".md") {
            let rel = if rel_prefix.is_empty() {
                fname
            } else {
                format!("{}/{}", rel_prefix, fname)
            };
            results.push((rel, path));
        }
    }
    results
}

#[async_trait]
impl Tool for KnowledgeSearchTool {
    fn name(&self) -> &str {
        "knowledge_search"
    }

    fn description(&self) -> &str {
        "Search knowledge files by pattern. Use '*' to list all files. Matches file names and content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern. Matches against file names and content. Use '*' to list all files."
                },
                "path": {
                    "type": "string",
                    "description": "Scope search to a subdirectory (e.g. 'playbooks/setup-openclaw'). Defaults to all knowledge."
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["files", "content"],
                    "description": "Output mode: 'files' lists matching file paths with titles (default), 'content' shows matching lines with context."
                },
                "context": {
                    "type": "number",
                    "description": "Lines of context around each match (only for output_mode: 'content'). Default: 1."
                }
            },
            "required": ["pattern"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .context("Missing 'pattern'")?;
        let scoped_path = input.get("path").and_then(|v| v.as_str());
        let output_mode = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files");
        let context_lines = input
            .get("context")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let is_wildcard = pattern == "*";
        let pattern_lower = pattern.to_lowercase();

        // Determine search root and relative prefix.
        let (search_root, rel_prefix) = if let Some(p) = scoped_path {
            let resolved = safe_resolve(&self.knowledge_dir, p)?;
            if !resolved.is_dir() {
                return Ok(ToolResult::read_only(
                    format!("Directory not found: {}", p),
                    json!({ "results": [] }),
                ));
            }
            (resolved, p.to_string())
        } else {
            (self.knowledge_dir.clone(), String::new())
        };

        let files = collect_files_under(&search_root, &rel_prefix);

        match output_mode {
            "content" => self.execute_content_mode(&files, &pattern_lower, is_wildcard, context_lines),
            _ => self.execute_files_mode(&files, &pattern_lower, is_wildcard),
        }
    }
}

impl KnowledgeSearchTool {
    fn execute_files_mode(
        &self,
        files: &[(String, PathBuf)],
        pattern_lower: &str,
        is_wildcard: bool,
    ) -> Result<ToolResult> {
        let mut results: Vec<Value> = Vec::new();

        for (rel_path, abs_path) in files {
            let content = std::fs::read_to_string(abs_path).unwrap_or_default();
            let title = extract_title(&content, rel_path.rsplit('/').next().unwrap_or(rel_path));

            let matches = is_wildcard
                || rel_path.to_lowercase().contains(pattern_lower)
                || content.to_lowercase().contains(pattern_lower);

            if matches {
                results.push(json!({
                    "path": rel_path,
                    "title": title,
                }));
            }
        }

        if results.is_empty() {
            return Ok(ToolResult::read_only(
                if is_wildcard {
                    "No knowledge files found.".to_string()
                } else {
                    format!("No knowledge files match '{}'.", pattern_lower)
                },
                json!({ "results": [] }),
            ));
        }

        // Group by category (first path segment) for display.
        let mut lines = Vec::new();
        let mut current_cat = String::new();
        for r in &results {
            let path = r["path"].as_str().unwrap_or("");
            let title = r["title"].as_str().unwrap_or("");
            let cat = path.split('/').next().unwrap_or("");
            if cat != current_cat {
                current_cat = cat.to_string();
                lines.push(format!("\n### {}", current_cat));
            }
            lines.push(format!("- {} (`{}`)", title, path));
        }

        Ok(ToolResult::read_only(
            format!("{} file(s):{}", results.len(), lines.join("\n")),
            json!({ "results": results }),
        ))
    }

    fn execute_content_mode(
        &self,
        files: &[(String, PathBuf)],
        pattern_lower: &str,
        is_wildcard: bool,
        context_lines: usize,
    ) -> Result<ToolResult> {
        let mut results: Vec<Value> = Vec::new();

        for (rel_path, abs_path) in files {
            let content = match std::fs::read_to_string(abs_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // For wildcard in content mode, skip — it doesn't make sense to show all content.
            if is_wildcard {
                continue;
            }

            let lines: Vec<&str> = content.lines().collect();
            let mut snippets: Vec<String> = Vec::new();
            let mut last_end: usize = 0; // Track to avoid overlapping snippets.

            for (i, line) in lines.iter().enumerate() {
                if i < last_end {
                    continue; // Skip lines already included in a previous snippet.
                }
                if line.to_lowercase().contains(pattern_lower) {
                    let start = i.saturating_sub(context_lines);
                    let end = (i + context_lines + 1).min(lines.len());
                    let snippet: String = lines[start..end].join("\n");
                    snippets.push(snippet);
                    last_end = end;
                    if snippets.len() >= 3 {
                        break;
                    }
                }
            }

            if !snippets.is_empty() {
                results.push(json!({
                    "path": rel_path,
                    "title": extract_title(&content, rel_path.rsplit('/').next().unwrap_or(rel_path)),
                    "snippets": snippets,
                }));
            }

            if results.len() >= 15 {
                break;
            }
        }

        if results.is_empty() {
            return Ok(ToolResult::read_only(
                format!("No knowledge files match '{}'.", pattern_lower),
                json!({ "results": [] }),
            ));
        }

        let mut output = vec![format!("Found {} matching file(s):", results.len())];
        for r in &results {
            let path = r["path"].as_str().unwrap_or("");
            let title = r["title"].as_str().unwrap_or("");
            output.push(format!("\n### {} (`{}`)", title, path));
            if let Some(snippets) = r["snippets"].as_array() {
                for s in snippets {
                    output.push(format!("  {}", s.as_str().unwrap_or("")));
                }
            }
        }

        Ok(ToolResult::read_only(
            output.join("\n"),
            json!({ "results": results }),
        ))
    }
}

// -- KnowledgeRead --

pub struct KnowledgeReadTool {
    knowledge_dir: PathBuf,
}

impl KnowledgeReadTool {
    pub fn new(knowledge_dir: PathBuf) -> Self {
        Self { knowledge_dir }
    }
}

#[async_trait]
impl Tool for KnowledgeReadTool {
    fn name(&self) -> &str {
        "knowledge_read"
    }

    fn description(&self) -> &str {
        "Read the full content of a knowledge file by its relative path."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path, e.g. 'devices/hp-laserjet-pro-m404n.md' or 'playbooks/setup-openclaw/configure.md'."
                },
                "offset": {
                    "type": "number",
                    "description": "Line number to start reading from (0-based). Default: 0."
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of lines to return. Default: all."
                }
            },
            "required": ["path"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path'")?;
        let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = input.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

        let full_path = safe_resolve(&self.knowledge_dir, path)?;

        let content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("File not found: {}", path))?;

        // Apply offset/limit if specified.
        let output = if offset > 0 || limit.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let start = offset.min(lines.len());
            let end = if let Some(lim) = limit {
                (start + lim).min(lines.len())
            } else {
                lines.len()
            };
            lines[start..end].join("\n")
        } else {
            content.clone()
        };

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "path": path, "content": output }),
        ))
    }
}

// ── Migration ───────────────────────────────────────────────────────────

/// Migrate existing artifacts from SQLite to markdown files.
/// Called by journal migration 4.
pub fn migrate_artifacts_to_files(conn: &rusqlite::Connection, knowledge_dir: &Path) -> Result<()> {
    // Check if the artifacts table exists.
    let table_exists: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='artifacts'")
        .and_then(|mut stmt| stmt.exists([]))
        .unwrap_or(false);

    if !table_exists {
        return Ok(());
    }

    let mut stmt =
        conn.prepare("SELECT category, title, content FROM artifacts ORDER BY updated_at ASC")?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        let (old_category, title, content) = row?;

        // Map old category to new folder.
        let new_folder = match old_category.as_str() {
            "device_fact" => "devices",
            "resolved_issue" => "issues",
            "config_note" => "software",
            "recurring_pattern" => "issues",
            "preference" => "preferences",
            "general" => "software",
            other => other, // Pass-through for any unknown
        };

        let slug = slugify(&title);
        let rel_path = format!("{}/{}.md", new_folder, slug);

        // Ensure the category dir exists.
        let cat_dir = knowledge_dir.join(new_folder);
        std::fs::create_dir_all(&cat_dir)?;

        let full_path = knowledge_dir.join(&rel_path);

        // Build markdown content.
        let md = format!("# {}\n\n{}", title, content);
        std::fs::write(&full_path, md)
            .with_context(|| format!("Failed to write migrated file: {}", rel_path))?;
    }

    Ok(())
}

// ── Delete ──────────────────────────────────────────────────────────────

/// Delete a knowledge file by relative path.
pub fn delete_knowledge_file(knowledge_dir: &Path, relative: &str) -> Result<()> {
    let full_path = safe_resolve(knowledge_dir, relative)?;
    if !full_path.exists() {
        anyhow::bail!("File not found: {}", relative);
    }
    std::fs::remove_file(&full_path).with_context(|| format!("Failed to delete: {}", relative))?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let kdir = init_knowledge_dir(tmp.path()).unwrap();
        (tmp, kdir)
    }

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("HP LaserJet Pro M404n"), "hp-laserjet-pro-m404n");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(
            slugify("Slow WiFi fixed (DNS change)"),
            "slow-wifi-fixed-dns-change"
        );
    }

    #[test]
    fn test_slugify_leading_trailing() {
        assert_eq!(slugify("---hello---"), "hello");
    }

    #[test]
    fn test_slugify_empty() {
        assert_eq!(slugify(""), "untitled");
        assert_eq!(slugify("---"), "untitled");
    }

    #[test]
    fn test_safe_resolve_valid() {
        let (_tmp, kdir) = setup();
        let path = safe_resolve(&kdir, "devices/test.md").unwrap();
        // Canonicalize kdir too (on macOS /var -> /private/var).
        let canonical_kdir = kdir.canonicalize().unwrap();
        assert!(path.starts_with(&canonical_kdir));
    }

    #[test]
    fn test_safe_resolve_traversal() {
        let (_tmp, kdir) = setup();
        let result = safe_resolve(&kdir, "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_init_creates_dirs() {
        let (_tmp, kdir) = setup();
        for cat in DEFAULT_CATEGORIES {
            assert!(kdir.join(cat).is_dir(), "Missing category dir: {}", cat);
        }
    }

    #[test]
    fn test_list_knowledge_tree_empty() {
        let (_tmp, kdir) = setup();
        let entries = list_knowledge_tree(&kdir, None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_knowledge_tree_with_files() {
        let (_tmp, kdir) = setup();
        std::fs::write(kdir.join("devices/printer.md"), "# HP LaserJet\n\nDetails").unwrap();
        std::fs::write(
            kdir.join("issues/slow-wifi.md"),
            "# Slow WiFi Fixed\n\nChanged DNS",
        )
        .unwrap();

        let entries = list_knowledge_tree(&kdir, None).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].category, "devices");
        assert_eq!(entries[0].title, "HP LaserJet");
        assert_eq!(entries[1].category, "issues");
    }

    #[test]
    fn test_list_knowledge_tree_filter_category() {
        let (_tmp, kdir) = setup();
        std::fs::write(kdir.join("devices/printer.md"), "# Printer").unwrap();
        std::fs::write(kdir.join("issues/bug.md"), "# Bug").unwrap();

        let entries = list_knowledge_tree(&kdir, Some("devices")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Printer");
    }

    #[test]
    fn test_list_knowledge_tree_sets_playbook_type() {
        let (_tmp, kdir) = setup();
        let content = "---
name: Network Diagnostics
description: Diagnose network issues
type: system
---
# Network";
        std::fs::write(kdir.join("playbooks/network.md"), content).unwrap();

        let entries = list_knowledge_tree(&kdir, Some("playbooks")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].playbook_type.as_deref(), Some("system"));
        assert_eq!(entries[0].description.as_deref(), Some("Diagnose network issues"));
    }

    #[test]
    fn test_list_knowledge_tree_folder_playbooks() {
        let (_tmp, kdir) = setup();
        let pb_dir = kdir.join("playbooks/setup-openclaw");
        std::fs::create_dir_all(&pb_dir).unwrap();
        std::fs::write(
            pb_dir.join("playbook.md"),
            "---\nname: setup-openclaw\ndescription: Install and configure OpenClaw\ntype: system\n---\n# Set Up OpenClaw",
        ).unwrap();
        std::fs::write(
            pb_dir.join("configure.md"),
            "---\nname: configure\ndescription: Configure OpenClaw\ntype: system\n---\n# Configure OpenClaw",
        ).unwrap();

        let entries = list_knowledge_tree(&kdir, Some("playbooks")).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.path == "playbooks/setup-openclaw/playbook.md"));
        assert!(entries.iter().any(|e| e.path == "playbooks/setup-openclaw/configure.md"));
    }

    #[test]
    fn test_knowledge_toc_empty() {
        let (_tmp, kdir) = setup();
        let toc = knowledge_toc(&kdir).unwrap();
        assert!(toc.is_empty());
    }

    #[test]
    fn test_knowledge_toc_with_files() {
        let (_tmp, kdir) = setup();
        std::fs::write(kdir.join("devices/printer.md"), "# HP LaserJet\n\nDetails").unwrap();

        let toc = knowledge_toc(&kdir).unwrap();
        assert!(toc.contains("Knowledge Base"));
        assert!(toc.contains("devices:"));
        assert!(toc.contains("printer"));
    }

    #[test]
    fn test_knowledge_toc_enriched_playbook_descriptions() {
        let (_tmp, kdir) = setup();
        std::fs::write(
            kdir.join("playbooks/network-diagnostics.md"),
            "---\nname: network-diagnostics\ndescription: Diagnose and fix network issues\ntype: system\n---\n# Network",
        ).unwrap();

        let pb_dir = kdir.join("playbooks/setup-openclaw");
        std::fs::create_dir_all(&pb_dir).unwrap();
        std::fs::write(
            pb_dir.join("playbook.md"),
            "---\nname: setup-openclaw\ndescription: Install and configure OpenClaw\ntype: system\n---\n# OpenClaw",
        ).unwrap();
        std::fs::write(
            pb_dir.join("configure.md"),
            "---\nname: configure\ndescription: Configure OpenClaw\ntype: system\n---\n# Configure",
        ).unwrap();

        let toc = knowledge_toc(&kdir).unwrap();
        assert!(toc.contains("network-diagnostics — Diagnose and fix network issues"));
        assert!(toc.contains("setup-openclaw — Install and configure OpenClaw"));
        // Sub-modules should NOT be listed individually in TOC.
        assert!(!toc.contains("configure.md"));
    }

    #[test]
    fn test_extract_title_from_heading() {
        assert_eq!(
            extract_title("# My Title\n\nContent", "file.md"),
            "My Title"
        );
    }

    #[test]
    fn test_extract_title_from_filename() {
        assert_eq!(
            extract_title("No heading here", "my-cool-file.md"),
            "My cool file"
        );
    }

    #[test]
    fn test_delete_knowledge_file() {
        let (_tmp, kdir) = setup();
        let path = kdir.join("devices/test.md");
        std::fs::write(&path, "content").unwrap();
        assert!(path.exists());

        delete_knowledge_file(&kdir, "devices/test.md").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_delete_knowledge_file_not_found() {
        let (_tmp, kdir) = setup();
        let result = delete_knowledge_file(&kdir, "devices/nonexistent.md");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_knowledge_tool() {
        let (_tmp, kdir) = setup();
        let tool = WriteKnowledgeTool::new(kdir.clone());

        let input = json!({
            "category": "devices",
            "filename": "hp-printer",
            "content": "# HP Printer\n\nModel: M404n"
        });

        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("devices/hp-printer.md"));

        let content = std::fs::read_to_string(kdir.join("devices/hp-printer.md")).unwrap();
        assert!(content.contains("HP Printer"));
    }

    #[tokio::test]
    async fn test_knowledge_search_content_mode() {
        let (_tmp, kdir) = setup();
        std::fs::write(
            kdir.join("devices/printer.md"),
            "# HP LaserJet\n\nModel M404n",
        )
        .unwrap();
        std::fs::write(
            kdir.join("network/wifi.md"),
            "# WiFi Config\n\nDNS: 8.8.8.8",
        )
        .unwrap();

        let tool = KnowledgeSearchTool::new(kdir);
        let input = json!({ "pattern": "DNS", "output_mode": "content" });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("1 matching file"));
        assert!(result.output.contains("WiFi Config"));
    }

    #[tokio::test]
    async fn test_knowledge_search_no_results() {
        let (_tmp, kdir) = setup();
        let tool = KnowledgeSearchTool::new(kdir);
        let input = json!({ "pattern": "nonexistent" });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("No knowledge files"));
    }

    #[tokio::test]
    async fn test_knowledge_search_wildcard_lists_all() {
        let (_tmp, kdir) = setup();
        std::fs::write(kdir.join("devices/printer.md"), "# Printer").unwrap();
        std::fs::write(kdir.join("issues/bug.md"), "# Bug").unwrap();

        let tool = KnowledgeSearchTool::new(kdir);
        let input = json!({ "pattern": "*" });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("2 file(s)"));
        assert!(result.output.contains("Printer"));
        assert!(result.output.contains("Bug"));
    }

    #[tokio::test]
    async fn test_knowledge_search_scoped_path() {
        let (_tmp, kdir) = setup();
        let pb_dir = kdir.join("playbooks/setup-openclaw");
        std::fs::create_dir_all(&pb_dir).unwrap();
        std::fs::write(pb_dir.join("playbook.md"), "# Set Up OpenClaw").unwrap();
        std::fs::write(pb_dir.join("configure.md"), "# Configure OpenClaw").unwrap();
        std::fs::write(kdir.join("devices/printer.md"), "# Printer").unwrap();

        let tool = KnowledgeSearchTool::new(kdir);
        let input = json!({ "pattern": "*", "path": "playbooks/setup-openclaw" });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("2 file(s)"));
        assert!(result.output.contains("Configure OpenClaw"));
        assert!(!result.output.contains("Printer"));
    }

    #[tokio::test]
    async fn test_knowledge_search_content_with_context() {
        let (_tmp, kdir) = setup();
        std::fs::write(
            kdir.join("network/wifi.md"),
            "# WiFi Config\n\nLine 1\nLine 2\nDNS: 8.8.8.8\nLine 4\nLine 5",
        )
        .unwrap();

        let tool = KnowledgeSearchTool::new(kdir);
        let input = json!({ "pattern": "DNS", "output_mode": "content", "context": 2 });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("Line 2"));
        assert!(result.output.contains("DNS: 8.8.8.8"));
        assert!(result.output.contains("Line 4"));
    }

    #[tokio::test]
    async fn test_knowledge_search_matches_filename() {
        let (_tmp, kdir) = setup();
        std::fs::write(kdir.join("devices/printer.md"), "# HP LaserJet").unwrap();

        let tool = KnowledgeSearchTool::new(kdir);
        let input = json!({ "pattern": "printer" });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("1 file(s)"));
        assert!(result.output.contains("HP LaserJet"));
    }

    #[tokio::test]
    async fn test_knowledge_read_tool() {
        let (_tmp, kdir) = setup();
        std::fs::write(
            kdir.join("devices/printer.md"),
            "# HP LaserJet\n\nDetails here",
        )
        .unwrap();

        let tool = KnowledgeReadTool::new(kdir);
        let input = json!({ "path": "devices/printer.md" });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("HP LaserJet"));
    }

    #[tokio::test]
    async fn test_knowledge_read_with_offset_limit() {
        let (_tmp, kdir) = setup();
        std::fs::write(
            kdir.join("devices/printer.md"),
            "line 0\nline 1\nline 2\nline 3\nline 4",
        )
        .unwrap();

        let tool = KnowledgeReadTool::new(kdir);
        let input = json!({ "path": "devices/printer.md", "offset": 1, "limit": 2 });
        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("line 1"));
        assert!(result.output.contains("line 2"));
        assert!(!result.output.contains("line 0"));
        assert!(!result.output.contains("line 3"));
    }

    #[test]
    fn test_migrate_artifacts_to_files() {
        let (_tmp, kdir) = setup();
        let conn = crate::safety::journal::init_db(":memory:").unwrap();

        // Seed some artifacts.
        conn.execute(
            "INSERT INTO artifacts (id, category, title, content, source, created_at, updated_at)
             VALUES ('1', 'device_fact', 'HP Printer Model', 'LaserJet Pro M404n', 'agent', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO artifacts (id, category, title, content, source, created_at, updated_at)
             VALUES ('2', 'resolved_issue', 'Slow WiFi Fixed', 'Changed DNS to 8.8.8.8', 'agent', '2026-01-02', '2026-01-02')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO artifacts (id, category, title, content, source, created_at, updated_at)
             VALUES ('3', 'preference', 'Prefers Chrome', 'User likes Chrome over Safari', 'agent', '2026-01-03', '2026-01-03')",
            [],
        ).unwrap();

        migrate_artifacts_to_files(&conn, &kdir).unwrap();

        // Check files were created.
        assert!(kdir.join("devices/hp-printer-model.md").exists());
        assert!(kdir.join("issues/slow-wifi-fixed.md").exists());
        assert!(kdir.join("preferences/prefers-chrome.md").exists());

        // Check content.
        let content = std::fs::read_to_string(kdir.join("devices/hp-printer-model.md")).unwrap();
        assert!(content.contains("# HP Printer Model"));
        assert!(content.contains("LaserJet Pro M404n"));
    }
}
