# Playbook Authoring Guide

This template captures how to write effective Noah playbooks. The goal is to
pass expert IT knowledge to the LLM while giving it flexibility to adapt.

## Design Philosophy

**Encode what the LLM doesn't know.** The LLM can already reason, read errors,
and talk to users. What it lacks is: specific file paths, safe-to-delete
locations, known-good fix sequences, obscure error meanings, and "this looks
scary but it's normal" knowledge.

**Lead with the fix, not the diagnosis.** Most IT problems have a standard
escalation path that works 90%+ of the time. Put that front and center.
Add diagnostic branching as caveats, not as the primary flow.

**Think "training a junior IT tech."** Here's what you do most of the time.
Here's when you do something different. Use your head for everything else.

## File Format

```markdown
---
name: problem-name
description: One line — what this fixes and when to use it
platform: macos|windows|linux|all
last_reviewed: 2026-03-04
author: your-name
---

# Problem Name

## When to activate
Trigger phrases and symptoms. Help the LLM decide when to load this playbook.

## Quick check
One fast diagnostic to confirm this is the right playbook.
Saves a full investigation if the problem is something else entirely.

## Standard fix path (try in order)
Numbered steps, escalating from least to most disruptive.
Each step should say what to do, what tool to use, and what success looks like.

1. **Least disruptive fix** — what to do, expected result
2. **Next escalation** — what to do if step 1 didn't work
3. **More invasive fix** — bigger action, may need user confirmation
4. **Nuclear option** — last resort, explain trade-offs

> This sequence resolves ~X% of [problem type] issues.

## Caveats
Conditions that change the standard path. Format:
- If [condition], then [what to do differently and why].

These give the LLM permission to skip steps or take a different route.
Only include caveats backed by real IT experience, not hypotheticals.

## Key signals
Specific user phrases or tool outputs that should immediately redirect:
- "[User says X]" → jump to step N / different problem entirely
- "[Tool shows Y]" → skip steps 1-2, this is definitely Z

## Tools referenced
List the tools this playbook uses, so authors can verify they exist:
- `tool_name` — what it's used for in this playbook

## Escalation
When to give up and what to tell the user (contact IT, hardware issue, etc.)
```

## Tips

- **Be specific.** "Delete ~/Library/Caches/com.microsoft.Outlook/" is better
  than "clear the cache." The LLM can figure out *how to explain* it; you need
  to tell it *what to do*.

- **Name the percentage.** "This fixes 90% of cases" tells the LLM to try it
  confidently. "This sometimes helps" tells the LLM it might not be worth the
  disruption.

- **Warn about gotchas.** If an OST rebuild takes 30 minutes on a large
  mailbox, say so. The LLM needs to set user expectations.

- **Cross-platform playbooks** (platform: all) must not reference `mac_*` or
  `win_*` tool names. Use generic instructions the LLM can translate to the
  right platform commands.

- **Platform-specific playbooks** should reference the exact tool names
  (e.g., `mac_flush_dns`, `mac_restart_cups`). The test suite validates these
  against the tool registry.

- **Keep it under 120 lines.** The full playbook loads into the LLM context
  when activated. Concise = cheaper and more likely to be followed precisely.
