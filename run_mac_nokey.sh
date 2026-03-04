#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# Clear stored auth so the setup screen appears (for testing onboarding flows)
APP_DIR="$HOME/Library/Application Support/com.itman.app"
rm -f "$APP_DIR/api_key.txt" "$APP_DIR/proxy.json"

unset ANTHROPIC_API_KEY

exec pnpm dev
