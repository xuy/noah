# Contributing

Noah is built in public. Issues, ideas, and PRs are welcome.

## Development setup

**Prerequisites:** Node.js (v18+), pnpm, Rust ([rustup.rs](https://rustup.rs))

```bash
git clone https://github.com/xuy/noah.git
cd noah
pnpm install
```

Run in development mode:
```bash
export ANTHROPIC_API_KEY="your-key"   # or paste it in the app's setup screen
pnpm --filter @itman/desktop tauri dev
```

Or on macOS: `./run_mac.sh`

### Build for production

```bash
pnpm release:build
```

This produces platform installers (`.dmg` on macOS, `.msi`/`.exe` on Windows, Linux bundles on Linux).

Shell wrappers are also available:

```bash
./release.sh --build
```

```powershell
.\release.ps1 -BuildOnly
```

### Run tests

```bash
cargo test --workspace          # Rust tests
pnpm --filter @itman/desktop test   # Frontend tests
npx tsc --noEmit                # TypeScript type check
```

## Architecture

```
┌─────────────────────────────────────┐
│         React + TypeScript UI       │
│  (Chat, ActionCards, SessionHistory)│
├─────────────────────────────────────┤
│              Tauri 2                │
├─────────────────────────────────────┤
│          Rust Backend               │
│  ┌─────────────┐  ┌──────────────┐ │
│  │ Orchestrator │  │ Tool Router  │ │
│  │ (agentic     │  │ (40+ tools,  │ │
│  │  loop)       │  │  Mac + Win)  │ │
│  └──────┬───────┘  └──────┬──────┘ │
│         │                 │        │
│  ┌──────▼───────┐  ┌──────▼──────┐ │
│  │  Claude API  │  │ Local System│ │
│  │  (thinking)  │  │ (executing) │ │
│  └──────────────┘  └─────────────┘ │
├─────────────────────────────────────┤
│   SQLite (journal, sessions,       │
│           artifacts/knowledge)     │
└─────────────────────────────────────┘
```

**Key design decision:** The LLM thinks, the local machine acts. Claude decides what tools to call, but all execution happens locally via Rust. Your data never leaves your machine (except the conversation with Claude).

## Project structure

```
apps/desktop/
  src/                    # React frontend (Vite + Tailwind)
    components/           # ChatPanel, SessionBar, ActionApproval, etc.
    stores/               # Zustand stores (chat, session, debug)
    hooks/                # useSession, useAgent
    lib/                  # Tauri command wrappers, response parser
  src-tauri/
    src/
      agent/              # Orchestrator, LLM client, tool router, prompts
      artifacts.rs        # Knowledge persistence (save/query facts across sessions)
      platform/linux/     # Linux tool implementations
      platform/macos/     # macOS tool implementations
      platform/windows/   # Windows tool implementations
      safety/             # Journal (change logging + undo), safety tiers
      commands/           # Tauri command handlers
crates/
  itman-tools/            # Shared Tool trait, SafetyTier types
```

## Code style

- **Rust:** follow existing patterns. `#[cfg]`-gate platform code. Graceful fallback over panics.
- **TypeScript/React:** functional components, Zustand stores, Tailwind classes.
- **No over-engineering.** Minimum code for the current task.

## Commit conventions

- Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `docs:`, `test:`
- One logical change per commit
- Don't commit code that fails `cargo test --workspace` or `npx tsc --noEmit`

## Version and release

Version lives in 4 files — keep them in sync:
- `apps/desktop/src-tauri/tauri.conf.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `crates/itman-tools/Cargo.toml`

Tag format: `v{VERSION}`.

Cross-platform release commands:
- Build locally only: `pnpm release:build`
- Build + upload artifacts to GitHub Release: `pnpm release:upload`

Equivalent shell wrappers:
- Bash: `./release.sh --upload`
- PowerShell: `.\release.ps1 -Upload`
