use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{SafetyTier, Tool, ToolResult};

// ── WinNetworkInfo ────────────────────────────────────────────────────

pub struct WinNetworkInfo;

#[async_trait]
impl Tool for WinNetworkInfo {
    fn name(&self) -> &str {
        "win_network_info"
    }

    fn description(&self) -> &str {
        "Get current network configuration including IP addresses, DNS settings, and Wi-Fi info."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let ipconfig = Command::new("ipconfig")
            .arg("/all")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("ipconfig failed: {}", e));

        let wifi = Command::new("netsh")
            .args(["wlan", "show", "interfaces"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("netsh wlan failed: {}", e));

        let output = format!(
            "=== Network Configuration ===\n{}\n\n=== Wi-Fi Info ===\n{}",
            ipconfig.trim(),
            wifi.trim()
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "ipconfig": ipconfig.trim(),
                "wifi": wifi.trim(),
            }),
        ))
    }
}

// ── WinPing ───────────────────────────────────────────────────────────

pub struct WinPing;

#[async_trait]
impl Tool for WinPing {
    fn name(&self) -> &str {
        "win_ping"
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
            "required": ["host"]
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
            .args(["-n", &count.to_string(), host])
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

// ── WinDnsCheck ───────────────────────────────────────────────────────

pub struct WinDnsCheck;

#[async_trait]
impl Tool for WinDnsCheck {
    fn name(&self) -> &str {
        "win_dns_check"
    }

    fn description(&self) -> &str {
        "Perform DNS lookup for a domain using nslookup and PowerShell Resolve-DnsName."
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
            "required": ["domain"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let domain = input["domain"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: domain"))?;

        let nslookup = Command::new("nslookup")
            .arg(domain)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("nslookup failed: {}", e));

        let resolve = Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("Resolve-DnsName '{}' | Format-List", domain)])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("Resolve-DnsName failed: {}", e));

        let output = format!(
            "=== nslookup {} ===\n{}\n\n=== Resolve-DnsName {} ===\n{}",
            domain,
            nslookup.trim(),
            domain,
            resolve.trim()
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "nslookup": nslookup.trim(),
                "resolve_dns": resolve.trim(),
            }),
        ))
    }
}

// ── WinHttpCheck ──────────────────────────────────────────────────────

pub struct WinHttpCheck;

#[async_trait]
impl Tool for WinHttpCheck {
    fn name(&self) -> &str {
        "win_http_check"
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
            "required": ["url"]
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: url"))?;

        // curl is available on Windows 10+ by default
        let output = Command::new("curl")
            .args([
                "-o", "NUL",
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

// ── WinFlushDns ───────────────────────────────────────────────────────

pub struct WinFlushDns;

#[async_trait]
impl Tool for WinFlushDns {
    fn name(&self) -> &str {
        "win_flush_dns"
    }

    fn description(&self) -> &str {
        "Flush the Windows DNS resolver cache. This is a safe action that clears cached DNS records."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, _input: &Value) -> Result<ToolResult> {
        let output = Command::new("ipconfig")
            .arg("/flushdns")
            .output();

        let msg = match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if o.status.success() {
                    format!("DNS cache flushed successfully.\n{}", stdout.trim())
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("DNS flush completed with warnings: {}", stderr.trim())
                }
            }
            Err(e) => format!("Failed to flush DNS cache: {}", e),
        };

        Ok(ToolResult::read_only(msg.clone(), json!({ "status": msg })))
    }
}
