#!/usr/bin/env bash
# Expose local faucet (and optional host) via Cloudflare quick tunnel — no account required.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${FAUCET_PORT:-8787}"
LOG="$ROOT/data/host/cloudflared-faucet.log"
PIDF="$ROOT/data/host/cloudflared.pid"
mkdir -p "$ROOT/data/host"

# ensure faucet is up
if ! curl -sf "http://127.0.0.1:$PORT/health" >/dev/null; then
  echo "Faucet not running on :$PORT — start host first:"
  echo "  ./scripts/start_testnet_host.sh"
  exit 1
fi

if [[ -f "$PIDF" ]] && kill -0 "$(cat "$PIDF")" 2>/dev/null; then
  echo "cloudflared already running pid=$(cat "$PIDF")"
  grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG" | tail -1 || true
  exit 0
fi

if ! command -v cloudflared >/dev/null; then
  echo "Install cloudflared: brew install cloudflared"
  exit 1
fi

nohup cloudflared tunnel --url "http://127.0.0.1:$PORT" >"$LOG" 2>&1 &
echo $! >"$PIDF"
echo "Starting tunnel… (pid $(cat "$PIDF"))"
for i in $(seq 1 30); do
  URL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG" 2>/dev/null | head -1 || true)
  if [[ -n "$URL" ]]; then
    echo "$URL" | tee "$ROOT/data/host/public_faucet_url.txt"
    # write public config for site consumers
    cat >"$ROOT/data/host/public_endpoints.json" <<EOF
{
  "faucet_api": "$URL",
  "faucet_ui": "https://meshchain-sigma.vercel.app/faucet/",
  "note": "Cloudflare quick tunnel — URL changes when tunnel restarts",
  "testnet": "meshchain-testnet-1"
}
EOF
    # smoke
    curl -sf "$URL/health" && echo " tunnel health OK"
    echo
    echo "Public faucet API: $URL"
    echo "Set this URL on https://meshchain-sigma.vercel.app/faucet/"
    exit 0
  fi
  sleep 1
done
echo "Timed out waiting for tunnel URL — see $LOG"
exit 1
