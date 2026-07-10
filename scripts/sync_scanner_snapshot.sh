#!/usr/bin/env bash
# Copy local chain_state into web/scanner/data for Vercel public explorer.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${1:-$ROOT/data/chain_state.json}"
DST_DIR="$ROOT/web/scanner/data"
mkdir -p "$DST_DIR"

if [[ ! -f "$SRC" ]]; then
  echo "Missing $SRC — run: mesh testnet-setup && mesh demo"
  exit 1
fi

python3 - "$SRC" "$DST_DIR" <<'PY'
import json, sys, time
from pathlib import Path
src, dst_dir = Path(sys.argv[1]), Path(sys.argv[2])
d = json.loads(src.read_text())
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
    "minters": d["minters"] if isinstance(d.get("minters"), list) else [],
}
(dst_dir / "chain_state.json").write_text(json.dumps(out, separators=(",", ":")))
(dst_dir / "meta.json").write_text(json.dumps({
    "snapshot_unix": int(time.time()),
    "source": str(src),
    "note": "Public testnet snapshot for Vercel. Re-run scripts/sync_scanner_snapshot.sh && deploy to refresh.",
    "auth": "open",
    "mesh2fa": "planned",
}, indent=2))
print(f"synced height={out['height']} accounts={len(out['accounts'])} → {dst_dir}")
PY

# network params
if [[ -f "$ROOT/testnet/network.json" ]]; then
  cp "$ROOT/testnet/network.json" "$DST_DIR/network.json"
fi
echo "Done. Deploy web/ to publish: vercel --prod"
