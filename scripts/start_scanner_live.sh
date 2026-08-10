#!/usr/bin/env bash
# Point the Vercel scanner UI at a live API.
#
# Default: stable public seed  https://34.172.103.125.sslip.io
# Lab/laptop: FORCE_TUNNEL=1 ./scripts/start_scanner_live.sh
# Override:   SCANNER_PUBLIC_URL=https://host ./scripts/start_scanner_live.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

STABLE_SCANNER="${SCANNER_PUBLIC_URL:-https://34.172.103.125.sslip.io}"
SCANNER_PORT="${SCANNER_PORT:-8788}"
LOG="$ROOT/data/host/cloudflared-scanner.log"
PIDF="$ROOT/data/host/cloudflared-scanner.pid"
URL_FILE="$ROOT/data/host/public_scanner_url.txt"
mkdir -p "$ROOT/data/host"

if [[ "${FORCE_TUNNEL:-}" == "1" ]]; then
  # Lab path: local scanner + ephemeral Cloudflare tunnel
  if ! curl -sf "http://127.0.0.1:$SCANNER_PORT/api/v1/status" >/dev/null; then
    echo "Local scanner not on :$SCANNER_PORT — starting testnet host…"
    if [[ ! -x "$ROOT/target/release/meshchain-scanner" && ! -x "$ROOT/target/debug/meshchain-scanner" ]]; then
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
    exit 1
  fi

  if [[ -f "$PIDF" ]] && kill -0 "$(cat "$PIDF")" 2>/dev/null; then
    URL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG" 2>/dev/null | tail -1 || true)
  fi
  if [[ -z "${URL:-}" ]]; then
    command -v cloudflared >/dev/null || { echo "Install cloudflared: brew install cloudflared"; exit 1; }
    rm -f "$PIDF"
    : >"$LOG"
    nohup cloudflared tunnel --url "http://127.0.0.1:$SCANNER_PORT" >"$LOG" 2>&1 &
    echo $! >"$PIDF"
    URL=""
    for _ in $(seq 1 40); do
      URL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG" 2>/dev/null | head -1 || true)
      [[ -n "$URL" ]] && break
      sleep 1
    done
  fi
  [[ -n "${URL:-}" ]] || { echo "Timed out waiting for tunnel URL — see $LOG"; exit 1; }
else
  URL="$STABLE_SCANNER"
fi

echo "$URL" | tee "$URL_FILE"
echo "Public live API: $URL"

if curl -sf "$URL/api/v1/status" >/dev/null; then
  echo "Scanner health OK"
else
  echo "WARN: $URL/api/v1/status not serving yet"
fi

# Write public endpoints
cat >"$ROOT/data/host/public_endpoints.json" <<EOF
{
  "scanner_api": "$URL",
  "scanner_ui": "https://meshchain-sigma.vercel.app/scanner/",
  "scanner_ui_live": "https://meshchain-sigma.vercel.app/scanner/?api=$URL",
  "faucet_api": "https://faucet.34.172.103.125.sslip.io",
  "note": "Default live_api is the public seed. FORCE_TUNNEL=1 writes a temporary CF URL.",
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
    "live_api set by scripts/start_scanner_live.sh",
    "Default is the stable public seed (sslip.io). FORCE_TUNNEL=1 for a local CF tunnel.",
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
if [[ "${FORCE_TUNNEL:-}" == "1" ]]; then
  echo "Keep this machine awake. Tunnel dies if laptop sleeps."
  echo "Stop tunnel: ./scripts/stop_scanner_live.sh"
fi
