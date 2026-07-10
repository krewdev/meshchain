#!/usr/bin/env bash
# Run 3 validators on one machine (TCP gossip lab for meshchain-testnet-1).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/debug/meshchain-node}"
DATA="${DATA:-$ROOT/data}"

if [[ ! -x "$BIN" ]]; then
  echo "Build first: cargo build -p meshchain-node"
  echo "Looking for: $BIN"
  exit 1
fi

if [[ ! -f "$DATA/genesis.json" ]]; then
  echo "No genesis. Run: mesh testnet-setup"
  exit 1
fi

# Shared genesis/keys; separate state files via MESHCHAIN_STATE is not supported —
# each process uses same data_dir (state races). For lab, use one writer OR copy trees.
mkdir -p "$DATA/v0" "$DATA/v1" "$DATA/v2"
for i in 0 1 2; do
  cp -f "$DATA/genesis.json" "$DATA/v$i/"
  mkdir -p "$DATA/v$i/keys"
  cp -f "$DATA/keys"/validator-*.json "$DATA/v$i/keys/" 2>/dev/null || true
  cp -f "$DATA/testnet_profile.json" "$DATA/v$i/" 2>/dev/null || true
done

echo "Starting validators on :9100 :9101 :9102"
"$BIN" run --data-dir "$DATA/v0" --validator-index 0 --listen 127.0.0.1:9100 \
  --peer 127.0.0.1:9101 --peer 127.0.0.1:9102 &
PID0=$!
"$BIN" run --data-dir "$DATA/v1" --validator-index 1 --listen 127.0.0.1:9101 \
  --peer 127.0.0.1:9100 --peer 127.0.0.1:9102 &
PID1=$!
"$BIN" run --data-dir "$DATA/v2" --validator-index 2 --listen 127.0.0.1:9102 \
  --peer 127.0.0.1:9100 --peer 127.0.0.1:9101 &
PID2=$!

cleanup() {
  kill $PID0 $PID1 $PID2 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "PIDs: $PID0 $PID1 $PID2"
echo "Submit tx: meshchain-node submit-tx --tx ./data/last_payment.json --peer 127.0.0.1:9100"
echo "Ctrl+C to stop"
wait
