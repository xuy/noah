#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# ── Parse flags ────────────────────────────────────────────────────────
LOCAL_URL=""
LOCAL_MODEL=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --local)
      LOCAL_URL="${2:-http://127.0.0.1:8082}"
      LOCAL_MODEL="${3:-local}"
      shift; shift 2>/dev/null || true; shift 2>/dev/null || true
      ;;
    *)
      echo "Usage: ./run_mac.sh [--local [URL] [MODEL]]"
      echo "  --local              Use local LLM server (default: http://127.0.0.1:8082, model: local)"
      echo "  --local URL MODEL    Use custom local server URL and model name"
      exit 1
      ;;
  esac
done

if [ -n "$LOCAL_URL" ]; then
  export NOAH_API_URL="$LOCAL_URL"
  export NOAH_MODEL="$LOCAL_MODEL"
  echo "[run_mac] Using local LLM: $NOAH_API_URL (model: $NOAH_MODEL)"
else
  # Load API key
  export ANTHROPIC_API_KEY="$(sed 's/^ANTHROPIC_API_KEY=//' ~/.secrets/claude.txt)"
fi

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
