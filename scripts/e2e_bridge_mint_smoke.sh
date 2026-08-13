#!/usr/bin/env bash
# Mesh-side bridge smoke: mint-for-deposit via peer (no Solana deposit required).
# Proves relayer mint path works on public seed / local host.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MESH="${MESH:-$ROOT/target/release/mesh}"
NODE="${NODE:-$ROOT/target/release/meshchain-node}"
DIR="${MESHCHAIN_DATA:-$ROOT/data}"
PEER="${MESH_MINT_PEER:-127.0.0.1:9100}"
[[ -x "$MESH" ]] || MESH="$ROOT/target/debug/mesh"
[[ -x "$NODE" ]] || NODE="$ROOT/target/debug/meshchain-node"

mkdir -p "$DIR/keys"
if [[ ! -f "$DIR/keys/bridge_smoke.json" ]]; then
  "$MESH" --dir "$DIR" new-wallet --name bridge_smoke.json || true
fi
PUB=$(python3 -c "import json;print(json.load(open('$DIR/keys/bridge_smoke.json'))['public_hex'])")
REF=$(openssl rand -hex 16)
echo "mint to $PUB ref=$REF peer=$PEER"
"$NODE" mint-for-deposit \
  --data-dir "${MESH_DATA_V0:-$DIR/host/v0}" \
  --to-pubkey "$PUB" \
  --amount 1000000 \
  --external-ref-hex "$REF" \
  --validator-index 0 \
  --peer "$PEER"
echo "submitted — poll scanner / chain_state for balance"
