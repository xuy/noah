---
name: release
description: Bump version, build, and release Noah for the current platform. Handles signing keys, version bumps across all config files, testing, committing, pushing, and uploading to GitHub Releases via the release script.
user-invocable: true
---

# Release Noah

Cut a release: Mac locally (universal binary), Windows + Linux via GitHub Actions.

## Pre-flight

1. Run `cargo test` — abort if any fail.
2. Check `git status` — warn if there are uncommitted changes unrelated to the release.
3. **Lockfile check**: Verify `pnpm-lock.yaml` and `Cargo.lock` are committed and in sync with their respective manifest files. If either is modified/untracked, commit them first. Out-of-sync lockfiles cause `--frozen-lockfile` failures.

## Version bump

The version lives in **three** files that must stay in sync:

- `apps/desktop/src-tauri/Cargo.toml` (`version = "X.Y.Z"`)
- `apps/desktop/src-tauri/tauri.conf.json` (`"version": "X.Y.Z"`)
- `apps/desktop/package.json` (`"version": "X.Y.Z"`)

After editing Cargo.toml, run `cargo check` to update `Cargo.lock`, then stage it too.

Determine the bump type from context:
- **Patch** (0.9.2 -> 0.9.3): bug fixes, minor tweaks
- **Minor** (0.9.2 -> 0.10.0): new features, notable changes
- **Major** (0.9.2 -> 1.0.0): breaking changes, major milestones

If the user specifies a bump type or target version, use that. Otherwise ask.

Update all three files + Cargo.lock, then commit:
```
Bump version to X.Y.Z

<one-line summary of what changed since last release>
```

Push the commit before building (CI needs the latest code).

## Build and upload (Mac — local)

Set signing keys and run the release script:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/noah.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="searchformeaning"
./release.sh --upload --skip-install
```

This builds a **universal macOS binary** (ARM + Intel via `--target universal-apple-darwin`), signs, notarizes, and uploads the `.dmg` + `.tar.gz` (with updater signature) to GitHub Releases. The `latest.json` registers both `darwin-aarch64` and `darwin-x86_64` for the updater.

The release script automatically:
- Installs `x86_64-apple-darwin` Rust target if missing
- Cleans stale bundle artifacts before building
- Output goes to `target/universal-apple-darwin/release/bundle/`

## Build and upload (Windows + Linux — GitHub Actions)

After the Mac build uploads and creates the release, trigger CI for Windows and Linux:

```bash
gh workflow run release.yml --field tag=vX.Y.Z
```

This runs `.github/workflows/release.yml` which builds on both `windows-latest` and `ubuntu-22.04` in parallel, then uploads artifacts (NSIS .exe, MSI .msi, deb, rpm, AppImage) to the same GitHub release.

Monitor progress:
```bash
gh run list --workflow=release.yml --limit 1
gh run watch          # live tail
```

**Signing secrets** (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) are configured as GitHub repo secrets — no manual setup needed per build.

### Fallback: manual Windows build

If CI is broken, Windows can still be built manually:

1. Tell the user to run on the Windows machine: `powershell -File build-windows.ps1`
2. SCP both NSIS (.exe) and MSI (.msi) artifacts back
3. Upload with `gh release upload vX.Y.Z /tmp/Noah_*.exe /tmp/Noah_*.msi --clobber`

## Post-release

1. Push all commits: `git push`
2. Print the release URL for the user.
3. Optionally monitor CI: `gh run list --workflow=release.yml --limit 1`

## CI Notes

- **macOS builds are local only** — macOS CI runners cost 10x (would eat free tier fast).
- **Windows + Linux via GitHub Actions** — `release.yml` is `workflow_dispatch` only (manual trigger). Free tier: 2,000 min/month; a typical release uses ~30 min (10 min Windows ×2 + 10 min Linux ×1).
- `ci.yml` is also `workflow_dispatch` only (test-only, no builds).
- The `--skip-install` flag skips `pnpm install` in the release script (saves time when deps haven't changed). Omit it if dependencies were recently added.
