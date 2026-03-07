# OpenClaw UI Parity Plan

## Goal
Ensure Noah uses one interaction contract across:
- Desktop UI runtime
- Backend debug runner/harness
- Session replay tests

So setup flows (especially OpenClaw) are testable without behavior drift.

## Task 1: UI Axiom + Limited User Events

### 1) Structured Assistant UI Object
- Add a backend response envelope for assistant output:
  - `kind: card` with `{ situation, plan, action }`
  - `kind: done` with `{ summary }`
  - `kind: info` with `{ summary }`
- `action` includes:
  - `label`
  - `type` from a small enum:
    - `RUN_STEP`
    - `OPENCLAW_SECURE_CAPTURE`
    - `ASK_USER_QUESTION`
- Preserve raw text for backward compatibility and old-session replay.

### 2) Limited User Event Vocabulary
- Add one event ingress command with strict event types:
  - `USER_CONFIRM`
  - `USER_SKIP_OPTIONAL`
  - `USER_SUBMIT_SECURE_FORM`
  - `USER_ANSWER_QUESTION`
- Event payload is structured JSON; no control flow via free text.
- Backend converts events into orchestrator turns and persists flags.

### 3) AskUserQuestion-style Interaction
- Support a compact assistant action payload that can carry:
  - one or more questions
  - short headers
  - option labels/descriptions
  - optional multi-select
- UI renders this action; selected answers are sent via `USER_ANSWER_QUESTION`.

### 4) Unified Harness Parity
- Debug runner uses the same event API used by UI.
- No special behavior based on text-matching card content.

### 5) Tests for Regression Safety
- Backward compatibility tests for old marker-based assistant messages.
- Session replay tests for OpenClaw setup turns.
- Event-ingress tests for each supported user event type.

## Task 2: Playbook FSM Tool (next)

### FSM as Tool (not hardcoded flow)
- Add playbook-scoped `fsm` tool:
  - `fsm.get_state`
  - `fsm.allowed_actions`
  - `fsm.transition(event)`
  - `fsm.guardrail_hint`
- State persists with session.
- LLM remains flexible, but uses FSM for orientation and anti-loop guardrails.

