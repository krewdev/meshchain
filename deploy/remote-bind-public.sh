#!/usr/bin/env bash
# Run ON the seed host: rebind validators to 0.0.0.0 and restart faucet+scanner.
#
# Env:
#   EXTRA_PEERS  space-separated host:port remote peers (e.g. multi-host observers)
#                default: 35.192.20.103:9100 (lab remote observer)
set -euo pipefail
cd /opt/meshchain
export MESHCHAIN_BIN=/opt/meshchain/target/release/meshchain-node
export MESHCHAIN_SCANNER=/opt/meshchain/target/release/meshchain-scanner
HOST_DATA=/opt/meshchain/data/host
LOG=$HOST_DATA/logs
PIDFILE=$HOST_DATA/host.pids
mkdir -p "$LOG"

# Remote multi-host peers (observers / other operators). Overridable.
EXTRA_PEERS="${EXTRA_PEERS:-35.192.20.103:9100}"
# shellcheck disable=SC2206
EXTRA_ARR=($EXTRA_PEERS)
EXTRA_FLAGS=()
for p in "${EXTRA_ARR[@]}"; do
  [[ -n "$p" ]] && EXTRA_FLAGS+=(--peer "$p")
done

if [[ -f "$PIDFILE" ]]; then
  while read -r pid; do
    kill "$pid" 2>/dev/null || true
  done <"$PIDFILE" || true
fi
sleep 1
# free ports without pkill -f
for p in 9100 9101 9102 8787 8788 9110; do
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
  peers+=("${EXTRA_FLAGS[@]}")
  echo "starting validator $idx on 0.0.0.0:$port extra_peers=${EXTRA_PEERS}"
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

# Ensure faucet mints via gossip (never offline fork on public seed)
export MESH_MINT_PEER="${MESH_MINT_PEER:-127.0.0.1:9100}"
unset MESH_ALLOW_OFFLINE_MINT || true

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

# Local non-PoA observer on seed (optional relay); skip if port busy
if [[ "${START_LOCAL_OBSERVER:-1}" == "1" ]]; then
  OBS=$HOST_DATA/../observer-ext
  # path: /opt/meshchain/data/observer-ext
  OBS=/opt/meshchain/data/observer-ext
  mkdir -p "$OBS"
  cp -f "$HOST_DATA/v0/genesis.json" "$OBS/" 2>/dev/null || true
  if [[ ! -f "$OBS/chain_state.json" ]]; then
    cp -f "$HOST_DATA/v0/chain_state.json" "$OBS/" 2>/dev/null || true
  fi
  nohup "$NODE" run \
    --data-dir "$OBS" --observer \
    --listen "0.0.0.0:9110" \
    --peer 127.0.0.1:9100 --peer 127.0.0.1:9101 --peer 127.0.0.1:9102 \
    "${EXTRA_FLAGS[@]}" \
    --slot-ms 100 \
    >>"$LOG/observer-ext.log" 2>&1 &
  echo $! >>"$PIDFILE"
  echo "local observer-ext on :9110"
fi

sleep 2
ss -lntp | grep -E '9100|9101|9102|9110|8787|8788' || true
curl -s http://127.0.0.1:8787/info; echo
curl -s http://127.0.0.1:8788/api/v1/status | head -c 400; echo
echo "DONE public bind (EXTRA_PEERS=${EXTRA_PEERS})"
