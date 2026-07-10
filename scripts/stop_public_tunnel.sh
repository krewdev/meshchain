#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PIDF="$ROOT/data/host/cloudflared.pid"
if [[ -f "$PIDF" ]]; then
  kill "$(cat "$PIDF")" 2>/dev/null || true
  rm -f "$PIDF"
  echo "stopped cloudflared tunnel"
else
  pkill -f "cloudflared tunnel --url" 2>/dev/null || true
  echo "no pidfile; attempted pkill"
fi
