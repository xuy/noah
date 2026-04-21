#!/usr/bin/env bash
# Reset Noah desktop auth state for local dev, so the next launch shows
# the SignInScreen as a fresh user would.
#
# Clears:
#   • macOS Keychain session token (service app.onnoah.noah, account session_token)
#   • entitlement_cache.json (server-side state snapshot)
#   • api_key.txt + proxy.json (BYOK and legacy invite-code auth)
#
# Keeps: journal.db, knowledge/, playbooks/, machine_context.json, etc.
#
# Usage:
#   ./scripts/dev-reset-auth.sh            # just wipe
#   ./scripts/dev-reset-auth.sh --launch   # wipe then start `pnpm tauri dev`
#                                          # with ANTHROPIC_API_KEY unset so
#                                          # `load_auth` falls through to
#                                          # the new sign-in flow.

set -euo pipefail

APPDIR="$HOME/Library/Application Support/app.onnoah.desktop"
KEYCHAIN_SERVICE="app.onnoah.noah"
KEYCHAIN_ACCOUNT="session_token"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script is macOS-only (Keychain via /usr/bin/security)." >&2
  exit 1
fi

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

echo "✓ Noah auth state cleared."

if [[ "${1:-}" == "--launch" ]]; then
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
