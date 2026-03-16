use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use noah_tools::{SafetyTier, Tool, ToolResult};

// ── MacNetworkInfo ─────────────────────────────────────────────────────

pub struct MacNetworkInfo;

#[async_trait]
impl Tool for MacNetworkInfo {
    fn name(&self) -> &str {
        "mac_network_info"
    }

    fn description(&self) -> &str {
        "Get current network configuration including interfaces, DNS settings, and Wi-Fi info."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let ifconfig = Command::new("ifconfig")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("ifconfig failed: {}", e));

        let dns = Command::new("scutil")
            .arg("--dns")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("scutil --dns failed: {}", e));

        let wifi = Command::new("networksetup")
            .args(["-getinfo", "Wi-Fi"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("networksetup failed: {}", e));

        let output = format!(
            "=== Network Interfaces ===\n{}\n\n=== DNS Configuration ===\n{}\n\n=== Wi-Fi Info ===\n{}",
            ifconfig.trim(),
            dns.trim(),
            wifi.trim()
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "ifconfig": ifconfig.trim(),
                "dns": dns.trim(),
                "wifi": wifi.trim(),
            }),
        ))
    }
}

// ── MacPing ────────────────────────────────────────────────────────────

pub struct MacPing;

#[async_trait]
impl Tool for MacPing {
    fn name(&self) -> &str {
        "mac_ping"
    }

    fn description(&self) -> &str {
        "Ping a host to test network connectivity. Returns ping statistics."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "description": "Hostname or IP address to ping"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of pings to send (default 4)",
                    "default": 4
                }
            },
            "required": ["host"],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let host = input["host"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: host"))?;
        let count = input["count"].as_u64().unwrap_or(4);

        let output = Command::new("ping")
            .args(["-c", &count.to_string(), host])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if stdout.is_empty() { stderr } else { stdout }
            })
            .unwrap_or_else(|e| format!("ping failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── MacDnsCheck ────────────────────────────────────────────────────────

pub struct MacDnsCheck;

#[async_trait]
impl Tool for MacDnsCheck {
    fn name(&self) -> &str {
        "mac_dns_check"
    }

    fn description(&self) -> &str {
        "Perform DNS lookup for a domain using dig and nslookup."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Domain name to look up"
                }
            },
            "required": ["domain"],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let domain = input["domain"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: domain"))?;

        let dig = Command::new("dig")
            .arg(domain)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("dig failed: {}", e));

        let nslookup = Command::new("nslookup")
            .arg(domain)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("nslookup failed: {}", e));

        let output = format!(
            "=== dig {} ===\n{}\n\n=== nslookup {} ===\n{}",
            domain,
            dig.trim(),
            domain,
            nslookup.trim()
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "dig": dig.trim(),
                "nslookup": nslookup.trim(),
            }),
        ))
    }
}

// ── MacHttpCheck ───────────────────────────────────────────────────────

pub struct MacHttpCheck;

#[async_trait]
impl Tool for MacHttpCheck {
    fn name(&self) -> &str {
        "mac_http_check"
    }

    fn description(&self) -> &str {
        "Test HTTP connectivity to a URL and report status code, redirect chain, and timing."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to test (e.g. https://example.com)"
                }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: url"))?;

        let output = Command::new("curl")
            .args([
                "-o", "/dev/null",
                "-s",
                "-w", "HTTP Status: %{http_code}\nRedirect URL: %{redirect_url}\nTime Total: %{time_total}s\nTime Connect: %{time_connect}s\nTime DNS: %{time_namelookup}s\n",
                "-L",
                "--max-time", "15",
                url,
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("curl failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── MacFlushDns ────────────────────────────────────────────────────────

pub struct MacFlushDns;

#[async_trait]
impl Tool for MacFlushDns {
    fn name(&self) -> &str {
        "mac_flush_dns"
    }

    fn description(&self) -> &str {
        "Flush the macOS DNS cache. This is a safe action that clears cached DNS records."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let output = Command::new("dscacheutil")
            .arg("-flushcache")
            .output();

        let _ = Command::new("sudo")
            .args(["killall", "-HUP", "mDNSResponder"])
            .output();

        let msg = match output {
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if o.status.success() {
                    "DNS cache flushed successfully.".to_string()
                } else {
                    format!("DNS flush completed with warnings: {}", stderr.trim())
                }
            }
            Err(e) => format!("Failed to flush DNS cache: {}", e),
        };

        Ok(ToolResult::read_only(msg.clone(), json!({ "status": msg })))
    }
}
