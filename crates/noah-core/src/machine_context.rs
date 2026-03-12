use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

const CACHE_FILE: &str = "machine_context.json";
const STALE_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineContext {
    pub platform: String,
    pub arch: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub os_version: String,
    #[serde(default)]
    pub cpu: String,
    #[serde(default)]
    pub cpu_cores: String,
    #[serde(default)]
    pub memory: String,
    pub gathered_at: String,
}

/// Run a command and return trimmed stdout, or `fallback` on any failure.
fn run_cmd(program: &str, args: &[&str], fallback: &str) -> String {
    let mut cmd = Command::new(program);
    cmd.args(args);

    // On Windows, prevent console windows from flashing during startup.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if s.is_empty() {
                fallback.to_string()
            } else {
                s
            }
        }
        _ => fallback.to_string(),
    }
}

// ── Platform-specific runtime gathering ──────────────────────────────

#[cfg(target_os = "macos")]
fn gather_runtime() -> (String, String, String, String, String) {
    let hostname = run_cmd("hostname", &[], "Unknown");

    let product_name = run_cmd("sw_vers", &["-productName"], "");
    let product_version = run_cmd("sw_vers", &["-productVersion"], "");
    let os_version = match (product_name.as_str(), product_version.as_str()) {
        ("", "") => "Unknown".to_string(),
        (name, "") => name.to_string(),
        ("", ver) => ver.to_string(),
        (name, ver) => format!("{} {}", name, ver),
    };

    let cpu = run_cmd("sysctl", &["-n", "machdep.cpu.brand_string"], "Unknown");
    let cpu_cores = run_cmd("sysctl", &["-n", "hw.logicalcpu"], "Unknown");

    let memory = {
        let raw = run_cmd("sysctl", &["-n", "hw.memsize"], "");
        raw.parse::<u64>()
            .map(|b| format!("{} GB", b / (1024 * 1024 * 1024)))
            .unwrap_or_else(|_| "Unknown".to_string())
    };

    (hostname, os_version, cpu, cpu_cores, memory)
}

#[cfg(target_os = "windows")]
fn gather_runtime() -> (String, String, String, String, String) {
    let hostname = run_cmd("hostname", &[], "Unknown");

    let os_version = run_cmd(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_OperatingSystem).Caption",
        ],
        "Unknown",
    );

    let cpu = run_cmd(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor).Name",
        ],
        "Unknown",
    );

    let cpu_cores = run_cmd(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor).NumberOfLogicalProcessors",
        ],
        "Unknown",
    );

    let memory = run_cmd(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "[math]::Round((Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize / 1MB, 0).ToString() + ' GB'",
        ],
        "Unknown",
    );

    (hostname, os_version, cpu, cpu_cores, memory)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn gather_runtime() -> (String, String, String, String, String) {
    let hostname = run_cmd("hostname", &[], "Unknown");

    let os_version = {
        // Try /etc/os-release PRETTY_NAME
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|contents| {
                contents.lines().find_map(|line| {
                    line.strip_prefix("PRETTY_NAME=")
                        .map(|v| v.trim_matches('"').to_string())
                })
            })
            .unwrap_or_else(|| "Unknown".to_string())
    };

    let cpu = run_cmd("uname", &["-p"], "Unknown");
    let cpu_cores = run_cmd("nproc", &[], "Unknown");

    let memory = {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|contents| {
                contents.lines().find_map(|line| {
                    line.strip_prefix("MemTotal:").map(|v| {
                        let kb_str = v.trim().trim_end_matches("kB").trim();
                        kb_str
                            .parse::<u64>()
                            .map(|kb| format!("{} GB", kb / (1024 * 1024)))
                            .unwrap_or_else(|_| "Unknown".to_string())
                    })
                })
            })
            .unwrap_or_else(|| "Unknown".to_string())
    };

    (hostname, os_version, cpu, cpu_cores, memory)
}

// ── MachineContext impl ──────────────────────────────────────────────

impl MachineContext {
    /// Gather fresh context from compile-time consts + runtime commands.
    pub fn gather() -> Self {
        let (hostname, os_version, cpu, cpu_cores, memory) = gather_runtime();
        Self {
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            hostname,
            os_version,
            cpu,
            cpu_cores,
            memory,
            gathered_at: Utc::now().to_rfc3339(),
        }
    }

    /// Load cached context from disk. Returns None if missing or corrupt.
    pub fn load(app_dir: &Path) -> Option<Self> {
        let path = app_dir.join(CACHE_FILE);
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Save context to disk as pretty JSON. Non-fatal, but logs a warning on failure.
    pub fn save(&self, app_dir: &Path) {
        let path = app_dir.join(CACHE_FILE);
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    eprintln!("[warn] Failed to save machine context cache: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[warn] Failed to serialize machine context: {}", e);
            }
        }
    }

    /// True if gathered_at is >24h ago or unparseable.
    pub fn is_stale(&self) -> bool {
        match self.gathered_at.parse::<DateTime<Utc>>() {
            Ok(ts) => Utc::now().signed_duration_since(ts).num_hours() >= STALE_HOURS,
            Err(_) => true,
        }
    }

    /// Primary entry point: load cached if available, otherwise gather + save.
    ///
    /// Always returns immediately if a cache file exists (even if stale) to avoid
    /// blocking the main thread. On Windows, `gather()` runs PowerShell/WMI commands
    /// that can take 10-30 seconds, freezing the UI if run during `setup()`.
    ///
    /// Use `refresh_if_stale()` in the background after startup to update the cache.
    pub fn load_or_gather(app_dir: &Path) -> Self {
        if let Some(cached) = Self::load(app_dir) {
            return cached;
        }
        let ctx = Self::gather();
        ctx.save(app_dir);
        ctx
    }

    /// Refresh the cache in the background if stale. Call from an async context.
    pub fn refresh_if_stale(app_dir: &Path) {
        if let Some(cached) = Self::load(app_dir) {
            if !cached.is_stale() {
                return;
            }
        }
        let ctx = Self::gather();
        ctx.save(app_dir);
    }

    /// Format for the system prompt. Omits fields that are "Unknown" or empty.
    pub fn to_prompt_string(&self) -> String {
        let mut lines = vec![format!("Platform: {} ({})", self.platform, self.arch)];

        let fields: &[(&str, &str)] = &[
            ("Hostname", &self.hostname),
            ("OS Version", &self.os_version),
            ("CPU", &self.cpu),
            ("CPU Cores", &self.cpu_cores),
            ("Memory", &self.memory),
        ];

        for (label, value) in fields {
            if !value.is_empty() && *value != "Unknown" {
                lines.push(format!("{}: {}", label, value));
            }
        }

        lines.join("\n")
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn gather_always_has_platform_and_arch() {
        let ctx = MachineContext::gather();
        assert!(!ctx.platform.is_empty());
        assert!(!ctx.arch.is_empty());
        assert_eq!(ctx.platform, std::env::consts::OS);
        assert_eq!(ctx.arch, std::env::consts::ARCH);
    }

    #[test]
    fn gathered_at_is_valid_rfc3339() {
        let ctx = MachineContext::gather();
        assert!(ctx.gathered_at.parse::<DateTime<Utc>>().is_ok());
    }

    #[test]
    fn prompt_string_starts_with_platform() {
        let ctx = MachineContext::gather();
        let prompt = ctx.to_prompt_string();
        assert!(prompt.starts_with("Platform: "));
        assert!(prompt.contains(std::env::consts::OS));
        assert!(prompt.contains(std::env::consts::ARCH));
    }

    #[test]
    fn prompt_string_omits_unknown_fields() {
        let ctx = MachineContext {
            platform: "testplat".to_string(),
            arch: "testarch".to_string(),
            hostname: "Unknown".to_string(),
            os_version: String::new(),
            cpu: "Test CPU".to_string(),
            cpu_cores: "Unknown".to_string(),
            memory: String::new(),
            gathered_at: Utc::now().to_rfc3339(),
        };
        let prompt = ctx.to_prompt_string();
        assert!(prompt.contains("Platform: testplat (testarch)"));
        assert!(prompt.contains("CPU: Test CPU"));
        assert!(!prompt.contains("Hostname"));
        assert!(!prompt.contains("OS Version"));
        assert!(!prompt.contains("CPU Cores"));
        assert!(!prompt.contains("Memory"));
    }

    #[test]
    fn round_trip_json_serialization() {
        let ctx = MachineContext::gather();
        let json = serde_json::to_string_pretty(&ctx).unwrap();
        let deserialized: MachineContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx.platform, deserialized.platform);
        assert_eq!(ctx.arch, deserialized.arch);
        assert_eq!(ctx.gathered_at, deserialized.gathered_at);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join("noah_test_machine_ctx");
        let _ = fs::create_dir_all(&dir);
        let ctx = MachineContext::gather();
        ctx.save(&dir);
        let loaded = MachineContext::load(&dir).expect("should load saved context");
        assert_eq!(ctx.platform, loaded.platform);
        assert_eq!(ctx.arch, loaded.arch);
        assert_eq!(ctx.gathered_at, loaded.gathered_at);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_none_for_missing_file() {
        let dir = std::env::temp_dir().join("noah_test_missing_ctx");
        let _ = fs::remove_dir_all(&dir);
        assert!(MachineContext::load(&dir).is_none());
    }

    #[test]
    fn load_returns_none_for_corrupt_json() {
        let dir = std::env::temp_dir().join("noah_test_corrupt_ctx");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join(CACHE_FILE), "not valid json{{{").unwrap();
        assert!(MachineContext::load(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fresh_context_is_not_stale() {
        let ctx = MachineContext::gather();
        assert!(!ctx.is_stale());
    }

    #[test]
    fn stale_context_triggers_regather() {
        let old_time = Utc::now() - chrono::Duration::hours(25);
        let ctx = MachineContext {
            platform: "test".to_string(),
            arch: "test".to_string(),
            hostname: String::new(),
            os_version: String::new(),
            cpu: String::new(),
            cpu_cores: String::new(),
            memory: String::new(),
            gathered_at: old_time.to_rfc3339(),
        };
        assert!(ctx.is_stale());
    }

    #[test]
    fn unparseable_timestamp_treated_as_stale() {
        let ctx = MachineContext {
            platform: "test".to_string(),
            arch: "test".to_string(),
            hostname: String::new(),
            os_version: String::new(),
            cpu: String::new(),
            cpu_cores: String::new(),
            memory: String::new(),
            gathered_at: "not-a-timestamp".to_string(),
        };
        assert!(ctx.is_stale());
    }
}
