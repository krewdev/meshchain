#!/usr/bin/env bash
# Always-on local testnet: 3 validators + faucet (no Docker required).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

NODE="${MESHCHAIN_BIN:-$ROOT/target/release/meshchain-node}"
if [[ ! -x "$NODE" ]]; then
  NODE="$ROOT/target/debug/meshchain-node"
fi
if [[ ! -x "$NODE" ]]; then
  echo "Build first: cargo build -p meshchain-node --release"
  echo "Or: ./scripts/host_bootstrap.sh"
  exit 1
fi

HOST_DATA="$ROOT/data/host"
if [[ ! -f "$HOST_DATA/v0/genesis.json" ]]; then
  ./scripts/host_bootstrap.sh
fi

LOG="$HOST_DATA/logs"
mkdir -p "$LOG"
PIDFILE="$HOST_DATA/host.pids"
: >"$PIDFILE"

start_one() {
  local idx=$1 port=$2
  local peers=()
  case $idx in
    0) peers=(--peer 127.0.0.1:9101 --peer 127.0.0.1:9102) ;;
    1) peers=(--peer 127.0.0.1:9100 --peer 127.0.0.1:9102) ;;
    2) peers=(--peer 127.0.0.1:9100 --peer 127.0.0.1:9101) ;;
  esac
  echo "starting validator $idx on :$port"
  nohup "$NODE" run \
    --data-dir "$HOST_DATA/v$idx" \
    --validator-index "$idx" \
    --listen "127.0.0.1:$port" \
    "${peers[@]}" \
    >"$LOG/v$idx.log" 2>&1 &
  echo $! >>"$PIDFILE"
}

start_one 0 9100
start_one 1 9101
start_one 2 9102

echo "starting faucet on :8787"
export MESHCHAIN_DATA="$HOST_DATA/v0"
export MESHCHAIN_BIN="$NODE"
export FAUCET_PORT=8787
export FAUCET_AMOUNT="${FAUCET_AMOUNT:-100000000}"
export FAUCET_COOLDOWN="${FAUCET_COOLDOWN:-60}"
export CORS_ORIGIN="*"
nohup python3 "$ROOT/services/faucet/faucet_server.py" >"$LOG/faucet.log" 2>&1 &
echo $! >>"$PIDFILE"

echo "Host running. PIDs in $PIDFILE"
echo "  Logs:   $LOG"
echo "  Faucet: http://127.0.0.1:8787/info"
echo "  Stop:   ./scripts/stop_testnet_host.sh"
sleep 2
curl -s http://127.0.0.1:8787/info || true
echo
