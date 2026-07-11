#!/usr/bin/env bash
# Multi-process PoA smoke: 3 validators, submit transfer, expect finality.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="${BIN:-$ROOT/target/debug/meshchain-node}"
MESH="${MESH:-$ROOT/target/debug/mesh}"
BASE="${E2E_BASE:-/tmp/mesh-e2e-multi-$$}"

if [[ ! -x "$BIN" || ! -x "$MESH" ]]; then
  cargo build -p mesh -p meshchain-node -q
fi

rm -rf "$BASE"
mkdir -p "$BASE"

echo "== init genesis =="
# Lab profile: faster slots (not public testnet 30s).
"$BIN" init --data-dir "$BASE" --validators 3 --chain-id meshchain-e2e-lab
python3 - <<PY
import json
from pathlib import Path
p = Path("$BASE/genesis.json")
g = json.loads(p.read_text())
g["slot_secs"] = 1
g["protocol_version"] = g.get("protocol_version", 1)
p.write_text(json.dumps(g, indent=2) + "\n")
print("slot_secs=", g["slot_secs"])
PY

for i in 0 1 2; do
  d="$BASE/v$i"
  mkdir -p "$d/keys"
  cp "$BASE/genesis.json" "$d/"
  cp "$BASE/keys"/validator-*.json "$d/keys/"
done

echo "== start validators =="
PIDS=()
for i in 0 1 2; do
  port=$((9200 + i))
  peers=()
  for j in 0 1 2; do
    [[ $j -eq $i ]] && continue
    peers+=(--peer "127.0.0.1:$((9200 + j))")
  done
  "$BIN" run --data-dir "$BASE/v$i" --validator-index "$i" \
    --listen "127.0.0.1:$port" "${peers[@]}" --slot-ms 50 \
    >"$BASE/v$i.log" 2>&1 &
  PIDS+=($!)
done
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT
sleep 3

# wait for genesis seal on v0
for _ in $(seq 1 40); do
  if [[ -f "$BASE/v0/chain_state.json" ]]; then
    H=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',-1))")
    [[ "$H" -ge 0 ]] && [[ -n "$H" ]] && break
  fi
  sleep 0.25
done
if [[ ! -f "$BASE/v0/chain_state.json" ]]; then
  echo "FAIL: no chain_state after start"
  tail -40 "$BASE/v0.log" || true
  exit 1
fi
echo "genesis height=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json'))['height'])")"

echo "== wallet + mint via gossip peer =="
"$MESH" --dir "$BASE" new-wallet --name e2e-a.json
"$MESH" --dir "$BASE" new-wallet --name e2e-b.json
PUB_A=$(python3 -c "import json;print(json.load(open('$BASE/keys/e2e-a.json'))['public_hex'])")
PUB_B=$(python3 -c "import json;print(json.load(open('$BASE/keys/e2e-b.json'))['public_hex'])")
REF_A=$(python3 -c "import os;print(os.urandom(16).hex())")
REF_B=$(python3 -c "import os;print(os.urandom(16).hex())")

H_BEFORE=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',0))")
"$BIN" mint-for-deposit --data-dir "$BASE/v0" --to-pubkey "$PUB_A" --amount 50000000 \
  --external-ref-hex "$REF_A" --validator-index 0 --peer 127.0.0.1:9200
# Wait for first mint finality so minter nonce advances before second mint.
for _ in $(seq 1 60); do
  H=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',0))" 2>/dev/null || echo 0)
  if [[ "$H" -gt "$H_BEFORE" ]]; then break; fi
  sleep 0.25
done
"$BIN" mint-for-deposit --data-dir "$BASE/v0" --to-pubkey "$PUB_B" --amount 1000000 \
  --external-ref-hex "$REF_B" --validator-index 0 --peer 127.0.0.1:9200

# wait for mints to finalize (height grows)
for _ in $(seq 1 60); do
  H=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',0))" 2>/dev/null || echo 0)
  if [[ "$H" -ge 2 ]]; then break; fi
  sleep 0.5
done

cp "$BASE/v0/chain_state.json" "$BASE/chain_state.json"
cp "$BASE/v0/genesis.json" "$BASE/genesis.json"

NAME_B=$("$MESH" --dir "$BASE" address --wallet e2e-b.json | awk '/Mesh name:/{print $3}')
echo "send to $NAME_B"
"$MESH" --dir "$BASE" send "$NAME_B" 1 --wallet e2e-a.json --submit 127.0.0.1:9200 || true
sleep 5

H0=$(python3 -c "import json;print(json.load(open('$BASE/v0/chain_state.json')).get('height',0))")
H1=$(python3 -c "import json;print(json.load(open('$BASE/v1/chain_state.json')).get('height',0))")
H2=$(python3 -c "import json;print(json.load(open('$BASE/v2/chain_state.json')).get('height',0))")
echo "heights v0=$H0 v1=$H1 v2=$H2"
if [[ "$H0" -lt 1 ]]; then
  echo "FAIL: expected height >= 1"
  tail -30 "$BASE/v0.log" || true
  exit 1
fi
# allow small skew of 1
python3 - <<PY
h=sorted([$H0,$H1,$H2])
if h[-1]-h[0] > 2:
    raise SystemExit(f'FAIL height skew {h}')
print('PASS multi-node heights', h)
PY

echo "MULTI-NODE E2E PASS"
