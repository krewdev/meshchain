#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PIDF="$ROOT/data/host/cloudflared-scanner.pid"
if [[ -f "$PIDF" ]]; then
  kill "$(cat "$PIDF")" 2>/dev/null || true
  rm -f "$PIDF"
  echo "stopped scanner cloudflared tunnel"
else
  pkill -f "cloudflared tunnel --url http://127.0.0.1:8788" 2>/dev/null || true
  echo "no scanner tunnel pidfile (pkill attempted)"
fi
