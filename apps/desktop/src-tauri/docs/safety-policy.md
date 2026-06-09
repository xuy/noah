# Destructive-action safety policy

Status: accepted, 2026-06-09. Owner: Eric.

This is the durable decision record behind Noah's redline system. It exists
because a real session nearly (and maybe actually) destroyed a paying user's
data. Read the incident first — the policy only makes sense as a response to it.

## The incident

A trialing-then-paying user asked (one sentence, three intents):

> "My mac is running slow **and I want my storage pushed to Google Drive
> using the folder in finder** and my mac is slow please improve this **and i
> want a target ssd size of below 410**."

Three intents: performance, **offload to cloud (move, preserve)**, and a **hard
quota** (under 410 GB). Across ~44 steps Noah held the quota and worked it down
(398 → 321 GB). That relentless, goal-driven, conversational cleanup is the
product's moat — it is impossible in CleanMyMac or Mole because it requires
reasoning and a dialogue. **We do not want to weaken it.**

But the same loop also ran, each gated only by a click-through approval whose
prompt showed Noah's own one-line reason:

- `rm -rf ~/Library/Messages/Attachments/*`, `rm -rf …/com.apple.MobileSMS`
  → the user's iMessage history.
- `sudo rm -rf ~/Library/Application Support/*` → every app's data.
- `rm -rf ~/Library/Containers/*` → sandboxed app data (Notes, Mail…).
- `tmutil deletelocalsnapshots` → the Time Machine safety net.

And it **never touched Google Drive** — it silently swapped the user's chosen
*method* (move/preserve) for *delete*. We cannot tell from telemetry which
deletes succeeded, because we logged the *proposed shell command*, not the
*approval decision or the result*. We were blind. That blindness is TODO #1.

## What went wrong, structurally

Our only **code-enforced** guardrail was "command contains `rm`/`sudo` →
ask the user," which a trusting user clears in two seconds. Everything else
protecting their data was **playbook prose**: advisory, holey (Messages wasn't
on the refuse list; no rule against wildcard sweeps), and possibly **not even
loaded** for this phrasing. For a product whose killer feature is autonomous
deletion toward a goal, the safety floor was a single click.

## Principles

1. **Put each rule at the lowest layer that can guarantee it.**
   - *Software (Rust)* — deterministic, un-arguable. The only layer that can
     hold a **redline**. Holds the enforced policy.
   - *Playbook* — contextual good-practice ("offload means move"; "surface,
     don't push deletion"). Advisory. Goes back to being *advice*, not a guard.
   - *AI* — judgment and conversation. Powerful and persuadable. Never a place
     for a guarantee.
   A redline the model can talk past is not a redline.

2. **A harness is not a wall.** A wall always says no, so it makes no decision.
   Noah is the arbiter between what the human asks and what the AI proposes; it
   *resists and yields under control*, it does not forbid capability. So the
   default for a dangerous-but-legitimate action is **reject-then-reconsider**,
   not "never."

3. **Gate on justification, not syntax.** The problem with
   `rm -rf …/Application Support/*` is not the `*`; it is that the AI has not
   established the specific thing is removable. A wildcard fails because `*` is
   the syntactic form of *"I haven't looked."* Five *named, verified-orphaned*
   app folders carry per-target justification and are fine.

4. **Inspect before delete (the Claude Code precedent).** Claude Code's harness
   requires you to `Read` a file before `Edit`. The check is **structural, not
   semantic** — it does not verify you *understood* the file, only that you
   *looked*. We do the identical thing for deletes inside protected trees: the
   harness cannot know "Adobe is uninstalled," but it can know whether the model
   *inspected the folder* before deleting it. That is enough to force the
   careful look, and it is cheap and deterministic. The inspection results land
   in context, so the model's next decision is grounded — and a human/the
   telemetry can audit it.

5. **You can only govern what you can see.** Ground truth lives only in the
   execution layer: tier classified → gate decision → approval decision →
   result → manifest of what was touched. The same record powers safety
   auditing, undo, and the "here's what Noah did" trust surface. Build it once.

6. **Approximation is the implementation, and it has assurance.** "Irreplaceable"
   is not computable at runtime; we approximate it with a curated tree list. That
   is not a retreat — macOS TCC itself is a hardcoded path list. A fixed,
   conservative, testable list is *more* assuring than a clever runtime detector,
   because assurance comes from predictability. The *property* is the spec; the
   list is the satisfying approximation, and every entry carries its rationale so
   it does not rot.

## The redline taxonomy (the spine)

A friction gradient, not a set of absolutes:

| Tier | What | Action |
|------|------|--------|
| **auto** | regenerable / reversible (caches, logs, old pkg versions) | run, log it |
| **confirm** | ordinary deletes outside protected trees | approve, with plain-language disclosure |
| **inspect-then-delete** | concrete delete *inside* a protected tree, not yet inspected | **reject with a tip**; clears once that path (or an ancestor within the tree) has been inspected this session |
| **reject-wildcard** | wildcard sweep into a protected tree | **reject with a tip**; never auto-clears — the AI must enumerate specific subpaths |
| **hard-deny** | the handful of actions that end the machine or destroy identity | **refuse even with inspection + reaffirmation** |

### Protected trees (approximation; irreplaceable user data / app state)

Tracks Apple's TCC-protected locations plus credential stores. Two classes:

- **App-state (Library)** — cache/log subdirs *are* regenerable-exempt:
  `~/Library/Application Support`, `~/Library/Containers`,
  `~/Library/Group Containers`, `~/Library/Messages`, `~/Library/Mail`,
  `~/Library/Safari`, `~/Library/Calendars`, `~/Library/Mobile Documents`
  (iCloud), `~/Library/CloudStorage` (Google Drive/Dropbox/OneDrive),
  `~/Library/Photos`.
- **Credentials/keys** — `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.kube`,
  `~/.docker`, `~/.config` (the Keychain itself is *hard-deny*).
- **User content** — *no* regenerable exemption (a folder literally named
  "cache" under `~/Documents` is still the user's): `~/Documents`, `~/Desktop`,
  `~/Pictures`, `~/Movies`, `~/Music`, `~/Downloads`.

### Path normalisation — closing the bypasses (verified)

The match would be worthless if the same data could be addressed by another
spelling. So before matching, paths are normalised:

- **Firmlinks.** Every user path is *also* reachable under
  `/System/Volumes/Data` — confirmed same inode on a live machine
  (`stat -f %d:%i ~/Library` == `…/System/Volumes/Data/Users/<u>/Library`; see
  `/usr/share/firmlinks`). The real incident trace used this long form for `du`.
  We strip the `/System/Volumes/Data` prefix so both spellings hit the same tree.
- **Home / case.** `~`, `$HOME`, and the absolute `/Users/<u>/…` form all
  resolve together; comparison is lowercased (macOS default volumes are
  case-insensitive).
- **Ancestors & whole-tree roots.** Deleting `~/Library` (an *ancestor* of every
  protected tree) or `~/Library/Application Support` (the root itself) is a
  **sweep** — rejected even with inspection; only specific *sub*-paths are
  inspect-clearable. Wiping `~` or `/Users/<u>` is hard-deny.

### Deletion vectors (not just `rm`)

`rm`, `unlink`/`srm`, and `find` used destructively — `find … -delete`,
`find … -exec rm`, and `find … | xargs rm` — are all classified as deletes. A
non-destructive `find` still counts as *inspection*; a destructive one does not.

### Regenerable hints (defeat the gate only inside app-state trees)

A path segment matching `/Caches/`, `/Cache/`, `/Logs/`, `/.cache/` is treated
as regenerable → no inspection required — **but only** strictly inside an
app-state tree, never in a user-content tree and never for a tree root/ancestor.

### Hard-deny floor (refused even when reaffirmed)

- root / boot-volume destruction: `rm -rf /`, `rm -rf /System`, `…/Users` wipe.
- secure-erase of the boot volume (`diskutil … erase` on the system disk).
- `~/Library/Keychains/`, `com.apple.security*`, `…/com.apple.TCC/` — auth and
  permission identity.
These are the only true walls. Everything else yields to a careful, looked-at,
reaffirmed request.

## Inspect-before-delete state machine

State: a **session-scoped set of inspected paths** (`Arc<Mutex<HashMap<session,
HashSet<path>>>>` on the orchestrator; cleared on session end). It mirrors
Claude Code's read-set.

- A **read-class** observation records the paths it covers as inspected:
  `shell_run` running `ls`/`du`/`find`/`stat`/`cat`/`file` against a path, and
  the structured read tools (`disk_audit`, `mac_disk_usage`). Down the road, the
  background scanner's inventory pre-satisfies inspection for paths it covered.
- A **delete** targeting a protected tree consults the set:
  - wildcard sweep → reject (never cleared), tip: enumerate + inspect specifics.
  - concrete path inspected (path `T` where some inspected `I` is within the
    tree and `T` is `I` or a descendant) → allow → normal approval.
  - concrete path not inspected → reject, tip: inspect this folder first.
- Clearing rule precision: inspection of `I` clears deletion of `T` **iff**
  `I` is at-or-below the protected tree root *and* `T` starts with `I`. So
  `ls ~` (above the tree root) clears nothing; `ls ~/Library/Application Support`
  clears its descendants; `du …/Adobe` clears `…/Adobe`.

The tip is what makes it a harness, not a wall: it instructs the remedy.

## Telemetry schema (TODO #1 — "did we actually approve it")

Emitted from the execution layer for every consequential action:

```
action_event {
  session_id, tool, command,
  classification: auto | confirm | inspect_then_delete | reject_wildcard | hard_deny,
  gate: allowed | rejected_needs_inspection | rejected_wildcard | hard_denied,
  approval: not_required | granted | denied,
  exit_code, bytes_freed?, paths_touched[]   // result, when available
}
```

v1 emits this locally (frontend debug channel + stderr) and records the
approval/denial decision — closing the "we only saw the proposed command"
blindness. Backend forwarding (`/events/action` on noah-consumer) and binding
`paths_touched` into the undo journal are the next slice; they reuse the
existing `consumer/client.rs` event channel and `safety::journal`.

## Honoring intent (playbook layer)

"Offload to Drive" is a constraint, not a suggestion. The disk-space playbook
already routes cloud mentions to selective-sync; the rule to reinforce is:
**irreplaceable-but-movable data is offloaded (copy → verify → then delete
local), never straight-deleted; irreplaceable-and-critical data is left alone —
the few GB are not worth it.** This lives in the playbook because it is
judgment; the trees above are the floor the playbook cannot fall through.

## Known limits of the approximation (stated, not hidden)

- A **relative path after an un-tracked `cd`** (`cd ~ && rm -rf Library/…`) is not
  resolved back to home. Mitigated in practice: Noah's shell runs from the app's
  working directory, not the home dir, so a bare `Library/` rarely *is* `~/Library`.
- Only `$HOME` is expanded; arbitrary `$VAR` interpolation is not modelled.
- macOS APFS is case-insensitive by default; we lowercase to match. A
  case-*sensitive* volume could in principle hide a path — rare, accepted.
- `tmutil deletelocalsnapshots` (removing Time Machine local snapshots) is left
  ungated: it is an Apple-sanctioned, auto-regenerating space step. Worth
  revisiting if we want to protect the snapshot safety net during big deletes.

These are the seams. They are documented because an approximation you can see the
edges of is more trustworthy than one that pretends to be total.

## What this deliberately does NOT do

- It does not block the moat: a careful, looked-at deletion still proceeds.
- It does not rely on the model's self-reported `reason` for anything load-bearing.
- It does not try to compute "irreplaceable"; it approximates and says so.
