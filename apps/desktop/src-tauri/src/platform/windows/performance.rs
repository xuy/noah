use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use noah_tools::{ChangeRecord, SafetyTier, Tool, ToolResult};

// ── WinSystemInfo ─────────────────────────────────────────────────────

pub struct WinSystemInfo;

#[async_trait]
impl Tool for WinSystemInfo {
    fn name(&self) -> &str {
        "win_system_info"
    }

    fn description(&self) -> &str {
        "Get Windows version, CPU model, core count, and total memory."
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
        // Use Win32_OperatingSystem.Caption for OS name — Get-ComputerInfo's
        // WindowsProductName is broken on Windows 11 (reports "Windows 10").
        let info = super::hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                "$os = Get-CimInstance Win32_OperatingSystem; \
                $cpu = Get-CimInstance Win32_Processor; \
                $cs = Get-CimInstance Win32_ComputerSystem; \
                \"CsName           : $($cs.Name)\"; \
                \"OS                : $($os.Caption)\"; \
                \"Version           : $($os.Version)\"; \
                \"Build             : $($os.BuildNumber)\"; \
                \"CPU               : $($cpu.Name)\"; \
                \"LogicalProcessors : $($cpu.NumberOfLogicalProcessors)\"; \
                \"Memory            : $([math]::Round($os.TotalVisibleMemorySize / 1MB, 1)) GB\"",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("System info query failed: {}", e));

        let output = format!("=== Windows System Info ===\n{}", info);

        Ok(ToolResult::read_only(
            output,
            json!({ "raw_output": info }),
        ))
    }
}

// ── WinProcessList ────────────────────────────────────────────────────

pub struct WinProcessList;

#[async_trait]
impl Tool for WinProcessList {
    fn name(&self) -> &str {
        "win_process_list"
    }

    fn description(&self) -> &str {
        "List running processes sorted by CPU or memory usage."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sort_by": {
                    "type": "string",
                    "description": "Sort by 'cpu' or 'mem' (default: cpu)",
                    "enum": ["cpu", "mem"],
                    "default": "cpu"
                }
            },
            "required": [],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::ReadOnly
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let sort_by = input["sort_by"].as_str().unwrap_or("cpu");

        let sort_prop = if sort_by == "mem" { "WorkingSet64" } else { "CPU" };

        let ps_cmd = format!(
            "Get-Process | Sort-Object {} -Descending | Select-Object -First 25 \
            Id, ProcessName, \
            @{{Name='CPU(s)';Expression={{[math]::Round($_.CPU, 2)}}}}, \
            @{{Name='Mem(MB)';Expression={{[math]::Round($_.WorkingSet64 / 1MB, 1)}}}} \
            | Format-Table -AutoSize",
            sort_prop
        );

        let output = super::hidden_cmd("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("Get-Process failed: {}", e));

        let combined = format!(
            "=== Top Processes (sorted by {}) ===\n{}",
            sort_by, output.trim()
        );

        Ok(ToolResult::read_only(
            combined.clone(),
            json!({
                "sort_by": sort_by,
                "raw_output": output.trim(),
            }),
        ))
    }
}

// ── WinDiskUsage ──────────────────────────────────────────────────────

pub struct WinDiskUsage;

#[async_trait]
impl Tool for WinDiskUsage {
    fn name(&self) -> &str {
        "win_disk_usage"
    }

    fn description(&self) -> &str {
        "Show disk usage for all volumes."
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
        let output = super::hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                "Get-Volume | Where-Object { $_.DriveLetter } | \
                Select-Object DriveLetter, FileSystemLabel, FileSystem, \
                @{Name='Size(GB)';Expression={[math]::Round($_.Size / 1GB, 2)}}, \
                @{Name='Free(GB)';Expression={[math]::Round($_.SizeRemaining / 1GB, 2)}}, \
                @{Name='Used%';Expression={if($_.Size -gt 0){[math]::Round(($_.Size - $_.SizeRemaining) / $_.Size * 100, 1)}else{0}}} \
                | Format-Table -AutoSize",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|e| format!("Get-Volume failed: {}", e));

        Ok(ToolResult::read_only(
            output.clone(),
            json!({ "raw_output": output.trim() }),
        ))
    }
}

// ── WinKillProcess ────────────────────────────────────────────────────

pub struct WinKillProcess;

#[async_trait]
impl Tool for WinKillProcess {
    fn name(&self) -> &str {
        "win_kill_process"
    }

    fn description(&self) -> &str {
        "Kill a process by PID using taskkill. Requires user approval."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pid": {
                    "type": "integer",
                    "description": "Process ID to kill"
                }
            },
            "required": ["pid"],
            "additionalProperties": false
        })
    }

    fn safety_tier(&self) -> SafetyTier {
        SafetyTier::SafeAction
    }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let pid = input["pid"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: pid"))?;

        // Get process info before killing
        let ps_info = super::hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!(
                    "Get-Process -Id {} -ErrorAction SilentlyContinue | \
                    Select-Object Id, ProcessName, CPU, \
                    @{{Name='Mem(MB)';Expression={{[math]::Round($_.WorkingSet64 / 1MB, 1)}}}} \
                    | Format-List",
                    pid
                ),
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let output = super::hidden_cmd("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if o.status.success() {
                    format!(
                        "Process {} terminated.\n\nProcess info:\n{}{}",
                        pid,
                        ps_info.trim(),
                        if !stdout.trim().is_empty() { format!("\n{}", stdout.trim()) } else { String::new() }
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    format!("Failed to kill process {}: {}", pid, stderr.trim())
                }
            })
            .unwrap_or_else(|e| format!("taskkill failed: {}", e));

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({ "pid": pid }),
            vec![ChangeRecord {
                description: format!("Killed process {}", pid),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}

// ── WinClearCaches ────────────────────────────────────────────────────

pub struct WinClearCaches;

#[async_trait]
impl Tool for WinClearCaches {
    fn name(&self) -> &str {
        "win_clear_caches"
    }

    fn description(&self) -> &str {
        "Clear temporary files from %TEMP% and C:\\Windows\\Temp to free disk space."
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
        // Get user TEMP directory
        let user_temp = std::env::var("TEMP")
            .unwrap_or_else(|_| std::env::var("TMP").unwrap_or_else(|_| "C:\\Windows\\Temp".to_string()));

        // Get size before clearing
        let before_size = super::hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!(
                    "$userTemp = '{}'; \
                    $sysTemp = 'C:\\Windows\\Temp'; \
                    $userSize = (Get-ChildItem $userTemp -Recurse -ErrorAction SilentlyContinue | \
                        Measure-Object -Property Length -Sum -ErrorAction SilentlyContinue).Sum; \
                    $sysSize = (Get-ChildItem $sysTemp -Recurse -ErrorAction SilentlyContinue | \
                        Measure-Object -Property Length -Sum -ErrorAction SilentlyContinue).Sum; \
                    'User temp: {{0:N2}} MB, System temp: {{1:N2}} MB' -f ($userSize/1MB), ($sysSize/1MB)",
                    user_temp
                ),
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Clear user temp
        let clear_result = super::hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!(
                    "$ErrorActionPreference = 'SilentlyContinue'; \
                    $removed = 0; $failed = 0; \
                    Get-ChildItem '{}' -Recurse | ForEach-Object {{ \
                        try {{ Remove-Item $_.FullName -Recurse -Force; $removed++ }} \
                        catch {{ $failed++ }} \
                    }}; \
                    Get-ChildItem 'C:\\Windows\\Temp' -Recurse | ForEach-Object {{ \
                        try {{ Remove-Item $_.FullName -Recurse -Force; $removed++ }} \
                        catch {{ $failed++ }} \
                    }}; \
                    \"Removed $removed items ($failed skipped - in use)\"",
                    user_temp
                ),
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|e| format!("Failed to clear caches: {}", e));

        let output = format!(
            "Temp files cleared.\nBefore: {}\n{}",
            before_size, clear_result
        );

        Ok(ToolResult::with_changes(
            output.clone(),
            json!({
                "user_temp": user_temp,
                "before_size": before_size,
            }),
            vec![ChangeRecord {
                description: format!("Cleared temp files from {} and C:\\Windows\\Temp", user_temp),
                undo_tool: String::new(),
                undo_input: json!(null),
            }],
        ))
    }
}
