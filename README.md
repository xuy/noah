<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <img src="https://img.shields.io/badge/key-bring%20your%20own-5b8def.svg" alt="Bring your own key">
</p>

<p align="center">
  <strong>English</strong> | <a href="docs/README.es.md">Español</a> | <a href="docs/README.ja.md">日本語</a> | <a href="docs/README.zh-CN.md">中文</a>
</p>

# Noah for Tinkerers

**Let an AI fix your computer — without letting it run wild.** Noah for Tinkerers is the
open-source, bring-your-own-key build of Noah: a desktop app that diagnoses and resolves
computer problems in plain English, with a safety harness around every action.

You bring an Anthropic API key; Noah talks to Claude directly from your machine. No
subscription, no account, no backend.

> ### 👉 Just want it to work?
> **[onnoah.app](https://onnoah.app)** is the commercial version — the same app, fully
> managed. No API key, no setup, nothing to configure: download, describe the problem, done.
> Free to start. **This repo is the BYOK build** for tinkerers who'd rather bring their own
> key, read the source, and hack on it.

> **Why not just point Claude or Codex at your machine?** Raw models have no guardrails —
> they'll happily `rm -rf` the wrong thing — and sandboxed tools can't touch the real system
> you're trying to fix. Noah is the middle path: a real desktop agent that runs read-only
> diagnostics first, shows you the plan, gates destructive actions behind explicit approval,
> and keeps boot config, firmware, and security software permanently off-limits.

<p align="center">
  <img src="docs/images/noah-hero.png" width="800" alt="Noah diagnosing a slow computer, finding runaway processes, and fixing the issue in one click" />
</p>
<p align="center"><i>You say "my computer is slow." Noah finds the problem, explains the fix, and handles it.</i></p>

## How it works

1. **Describe the problem** — in your own words, no jargon needed
2. **Noah investigates** — runs diagnostics silently in the background
3. **Noah shows you the plan** — what it found and what it wants to do
4. **You approve** — Noah handles the rest and confirms the fix

Every action is logged. Dangerous operations require your explicit approval. Noah never touches boot config, firmware, security software, or system-protected files.

## Beyond chat: Health, Playbooks, Auto-Heal

Noah isn't just a chatbot — it monitors your machine and can fix problems before you notice them.

- **Health Scorecards** — background checks across five categories (Security, Updates, Performance, Backups, Network) grade your machine A–F, with one-click fixes for what's failing.
- **Playbooks** — 25+ built-in Markdown remediation scripts (disk recovery, network diagnostics, printer repair, VPN, backups, browser security, performance forensics, and more). Drop in your own and Noah runs them as guided or automated fixes.
- **Auto-Heal** — when enabled, Noah triages failing checks, picks the right playbook, runs it, and measures the result in the background. Your machine fixes itself.

## What Noah can do

| Category | Mac | Windows |
|---|---|---|
| **Network** — DNS, connectivity, flush cache, test hosts | Yes | Yes |
| **Printers** — queue, cancel jobs, restart print service | Yes | Yes |
| **Performance** — CPU/memory/disk, stop runaway processes | Yes | Yes |
| **Apps** — logs, clear caches, troubleshoot crashes | Yes | Yes |
| **System** — diagnostics, health checks, shell commands | Yes | Yes |
| **Updates** — detect stale OS, troubleshoot stuck updates | Yes | Yes |
| **Security** — firewall, encryption, endpoint checks | Yes | Yes |
| **Backups** — Time Machine status, backup verification | Yes | — |
| **Knowledge** — remembers your system, past fixes, preferences | Yes | Yes |
| **Health Scorecards** — continuous monitoring with A-F grades | Yes | Yes |
| **Playbooks** — guided and automated remediation | Yes | Yes |
| **Auto-Heal** — background self-repair on failing checks | Yes | Yes |

## Get started

### Download

Grab the BYOK build from the [Releases page](https://github.com/xuy/noah/releases):
- **macOS** — `.dmg` (universal, Apple Silicon + Intel)
- **Windows** — `.msi` / `.exe` (x64)
- **Linux** — `.AppImage`

Noah keeps itself up to date after install.

> Don't want to manage an API key? The managed version at **[onnoah.app](https://onnoah.app)**
> is the same app with nothing to set up — download and go.

### Bring your own Anthropic key

Open **Settings** and paste an Anthropic key (it starts with `sk-ant-`) — or set
`ANTHROPIC_API_KEY` before launching. Your key is stored in your system keychain, stays on
your machine, and is used only for direct calls to Anthropic. Noah for Tinkerers has no
backend of its own.

## Safety

- **Looks before it leaps** — always runs read-only diagnostics first
- **Shows you the plan** — you see exactly what Noah will do before it does it
- **Flags risky actions** — `rm`, `sudo`, disk formatting, and similar commands require explicit approval with a plain-language explanation
- **Logs everything** — every action is recorded in a session journal you can review
- **Hard limits** — boot config, firmware, security software, disk partitions, and system integrity protection are permanently off-limits
- **Credentials stay local** — API keys and secrets stay on your device and are never sent to any server

## Open source & contributing

Noah for Tinkerers is built in public. Issues, ideas, and pull requests are welcome — see
[CONTRIBUTING.md](CONTRIBUTING.md) to get a dev build running in a couple of minutes.

## License

Apache-2.0 — see [LICENSE](LICENSE).

---

*For development setup, architecture, and contributing guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).*
