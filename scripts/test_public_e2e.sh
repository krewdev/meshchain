#!/usr/bin/env bash
# End-to-end smoke against the LIVE public seed.
# join-public → wallet → register → faucet-drip → balance check
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MESH="${MESH:-$ROOT/target/debug/mesh}"
if [[ ! -x "$MESH" ]]; then
  echo "Building mesh…"
  cargo build -p mesh -q
  MESH="$ROOT/target/debug/mesh"
fi

DIR="${E2E_DIR:-/tmp/mesh-public-e2e-$$}"
rm -rf "$DIR"
mkdir -p "$DIR"

echo "== 1) join-public =="
"$MESH" --dir "$DIR" join-public

echo
echo "== 2) new-wallet --publish =="
"$MESH" --dir "$DIR" new-wallet --name e2e.json --publish

echo
echo "== 3) faucet-drip =="
"$MESH" --dir "$DIR" faucet-drip --wallet e2e.json

echo
echo "== 4) balance =="
OUT=$("$MESH" --dir "$DIR" balance --wallet e2e.json)
echo "$OUT"

# Parse "Balance:   X.YYYYYY MESH"
BAL=$(echo "$OUT" | awk '/^Balance:/{print $2}')
python3 - "$BAL" <<'PY'
import sys
bal = float(sys.argv[1])
if bal < 99.0:
    print(f"FAIL: expected balance >= 99 tMESH after faucet, got {bal}")
    sys.exit(1)
print(f"PASS: balance {bal} tMESH")
PY

echo
echo "== 5) optional send self-check (register second wallet) =="
"$MESH" --dir "$DIR" new-wallet --name e2e-b.json --publish || true
NAME_B=$("$MESH" --dir "$DIR" address --wallet e2e-b.json | awk '/Mesh name:/{print $3}')
if [[ -n "${NAME_B:-}" ]]; then
  # small send needs recipient on chain; faucet-drip b so it exists with funds optional
  "$MESH" --dir "$DIR" faucet-drip --wallet e2e-b.json 2>/dev/null || true
  "$MESH" --dir "$DIR" send "$NAME_B" 1 --wallet e2e.json --fee 0.01 --submit 34.172.103.125:9100 2>&1 || {
    echo "WARN: send step failed (non-fatal for smoke); register/drip already passed"
  }
  "$MESH" --dir "$DIR" sync-state 2>/dev/null || true
  "$MESH" --dir "$DIR" balance --wallet e2e.json || true
fi

echo
echo "=============================================="
echo " PUBLIC E2E PASS"
echo "  dir: $DIR"
echo "=============================================="
