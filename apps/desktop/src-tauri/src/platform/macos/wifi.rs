use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use itman_tools::{SafetyTier, Tool, ToolResult};

// ── WifiScan ──────────────────────────────────────────────────────────

pub struct WifiScan;

#[async_trait]
impl Tool for WifiScan {
    fn name(&self) -> &str {
        "wifi_scan"
    }

    fn description(&self) -> &str {
        "Scan the Wi-Fi environment: current connection details, signal quality, nearby networks, and channel congestion analysis."
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
        // Get detailed Wi-Fi info from system_profiler.
        let _airport_info = Command::new("system_profiler")
            .args(["SPAirPortDataType", "-json"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e));

        // Get current Wi-Fi interface info via airport utility.
        let airport_path = "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport";
        let current_info = Command::new(airport_path)
            .arg("-I")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("airport -I failed: {}", e));

        // Scan for nearby networks.
        let scan_output = Command::new(airport_path)
            .arg("-s")
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if stdout.is_empty() {
                    format!("Scan returned no results. {}", stderr.trim())
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|e| format!("airport -s failed: {}", e));

        // Parse current connection details for a summary.
        let mut ssid = "unknown";
        let mut rssi = "unknown";
        let mut noise = "unknown";
        let mut channel = "unknown";
        let mut phy_mode = "unknown";

        for line in current_info.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("SSID:") {
                ssid = val.trim();
            } else if let Some(val) = line.strip_prefix("agrCtlRSSI:") {
                rssi = val.trim();
            } else if let Some(val) = line.strip_prefix("agrCtlNoise:") {
                noise = val.trim();
            } else if let Some(val) = line.strip_prefix("channel:") {
                channel = val.trim();
            } else if let Some(val) = line.strip_prefix("lastTxRate:") {
                phy_mode = val.trim();
            }
        }

        // Signal quality assessment.
        let signal_quality = match rssi.parse::<i32>() {
            Ok(r) if r > -50 => "Excellent",
            Ok(r) if r > -60 => "Good",
            Ok(r) if r > -70 => "Fair",
            Ok(r) if r > -80 => "Weak",
            Ok(_) => "Very Weak",
            Err(_) => "Unknown",
        };

        let output = format!(
            "=== Current Wi-Fi Connection ===\n\
             SSID: {}\n\
             Signal (RSSI): {} dBm ({})\n\
             Noise: {} dBm\n\
             Channel: {}\n\
             Last TX Rate: {} Mbps\n\
             \n\
             === Current Interface Details ===\n\
             {}\n\
             \n\
             === Nearby Networks ===\n\
             {}",
            ssid,
            rssi,
            signal_quality,
            noise,
            channel,
            phy_mode,
            current_info.trim(),
            scan_output.trim()
        );

        Ok(ToolResult::read_only(
            output,
            json!({
                "ssid": ssid,
                "rssi": rssi,
                "noise": noise,
                "channel": channel,
                "signal_quality": signal_quality,
                "airport_info": current_info.trim(),
            }),
        ))
    }
}
