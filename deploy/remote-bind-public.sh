#!/usr/bin/env bash
# Run ON the seed host: rebind validators to 0.0.0.0 and restart faucet+scanner.
set -euo pipefail
cd /opt/meshchain
export MESHCHAIN_BIN=/opt/meshchain/target/release/meshchain-node
export MESHCHAIN_SCANNER=/opt/meshchain/target/release/meshchain-scanner
HOST_DATA=/opt/meshchain/data/host
LOG=$HOST_DATA/logs
PIDFILE=$HOST_DATA/host.pids
mkdir -p "$LOG"

if [[ -f "$PIDFILE" ]]; then
  while read -r pid; do
    kill "$pid" 2>/dev/null || true
  done <"$PIDFILE" || true
fi
sleep 1
# free ports without pkill -f
for p in 9100 9101 9102 8787 8788; do
  fuser -k "${p}/tcp" 2>/dev/null || true
done
sleep 1
: >"$PIDFILE"
NODE=$MESHCHAIN_BIN

start_one() {
  local idx=$1 port=$2
  local peers=()
  case $idx in
    0) peers=(--peer 127.0.0.1:9101 --peer 127.0.0.1:9102) ;;
    1) peers=(--peer 127.0.0.1:9100 --peer 127.0.0.1:9102) ;;
    2) peers=(--peer 127.0.0.1:9100 --peer 127.0.0.1:9101) ;;
  esac
  echo "starting validator $idx on 0.0.0.0:$port"
  nohup "$NODE" run \
    --data-dir "$HOST_DATA/v$idx" \
    --validator-index "$idx" \
    --listen "0.0.0.0:$port" \
    "${peers[@]}" \
    >>"$LOG/v$idx.log" 2>&1 &
  echo $! >>"$PIDFILE"
}

start_one 0 9100
start_one 1 9101
start_one 2 9102

export MESHCHAIN_DATA=$HOST_DATA/v0
export FAUCET_PORT=8787
export FAUCET_AMOUNT=100000000
export FAUCET_COOLDOWN=60
export CORS_ORIGIN=*
nohup python3 /opt/meshchain/services/faucet/faucet_server.py >>"$LOG/faucet.log" 2>&1 &
echo $! >>"$PIDFILE"

nohup "$MESHCHAIN_SCANNER" \
  --data-dir "$HOST_DATA/v0" \
  --listen 0.0.0.0:8788 \
  --auth open \
  --reload-secs 5 \
  >>"$LOG/scanner.log" 2>&1 &
echo $! >>"$PIDFILE"

sleep 2
ss -lntp | grep -E '9100|9101|9102|8787|8788' || true
curl -s http://127.0.0.1:8787/info; echo
curl -s http://127.0.0.1:8788/api/v1/status | head -c 400; echo
echo "DONE public bind"
