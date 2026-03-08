# Playbook FSM VM Design

## Goal

Move playbook control logic from Rust hardcoded domain logic (e.g., OpenClaw-specific runtime code) into playbook-authored data.

End state:
- `orchestrator` is generic.
- `playbook_runtime` is a generic FSM VM.
- Domain behavior (OpenClaw, etc.) is defined in each playbook file.

This supports a co-working model:
- FSM provides structure, progression, and guardrails.
- LLM provides conversational guidance, tool strategy, and UX quality.

## Non-goals

- FSM does **not** render UI cards.
- FSM does **not** encode low-level procedural scripts for every transition.
- FSM does **not** replace the agent loop.

## Design Principles

1. Authoring must be lightweight and semantic.
2. Runtime verification must be optional and focused on critical guardrails.
3. LLM should receive clear "what next" guidance from FSM, not tedious machine constraints.
4. Backward compatibility with existing sessions should be best-effort and explicit.

## Playbook File Structure

Playbook markdown has two layers:

1. Human-readable sections (intent, guidance, caveats, examples).
2. Structured FSM section (`## FSM` + fenced JSON/DSL block).

Example:

```md
## FSM
```json
{ ... }
```
```

## FSM Schema (Author-facing)

Minimal and semantic.

```json
{
  "version": 1,
  "machine": "openclaw-install-config",
  "initial_state": "INSTALL_CHECK",
  "states": {
    "INSTALL_CHECK": {
      "summary": "Confirm OpenClaw is installed and runnable.",
      "llm_guidance": [
        "Use one non-destructive check first.",
        "If missing, install then re-check."
      ]
    },
    "PROVIDER_CAPTURE": {
      "summary": "Collect provider credentials through secure form.",
      "llm_guidance": [
        "Never ask user to paste secrets in chat.",
        "Use plain-language provider names."
      ]
    }
  },
  "events": {
    "install_verified": { "source": "llm_or_runtime" },
    "install_missing": { "source": "llm_or_runtime" },
    "secure_form_submitted": { "source": "user_event" },
    "provider_verified": { "source": "llm_or_runtime" },
    "channel_skipped": { "source": "user_event" }
  },
  "transitions": [
    {
      "id": "t_install_ok",
      "from": "INSTALL_CHECK",
      "to": "PROVIDER_CAPTURE",
      "goal": "OpenClaw install is confirmed.",
      "acceptance": [
        "Version command succeeds OR user confirms OpenClaw is already installed."
      ],
      "triggers": ["install_verified"],
      "llm_guidance": [
        "If check fails, transition to install path instead of retry loop."
      ]
    },
    {
      "id": "t_capture_to_verify",
      "from": "PROVIDER_CAPTURE",
      "to": "PROVIDER_VERIFY",
      "goal": "Provider secret captured via secure form.",
      "acceptance": [
        "Secure form submission event received with provider metadata."
      ],
      "triggers": ["secure_form_submitted"]
    }
  ],
  "terminal": {
    "states": ["DONE"],
    "goal": "Basic setup complete with verified provider; optional channel may be pending."
  },
  "guards": {
    "blocked_commands": {
      "PROVIDER_VERIFY": [
        "openclaw doctor --fix",
        "openclaw channels add"
      ]
    }
  }
}
```

### Optional strict runtime checks

For critical transitions only, an optional low-level block can be included:

```json
"runtime_checks": {
  "assertions": [
    {"type":"tool_success","tool":"shell_run","contains":"openclaw --version"}
  ]
}
```

This is optional and intended for safety/completion guardrails, not default authoring.

## Runtime Data Model

```rust
struct FsmSpec {
  version: u32,
  machine: String,
  initial_state: String,
  states: HashMap<String, StateSpec>,
  events: HashMap<String, EventSpec>,
  transitions: Vec<TransitionSpec>,
  terminal: TerminalSpec,
  guards: Option<GuardSpec>,
}

struct FsmSession {
  playbook_name: String,
  state: String,
  vars: serde_json::Value,        // non-secret metadata only
  history: Vec<FsmTransitionLog>,
  pending_requirements: Vec<String>,
  attempts_by_state: HashMap<String, u32>,
}
```

## FSM Tool Surface (for LLM)

Expose one logical tool `fsm` with operations.

### 1) `fsm.get`
Returns current state, state summary, candidate transitions, unmet acceptance, terminal status.

### 2) `fsm.emit_event`
Input: event name + optional metadata.
Runtime evaluates transitions and moves state when criteria are met.

### 3) `fsm.set_vars`
Stores non-secret metadata only (provider name, chosen channel, verification flags, refs).

### 4) `fsm.next`
Returns a concise "what needs to happen next" bundle for LLM:
- current state summary
- top candidate transitions
- unmet acceptance conditions
- guidance bullets

No UI payload in FSM tool. UI remains `ui_spa`, `ui_user_question`, `ui_info`, `ui_done`.

## Co-working Flow

1. LLM calls `fsm.get` or `fsm.next`.
2. LLM runs tools / asks user / opens secure form.
3. Runtime ingests results/user events.
4. LLM emits `fsm.emit_event` as milestones occur.
5. FSM advances state and returns new requirements.
6. LLM continues until terminal condition met.

## Guardrails

Guardrails are generic runtime behavior driven by FSM spec:
- command blocklist by state
- max attempts per state (optional)
- disallowed response patterns (optional, playbook-provided)
- done gating: `ui_done` only when terminal condition true

## Migration Plan

### Phase 1: Infrastructure
- Add FSM block parser in `playbooks.rs`.
- Add generic `FsmSpec` + `FsmSession` in `playbook_runtime.rs`.
- Add `fsm` tool with `get/next/emit_event/set_vars`.

### Phase 2: Adapter
- Replace hardcoded OpenClaw runtime functions with spec-driven equivalents.
- Keep backward compatibility shim for old sessions/events.

### Phase 3: Playbook Port
- Move OpenClaw stage machine rules into `openclaw-install-config.md` FSM section.
- Keep narrative guidance in markdown sections.

### Phase 4: Enforcement
- `orchestrator` asks playbook runtime for:
  - governance overlay (generic)
  - shell guard feedback (generic)
  - final response validation (generic + optional transition checks)

### Phase 5: Cleanup
- Remove OpenClaw-specific structs/enums/functions from runtime.
- Keep only generic FSM engine and parser.

## Backward Compatibility

- Existing historical sessions with marker/legacy formats remain readable.
- Legacy action type alias should remain accepted during migration:
  - `OPENCLAW_SECURE_CAPTURE` -> `OPEN_SECURE_FORM`.
- If no FSM block exists in a playbook, runtime falls back to "playbook text only" mode.

## Open Questions

1. Should transition acceptance allow boolean expressions (`all/any`) in v1, or only flat lists?
2. Do we persist FSM session state in DB immediately (recommended) or derive from messages each turn (slower/fragile)?
3. Should runtime auto-emit some events from tool outcomes (e.g., successful install check), or keep all event emission explicit from LLM?

## Recommended v1 Choices

1. Acceptance: `all_of` and `any_of` only.
2. Persist FSM session state in DB per session.
3. Hybrid event emission:
- runtime auto-emits obvious objective events (tool success/failure),
- LLM emits semantic events (user readiness, optional skip, etc.).

## Success Criteria

- No OpenClaw-specific runtime logic in orchestrator or generic runtime.
- OpenClaw setup behavior driven by playbook FSM block + markdown guidance.
- Non-technical user flow remains smooth and does not dead-loop.
- Historical sessions remain readable and renderable.
