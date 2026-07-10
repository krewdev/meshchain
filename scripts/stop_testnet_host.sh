#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PIDFILE="$ROOT/data/host/host.pids"
if [[ -f "$PIDFILE" ]]; then
  while read -r pid; do
    kill "$pid" 2>/dev/null || true
  done <"$PIDFILE"
  rm -f "$PIDFILE"
  echo "stopped host processes"
else
  echo "no pidfile; pkill meshchain-node / faucet if needed"
  pkill -f "meshchain-node run" 2>/dev/null || true
  pkill -f "faucet_server.py" 2>/dev/null || true
fi
