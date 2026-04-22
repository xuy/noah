#!/usr/bin/env bash
# Reset Noah desktop state for local dev.
#
# Default (no flags):
#   • Keychain session token
#   • entitlement_cache.json
#   • api_key.txt + proxy.json (BYOK and legacy invite-code auth)
#   Keeps journal.db so your past chat history is intact when you sign in again.
#
# --fresh adds:
#   • Archives journal.db as journal.db.bak-<timestamp>
#   • Archives the device_id in the Keychain so you get a brand-new device
#   This simulates a truly first-ever install so you can preview the
#   TilePicker onboarding. Restore with:
#       mv journal.db.bak-<ts> journal.db
#
# --launch adds: starts `pnpm tauri dev` with ANTHROPIC_API_KEY unset.
# Combine: --fresh --launch for the full "brand-new user" experience.

set -euo pipefail

APPDIR="$HOME/Library/Application Support/app.onnoah.desktop"
KEYCHAIN_SERVICE="app.onnoah.noah"
KEYCHAIN_ACCOUNT="session_token"
KEYCHAIN_DEVICE_ACCOUNT="device_id"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script is macOS-only (Keychain via /usr/bin/security)." >&2
  exit 1
fi

FRESH=0
LAUNCH=0
for arg in "$@"; do
  case "$arg" in
    --fresh) FRESH=1 ;;
    --launch) LAUNCH=1 ;;
    *) echo "unknown flag: $arg (valid: --fresh --launch)" >&2; exit 2 ;;
  esac
done

echo "• Deleting Keychain entry ${KEYCHAIN_SERVICE}/${KEYCHAIN_ACCOUNT} (if present)..."
if security delete-generic-password -s "$KEYCHAIN_SERVICE" -a "$KEYCHAIN_ACCOUNT" 2>/dev/null; then
  echo "  ✓ removed"
else
  echo "  (none — nothing to delete)"
fi

for f in entitlement_cache.json api_key.txt proxy.json; do
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
  # Archive WAL/SHM side-files too so SQLite starts truly clean.
  for s in journal.db-wal journal.db-shm; do
    if [[ -f "$APPDIR/$s" ]]; then
      rm -f "$APPDIR/$s"
    fi
  done
  if security delete-generic-password -s "$KEYCHAIN_SERVICE" -a "$KEYCHAIN_DEVICE_ACCOUNT" 2>/dev/null; then
    echo "• Removed Keychain device_id"
  fi
  echo "✓ Fresh-install state — journal archived, device_id cleared."
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
