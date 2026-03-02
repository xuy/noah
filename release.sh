#!/usr/bin/env bash
# Build a release for the current platform and create a GitHub release.
#
# Usage:
#   ./release.sh           # build + upload to the latest git tag
#   ./release.sh --build   # build only, no GitHub release
#
# Prerequisites:
#   - Rust toolchain (rustup)
#   - Node.js + pnpm
#   - gh CLI (for upload step)
#
# On macOS this produces a .dmg; on Windows (run from Git Bash / PowerShell
# with bash) it produces an .msi / .exe installer via NSIS.
set -euo pipefail
cd "$(dirname "$0")"

BUILD_ONLY=false
if [[ "${1:-}" == "--build" ]]; then
  BUILD_ONLY=true
fi

# ── Detect version from tauri.conf.json ──────────────────────────────
VERSION=$(grep '"version"' apps/desktop/src-tauri/tauri.conf.json | head -1 | sed 's/.*"version": *"\([^"]*\)".*/\1/')
TAG="v${VERSION}"
echo "==> Building Noah ${TAG} for $(uname -s)/$(uname -m)"

# ── Install JS dependencies ──────────────────────────────────────────
echo "==> Installing dependencies..."
pnpm install --frozen-lockfile

# ── Build ─────────────────────────────────────────────────────────────
echo "==> Running tauri build..."
pnpm --filter @itman/desktop tauri build

# ── Locate artifacts ──────────────────────────────────────────────────
BUNDLE_DIR="apps/desktop/src-tauri/target/release/bundle"
ARTIFACTS=()

# macOS .dmg
for f in "$BUNDLE_DIR"/dmg/*.dmg; do
  [ -f "$f" ] && ARTIFACTS+=("$f")
done

# macOS .app.tar.gz (updater)
for f in "$BUNDLE_DIR"/macos/*.tar.gz; do
  [ -f "$f" ] && ARTIFACTS+=("$f")
done

# Windows .msi
for f in "$BUNDLE_DIR"/msi/*.msi; do
  [ -f "$f" ] && ARTIFACTS+=("$f")
done

# Windows NSIS .exe
for f in "$BUNDLE_DIR"/nsis/*.exe; do
  [ -f "$f" ] && ARTIFACTS+=("$f")
done

# Linux .deb / .AppImage
for f in "$BUNDLE_DIR"/deb/*.deb; do
  [ -f "$f" ] && ARTIFACTS+=("$f")
done
for f in "$BUNDLE_DIR"/appimage/*.AppImage; do
  [ -f "$f" ] && ARTIFACTS+=("$f")
done

if [ ${#ARTIFACTS[@]} -eq 0 ]; then
  echo "ERROR: No build artifacts found in $BUNDLE_DIR"
  exit 1
fi

echo "==> Artifacts:"
printf '    %s\n' "${ARTIFACTS[@]}"

if $BUILD_ONLY; then
  echo "==> Build-only mode — skipping GitHub release."
  exit 0
fi

# ── Upload to GitHub release ──────────────────────────────────────────
echo "==> Uploading to GitHub release ${TAG}..."

# Create the release if it doesn't exist yet
if ! gh release view "$TAG" >/dev/null 2>&1; then
  gh release create "$TAG" \
    --title "Noah ${TAG}" \
    --generate-notes
fi

# Upload all artifacts
gh release upload "$TAG" "${ARTIFACTS[@]}" --clobber

echo "==> Done! Release: $(gh release view "$TAG" --json url -q .url)"
