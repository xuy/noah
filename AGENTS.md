# AGENTS.md

## Commit Policy
- **Commit after every meaningful change.** Bug fix, feature, refactor — commit it immediately. Don't batch unrelated changes.
- Use conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `docs:`, `test:`.
- Keep commits atomic — one logical change per commit.

## Code Style
- Rust: follow existing patterns. `#[cfg]`-gate platform code. Graceful fallback over panics.
- TypeScript/React: functional components, zustand stores, Tailwind classes.
- No over-engineering. Minimum code for the current task.

## Testing
- Run `cargo test --workspace` after Rust changes.
- Run `npx tsc --noEmit` after frontend changes.
- Don't commit code that fails tests or type-checking.

## Version & Release
- Version lives in 4 files — keep in sync: `tauri.conf.json`, `apps/desktop/package.json`, `apps/desktop/src-tauri/Cargo.toml`, `crates/itman-tools/Cargo.toml`.
- Tag format: `v{VERSION}`. Use `./release.sh` to build + publish.

## Project Structure
- `apps/desktop/src/` — React frontend (Vite + Tailwind)
- `apps/desktop/src-tauri/src/` — Rust backend (Tauri 2)
- `crates/itman-tools/` — shared tool traits
- Platform tools: `src-tauri/src/platform/{macos,windows}/`
