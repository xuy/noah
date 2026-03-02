# Noah 🔧

**Your friendly computer helper.** Noah is an open-source desktop app that diagnoses and fixes computer problems for you. Just describe what's wrong in plain English, and Noah will figure out the situation, tell you what he's going to do, and do it — with one click.

Built for people who aren't "computer people." Think: small business owners, your aunt who calls you about her printer, anyone who just wants their stuff to work.

> *"My internet is slow"* → Noah checks your network, finds your iPhone hotspot is available, connects you, and verifies it's working. One button. Done.

## How It Works

1. **You describe the problem** — in your own words, no jargon needed
2. **Noah investigates** — runs diagnostics on your computer silently in the background
3. **Noah tells you what he found** — and what he plans to do about it
4. **You click "Do it"** — Noah handles the rest and confirms the fix

No tickets. No waiting. No confusing menus.

## 🚀 Getting Started

### Option A: Download the App

1. Go to the [Releases](https://github.com/xuy/noah/releases) page
2. Download the latest `.dmg` for macOS
3. Open it and drag Noah to your Applications folder
4. Launch Noah — on first run, it will ask you for your Anthropic API key

> **Note:** Since Noah isn't signed with an Apple Developer certificate yet, macOS will show a warning. To open it: right-click the app → "Open" → "Open" again. You only need to do this once.

### 🔑 Bring Your Own API Key

Noah uses Claude (by Anthropic) to think through your problems. You'll need your own API key:

1. Get a key from [console.anthropic.com](https://console.anthropic.com)
2. When you first launch Noah, paste it in the setup screen — done!

Your key is saved locally on your machine and never shared with anyone except Anthropic's API directly.

**For developers** — you can also set it via environment variable:
```bash
export ANTHROPIC_API_KEY="sk-ant-your-key-here"
```

### Option B: Build from Source

**Prerequisites:** macOS, Node.js (v18+), pnpm, Rust ([rustup.rs](https://rustup.rs))

```bash
git clone https://github.com/xuy/noah.git
cd noah
pnpm install
./run_mac.sh
```

Or manually:

```bash
pnpm install
export ANTHROPIC_API_KEY="your-key"
pnpm dev
```

#### Build for Production

```bash
pnpm build
```

This creates a native `.app` bundle via Tauri.

## 🛠 What Noah Can Do (macOS)

**Network issues**
- Check Wi-Fi status, DNS, connectivity
- Flush DNS cache, test specific hosts
- Connect to hotspots

**Printer problems**
- List printers, check print queues
- Cancel stuck jobs, restart print services

**Slow computer**
- Check memory, CPU, disk usage
- Find and stop runaway processes
- Clear caches to free space

**App issues**
- List installed apps, read app logs
- Clear app caches, move/copy files

**General diagnostics**
- Read system logs
- Run shell commands (safe ones auto-approved, dangerous ones ask you first)
- System summary and health checks

## 🛡 Safety

Noah is careful:

- **Investigates before acting** — always runs read-only diagnostics first
- **Tells you the plan** — you see exactly what Noah wants to do before he does it
- **One-click approval** — "Do it" means do it. No confusing permission dialogs for normal operations
- **Dangerous commands are flagged** — things like `rm`, `sudo`, or disk formatting require explicit approval with a plain-language explanation of why
- **Everything is logged** — every action Noah takes is recorded in a session journal you can review
- **Never touches dangerous stuff** — boot config, firmware, security software, disk partitions, and system integrity protection are off limits. Always.

## Architecture

```
┌─────────────────────────────────────┐
│         React + TypeScript UI       │
│  (Chat, ActionCards, SessionHistory)│
├─────────────────────────────────────┤
│              Tauri 2                │
├─────────────────────────────────────┤
│          Rust Backend               │
│  ┌───────────┐  ┌────────────────┐  │
│  │ Orchestrator│  │  Tool Router  │  │
│  │ (agentic   │  │  (20+ macOS   │  │
│  │  loop)     │  │   tools)      │  │
│  └──────┬─────┘  └───────┬───────┘  │
│         │                │          │
│  ┌──────▼─────┐  ┌───────▼───────┐  │
│  │ Claude API │  │ Local System  │  │
│  │ (thinking) │  │ (executing)   │  │
│  └────────────┘  └───────────────┘  │
├─────────────────────────────────────┤
│     SQLite (session journal)        │
└─────────────────────────────────────┘
```

Key design decision: **The LLM thinks, the local machine acts.** Claude decides what tools to call, but all execution happens locally on your computer via Rust. Your data never leaves your machine (except the conversation with Claude).

## Running Tests

```bash
pnpm test
```

This runs both Rust (`cargo test`) and frontend (`vitest`) test suites.

## 📁 Project Structure

```
apps/desktop/
  src/                  # React frontend
    components/         # ChatPanel, SessionBar, ActionApproval, etc.
    stores/             # Zustand stores (chat, session, debug)
    hooks/              # useSession, useAgent
    lib/                # Tauri command wrappers, response parser
  src-tauri/
    src/
      agent/            # Orchestrator, LLM client, tool router, prompts
      platform/macos/   # All macOS tool implementations
      safety/           # Journal (change logging), safety tiers
      commands/         # Tauri command handlers
crates/
  itman-tools/          # Tool trait, safety tier types, shared types
```

## Contributing

This project is built in public. Issues, ideas, and PRs are welcome.

The codebase is intentionally simple — Rust backend with direct Anthropic API calls, React frontend with Zustand, no ORMs, no complex abstractions. If you can read the code, you can contribute.

## License

MIT
