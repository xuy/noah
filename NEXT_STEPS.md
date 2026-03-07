# Next Steps

1. Install deps: `pnpm install`
2. Run app: `pnpm --filter @itman/desktop tauri dev` (or `./run_mac.sh`)
3. Verify baseline: `cargo test --workspace` and `npx tsc --noEmit`
4. Read code in order:
   - Frontend: `apps/desktop/src/`
   - Tauri commands: `apps/desktop/src-tauri/src/commands/`
   - Agent/tools: `apps/desktop/src-tauri/src/agent/`
5. Pick one small issue and ship one atomic commit (`feat|fix|chore|refactor|docs|test`).

Version-sync reminder before release:
- `apps/desktop/src-tauri/tauri.conf.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `crates/itman-tools/Cargo.toml`
