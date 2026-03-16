# Noah Tool Schema Audit

**Date:** 2026-03-16
**Total tools:** 60 (5 UI + 4 knowledge/playbook + 1 web + 18 macOS + 19 Windows + 13 Linux)

---

## Executive Summary

The tool system works but has grown organically. Key issues:

1. **3 near-identical implementations** of every common tool (one per platform), with only the shell command differing
2. **Linux is severely under-served** — missing app management, printer, cache clearing, crash logs, wifi
3. **Naming is platform-prefixed** (`mac_ping`, `win_ping`, `linux_ping`) when schemas are identical — forces the LLM to learn 3 names for the same thing
4. **Inconsistent schema quality** — some tools missing `additionalProperties: false`, vague descriptions, no documented limits
5. **Copy-paste code smell** — `shell_run`, `is_dangerous_command()`, `read_file`, and most network tools are duplicated verbatim across 3 files with only the shell binary name changed

---

## 1. Consolidation Opportunities

### 1a. Unified tool names (HIGH IMPACT)

These tools have **identical schemas** across all 3 platforms. The only difference is the underlying command. They should be a single tool with platform dispatch:

| Current (3 tools each) | Proposed (1 tool) | Schema identical? |
|---|---|---|
| `mac_system_summary` / `win_system_summary` / `linux_system_summary` | `system_summary` | Yes — empty `{}` |
| `mac_system_info` / `win_system_info` / `linux_system_info` | `system_info` | Yes — empty `{}` |
| `mac_network_info` / `win_network_info` / `linux_network_info` | `network_info` | Yes — empty `{}` |
| `mac_ping` / `win_ping` / `linux_ping` | `ping` | Yes — `{host, count}` |
| `mac_dns_check` / `win_dns_check` / `linux_dns_check` | `dns_check` | Yes — `{domain}` |
| `mac_http_check` / `win_http_check` / `linux_http_check` | `http_check` | Yes — `{url}` |
| `mac_flush_dns` / `win_flush_dns` / `linux_flush_dns` | `flush_dns` | Yes — empty `{}` |
| `mac_process_list` / `win_process_list` / `linux_process_list` | `process_list` | Yes — `{sort_by}` |
| `mac_disk_usage` / `win_disk_usage` / `linux_disk_usage` | `disk_usage` | Yes — empty `{}` |
| `mac_kill_process` / `win_kill_process` / `linux_kill_process` | `kill_process` | Yes — `{pid, signal}` |
| `mac_read_file` / `win_read_file` / `linux_read_file` | `read_file` | Yes — `{path}` |

**Result:** Eliminates **22 redundant tool definitions** from the LLM context. The model sees 11 tools instead of 33.

`shell_run` already uses a unified name — proof this pattern works.

### 1b. Log tools need platform-specific schemas (KEEP SEPARATE or use union schema)

| Platform | Tool | Schema |
|---|---|---|
| macOS | `mac_read_log` | `{predicate, duration}` |
| Windows | `win_read_log` | `{log_name (enum), level, duration}` |
| Linux | `linux_read_log` | `{unit, priority, since}` |

**Option A:** Keep 3 separate tools (current). LLM knows which to call based on OS context.
**Option B:** Unified `read_log` with a union schema — include all fields, platform dispatch ignores irrelevant ones. Risk: LLM may hallucinate wrong fields.

**Recommendation:** Option A for now. Log semantics genuinely differ per platform.

### 1c. App/printer tools — add Linux parity (MEDIUM IMPACT)

Missing on Linux but feasible:

| Tool | macOS | Windows | Linux (proposed) |
|---|---|---|---|
| App list | `mac_app_list` | `win_app_list` | `dpkg --list` / `rpm -qa` / `flatpak list` |
| App data dir listing | `mac_app_support_ls` | `win_app_data_ls` | `ls ~/.local/share/{app}` or `~/.config/{app}` |
| Clear app cache | `mac_clear_app_cache` | `win_clear_app_cache` | `rm -rf ~/.cache/{app}` |
| Clear system caches | `mac_clear_caches` | `win_clear_caches` | `sync && echo 3 > /proc/sys/vm/drop_caches` (needs sudo) |
| Printer list | `mac_printer_list` | `win_printer_list` | `lpstat -p` (CUPS exists on Linux too) |
| Print queue | `mac_print_queue` | `win_print_queue` | `lpstat -o` |

These would all get unified names if 1a is implemented: `app_list`, `app_data_ls`, `clear_app_cache`, etc.

---

## 2. Schema Quality Issues

### 2a. Missing `additionalProperties: false`

These tools lack it, allowing the LLM to send extra fields that get silently ignored:

- `write_knowledge`
- `knowledge_search`
- `knowledge_read`
- `web_fetch`
- All macOS tools in `network.rs`, `performance.rs`, `apps.rs`

**Fix:** Add `"additionalProperties": false` to all input schemas for consistency.

### 2b. Undocumented limits in descriptions

| Tool | Hidden limit | Should document |
|---|---|---|
| `web_fetch` | 100K char truncation | Yes — LLM should know to use shell_run + curl for large pages |
| `read_file` | 500 lines max | Yes — LLM should know to use offset/limit or shell_run |
| `read_log` (all) | 200 lines max | Yes — LLM should narrow predicate if truncated |
| `shell_run` | 10K char output, 60s timeout | Yes — LLM should stream/paginate for large outputs |
| `knowledge_search` (content mode) | 3 snippets per file, ±1 line context default | Yes |

### 2c. `ui_user_question` schema allows contradictory inputs

Schema lets you specify `options`, `text_input`, AND `secure_input` on the same question. The Rust code picks one (options > text > secure), but the schema should enforce mutual exclusivity via `oneOf`.

### 2d. `ui_spa` has legacy fallback parsing

`normalize_action_from_input()` accepts 3 different shapes for the action:
- Flat: `{action_label, action_type}` (documented)
- Hoisted: `{label, ...}` (undocumented)
- Nested: `{action: {label, type}}` (undocumented)

This was added to handle LLM variability, but it means the schema lies about the expected shape. Either document all forms or remove the fallbacks and let the LLM learn the one true format.

---

## 3. Cross-Platform Architecture Proposal

### Current architecture (per-platform files)

```
platform/
  macos/
    diagnostics.rs   → SystemSummary, ReadFile, ReadLog, ShellRun
    network.rs       → NetworkInfo, Ping, DnsCheck, HttpCheck, FlushDns
    performance.rs   → SystemInfo, ProcessList, DiskUsage, KillProcess, ClearCaches
    apps.rs          → AppList, AppLogs, AppSupportLs, ClearAppCache, MoveFile
    printer.rs       → PrinterList, PrintQueue, CancelPrintJobs, RestartCups
    wifi.rs          → WifiScan
    disk_audit.rs    → DiskAudit
    crash_logs.rs    → CrashLogReader
  windows/
    diagnostics.rs   → (same pattern, win_ prefix)
    network.rs
    performance.rs
    apps.rs
    printer.rs
    startup.rs       → StartupPrograms
    services.rs      → ServiceList, RestartService
  linux/
    diagnostics.rs   → (same pattern, linux_ prefix)
    network.rs
    performance.rs
```

### Proposed architecture (shared trait + platform impl)

```
tools/
  shared/
    mod.rs           → PlatformTool trait (name, description, schema, execute)
    system_summary.rs → unified "system_summary" tool
    network_info.rs   → unified "network_info" tool
    ping.rs           → unified "ping" tool
    dns_check.rs      → unified "dns_check" tool
    http_check.rs     → unified "http_check" tool
    flush_dns.rs      → unified "flush_dns" tool
    process_list.rs   → unified "process_list" tool
    disk_usage.rs     → unified "disk_usage" tool
    kill_process.rs   → unified "kill_process" tool
    read_file.rs      → unified "read_file" tool
    shell_run.rs      → unified "shell_run" tool (already works this way)
  platform_specific/
    macos/
      read_log.rs     → mac_read_log (unique schema)
      wifi_scan.rs
      disk_audit.rs
      crash_logs.rs
    windows/
      read_log.rs     → win_read_log (unique schema)
      startup.rs
      services.rs
    linux/
      read_log.rs     → linux_read_log (unique schema)
```

Each shared tool would have a single struct with `#[cfg(target_os)]` in the `execute()` method:

```rust
pub struct Ping;

impl Tool for Ping {
    fn name(&self) -> &str { "ping" }
    fn description(&self) -> &str { "Ping a host to test connectivity." }
    fn input_schema(&self) -> Value { /* one schema */ }

    async fn execute(&self, input: &Value) -> Result<ToolResult> {
        let host = input["host"].as_str().unwrap();
        let count = input["count"].as_u64().unwrap_or(4);

        #[cfg(target_os = "macos")]
        let output = run_cmd("ping", &["-c", &count.to_string(), host]).await?;

        #[cfg(target_os = "windows")]
        let output = run_cmd("ping", &["-n", &count.to_string(), host]).await?;

        #[cfg(target_os = "linux")]
        let output = run_cmd("ping", &["-c", &count.to_string(), host]).await?;

        Ok(ToolResult::text(output))
    }
}
```

### Impact on LLM context

| | Before | After |
|---|---|---|
| macOS user sees | 23 platform + 10 shared = 33 tools | 11 unified + 6 mac-specific + 10 shared = 27 tools |
| Windows user sees | 20 platform + 10 shared = 30 tools | 11 unified + 5 win-specific + 10 shared = 26 tools |
| Linux user sees | 13 platform + 10 shared = 23 tools | 11 unified + 1 linux-specific + 10 shared = 22 tools |

Fewer tools = less schema tokens in system prompt = more room for conversation.

---

## 4. Duplicated Code to Extract

### `is_dangerous_command()` — duplicated 3x

Identical logic in all 3 `diagnostics.rs` files, differing only in the platform-specific dangerous patterns. Extract to a shared module:

```rust
// tools/shared/safety.rs
pub fn is_dangerous_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    for pattern in COMMON_DANGEROUS_PATTERNS {
        if lower.contains(pattern) { return true; }
    }
    #[cfg(target_os = "macos")]
    for pattern in MACOS_DANGEROUS_PATTERNS { ... }
    #[cfg(target_os = "windows")]
    for pattern in WINDOWS_DANGEROUS_PATTERNS { ... }
    #[cfg(target_os = "linux")]
    for pattern in LINUX_DANGEROUS_PATTERNS { ... }
    false
}
```

### Output formatting — duplicated 3x

The stdout/stderr/exit-code formatting in `shell_run` is identical across all three platforms. Extract to shared helper.

### Path validation — duplicated 3x

`read_file` validates allowed paths differently per platform, but the check structure is identical. Extract to trait method.

---

## 5. Quick Wins (no architecture change needed)

1. **Add `additionalProperties: false`** to all schemas missing it (1-line change each)
2. **Document limits** in tool descriptions (truncation, timeouts)
3. **Remove `ui_spa` legacy fallbacks** — keep only the flat schema format, let the LLM conform
4. **Add `oneOf` to `ui_user_question`** items to enforce single input mode
5. **Validate `write_knowledge` category** against known enum in schema
6. **Unify `reason` field description** in `shell_run` — macOS has an example, Linux doesn't

---

## 6. Priority Order

| Priority | Change | Effort | Impact |
|---|---|---|---|
| P0 | Unify 11 identical-schema tools to platform-agnostic names | Medium | -22 tool defs from LLM context |
| P0 | Extract `is_dangerous_command` + output formatting to shared module | Low | -500 lines of duplication |
| P1 | Add `additionalProperties: false` everywhere | Low | Better schema hygiene |
| P1 | Document hidden limits in descriptions | Low | Better LLM behavior |
| P1 | Remove `ui_spa` legacy input fallbacks | Low | Schema honesty |
| P2 | Add Linux app/printer/cache tools | Medium | Platform parity |
| P2 | Restructure into `tools/shared/` + `tools/platform_specific/` dirs | Medium | Maintainability |
| P3 | Add `oneOf` constraints to `ui_user_question` | Low | Schema correctness |
| P3 | Validate knowledge categories in schema | Low | Data hygiene |
