//! Destructive-action safety policy — the enforced redline layer.
//!
//! See `apps/desktop/src-tauri/docs/safety-policy.md` for the full rationale.
//! In short: deletions inside *protected trees* must be **inspected before
//! deleted** (the Claude Code read-before-edit precedent), sweeps over those
//! trees (wildcards, or deleting the tree root / an ancestor) are rejected
//! outright, and a tiny *hard-deny* floor refuses a handful of machine-ending
//! actions even when reaffirmed.
//!
//! This module is **pure and deterministic**: it parses a command string and a
//! set of already-inspected paths, and returns a verdict. State (the
//! inspected-set) lives in the orchestrator, which mirrors the harness model —
//! the tool stays stateless, the harness holds the gate.
//!
//! ## Quality of the approximation
//!
//! The whole value here is that the approximation is *good*. Verified facts
//! baked into this module:
//!
//! - **Firmlinks**: every user path is also addressable under
//!   `/System/Volumes/Data` (confirmed same inode on a live machine; see
//!   `/usr/share/firmlinks`). We strip that prefix so the gate can't be
//!   side-stepped by spelling the path the long way.
//! - **Protected trees** track Apple's TCC-protected locations (Desktop,
//!   Documents, Downloads, Mail, Messages, Safari, Contacts, Calendars,
//!   Photos) plus credential stores (`~/.ssh`, `~/.gnupg`, cloud creds).
//! - **Deletion vectors**: `rm`, `unlink`, and `find` (with `-delete`,
//!   `-exec rm`, or piped to `xargs rm`) are all classified as deletes.
//!
//! Known, documented limits (consistent with "approximation, stated"): a
//! relative path after an un-tracked `cd` is not resolved to home; and we do
//! not model `$VAR` expansion beyond `$HOME`.

use std::collections::HashSet;

/// A protected location. `content` = true marks user-authored content trees
/// (Documents, Photos…) where a cache/log name must NOT earn the regenerable
/// exemption — a folder literally named "cache" under `~/Documents` is still
/// the user's. `content` = false marks app-state/Library trees where
/// cache/log subdirs are genuinely regenerable.
pub struct ProtectedTree {
    pub path: &'static str,
    pub content: bool,
}

const fn app(path: &'static str) -> ProtectedTree {
    ProtectedTree {
        path,
        content: false,
    }
}
const fn content(path: &'static str) -> ProtectedTree {
    ProtectedTree {
        path,
        content: true,
    }
}

/// Protected trees — an approximation of "irreplaceable user data / app state."
/// Tilde-rooted; matching expands `~` against the home dir. Aligned with macOS
/// TCC-protected locations + credential stores.
pub const PROTECTED_TREES: &[ProtectedTree] = &[
    // App state / Library (regenerable cache subdirs are exempt).
    app("~/Library/Application Support"),
    app("~/Library/Containers"),
    app("~/Library/Group Containers"),
    app("~/Library/Messages"),
    app("~/Library/Mail"),
    app("~/Library/Safari"),
    app("~/Library/Calendars"),
    app("~/Library/Mobile Documents"), // iCloud Drive
    app("~/Library/CloudStorage"),     // Google Drive / Dropbox / OneDrive mounts
    app("~/Library/Photos"),
    // Credential / key stores (gated; Keychain itself is hard-deny).
    app("~/.ssh"),
    app("~/.gnupg"),
    app("~/.aws"),
    app("~/.kube"),
    app("~/.docker"),
    app("~/.config"),
    // User content (no regenerable exemption).
    content("~/Documents"),
    content("~/Desktop"),
    content("~/Pictures"),
    content("~/Movies"),
    content("~/Music"),
    content("~/Downloads"),
];

/// Path segments that mark regenerable data (caches/logs). A delete strictly
/// inside an *app-state* protected tree whose path contains one of these is
/// treated as regenerable → not gated. Lowercased substring match.
pub const REGENERABLE_HINTS: &[&str] = &["/caches/", "/cache/", "/logs/", "/.cache/"];

/// Read-class command leaders that count as *inspection* (when not used
/// destructively). Running one against a path records it as inspected.
const INSPECT_LEADERS: &[&str] = &[
    "ls", "du", "find", "stat", "cat", "file", "tree", "head", "tail", "wc", "grep",
];

/// Firmlink prefixes that re-address user data. Stripping these prevents the
/// gate from being side-stepped via the long path. Lowercased.
const FIRMLINK_PREFIXES: &[&str] = &["/system/volumes/data"];

/// The verdict the harness acts on.
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// Not a protected-tree delete (or it's regenerable). Proceed to normal flow.
    Allow,
    /// The command deletes, but not in a form the harness can fully analyse
    /// (a pipe, `find`/`xargs`, command substitution, a variable, `eval`, or a
    /// relative path). **Fail-closed**: re-express as a plain `rm`/`unlink` on
    /// an absolute or `~`-rooted path. The harness only runs deletions it can read.
    RejectNonCanonical { tip: String },
    /// Concrete delete of a specific path inside a protected tree that hasn't
    /// been inspected. Carries a tip instructing the model to inspect first.
    RejectNeedsInspection { path: String, tip: String },
    /// Unbounded sweep over a protected tree — a wildcard, or deleting the tree
    /// root / an ancestor of it. Never auto-clears; the model must enumerate.
    RejectSweep { tree: String, tip: String },
    /// Machine-ending / identity-destroying action. Refused even if reaffirmed.
    HardDeny { reason: String },
}

impl GateDecision {
    pub fn is_rejection(&self) -> bool {
        !matches!(self, GateDecision::Allow)
    }

    pub fn classification(&self) -> &'static str {
        match self {
            GateDecision::Allow => "allow",
            GateDecision::RejectNonCanonical { .. } => "non_canonical",
            GateDecision::RejectNeedsInspection { .. } => "inspect_then_delete",
            GateDecision::RejectSweep { .. } => "reject_sweep",
            GateDecision::HardDeny { .. } => "hard_deny",
        }
    }

    pub fn message(&self) -> String {
        match self {
            GateDecision::Allow => String::new(),
            GateDecision::RejectNonCanonical { tip } => tip.clone(),
            GateDecision::RejectNeedsInspection { tip, .. } => tip.clone(),
            GateDecision::RejectSweep { tip, .. } => tip.clone(),
            GateDecision::HardDeny { reason } => reason.clone(),
        }
    }
}

// ── Normalisation ────────────────────────────────────────────────────────

/// Expand a leading `~` or `$HOME` against `home`. Leaves other paths untouched.
fn expand_home(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", home, rest)
    } else if path == "~" {
        home.to_string()
    } else if let Some(rest) = path.strip_prefix("$HOME/") {
        format!("{}/{}", home, rest)
    } else if path == "$HOME" {
        home.to_string()
    } else {
        path.to_string()
    }
}

/// Strip a firmlink prefix (`/System/Volumes/Data`) so the long form of a user
/// path matches the same tree as the short form. Operates on a lowercased path.
fn strip_firmlink(lower: &str) -> String {
    for prefix in FIRMLINK_PREFIXES {
        if let Some(rest) = lower.strip_prefix(prefix) {
            if rest.is_empty() {
                return "/".to_string();
            }
            if rest.starts_with('/') {
                return rest.to_string();
            }
        }
    }
    lower.to_string()
}

/// Collapse `.` / `..` path segments after home expansion and firmlink
/// stripping. Destructive operands containing dot segments are rejected before
/// this, but normalising here keeps comparisons stable for inspected paths.
fn collapse_dot_segments(path: &str) -> String {
    if !path.starts_with('/') {
        return path.to_string();
    }

    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(seg),
        }
    }

    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

/// Normalise for comparison: expand home, lowercase (macOS default volumes are
/// case-insensitive), strip the firmlink prefix, drop a trailing slash. Root
/// `/` is preserved (not collapsed to empty).
fn norm(path: &str, home: &str) -> String {
    let expanded = expand_home(path, home).to_lowercase();
    let stripped = strip_firmlink(&expanded);
    let stripped = collapse_dot_segments(&stripped);
    let trimmed = stripped.trim_end_matches('/');
    if trimmed.is_empty() && stripped.starts_with('/') {
        return "/".to_string();
    }
    trimmed.to_string()
}

/// True if `child` is `ancestor` or a descendant of it (both pre-normalised).
fn is_within(child: &str, ancestor: &str) -> bool {
    if ancestor == "/" {
        return child.starts_with('/');
    }
    child == ancestor || child.starts_with(&format!("{}/", ancestor))
}

fn has_dot_segment(path: &str) -> bool {
    path.split('/').any(|seg| seg == "." || seg == "..")
}

fn has_glob(seg: &str) -> bool {
    seg.contains('*') || seg.contains('?') || seg.contains('[')
}

/// Split a path into the leading portion before the first glob-bearing segment,
/// plus whether any glob was present. `~/Foo/*` -> (`~/Foo`, true);
/// `~/Foo/App*/x` -> (`~/Foo`, true); `~/Foo/Bar` -> (`~/Foo/Bar`, false).
fn base_and_glob(path: &str) -> (String, bool) {
    let mut kept: Vec<&str> = Vec::new();
    let mut glob = false;
    // Preserve a leading "/" by tracking it separately.
    let leading_slash = path.starts_with('/');
    for seg in path.split('/') {
        if seg.is_empty() {
            continue;
        }
        if has_glob(seg) {
            glob = true;
            break;
        }
        kept.push(seg);
    }
    let joined = kept.join("/");
    let base = if leading_slash {
        format!("/{}", joined)
    } else if path.starts_with('~') || path.starts_with('$') {
        // keep the ~ / $HOME marker intact for expand_home downstream
        if joined.is_empty() {
            kept.first().map(|s| s.to_string()).unwrap_or_default()
        } else {
            joined
        }
    } else {
        joined
    };
    (base, glob)
}

// ── Tokenising ─────────────────────────────────────────────────────────────

/// Tokenise a single shell command honouring backslash-escaped spaces
/// (`Application\ Support`) and single/double quotes. Pragmatic, classification-
/// only; errs toward keeping a path whole so we don't under-detect a target.
fn tokenize(cmd: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut chars = cmd.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut started = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single => {
                if let Some(&next) = chars.peek() {
                    cur.push(next);
                    started = true;
                    chars.next();
                }
            }
            '\'' if !in_double => {
                in_single = !in_single;
                started = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                started = true;
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if started {
                    tokens.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        tokens.push(cur);
    }
    tokens
}

/// Split a compound command on `;`, newline, `&&`, `||`, `|` into simple parts.
fn split_commands(cmd: &str) -> Vec<String> {
    cmd.split(|c| c == ';' || c == '\n')
        .flat_map(|s| s.split("&&"))
        .flat_map(|s| s.split("||"))
        .flat_map(|s| s.split('|'))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_assignment(tok: &str) -> bool {
    let Some((name, _)) = tok.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
        && !name.as_bytes()[0].is_ascii_digit()
}

fn skip_sudo(toks: &[String], mut idx: usize) -> usize {
    if toks.get(idx).map(|s| s.as_str()) != Some("sudo") {
        return idx;
    }
    idx += 1;
    while let Some(tok) = toks.get(idx) {
        if !tok.starts_with('-') {
            break;
        }
        let needs_arg = matches!(
            tok.as_str(),
            "-u" | "--user"
                | "-g"
                | "--group"
                | "-h"
                | "--host"
                | "-p"
                | "--prompt"
                | "-C"
                | "--close-from"
                | "-D"
                | "--chdir"
        );
        idx += 1;
        if needs_arg && toks.get(idx).is_some() {
            idx += 1;
        }
    }
    idx
}

fn skip_command_prefixes(toks: &[String], mut idx: usize) -> usize {
    loop {
        while toks.get(idx).is_some_and(|tok| is_assignment(tok)) {
            idx += 1;
        }

        match toks.get(idx).map(|s| s.as_str()) {
            Some("sudo") => {
                idx = skip_sudo(toks, idx);
            }
            Some("command" | "builtin" | "noglob" | "nocorrect" | "time" | "nohup") => {
                idx += 1;
            }
            Some("env") => {
                idx += 1;
                while let Some(tok) = toks.get(idx) {
                    if tok.starts_with('-') || is_assignment(tok) {
                        idx += 1;
                    } else {
                        break;
                    }
                }
            }
            Some("nice") => {
                idx += 1;
                while let Some(tok) = toks.get(idx) {
                    if tok == "-n" || tok == "--adjustment" {
                        idx += 1;
                        if toks.get(idx).is_some() {
                            idx += 1;
                        }
                    } else if tok.starts_with('-') {
                        idx += 1;
                    } else {
                        break;
                    }
                }
            }
            Some("arch") => {
                idx += 1;
                while toks.get(idx).is_some_and(|tok| tok.starts_with('-')) {
                    idx += 1;
                }
            }
            _ => break,
        }
    }
    idx
}

/// Leader of a simple command, with common shell prefixes normalised.
fn leader_and_args(part: &str) -> (String, Vec<String>) {
    let toks = tokenize(part);
    let idx = skip_command_prefixes(&toks, 0);
    let leader = toks.get(idx).cloned().unwrap_or_default();
    let args = toks.get(idx + 1..).map(|s| s.to_vec()).unwrap_or_default();
    (leader, args)
}

/// Raw leader after `sudo`, before other wrappers. Used to reject wrapped
/// deletions rather than silently canonicalising them.
fn leader_after_sudo(part: &str) -> (String, Vec<String>) {
    let toks = tokenize(part);
    let idx = skip_sudo(&toks, 0);
    let leader = toks.get(idx).cloned().unwrap_or_default();
    let args = toks.get(idx + 1..).map(|s| s.to_vec()).unwrap_or_default();
    (leader, args)
}

/// Non-flag operands (for `rm`/`unlink`).
fn path_operands(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect()
}

/// Leading path operands of a `find` (before the first predicate/flag/`(`/`!`).
fn find_roots(args: &[String]) -> Vec<String> {
    let mut roots = Vec::new();
    for a in args {
        if a.starts_with('-') || a == "(" || a == "!" {
            break;
        }
        roots.push(a.clone());
    }
    roots
}

/// Does this `find` arg list delete (`-delete`, `-exec rm/unlink`, `-execdir …`)?
fn find_args_destructive(args: &[String]) -> bool {
    for (i, a) in args.iter().enumerate() {
        if a == "-delete" {
            return true;
        }
        if a == "-exec" || a == "-execdir" {
            if let Some(next) = args.get(i + 1) {
                let n = next.trim_start_matches("./");
                if n == "rm" || n == "unlink" || n == "srm" {
                    return true;
                }
            }
        }
    }
    false
}

fn shell_script_arg(args: &[String]) -> Option<&str> {
    for (i, arg) in args.iter().enumerate() {
        if arg == "-c" || arg == "-lc" {
            return args.get(i + 1).map(|s| s.as_str());
        }
    }
    None
}

fn is_shell_leader(leader: &str) -> bool {
    matches!(leader, "sh" | "bash" | "zsh")
}

fn is_delete_leader(leader: &str) -> bool {
    matches!(leader, "rm" | "unlink" | "srm" | "shred" | "rmdir")
}

/// Whole-command: is there an `xargs` that runs `rm`/`unlink`? (the `find | xargs
/// rm` vector). Used to treat preceding `find` roots as delete targets.
fn has_xargs_rm(cmd: &str) -> bool {
    for part in split_commands(cmd) {
        let (leader, args) = leader_and_args(&part);
        if leader == "xargs" {
            if args.iter().any(|a| {
                let n = a.trim_start_matches("./");
                n == "rm" || n == "unlink" || n == "srm"
            }) {
                return true;
            }
        }
    }
    false
}

/// All raw path operands a command would delete, across every vector. Not
/// normalised — caller normalises as needed.
fn delete_targets_raw(cmd: &str) -> Vec<String> {
    let xargs_rm = has_xargs_rm(cmd);
    let mut out = Vec::new();
    for part in split_commands(cmd) {
        let (leader, args) = leader_and_args(&part);
        match leader.as_str() {
            "rm" | "unlink" | "srm" => out.extend(path_operands(&args)),
            "find" => {
                if find_args_destructive(&args) || xargs_rm {
                    out.extend(find_roots(&args));
                }
            }
            _ => {}
        }
    }
    out
}

// ── Hard-deny floor ──────────────────────────────────────────────────────

/// Machine-ending or identity-destroying actions, refused even with inspection
/// and reaffirmation. Returns the refusal reason.
pub fn hard_denied(cmd: &str, home: &str) -> Option<String> {
    let home_n = norm(home, home);
    for op in delete_targets_raw(cmd) {
        let (base, _) = base_and_glob(&op);
        let n = norm(&base, home);
        // root / system / whole-Users / home-root wipes
        if n == "/"
            || n == "/system"
            || n.starts_with("/system/")
            || n == "/users"
            || n == "/usr"
            || n.starts_with("/usr/")
            || n == "/library"
            || n == home_n
        {
            return Some(format!(
                "Refused: `{}` would wipe the operating system or your entire home \
                 folder. This is a hard limit — Noah will not run it even if asked. \
                 To free space, target specific caches and large files instead.",
                op
            ));
        }
        // auth / identity stores
        if n.starts_with(&format!("{}/library/keychains", home_n))
            || n == format!("{}/library/keychains", home_n)
            || n.contains("/com.apple.tcc")
            || n.contains("com.apple.security")
        {
            return Some(
                "Refused: this targets your Keychain / security identity — deleting \
                 it would lock you out of saved passwords and app logins, \
                 irrecoverably. This is a hard limit."
                    .to_string(),
            );
        }
    }
    // secure-erase of a disk
    for part in split_commands(cmd) {
        let (leader, args) = leader_and_args(&part);
        if leader == "diskutil" {
            let joined = args.join(" ").to_lowercase();
            if joined.contains("erasedisk")
                || joined.contains("erasevolume")
                || joined.contains("securerase")
                || joined.contains("zerodisk")
            {
                return Some(
                    "Refused: erasing a disk/volume is irreversible and outside what \
                     storage cleanup should ever do. This is a hard limit."
                        .to_string(),
                );
            }
        }
    }
    None
}

// ── Canonical-form allowlist (fail-closed) ───────────────────────────────

/// True if the command performs (or could launder) a deletion at all. When this
/// is false we don't care about the command. When true, it must be canonical.
fn mentions_deletion(cmd: &str) -> bool {
    let xargs_rm = has_xargs_rm(cmd);
    for part in split_commands(cmd) {
        let (leader, args) = leader_and_args(&part);
        match leader.as_str() {
            leader if is_delete_leader(leader) => return true,
            "find" if find_args_destructive(&args) || xargs_rm => return true,
            "xargs" if xargs_rm => return true,
            "eval" => return true,
            // pipe-to-shell laundering (`… | sh`)
            "sh" | "bash" | "zsh" if has_pipe(cmd) => return true,
            leader if is_shell_leader(leader) => {
                if shell_script_arg(&args).is_some_and(|script| mentions_deletion(script)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// A `|` pipe that is not part of `||`.
fn has_pipe(cmd: &str) -> bool {
    cmd.replace("||", "\u{0}\u{0}").contains('|')
}

/// Is this token an acceptable `rm` flag? Short flags only, from a safe set;
/// long flags (`--no-preserve-root`) and unknown flags are rejected.
fn valid_rm_flag(a: &str) -> bool {
    a.len() > 1
        && a.starts_with('-')
        && !a.starts_with("--")
        && a[1..].chars().all(|c| "rRfidvPW".contains(c))
}

/// An operand the harness can resolve to a concrete location: absolute, or
/// `~`/`$HOME`-rooted. Globs are allowed (the base is still concrete). Relative
/// paths, other variables, and command substitution are not.
fn canonical_operand(op: &str) -> bool {
    if op.contains("$(") || op.contains('`') {
        return false;
    }
    if has_dot_segment(op) {
        return false;
    }
    if op.starts_with('/') || op == "~" || op.starts_with("~/") {
        return true;
    }
    if op == "$HOME" || op.starts_with("$HOME/") {
        return true;
    }
    if op.contains('$') {
        return false; // some other variable
    }
    false // relative
}

fn tip_noncanonical(what: &str) -> String {
    format!(
        "Held back: this delete uses {} — a form Noah can't fully read before \
         running, so it won't run it. Re-express it so the target is statically \
         visible: a plain `rm -rf <path>` / `unlink <path>`, or a `find <path> … \
         -delete` / `-exec rm` whose root is an absolute or `~`-rooted path — no \
         pipes, no `xargs`, no `$(...)`, no variables, no relative paths. Inspect \
         first if it's inside a protected folder.",
        what
    )
}

fn operand_why(op: &str) -> String {
    let why = if op.contains("$(") || op.contains('`') {
        "command substitution"
    } else if op.contains('$') {
        "a shell variable"
    } else if has_dot_segment(op) {
        "a `.` or `..` path segment"
    } else {
        "a relative path"
    };
    format!("{} (`{}`)", why, op)
}

/// Why (if at all) a deletion command is not in canonical form. Permits
/// `rm`/`unlink` on literal paths, and a **non-piped** `find <literal-root> …
/// -delete/-exec rm` (its root is statically checkable; predicates only narrow
/// what's removed). Rejects pipes, `xargs`, substitution, variables, relative
/// paths, `eval`, and the secure/dir deleters.
fn canonical_violation(cmd: &str) -> Option<String> {
    if has_pipe(cmd) {
        return Some(tip_noncanonical("a pipe"));
    }
    if cmd.contains("$(") || cmd.contains('`') {
        return Some(tip_noncanonical("command substitution `$(...)`"));
    }
    for part in split_commands(cmd) {
        let (raw_leader, _) = leader_after_sudo(&part);
        let (leader, args) = leader_and_args(&part);
        if raw_leader != leader {
            let wrapped_delete = is_delete_leader(&leader)
                || (leader == "find" && find_args_destructive(&args))
                || (is_shell_leader(&leader)
                    && shell_script_arg(&args).is_some_and(|script| mentions_deletion(script)));
            if wrapped_delete {
                return Some(tip_noncanonical(&format!("the wrapper `{}`", raw_leader)));
            }
        }
        match leader.as_str() {
            "rm" | "unlink" => {
                for a in &args {
                    if a.starts_with('-') && (leader == "unlink" || !valid_rm_flag(a)) {
                        return Some(tip_noncanonical(&format!("the flag `{}`", a)));
                    }
                }
                for op in path_operands(&args) {
                    if !canonical_operand(&op) {
                        return Some(tip_noncanonical(&operand_why(&op)));
                    }
                }
            }
            "find" if find_args_destructive(&args) => {
                let roots = find_roots(&args);
                if roots.is_empty() {
                    return Some(tip_noncanonical(
                        "a `find` with no explicit path (it would default to the current directory)",
                    ));
                }
                for r in &roots {
                    if !canonical_operand(r) {
                        return Some(tip_noncanonical(&format!(
                            "a `find` root that isn't an absolute/`~`-rooted path ({})",
                            operand_why(r)
                        )));
                    }
                }
            }
            // Secure/dir deleters and indirection: funnel to the plain forms.
            "srm" | "shred" | "rmdir" | "xargs" | "eval" => {
                return Some(tip_noncanonical(&format!("`{}`", leader)));
            }
            leader if is_shell_leader(leader) => {
                if shell_script_arg(&args).is_some_and(|script| mentions_deletion(script)) {
                    return Some(tip_noncanonical(&format!(
                        "a nested `{}` shell script",
                        leader
                    )));
                }
            }
            _ => {}
        }
    }
    None
}

// ── The gate ─────────────────────────────────────────────────────────────

/// Classify a command against the inspected-set and return the gate decision.
///
/// `inspected` holds normalised paths recorded by prior read-class observations.
pub fn gate_decision(cmd: &str, home: &str, inspected: &HashSet<String>) -> GateDecision {
    if let Some(reason) = hard_denied(cmd, home) {
        return GateDecision::HardDeny { reason };
    }

    // Fail-closed allowlist: a command that deletes must do so in a form the
    // harness can fully analyse, or it doesn't run.
    if mentions_deletion(cmd) {
        if let Some(tip) = canonical_violation(cmd) {
            return GateDecision::RejectNonCanonical { tip };
        }
    }

    for op in delete_targets_raw(cmd) {
        let (base, is_glob) = base_and_glob(&op);
        let nbase = norm(&base, home);
        let nop = norm(&op, home);

        for tree in PROTECTED_TREES {
            let ntree = norm(tree.path, home);
            let within = is_within(&nbase, &ntree); // base at-or-below tree
            let contains = is_within(&ntree, &nbase); // base is tree or its ancestor
            if !within && !contains {
                continue;
            }

            // Regenerable cache/log strictly inside an app-state tree → not gated.
            let strictly_inside = within && !contains;
            if strictly_inside && !tree.content && REGENERABLE_HINTS.iter().any(|h| nop.contains(h))
            {
                break; // this operand is fine; move to next operand
            }

            // Unbounded: a wildcard, or deleting the tree root / an ancestor.
            if is_glob || contains {
                return GateDecision::RejectSweep {
                    tree: ntree.clone(),
                    tip: format!(
                        "Held back: `{}` would remove a protected folder's entire \
                         contents (or the folder itself) — an unbounded delete that \
                         includes data you may not be able to get back. Instead: \
                         inspect it (e.g. `du -sh '{}'/*`), then delete the specific \
                         subdirectories you've confirmed are safe, one explicit path \
                         at a time.",
                        op, ntree
                    ),
                };
            }

            // Concrete path inside a protected tree: cleared only if inspected.
            let cleared = inspected
                .iter()
                .any(|i| is_within(i, &ntree) && is_within(&nbase, i));
            if !cleared {
                return GateDecision::RejectNeedsInspection {
                    path: nbase.clone(),
                    tip: format!(
                        "Held back: `{}` is inside a protected folder and Noah hasn't \
                         looked at it yet. Inspect it first (e.g. `ls -la '{}'` or \
                         `du -sh '{}'`), confirm what it is and that it's safe to \
                         remove, then retry this exact delete.",
                        op, op, op
                    ),
                };
            }
            break; // cleared for this tree; next operand
        }
    }

    GateDecision::Allow
}

/// Paths a command inspects, normalised, to fold into the session inspected-set.
/// Empty unless the command is a non-destructive read-class observation.
pub fn inspected_paths(cmd: &str, home: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in split_commands(cmd) {
        let (leader, args) = leader_and_args(&part);
        if !INSPECT_LEADERS.contains(&leader.as_str()) {
            continue;
        }
        // A destructive find is a delete, not a look.
        if leader == "find" && (find_args_destructive(&args) || has_xargs_rm(cmd)) {
            continue;
        }
        let operands = if leader == "find" {
            find_roots(&args)
        } else {
            path_operands(&args)
        };
        for op in operands {
            // Record the directory being observed; strip a trailing glob segment
            // (`du -sh ~/Foo/*` means you saw ~/Foo's children).
            let (base, _) = base_and_glob(&op);
            let n = norm(&base, home);
            if !n.is_empty() && n != "/" {
                out.push(n);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/fbob";

    fn set(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|p| norm(p, HOME)).collect()
    }
    fn empty() -> HashSet<String> {
        HashSet::new()
    }

    // ── The incident commands: every one held back ───────────────────────

    #[test]
    fn incident_wildcard_app_support() {
        assert!(matches!(
            gate_decision("rm -rf ~/Library/Application\\ Support/*", HOME, &empty()),
            GateDecision::RejectSweep { .. }
        ));
    }
    #[test]
    fn incident_sudo_wildcard_app_support() {
        assert!(matches!(
            gate_decision(
                "sudo rm -rf ~/Library/Application\\ Support/*",
                HOME,
                &empty()
            ),
            GateDecision::RejectSweep { .. }
        ));
    }
    #[test]
    fn incident_wildcard_containers() {
        assert!(matches!(
            gate_decision("rm -rf ~/Library/Containers/*", HOME, &empty()),
            GateDecision::RejectSweep { .. }
        ));
    }
    #[test]
    fn incident_messages_container_specific() {
        assert!(matches!(
            gate_decision(
                "rm -rf ~/Library/Containers/com.apple.MobileSMS",
                HOME,
                &empty()
            ),
            GateDecision::RejectNeedsInspection { .. }
        ));
    }
    #[test]
    fn incident_messages_attachments_wildcard() {
        assert!(matches!(
            gate_decision("rm -rf ~/Library/Messages/Attachments/*", HOME, &empty()),
            GateDecision::RejectSweep { .. }
        ));
    }
    #[test]
    fn incident_group_containers_wildcard() {
        assert!(matches!(
            gate_decision("rm -rf ~/Library/Group\\ Containers/*", HOME, &empty()),
            GateDecision::RejectSweep { .. }
        ));
    }

    // ── Firmlink bypass (the long path) ──────────────────────────────────

    #[test]
    fn firmlink_wildcard_rejected() {
        // The exact long-form prefix the real trace used for du.
        let d = gate_decision(
            "rm -rf /System/Volumes/Data/Users/fbob/Library/Application\\ Support/*",
            HOME,
            &empty(),
        );
        assert!(matches!(d, GateDecision::RejectSweep { .. }), "{:?}", d);
    }
    #[test]
    fn firmlink_specific_rejected_uninspected() {
        let d = gate_decision(
            "rm -rf /System/Volumes/Data/Users/fbob/Library/Application\\ Support/Adobe",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNeedsInspection { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn firmlink_inspection_clears_firmlink_delete() {
        // Inspect via short path, delete via long path → still cleared (same inode).
        let inspected = set(&["~/Library/Application Support/Adobe"]);
        let d = gate_decision(
            "rm -rf /System/Volumes/Data/Users/fbob/Library/Application\\ Support/Adobe",
            HOME,
            &inspected,
        );
        assert_eq!(d, GateDecision::Allow);
    }

    // ── Ancestor / whole-tree sweeps ─────────────────────────────────────

    #[test]
    fn deleting_library_ancestor_rejected() {
        // ~/Library is an ancestor of every protected tree.
        let d = gate_decision("rm -rf ~/Library", HOME, &empty());
        assert!(matches!(d, GateDecision::RejectSweep { .. }), "{:?}", d);
    }
    #[test]
    fn deleting_whole_tree_root_rejected_even_inspected() {
        let inspected = set(&["~/Library/Application Support"]);
        let d = gate_decision("rm -rf ~/Library/Application\\ Support", HOME, &inspected);
        assert!(matches!(d, GateDecision::RejectSweep { .. }), "{:?}", d);
    }
    #[test]
    fn deleting_home_is_hard_deny() {
        assert!(matches!(
            gate_decision("rm -rf ~", HOME, &empty()),
            GateDecision::HardDeny { .. }
        ));
        assert!(matches!(
            gate_decision("rm -rf /Users/fbob", HOME, &empty()),
            GateDecision::HardDeny { .. }
        ));
    }
    #[test]
    fn root_globs_are_hard_denied() {
        for cmd in [
            "rm -rf /*",
            "rm -rf /Users/*",
            "rm -rf /System/Volumes/Data/*",
            "rm -rf /System/Volumes/Data/Users/fbob/*",
        ] {
            assert!(
                matches!(
                    gate_decision(cmd, HOME, &empty()),
                    GateDecision::HardDeny { .. }
                ),
                "should hard-deny root/system glob: {cmd}"
            );
        }
    }

    // ── Visible-root find: permitted, then gated on the root ─────────────

    #[test]
    fn find_delete_on_tree_root_is_sweep() {
        // Non-piped find with a checkable root → flows to the tree gate; root
        // IS the protected tree → sweep.
        let d = gate_decision(
            "find ~/Library/Application\\ Support -maxdepth 1 -type d -delete",
            HOME,
            &empty(),
        );
        assert!(matches!(d, GateDecision::RejectSweep { .. }), "{:?}", d);
    }
    #[test]
    fn find_exec_rm_on_specific_subdir_is_inspect_gated() {
        let d = gate_decision(
            "find ~/Library/Containers/com.apple.MobileSMS -exec rm -rf {} +",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNeedsInspection { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn find_delete_in_unprotected_age_filtered_allowed() {
        // The bundled-recipe shape: age-filtered cleanup of a non-protected dir.
        assert_eq!(
            gate_decision("find ~/Library/Logs -mtime +7 -delete", HOME, &empty()),
            GateDecision::Allow
        );
        assert_eq!(
            gate_decision(
                "find /Applications -maxdepth 1 -name 'Install macOS*' -mtime +14 -exec rm -rf {} +",
                HOME,
                &empty()
            ),
            GateDecision::Allow
        );
    }
    #[test]
    fn find_pipe_xargs_rm_is_non_canonical() {
        // The pipe is the disqualifier — a grep/sed could diverge the set.
        let d = gate_decision(
            "find ~/Library/Application\\ Support -name '*' | xargs rm -rf",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn find_with_relative_root_is_non_canonical() {
        for cmd in ["find . -name '*.tmp' -delete", "find Library -delete"] {
            assert!(
                matches!(
                    gate_decision(cmd, HOME, &empty()),
                    GateDecision::RejectNonCanonical { .. }
                ),
                "should reject relative find root: {cmd}"
            );
        }
    }
    #[test]
    fn find_with_no_explicit_root_is_non_canonical() {
        let d = gate_decision("find -name '*.log' -delete", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn rmdir_shred_srm_are_non_canonical() {
        for cmd in [
            "rmdir ~/Library/Application\\ Support/Adobe",
            "shred -u ~/Documents/x",
            "srm -rf ~/Documents/x",
        ] {
            assert!(
                matches!(
                    gate_decision(cmd, HOME, &empty()),
                    GateDecision::RejectNonCanonical { .. }
                ),
                "should be non-canonical: {cmd}"
            );
        }
    }
    #[test]
    fn unlink_canonical_then_gated() {
        // unlink IS a canonical leader; the path is then gated by the tree logic.
        let d = gate_decision("unlink ~/Documents/taxes.pdf", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNeedsInspection { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn nondestructive_find_is_not_a_delete() {
        assert_eq!(
            gate_decision(
                "find ~/Library/Application\\ Support -name '*.log'",
                HOME,
                &empty()
            ),
            GateDecision::Allow
        );
    }
    #[test]
    fn destructive_find_does_not_count_as_inspection() {
        assert!(inspected_paths("find ~/Library/Application\\ Support -delete", HOME).is_empty());
    }

    // ── Fail-closed: indirection & relative paths the harness can't read ──

    #[test]
    fn relative_path_delete_is_non_canonical() {
        // The cd-then-relative bypass.
        let d = gate_decision(
            "cd ~ && rm -rf Library/Application\\ Support/Adobe",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn command_substitution_delete_is_non_canonical() {
        let d = gate_decision("rm -rf $(cat ~/to-delete.txt)", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn variable_operand_is_non_canonical() {
        let d = gate_decision("rm -rf $TARGET/cache", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn pipe_in_delete_is_non_canonical() {
        let d = gate_decision("ls ~/Library | rm -rf ~/Library/Caches", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn eval_is_non_canonical() {
        let d = gate_decision(
            "eval \"rm -rf ~/Library/Application Support\"",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn pipe_to_shell_is_non_canonical() {
        let d = gate_decision("echo cm0gLXJmIH4v | base64 -d | sh", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn no_preserve_root_flag_is_non_canonical() {
        let d = gate_decision("rm -rf --no-preserve-root /tmp/x", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNonCanonical { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn wrapped_deletes_are_non_canonical() {
        for cmd in [
            "command rm -rf ~/Library/Application\\ Support/Adobe",
            "env FOO=1 rm -rf ~/Library/Application\\ Support/Adobe",
            "VAR=1 rm -rf ~/Library/Application\\ Support/Adobe",
            "nice -n 10 rm -rf ~/Library/Application\\ Support/Adobe",
        ] {
            assert!(
                matches!(
                    gate_decision(cmd, HOME, &empty()),
                    GateDecision::RejectNonCanonical { .. }
                ),
                "should reject wrapped delete: {cmd}"
            );
        }
    }
    #[test]
    fn sudo_option_delete_still_hits_redline() {
        let d = gate_decision(
            "sudo -u root rm -rf ~/Library/Application\\ Support/Adobe",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNeedsInspection { .. }),
            "{:?}",
            d
        );
    }
    #[test]
    fn nested_shell_deletes_are_non_canonical() {
        for cmd in [
            "bash -c 'rm -rf ~/Library/Application Support/Adobe'",
            "zsh -lc 'find ~/Library/Application Support -delete'",
            "sudo bash -c 'rm -rf /'",
        ] {
            assert!(
                matches!(
                    gate_decision(cmd, HOME, &empty()),
                    GateDecision::RejectNonCanonical { .. }
                ),
                "should reject nested shell delete: {cmd}"
            );
        }
    }
    #[test]
    fn dot_segment_operands_are_non_canonical() {
        for cmd in [
            "rm -rf ~/Library/Caches/../Application\\ Support/*",
            "rm -rf /Users/fbob/Library/./Messages/Attachments",
            "find ~/Library/Caches/../Application\\ Support -delete",
        ] {
            assert!(
                matches!(
                    gate_decision(cmd, HOME, &empty()),
                    GateDecision::RejectNonCanonical { .. }
                ),
                "should reject dot-segment operand: {cmd}"
            );
        }
    }
    #[test]
    fn canonical_absolute_rm_outside_protected_allowed() {
        // The blessed form, in a safe location → just runs.
        assert_eq!(
            gate_decision("rm -rf /tmp/build-cache", HOME, &empty()),
            GateDecision::Allow
        );
    }

    // ── New protected trees ──────────────────────────────────────────────

    #[test]
    fn ssh_keys_gated() {
        assert!(matches!(
            gate_decision("rm -rf ~/.ssh", HOME, &empty()),
            GateDecision::RejectSweep { .. } // whole-tree root
        ));
        assert!(matches!(
            gate_decision("rm -f ~/.ssh/id_ed25519", HOME, &empty()),
            GateDecision::RejectNeedsInspection { .. }
        ));
    }
    #[test]
    fn downloads_and_safari_and_calendars_gated() {
        for cmd in [
            "rm -rf ~/Downloads/old",
            "rm -rf ~/Library/Safari/History.db",
            "rm -rf ~/Library/Calendars/x",
        ] {
            assert!(
                gate_decision(cmd, HOME, &empty()).is_rejection(),
                "should gate: {cmd}"
            );
        }
    }

    // ── "Be my guest": inspected → allowed ───────────────────────────────

    #[test]
    fn specific_app_delete_allowed_after_inspecting_it() {
        let inspected = set(&["~/Library/Application Support/Adobe"]);
        assert_eq!(
            gate_decision(
                "rm -rf ~/Library/Application\\ Support/Adobe",
                HOME,
                &inspected
            ),
            GateDecision::Allow
        );
    }
    #[test]
    fn specific_app_delete_allowed_after_inspecting_parent_tree() {
        let inspected = set(&["~/Library/Application Support"]);
        assert_eq!(
            gate_decision(
                "rm -rf ~/Library/Application\\ Support/Adobe",
                HOME,
                &inspected
            ),
            GateDecision::Allow
        );
    }
    #[test]
    fn wildcard_never_clears_even_after_inspection() {
        let inspected = set(&["~/Library/Application Support"]);
        assert!(matches!(
            gate_decision("rm -rf ~/Library/Application\\ Support/*", HOME, &inspected),
            GateDecision::RejectSweep { .. }
        ));
    }
    #[test]
    fn inspecting_home_does_not_clear_protected_child() {
        let inspected = set(&["~"]);
        assert!(matches!(
            gate_decision(
                "rm -rf ~/Library/Application\\ Support/Adobe",
                HOME,
                &inspected
            ),
            GateDecision::RejectNeedsInspection { .. }
        ));
    }
    #[test]
    fn unrolled_sequence_rejected_at_first_uninspected() {
        let d = gate_decision(
            "rm -rf ~/Library/Application\\ Support/Adobe; rm -rf ~/Library/Application\\ Support/obs-studio",
            HOME,
            &empty(),
        );
        assert!(
            matches!(d, GateDecision::RejectNeedsInspection { .. }),
            "{:?}",
            d
        );
    }

    // ── Regenerable: scoped to app trees only ────────────────────────────

    #[test]
    fn caches_wildcard_outside_protected_allowed() {
        assert_eq!(
            gate_decision("rm -rf ~/Library/Caches/*", HOME, &empty()),
            GateDecision::Allow
        );
    }
    #[test]
    fn app_support_cache_subdir_allowed() {
        assert_eq!(
            gate_decision(
                "rm -rf ~/Library/Application\\ Support/Foo/Caches/blobs",
                HOME,
                &empty()
            ),
            GateDecision::Allow
        );
    }
    #[test]
    fn cache_named_folder_under_documents_not_exempt() {
        // The over-broad-hint hole: a user folder literally named "cache".
        let d = gate_decision("rm -rf ~/Documents/cache/report", HOME, &empty());
        assert!(
            matches!(d, GateDecision::RejectNeedsInspection { .. }),
            "{:?}",
            d
        );
    }

    // ── Non-protected and non-delete: untouched ──────────────────────────

    #[test]
    fn delete_outside_protected_tree_allowed() {
        assert_eq!(
            gate_decision("rm -rf ~/.npm/_cacache", HOME, &empty()),
            GateDecision::Allow
        );
    }
    #[test]
    fn non_rm_command_allowed() {
        assert_eq!(
            gate_decision("du -sh ~/Library/Application\\ Support/*", HOME, &empty()),
            GateDecision::Allow
        );
    }

    // ── Hard-deny floor ──────────────────────────────────────────────────

    #[test]
    fn rm_root_hard_denied() {
        assert!(matches!(
            gate_decision("rm -rf /", HOME, &empty()),
            GateDecision::HardDeny { .. }
        ));
    }
    #[test]
    fn rm_system_hard_denied() {
        assert!(matches!(
            gate_decision("sudo rm -rf /System", HOME, &empty()),
            GateDecision::HardDeny { .. }
        ));
    }
    #[test]
    fn firmlink_root_hard_denied() {
        assert!(matches!(
            gate_decision("rm -rf /System/Volumes/Data/Users/fbob", HOME, &empty()),
            GateDecision::HardDeny { .. }
        ));
    }
    #[test]
    fn keychain_hard_denied_even_if_inspected() {
        let inspected = set(&["~/Library/Keychains"]);
        assert!(matches!(
            gate_decision("rm -rf ~/Library/Keychains", HOME, &inspected),
            GateDecision::HardDeny { .. }
        ));
    }
    #[test]
    fn diskutil_erase_hard_denied() {
        assert!(matches!(
            gate_decision("diskutil eraseDisk APFS Blank disk0", HOME, &empty()),
            GateDecision::HardDeny { .. }
        ));
    }

    // ── inspected_paths extraction ───────────────────────────────────────

    #[test]
    fn inspected_paths_records_du_target() {
        assert_eq!(
            inspected_paths("du -sh ~/Library/Application\\ Support/Adobe", HOME),
            vec![norm("~/Library/Application Support/Adobe", HOME)]
        );
    }
    #[test]
    fn inspected_paths_strips_trailing_wildcard() {
        assert_eq!(
            inspected_paths("du -sh ~/Library/Application\\ Support/*", HOME),
            vec![norm("~/Library/Application Support", HOME)]
        );
    }
    #[test]
    fn inspected_paths_handles_firmlink() {
        assert_eq!(
            inspected_paths(
                "ls -la /System/Volumes/Data/Users/fbob/Library/Containers/com.apple.MobileSMS",
                HOME
            ),
            vec![norm("~/Library/Containers/com.apple.MobileSMS", HOME)]
        );
    }
    #[test]
    fn inspected_paths_empty_for_rm() {
        assert!(inspected_paths("rm -rf ~/Library/Caches/*", HOME).is_empty());
    }

    // ── End-to-end careful path ──────────────────────────────────────────

    #[test]
    fn careful_flow_inspect_then_delete_succeeds() {
        let mut inspected: HashSet<String> = HashSet::new();
        let d1 = gate_decision(
            "rm -rf ~/Library/Application\\ Support/Adobe",
            HOME,
            &inspected,
        );
        assert!(matches!(d1, GateDecision::RejectNeedsInspection { .. }));

        for p in inspected_paths("du -sh ~/Library/Application\\ Support/Adobe", HOME) {
            inspected.insert(p);
        }

        let d2 = gate_decision(
            "rm -rf ~/Library/Application\\ Support/Adobe",
            HOME,
            &inspected,
        );
        assert_eq!(d2, GateDecision::Allow);
    }
}
