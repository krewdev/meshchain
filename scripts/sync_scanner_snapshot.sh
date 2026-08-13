#!/usr/bin/env bash
# Copy chain_state into web/scanner/data as FALLBACK snapshot for Vercel.
# Prefer live_api (config.json) for tip; snapshot is only when the seed is down.
#
# Usage:
#   ./scripts/sync_scanner_snapshot.sh                         # local data/chain_state.json
#   ./scripts/sync_scanner_snapshot.sh /path/to/chain_state.json
#   ./scripts/sync_scanner_snapshot.sh --live                   # pull public seed API
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DST_DIR="$ROOT/web/scanner/data"
LIVE_URL="${MESHCHAIN_CHAIN_STATE_URL:-https://34.172.103.125.sslip.io/api/v1/chain_state}"
mkdir -p "$DST_DIR"

SRC="${1:-$ROOT/data/chain_state.json}"
if [[ "${1:-}" == "--live" ]]; then
  TMP="$(mktemp)"
  curl -fsSL -A "meshchain-snapshot-sync" "$LIVE_URL" -o "$TMP"
  SRC="$TMP"
  trap 'rm -f "$TMP"' EXIT
elif [[ ! -f "$SRC" ]]; then
  echo "Missing $SRC — run: mesh testnet-setup && mesh demo"
  echo "Or: $0 --live   # pull $LIVE_URL"
  exit 1
fi

python3 - "$SRC" "$DST_DIR" <<'PY'
import json, sys, time
from pathlib import Path
src, dst_dir = Path(sys.argv[1]), Path(sys.argv[2])
d = json.loads(src.read_text())
minters = d.get("minters")
if isinstance(minters, list):
    minters_out = minters
elif isinstance(minters, dict):
    minters_out = list(minters.keys()) if minters else []
else:
    minters_out = []
out = {
    "chain_id": d["chain_id"],
    "height": d["height"],
    "tip_hash": d["tip_hash"],
    "block_reward": d["block_reward"],
    "slot_secs": d["slot_secs"],
    "validators": d["validators"],
    "accounts": d["accounts"],
    "total_supply": d["total_supply"],
    "applied": d["applied"],
    "pq_required_above": d.get("pq_required_above", 100_000_000),
    "minters": minters_out,
}
(dst_dir / "chain_state.json").write_text(json.dumps(out, separators=(",", ":")))
(dst_dir / "meta.json").write_text(json.dumps({
    "snapshot_unix": int(time.time()),
    "source": str(src),
    "height": out["height"],
    "note": "FALLBACK only. Vercel scanner prefers live_api in config.json. Re-run this script (or --live) && deploy.",
    "auth": "open",
    "mesh2fa": "planned",
    "prefer": "live_api",
}, indent=2) + "\n")
print(f"synced height={out['height']} accounts={len(out['accounts'])} → {dst_dir}")
PY

# network params
if [[ -f "$ROOT/testnet/network.json" ]]; then
  cp "$ROOT/testnet/network.json" "$DST_DIR/network.json"
fi
echo "Done. Deploy web/ to publish: vercel --prod"
