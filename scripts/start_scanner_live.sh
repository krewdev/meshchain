#!/usr/bin/env bash
# Option 1: Live API for Vercel scanner auto-update.
# 1) Ensure host + local scanner on :8788
# 2) Cloudflare quick tunnel → public HTTPS URL
# 3) Write live_api into web/scanner/data/config.json
# 4) Deploy Vercel so the public UI polls the live API
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SCANNER_PORT="${SCANNER_PORT:-8788}"
LOG="$ROOT/data/host/cloudflared-scanner.log"
PIDF="$ROOT/data/host/cloudflared-scanner.pid"
URL_FILE="$ROOT/data/host/public_scanner_url.txt"
mkdir -p "$ROOT/data/host"

# Ensure local scanner is up
if ! curl -sf "http://127.0.0.1:$SCANNER_PORT/api/v1/status" >/dev/null; then
  echo "Local scanner not on :$SCANNER_PORT — starting testnet host (validators + faucet + scanner)…"
  if [[ ! -x "$ROOT/target/release/meshchain-scanner" && ! -x "$ROOT/target/debug/meshchain-scanner" ]]; then
    echo "Building release binaries…"
    cargo build -p meshchain-node -p meshchain-scanner --release
  fi
  if [[ ! -f "$ROOT/data/host/v0/genesis.json" ]]; then
    ./scripts/host_bootstrap.sh
  fi
  ./scripts/start_testnet_host.sh
  sleep 2
fi

if ! curl -sf "http://127.0.0.1:$SCANNER_PORT/api/v1/status" >/dev/null; then
  echo "ERROR: scanner still not responding on :$SCANNER_PORT"
  echo "Check: $ROOT/data/host/logs/scanner.log"
  exit 1
fi
echo "Local scanner OK on :$SCANNER_PORT"

# Reuse existing tunnel if alive
if [[ -f "$PIDF" ]] && kill -0 "$(cat "$PIDF")" 2>/dev/null; then
  URL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG" 2>/dev/null | tail -1 || true)
  if [[ -n "$URL" ]]; then
    echo "Tunnel already running: $URL"
  fi
fi

if [[ -z "${URL:-}" ]]; then
  if ! command -v cloudflared >/dev/null; then
    echo "Install cloudflared: brew install cloudflared"
    exit 1
  fi
  # stop old dead pid
  rm -f "$PIDF"
  : >"$LOG"
  nohup cloudflared tunnel --url "http://127.0.0.1:$SCANNER_PORT" >"$LOG" 2>&1 &
  echo $! >"$PIDF"
  echo "Starting Cloudflare tunnel… (pid $(cat "$PIDF"))"
  URL=""
  for i in $(seq 1 40); do
    URL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG" 2>/dev/null | head -1 || true)
    if [[ -n "$URL" ]]; then
      break
    fi
    sleep 1
  done
fi

if [[ -z "$URL" ]]; then
  echo "Timed out waiting for tunnel URL — see $LOG"
  exit 1
fi

echo "$URL" | tee "$URL_FILE"
echo "Public live API: $URL"

# Smoke
if curl -sf "$URL/api/v1/status" >/dev/null; then
  echo "Tunnel health OK"
else
  echo "WARN: tunnel URL not serving yet (may need a few seconds)"
fi

# Write public endpoints
cat >"$ROOT/data/host/public_endpoints.json" <<EOF
{
  "scanner_api": "$URL",
  "scanner_ui": "https://meshchain-sigma.vercel.app/scanner/",
  "scanner_ui_live": "https://meshchain-sigma.vercel.app/scanner/?api=$URL",
  "faucet_api": "http://127.0.0.1:8787",
  "note": "Cloudflare quick tunnel URL changes when tunnel restarts — re-run this script",
  "testnet": "meshchain-testnet-1"
}
EOF

# Point Vercel static site at live API
mkdir -p "$ROOT/web/scanner/data"
python3 - "$URL" "$ROOT/web/scanner/data/config.json" <<'PY'
import json, sys
from pathlib import Path
url, path = sys.argv[1], Path(sys.argv[2])
cfg = {}
if path.exists():
    try:
        cfg = json.loads(path.read_text())
    except Exception:
        cfg = {}
cfg["live_api"] = url
cfg["poll_secs"] = int(cfg.get("poll_secs") or 15)
cfg["fallback_to_snapshot"] = True
cfg["updated"] = __import__("time").strftime("%Y-%m-%dT%H:%MZ", __import__("time").gmtime())
cfg["notes"] = [
    "live_api set by scripts/start_scanner_live.sh (Cloudflare tunnel)",
    "Vercel UI polls this URL every poll_secs — no redeploy needed for chain changes",
    "Tunnel URL changes on restart — re-run start_scanner_live.sh",
]
path.write_text(json.dumps(cfg, indent=2) + "\n")
print("wrote", path)
PY

# Also sync snapshot as fallback
if [[ -f "$ROOT/data/host/v0/chain_state.json" ]]; then
  ./scripts/sync_scanner_snapshot.sh "$ROOT/data/host/v0/chain_state.json" || true
elif [[ -f "$ROOT/data/chain_state.json" ]]; then
  ./scripts/sync_scanner_snapshot.sh "$ROOT/data/chain_state.json" || true
fi

echo
echo "Deploying Vercel with live_api=$URL …"
if command -v vercel >/dev/null; then
  vercel --prod --yes
else
  echo "vercel CLI not found — commit web/scanner/data/config.json and push, or run: vercel --prod"
fi

echo
echo "╔════════════════════════════════════════════════════════╗"
echo "║  LIVE SCANNER READY                                    ║"
echo "╠════════════════════════════════════════════════════════╣"
echo "║  Public UI:  https://meshchain-sigma.vercel.app/scanner/║"
echo "║  Live API:   $URL"
echo "║  Direct UI:  https://meshchain-sigma.vercel.app/scanner/?api=$URL"
echo "╚════════════════════════════════════════════════════════╝"
echo
echo "Keep this machine awake. Tunnel dies if laptop sleeps."
echo "Stop tunnel: ./scripts/stop_scanner_live.sh"
