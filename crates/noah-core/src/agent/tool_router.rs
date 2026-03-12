use noah_tools::Tool;
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

    #[test]
    fn test_register_and_find_empty() {
        let router = ToolRouter::new();
        assert!(router.find_tool("nonexistent_tool").is_none());
        assert!(router.tool_definitions().is_empty());
    }
}
