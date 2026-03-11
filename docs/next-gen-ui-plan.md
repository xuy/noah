# Next-Gen UI Plan for Noah

**Date:** 2026-03-11
**Context:** Inspired by Claude Cowork Desktop and OpenAI Codex Desktop, adapted for IT support agent use case.

## Design Principle (Unchanged)

> Any UI is a transparent "representation" of user interaction as if they are directly talking to the underlying model.

All UI elements below preserve this principle. No UI should create semantic mismatch between what the user sees and what the LLM thread contains.

---

## Status

| Phase | Status | Commit |
|-------|--------|--------|
| Option A: Contextual Status | **Done** | `0aab294` |
| Phase 2: Plan Review Gate | **Done** | `0aab294` |
| Phase 4: Terminal-style Activity Log | **Done** | `0aab294` |
| Phase 3: Rich Diagnostic Panel | Designed, not started | — |
| Phase 5: Scheduled Tasks | Designed, not started | — |
| Phase 6: Pop-out Windows | Not started | — |

---

## Completed: Option A — Contextual Status Messages

**What:** `humanizeToolCall` now shows the tool's target alongside the action:
- "Testing connectivity — google.com" instead of "Testing connectivity"
- "Reading file — /etc/hosts" instead of "Reading file"

Uses the tool's input params (`host`, `domain`, `url`, `app_name`, `path`, `query`, etc.) to add context. Falls back to the plain i18n label if no relevant input found.

**Files changed:** `ChatPanel.tsx` (humanizeToolCall function)

---

## Completed: Phase 2 — Plan Review Gate

**What:** When a playbook activates, users see a plan card listing all steps with a "Let it run" button. Clicking it enables auto-run mode where RUN_STEP actions confirm automatically.

**How it works:**
1. `playbook_activated` debug-log event captures step list → stored in `sessionStore.playbookSteps`
2. `PlanReviewBanner` component renders the step list + "Let it run" button
3. Button sets `sessionStore.autoRun = true`
4. In `useAgent.ts`, `maybeAutoConfirm()` fires after each response:
   - Checks `autoRun` flag + response is `ui_spa` with `action_type: "RUN_STEP"`
   - Waits 400ms (card visibility), re-checks `autoRun` (user may have stopped)
   - Calls `sendConfirmation` via ref to continue the loop
5. WAIT_FOR_USER actions still pause (by design — human action required)
6. NeedsApproval actions still show the approval modal
7. Cancel button and "Stop" link both disable auto-run

**Files changed:** `useAgent.ts`, `sessionStore.ts`, `ChatPanel.tsx`, `en.json`, `zh.json`

---

## Completed: Phase 4 — Terminal-style Activity Log

**What:** Enhanced the existing ActivityLog with terminal aesthetics:
- Dark background (`#1a1a2e`) with syntax-colored text
- Blue commands, gray output, red errors, italic thinking
- Increased output preview to 18 lines (from 6)
- Chevron expand/collapse indicator with event count
- Taller max-height (18rem)

**Files changed:** `ChatPanel.tsx` (ActivityLog component + formatActivityEntry)

---

## Remaining: Phase 3 — Rich Diagnostic Panel

**Goal:** Render structured diagnostic data as visual cards instead of plain text.

**Design Decision:** Inline cards in chat, NOT a side panel. Side panels create dual representations that violate transparency. Instead, when the LLM calls `ui_info` or `ui_done`, it can include structured data that renders as a visual card.

### Implementation Plan

**Step 1: Extend `ui_info` with optional structured data**

Add an optional `data` field to `ui_info` tool:
```
ui_info({
  summary: "Your system looks healthy",
  data: {
    type: "system_summary",
    metrics: [
      { label: "CPU", value: "Apple M2", status: "ok" },
      { label: "Memory", value: "16 GB (3.2 GB free)", status: "warning" },
      { label: "Disk", value: "234 GB free of 500 GB", status: "ok" },
      { label: "Network", value: "Connected (72ms ping)", status: "ok" },
    ]
  }
})
```

**Step 2: `MetricsCard` component**
- Renders metrics as a clean grid with status indicators (green dot, yellow dot, red dot)
- Supports types: `system_summary`, `network_status`, `disk_usage`, `process_table`
- Falls back to plain text rendering if `data` is not present (backward compat)

**Step 3: System prompt instructions**
- Teach the LLM to include `data` when presenting diagnostic results
- Only for structured metrics, not for narrative explanations

**Estimated effort:** ~200 lines React + ~30 lines Rust (ui_tools.rs schema) + ~10 lines prompt
**Dependencies:** None — builds on existing ui_info infrastructure

---

## Remaining: Phase 5 — Scheduled Tasks

**Goal:** Let users define recurring automated checks with a scheduling UI.

### Implementation Plan

**Step 1: DB schema**

Add `automations` table:
```sql
CREATE TABLE automations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    schedule TEXT NOT NULL,     -- cron expression or simple interval ("6h", "1d", "weekly")
    enabled INTEGER DEFAULT 1,
    last_run TEXT,              -- RFC3339 timestamp
    last_result TEXT,           -- summary of last run result
    last_status TEXT,           -- "ok", "warning", "error"
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

**Step 2: Tauri commands**
```rust
create_automation(name, prompt, schedule) -> Automation
list_automations() -> Vec<Automation>
update_automation(id, updates) -> Automation
delete_automation(id)
run_automation(id) -> AutomationResult  // on-demand trigger
```

**Step 3: Scheduler integration**

Extend `ProactiveMonitor::run_forever()` to also check user automations:
```rust
// In the 6h loop (or more frequently for user tasks):
let automations = journal::list_enabled_automations(&conn)?;
for auto in automations {
    if is_due(&auto) {
        let result = self.run_automation(&auto).await;
        journal::update_automation_result(&conn, &auto.id, &result)?;
        if result.needs_attention {
            self.emit_suggestion(auto.name, result.summary);
        }
    }
}
```

**Step 4: Frontend — AutomationsPanel**

New tab in the sidebar (or section in DiagnosticsView):
- List of automations with name, schedule, last result
- "New automation" form (name + what to check + how often)
- Toggle enable/disable
- "Run now" button
- History of past results

**Presets:** Ship common automations as suggestions:
- "Check disk space daily"
- "Monitor for crashes weekly"
- "Verify internet connectivity every 2 hours"

**Estimated effort:** ~200 lines Rust (DB + commands) + ~300 lines React (panel + form)
**Dependencies:** None — extends existing proactive infrastructure

---

## Not Building

- **Step Checklist (Phase 1):** Rejected — feels procedural, not professional. Contextual status (Option A) achieves the same goal with less UI noise.
- **Multi-project/multi-thread:** Noah is single-purpose IT support, one conversation at a time.
- **Folder-scoped sandbox:** Noah operates on system config, not user files.
- **MCP Apps / iframe widgets:** Too much infrastructure for current scale.
- **IDE context sync:** Not a code tool.
- **Git integration:** Not relevant for IT support.

---

## Architecture Notes

All phases preserve the core architecture:
- LLM emits tool calls → Orchestrator executes → Frontend renders
- UI is a transparent view of the conversation thread
- No "wrapper" UI that creates semantic mismatch
- Auto-run preserves transparency: confirmation messages still appear in chat
- New components consume existing data (tool outputs, debug-log events, DB)
