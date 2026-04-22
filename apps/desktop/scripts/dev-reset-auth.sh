#!/usr/bin/env bash
# Reset Noah desktop state for local dev.
#
# Default (no flags) clears auth-related files in the app data dir:
#   • session.txt (signed-in session token)
#   • entitlement_cache.json
#   • api_key.txt + proxy.json (BYOK and legacy invite-code auth)
#   Keeps journal.db so your past chat history is intact.
#
# --fresh adds:
#   • Archives journal.db as journal.db.bak-<timestamp>
#   • Removes device_id.txt so you get a brand-new anonymous device
#   • Clears WKWebView localStorage under ~/Library/WebKit/<bundle> so
#     flags like noah.firstFixPromptShown don't carry over between runs
#   Simulates a truly first-ever install. Restore journal with:
#       mv journal.db.bak-<ts> journal.db
#
# --launch adds: starts `pnpm tauri dev` with ANTHROPIC_API_KEY unset.
# Combine: --fresh --launch for the full "brand-new user" experience.

set -euo pipefail

APPDIR="$HOME/Library/Application Support/app.onnoah.desktop"
# Tauri 2 WKWebView stores localStorage here, not in APPDIR. --fresh needs
# to nuke this to reset flags like noah.firstFixPromptShown and noah.pendingSeed.
WEBDIR="$HOME/Library/WebKit/app.onnoah.desktop"

FRESH=0
LAUNCH=0
for arg in "$@"; do
  case "$arg" in
    --fresh) FRESH=1 ;;
    --launch) LAUNCH=1 ;;
    *) echo "unknown flag: $arg (valid: --fresh --launch)" >&2; exit 2 ;;
  esac
done

for f in session.txt entitlement_cache.json api_key.txt proxy.json; do
  path="$APPDIR/$f"
  if [[ -f "$path" ]]; then
    rm -f "$path"
    echo "• Removed $path"
  fi
done

if [[ "$FRESH" == "1" ]]; then
  ts="$(date +%s)"
  if [[ -f "$APPDIR/journal.db" ]]; then
    mv "$APPDIR/journal.db" "$APPDIR/journal.db.bak-$ts"
    echo "• Archived journal.db → journal.db.bak-$ts"
  fi
  for s in journal.db-wal journal.db-shm; do
    if [[ -f "$APPDIR/$s" ]]; then
      rm -f "$APPDIR/$s"
    fi
  done
  if [[ -f "$APPDIR/device_id.txt" ]]; then
    rm -f "$APPDIR/device_id.txt"
    echo "• Removed device_id.txt"
  fi
  if [[ -d "$WEBDIR/WebsiteData" ]]; then
    rm -rf "$WEBDIR/WebsiteData"
    echo "• Cleared WKWebView storage (localStorage flags, etc.)"
  fi
  echo "✓ Fresh-install state — journal archived, device_id + localStorage cleared."
else
  echo "✓ Noah auth state cleared (journal.db kept)."
fi

if [[ "$LAUNCH" == "1" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  cd "$SCRIPT_DIR/.."
  echo
  echo "→ Launching dev build with ANTHROPIC_API_KEY unset"
  echo "  (NOAH_CONSUMER_URL=${NOAH_CONSUMER_URL:-http://localhost:8788})"
  echo
  unset ANTHROPIC_API_KEY
  export NOAH_CONSUMER_URL="${NOAH_CONSUMER_URL:-http://localhost:8788}"
  exec pnpm tauri dev
fi
