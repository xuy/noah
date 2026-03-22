//! Fleet policy enforcement — applies admin-defined rules from the fleet dashboard.
//!
//! Policies are delivered via the checkin response and persisted as `fleet_policy.json`.
//! Three rule types: safety (tool approval), health (check categories), finding (defaults).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const POLICY_FILE: &str = "fleet_policy.json";

/// The effect a safety rule can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyEffect {
    /// Execute without asking the user.
    AutoApprove,
    /// Show the approval modal.
    RequireApproval,
    /// Refuse to execute.
    Block,
}

/// A single policy rule delivered from the fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub selector: String,
    pub effect: String,
    #[serde(default)]
    pub condition: Option<serde_json::Map<String, Value>>,
}

/// The full policy blob from the fleet checkin response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetPolicy {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

impl FleetPolicy {
    /// Load from `fleet_policy.json` in the app data directory.
    pub fn load(app_dir: &Path) -> Option<Self> {
        let path = app_dir.join(POLICY_FILE);
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Persist to `fleet_policy.json`.
    pub fn save(&self, app_dir: &Path) -> anyhow::Result<()> {
        let path = app_dir.join(POLICY_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Remove the policy file (e.g. on fleet unlink).
    pub fn remove(app_dir: &Path) {
        let path = app_dir.join(POLICY_FILE);
        let _ = std::fs::remove_file(path);
    }
}

// ── Safety rule resolution ─────────────────────────────────────────

/// Resolve the safety effect for a tool call, if any policy rule matches.
///
/// Matching order:
/// 1. Exact tool name match (e.g. `mac_clear_caches`)
/// 2. Prefix glob match (e.g. `mac_*`)
/// 3. Wildcard `*`
///
/// When multiple rules match, the most restrictive wins:
///   Block > RequireApproval > AutoApprove
pub fn resolve_safety_effect(
    policy: &FleetPolicy,
    tool_name: &str,
    tool_input: &Value,
) -> Option<SafetyEffect> {
    let safety_rules: Vec<&PolicyRule> = policy
        .rules
        .iter()
        .filter(|r| r.rule_type == "safety")
        .collect();

    if safety_rules.is_empty() {
        return None;
    }

    let mut best: Option<SafetyEffect> = None;

    for rule in &safety_rules {
        if !selector_matches(&rule.selector, tool_name) {
            continue;
        }
        // Check condition if present (e.g. command_pattern for shell_run).
        if let Some(cond) = &rule.condition {
            if !condition_matches(cond, tool_input) {
                continue;
            }
        }
        let effect = match rule.effect.as_str() {
            "auto_approve" => SafetyEffect::AutoApprove,
            "require_approval" => SafetyEffect::RequireApproval,
            "block" => SafetyEffect::Block,
            _ => continue,
        };
        best = Some(most_restrictive_safety(best, effect));
    }

    best
}

fn selector_matches(selector: &str, tool_name: &str) -> bool {
    if selector == "*" {
        return true;
    }
    if selector == tool_name {
        return true;
    }
    // Glob: `mac_*` matches `mac_clear_caches`
    if let Some(prefix) = selector.strip_suffix('*') {
        return tool_name.starts_with(prefix);
    }
    false
}

fn condition_matches(cond: &serde_json::Map<String, Value>, tool_input: &Value) -> bool {
    // command_pattern: glob match against tool_input.command
    if let Some(Value::String(pattern)) = cond.get("command_pattern") {
        if let Some(command) = tool_input.get("command").and_then(|v| v.as_str()) {
            return command_glob_match(pattern, command);
        }
        return false;
    }
    // when_tier: not implemented on desktop (tier is computed locally)
    true
}

/// Simple glob: `sudo *` matches any string starting with `sudo `.
fn command_glob_match(pattern: &str, command: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix(" *") {
        return command.starts_with(&format!("{} ", prefix));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return command.starts_with(prefix);
    }
    command == pattern
}

fn most_restrictive_safety(current: Option<SafetyEffect>, new: SafetyEffect) -> SafetyEffect {
    match current {
        None => new,
        Some(SafetyEffect::Block) => SafetyEffect::Block,
        Some(cur) => {
            if restrictiveness(new) > restrictiveness(cur) {
                new
            } else {
                cur
            }
        }
    }
}

fn restrictiveness(e: SafetyEffect) -> u8 {
    match e {
        SafetyEffect::AutoApprove => 1,
        SafetyEffect::RequireApproval => 2,
        SafetyEffect::Block => 3,
    }
}

// ── Health rule helpers ─────────────────────────────────────────────

/// Check if a health category is enabled by policy.
/// Returns `None` if no policy or no rule for this category (use default).
pub fn is_category_enabled(policy: &FleetPolicy, category: &str) -> Option<bool> {
    let rule = policy
        .rules
        .iter()
        .find(|r| r.rule_type == "health" && r.selector == category)?;
    Some(rule.effect != "disable")
}

/// Check if auto-heal is enabled for a health category by policy.
/// Returns `None` if no policy rule (use existing `auto_heal_enabled` setting).
pub fn should_auto_heal(policy: &FleetPolicy, category: &str) -> Option<bool> {
    let rule = policy
        .rules
        .iter()
        .find(|r| r.rule_type == "health" && r.selector == category)?;
    Some(rule.effect == "auto_heal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_policy(rules: Vec<PolicyRule>) -> FleetPolicy {
        FleetPolicy { version: 1, rules }
    }

    fn safety_rule(selector: &str, effect: &str) -> PolicyRule {
        PolicyRule {
            rule_type: "safety".to_string(),
            selector: selector.to_string(),
            effect: effect.to_string(),
            condition: None,
        }
    }

    #[test]
    fn exact_match_auto_approve() {
        let policy = make_policy(vec![safety_rule("mac_clear_caches", "auto_approve")]);
        let result = resolve_safety_effect(&policy, "mac_clear_caches", &json!({}));
        assert_eq!(result, Some(SafetyEffect::AutoApprove));
    }

    #[test]
    fn exact_match_block() {
        let policy = make_policy(vec![safety_rule("mac_kill_process", "block")]);
        let result = resolve_safety_effect(&policy, "mac_kill_process", &json!({}));
        assert_eq!(result, Some(SafetyEffect::Block));
    }

    #[test]
    fn no_match_returns_none() {
        let policy = make_policy(vec![safety_rule("mac_clear_caches", "auto_approve")]);
        let result = resolve_safety_effect(&policy, "shell_run", &json!({}));
        assert_eq!(result, None);
    }

    #[test]
    fn glob_match() {
        let policy = make_policy(vec![safety_rule("mac_*", "auto_approve")]);
        let result = resolve_safety_effect(&policy, "mac_clear_caches", &json!({}));
        assert_eq!(result, Some(SafetyEffect::AutoApprove));
    }

    #[test]
    fn wildcard_match() {
        let policy = make_policy(vec![safety_rule("*", "require_approval")]);
        let result = resolve_safety_effect(&policy, "anything", &json!({}));
        assert_eq!(result, Some(SafetyEffect::RequireApproval));
    }

    #[test]
    fn most_restrictive_wins() {
        let policy = make_policy(vec![
            safety_rule("shell_run", "auto_approve"),
            safety_rule("*", "require_approval"),
        ]);
        let result = resolve_safety_effect(&policy, "shell_run", &json!({}));
        assert_eq!(result, Some(SafetyEffect::RequireApproval));
    }

    #[test]
    fn condition_command_pattern() {
        let policy = make_policy(vec![PolicyRule {
            rule_type: "safety".to_string(),
            selector: "shell_run".to_string(),
            effect: "block".to_string(),
            condition: Some(serde_json::from_value(json!({"command_pattern": "sudo *"})).unwrap()),
        }]);
        // Matches
        let result = resolve_safety_effect(&policy, "shell_run", &json!({"command": "sudo rm -rf /"}));
        assert_eq!(result, Some(SafetyEffect::Block));
        // Doesn't match
        let result = resolve_safety_effect(&policy, "shell_run", &json!({"command": "ls -la"}));
        assert_eq!(result, None);
    }

    #[test]
    fn health_category_enabled() {
        let policy = make_policy(vec![PolicyRule {
            rule_type: "health".to_string(),
            selector: "backups".to_string(),
            effect: "disable".to_string(),
            condition: None,
        }]);
        assert_eq!(is_category_enabled(&policy, "backups"), Some(false));
        assert_eq!(is_category_enabled(&policy, "security"), None);
    }

    #[test]
    fn health_auto_heal() {
        let policy = make_policy(vec![PolicyRule {
            rule_type: "health".to_string(),
            selector: "performance".to_string(),
            effect: "auto_heal".to_string(),
            condition: None,
        }]);
        assert_eq!(should_auto_heal(&policy, "performance"), Some(true));
        assert_eq!(should_auto_heal(&policy, "security"), None);
    }
}
