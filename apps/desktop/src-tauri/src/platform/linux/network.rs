use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{SafetyTier, Tool, ToolResult};

// ── LinuxNetworkInfo ──────────────────────────────────────────────────

pub struct LinuxNetworkInfo;

#[async_trait]
impl Tool for LinuxNetworkInfo {
    fn name(&self) -> &str {
        "linux_network_info"
    }

    fn description(&self) -> &str {
        "Get current network configuration including interfaces, DNS settings, and default route."
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
        let ip_addr = Command::new("ip")
            .args(["addr", "show"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("ip addr failed: {}", e));

        let ip_route = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("ip route failed: {}", e));

        // Read DNS config directly — no external command needed.
        let dns = std::fs::read_to_string("/etc/resolv.conf")
            .unwrap_or_else(|e| format!("failed to read /etc/resolv.conf: {}", e));

        let output = format!(
            "=== Network Interfaces ===\n{}\n\n=== Default Route ===\n{}\n\n=== DNS (/etc/resolv.conf) ===\n{}",
            ip_addr.trim(),
            ip_route.trim(),
            dns.trim()
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "ip_addr": ip_addr.trim(),
                "ip_route": ip_route.trim(),
                "dns": dns.trim(),
            }),
        ))
    }
}

// ── LinuxPing ─────────────────────────────────────────────────────────

pub struct LinuxPing;

#[async_trait]
impl Tool for LinuxPing {
    fn name(&self) -> &str {
        "linux_ping"
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

// ── LinuxDnsCheck ─────────────────────────────────────────────────────
// Uses std::net for resolution — no dig/nslookup dependency.

pub struct LinuxDnsCheck;

#[async_trait]
impl Tool for LinuxDnsCheck {
    fn name(&self) -> &str {
        "linux_dns_check"
    }

    fn description(&self) -> &str {
        "Perform DNS lookup for a domain. Resolves addresses and reports configured DNS servers."
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

        // Resolve using the OS resolver (works everywhere, no external tool).
        let lookup_target = format!("{}:0", domain);
        let resolved = std::net::ToSocketAddrs::to_socket_addrs(&lookup_target.as_str());

        let resolution = match resolved {
            Ok(addrs) => {
                let ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
                if ips.is_empty() {
                    format!("DNS lookup for '{}': no addresses returned", domain)
                } else {
                    // Deduplicate (v4 and v6 may repeat for port 0)
                    let unique: Vec<&String> = {
                        let mut seen = std::collections::HashSet::new();
                        ips.iter().filter(|ip| seen.insert(ip.as_str())).collect()
                    };
                    format!(
                        "DNS lookup for '{}':\n{}",
                        domain,
                        unique.iter().map(|ip| format!("  → {}", ip)).collect::<Vec<_>>().join("\n")
                    )
                }
            }
            Err(e) => format!("DNS lookup FAILED for '{}': {}", domain, e),
        };

        // Also show configured nameservers from resolv.conf.
        let nameservers = std::fs::read_to_string("/etc/resolv.conf")
            .map(|c| {
                c.lines()
                    .filter(|l| l.starts_with("nameserver"))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|_| "could not read /etc/resolv.conf".to_string());

        let output = format!(
            "{}\n\n=== Configured Nameservers ===\n{}",
            resolution,
            nameservers
        );

        Ok(ToolResult::read_only(
            output.clone(),
            json!({
                "domain": domain,
                "resolution": resolution,
                "nameservers": nameservers,
            }),
        ))
    }
}

// ── LinuxHttpCheck ────────────────────────────────────────────────────
// Uses reqwest (already a project dependency) — no curl needed.

pub struct LinuxHttpCheck;

#[async_trait]
impl Tool for LinuxHttpCheck {
    fn name(&self) -> &str {
        "linux_http_check"
    }

    fn description(&self) -> &str {
        "Test HTTP connectivity to a URL and report status code and timing."
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

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let start = std::time::Instant::now();
        let result = client.get(url).send().await;
        let elapsed = start.elapsed();

        let output = match result {
            Ok(resp) => {
                let status = resp.status();
                let final_url = resp.url().to_string();
                let redirected = if final_url != url {
                    format!("\nFinal URL: {} (redirected)", final_url)
                } else {
                    String::new()
                };
                format!(
                    "HTTP Status: {}\nTime Total: {:.3}s{}",
                    status.as_u16(),
                    elapsed.as_secs_f64(),
                    redirected,
                )
            }
            Err(e) => {
                if e.is_timeout() {
                    format!("Connection timed out after 15 seconds for {}", url)
                } else if e.is_connect() {
                    format!("Connection failed for {}: {}", url, e)
                } else {
                    format!("HTTP check failed for {}: {}", url, e)
                }
            }
        };

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "url": url, "result": output.trim() }),
        ))
    }
}

// ── LinuxFlushDns ─────────────────────────────────────────────────────

pub struct LinuxFlushDns;

#[async_trait]
impl Tool for LinuxFlushDns {
    fn name(&self) -> &str {
        "linux_flush_dns"
    }

    fn description(&self) -> &str {
        "Flush the DNS cache. Tries systemd-resolved first, then nscd."
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
        // Try systemd-resolved first (most modern distros)
        let resolved = Command::new("resolvectl")
            .arg("flush-caches")
            .output();

        let msg = match resolved {
            Ok(o) if o.status.success() => {
                "DNS cache flushed successfully (systemd-resolved).".to_string()
            }
            _ => {
                // Fall back to nscd
                let nscd = Command::new("nscd")
                    .args(["--invalidate=hosts"])
                    .output();
                match nscd {
                    Ok(o) if o.status.success() => {
                        "DNS cache flushed successfully (nscd).".to_string()
                    }
                    _ => {
                        "DNS flush attempted. Neither systemd-resolved nor nscd responded. DNS cache may not be active on this system (many Linux distros don't cache DNS by default).".to_string()
                    }
                }
            }
        };

        Ok(ToolResult::read_only(msg.clone(), json!({ "status": msg })))
    }
}
