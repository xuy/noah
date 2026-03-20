<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
</p>

<p align="center">
  <strong>English</strong> | <a href="docs/README.es.md">Español</a> | <a href="docs/README.ja.md">日本語</a> | <a href="docs/README.zh-CN.md">中文</a>
</p>

# Noah

**IT support that actually fixes things.** Noah is a desktop app that diagnoses and resolves computer problems in plain English — then keeps your machine healthy automatically.

No tickets. No hold music. No Googling error codes.

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

## Beyond chat: Health and Playbooks

Noah isn't just a chatbot. It continuously monitors your machine and can fix problems before you notice them.

### Health Scorecards

Noah runs background health checks across five categories — **Security**, **Updates**, **Performance**, **Backups**, and **Network** — and gives your machine a score (A through F). Open the Health tab to see what's passing, what's failing, and one-click fixes for each issue.

### Playbooks

Playbooks are step-by-step remediation scripts written in Markdown. Noah ships with 25+ built-in playbooks covering common IT problems:

- Disk space recovery
- Network diagnostics
- Printer repair
- Email setup
- VPN troubleshooting
- Backup configuration
- Browser security audit
- Performance forensics
- And more

Playbooks can also be pushed to your machine by your IT team through fleet management. Noah runs them automatically or shows them as guided fixes.

### Auto-Heal

When enabled, Noah automatically triages failing health checks, picks the right playbook, runs it, and measures the result — all in the background. Your machine fixes itself.

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

Go to [Releases](https://github.com/xuy/noah/releases) and grab the latest:
- **macOS** — `.dmg` (Apple Silicon)
- **Windows** — `.msi` or `.exe` installer (x64)

> **macOS note:** Noah isn't signed with an Apple Developer certificate yet. Right-click the app, click "Open", then "Open" again. One-time only.

### API key

Noah uses Claude (by Anthropic) to reason through problems. You need an API key:

1. Get one at [console.anthropic.com](https://console.anthropic.com)
2. Paste it on Noah's setup screen — done

Your key stays on your machine. It's only used to talk to Anthropic's API directly.

### Fleet enrollment (optional)

If your IT team uses Noah Fleet, they'll give you an enrollment link. Paste it into Noah's Health tab to connect your machine to their dashboard. This lets your IT team monitor health scores, push playbooks, and resolve issues remotely — while you keep full control of your device.

## Safety

- **Looks before it leaps** — always runs read-only diagnostics first
- **Shows you the plan** — you see exactly what Noah will do before it does it
- **Flags risky actions** — `rm`, `sudo`, disk formatting, and similar commands require explicit approval with a plain-language explanation
- **Logs everything** — every action is recorded in a session journal you can review
- **Hard limits** — boot config, firmware, security software, disk partitions, and system integrity protection are permanently off-limits
- **Credentials stay local** — API keys and secrets are stored in your system keychain, never sent to the LLM

## License

Apache-2.0

---

*For development setup, architecture, and contributing guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).*
