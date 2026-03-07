use itman_tools::Tool;
use serde_json::{json, Value};

use crate::agent::llm_client::ToolDef;

pub struct ToolRouter {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRouter {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register a tool with the router.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Look up a tool by name.
    pub fn find_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| &**t)
    }

    /// Return tool definitions in the format expected by the Anthropic API.
    pub fn tool_definitions(&self) -> Vec<ToolDef> {
        self.tools
            .iter()
            .map(|t| ToolDef {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    /// Return tool definitions as raw JSON values (convenience method).
    pub fn tool_definitions_json(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;

    #[test]
    fn test_register_and_find() {
        let mut router = ToolRouter::new();
        platform::register_platform_tools(&mut router, None);

        // Should find platform-appropriate tools
        #[cfg(target_os = "macos")]
        {
            assert!(router.find_tool("mac_system_summary").is_some());
            assert!(router.find_tool("mac_ping").is_some());
        }
        #[cfg(target_os = "windows")]
        {
            assert!(router.find_tool("win_system_summary").is_some());
            assert!(router.find_tool("win_ping").is_some());
        }
        // Should return None for unknown
        assert!(router.find_tool("nonexistent_tool").is_none());
    }

    #[test]
    fn test_tool_definitions_nonempty() {
        let mut router = ToolRouter::new();
        platform::register_platform_tools(&mut router, None);

        let defs = router.tool_definitions();
        assert!(!defs.is_empty(), "No tools registered");

        for def in &defs {
            assert!(!def.name.is_empty(), "Tool name is empty");
            assert!(!def.description.is_empty(), "Tool {} has empty description", def.name);
            // input_schema should be a valid JSON object with "type": "object"
            assert_eq!(
                def.input_schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "Tool {} input_schema missing type:object",
                def.name
            );
        }
    }

    #[test]
    fn test_no_duplicate_tool_names() {
        let mut router = ToolRouter::new();
        platform::register_platform_tools(&mut router, None);

        let defs = router.tool_definitions();
        let mut names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "Duplicate tool names registered");
    }
}
