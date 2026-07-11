#!/usr/bin/env bash
# Mock Meshtastic path: radio relay --mock + air-submit into multi-node lab.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="${BIN:-$ROOT/target/debug/meshchain-node}"
MESH="${MESH:-$ROOT/target/debug/mesh}"
BASE="${E2E_BASE:-/tmp/mesh-e2e-air-$$}"

if [[ ! -x "$BIN" || ! -x "$MESH" ]]; then
  cargo build -p mesh -p meshchain-node -q
fi

rm -rf "$BASE"
mkdir -p "$BASE"

echo "== init =="
"$BIN" init --data-dir "$BASE" --validators 3 --chain-id meshchain-air-lab
python3 - <<PY
import json
from pathlib import Path
p = Path("$BASE/genesis.json")
g = json.loads(p.read_text())
g["slot_secs"] = 1
p.write_text(json.dumps(g, indent=2)+"\n")
PY

for i in 0 1 2; do
  d="$BASE/v$i"
  mkdir -p "$d/keys"
  cp "$BASE/genesis.json" "$d/"
  cp "$BASE/keys"/validator-*.json "$d/keys/"
done

echo "== validators =="
PIDS=()
for i in 0 1 2; do
  port=$((9300 + i))
  peers=()
  for j in 0 1 2; do
    [[ $j -eq $i ]] && continue
    peers+=(--peer "127.0.0.1:$((9300 + j))")
  done
  "$BIN" run --data-dir "$BASE/v$i" --validator-index "$i" \
    --listen "127.0.0.1:$port" "${peers[@]}" --slot-ms 50 \
    >"$BASE/v$i.log" 2>&1 &
  PIDS+=($!)
done

echo "== mock radio relay =="
python3 "$ROOT/tools/mesh_radio_relay.py" --mock \
  --tcp 127.0.0.1:9300 --listen 127.0.0.1:9198 \
  >"$BASE/relay.log" 2>&1 &
PIDS+=($!)

cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT
sleep 3

# wait genesis
for _ in $(seq 1 40); do
  [[ -f "$BASE/v0/chain_state.json" ]] && break
  sleep 0.25
done

echo "== fund via peer mint =="
"$MESH" --dir "$BASE" new-wallet --name a.json
"$MESH" --dir "$BASE" new-wallet --name b.json
PUB_A=$(python3 -c "import json;print(json.load(open('$BASE/keys/a.json'))['public_hex'])")
PUB_B=$(python3 -c "import json;print(json.load(open('$BASE/keys/b.json'))['public_hex'])")
"$BIN" mint-for-deposit --data-dir "$BASE/v0" --to-pubkey "$PUB_A" --amount 50000000 \
  --external-ref-hex "$(python3 -c 'import os;print(os.urandom(16).hex())')" \
  --validator-index 0 --peer 127.0.0.1:9300
sleep 2
"$BIN" mint-for-deposit --data-dir "$BASE/v0" --to-pubkey "$PUB_B" --amount 1000000 \
  --external-ref-hex "$(python3 -c 'import os;print(os.urandom(16).hex())')" \
  --validator-index 0 --peer 127.0.0.1:9300
for _ in $(seq 1 40); do
  H=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',0))" 2>/dev/null || echo 0)
  [[ "$H" -ge 2 ]] && break
  sleep 0.25
done
cp "$BASE/v0/chain_state.json" "$BASE/chain_state.json"
cp "$BASE/v0/genesis.json" "$BASE/genesis.json"

NAME_B=$("$MESH" --dir "$BASE" address --wallet b.json | awk '/Mesh name:/{print $3}')
echo "== air send to $NAME_B =="
"$MESH" --dir "$BASE" send "$NAME_B" 1 --wallet a.json --air --relay 127.0.0.1:9198 --submit 127.0.0.1:9300
sleep 5

H=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',0))")
echo "height=$H"
if [[ "$H" -lt 3 ]]; then
  echo "FAIL expected height>=3"
  tail -40 "$BASE/v0.log" || true
  tail -40 "$BASE/relay.log" || true
  exit 1
fi
echo "AIR PATH E2E PASS (mock LoRa + MC frame inject)"
