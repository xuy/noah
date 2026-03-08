#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# Load API key
export ANTHROPIC_API_KEY="$(sed 's/^ANTHROPIC_API_KEY=//' ~/.secrets/claude.txt)"

# ── Find a free port for Vite ───────────────────────────────────────
BASE_PORT=1420
PORT=$BASE_PORT

while lsof -iTCP:"$PORT" -sTCP:LISTEN -t &>/dev/null; do
  echo "[run_mac] Port $PORT in use, trying next..."
  PORT=$((PORT + 2))  # +2 because HMR uses port+1
done

if [ "$PORT" -ne "$BASE_PORT" ]; then
  echo "[run_mac] Using port $PORT (default $BASE_PORT was busy)"
fi

export VITE_PORT="$PORT"

# Run tauri dev directly so we can pass --config without pnpm mangling args
cd apps/desktop
exec pnpm tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:$PORT\"}}"
