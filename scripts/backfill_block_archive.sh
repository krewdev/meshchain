#!/usr/bin/env bash
# Merge / index finalized block files across multi-validator data dirs.
# Also writes a tip checkpoint + MANIFEST for catch-up operators.
#
# Full historical blocks before archiving was enabled cannot be reconstructed
# from chain_state alone (applied[] is height/hash/tx_count only). This script:
#   1) Unions blocks/*.json from all host validator dirs
#   2) Writes MANIFEST.json (heights present + applied tip hashes)
#   3) Exports checkpoints/tip.json (full ChainState) for observers
#
# Usage:
#   ./scripts/backfill_block_archive.sh /opt/meshchain/data/host
#   ./scripts/backfill_block_archive.sh ./data
set -euo pipefail

HOST_DATA="${1:-}"
if [[ -z "$HOST_DATA" || ! -d "$HOST_DATA" ]]; then
  echo "Usage: $0 <host-data-dir>   e.g. /opt/meshchain/data/host or ./data"
  exit 1
fi
HOST_DATA="$(cd "$HOST_DATA" && pwd)"

# Discover validator dirs: v0 v1 v2 ... or single tree with chain_state.json
DIRS=()
if [[ -f "$HOST_DATA/chain_state.json" ]]; then
  DIRS+=("$HOST_DATA")
fi
for d in "$HOST_DATA"/v[0-9]*; do
  [[ -d "$d" ]] && DIRS+=("$d")
done
if [[ ${#DIRS[@]} -eq 0 ]]; then
  echo "No chain_state or vN dirs under $HOST_DATA"
  exit 1
fi

PRIMARY="${DIRS[0]}"
# Prefer v0 if present
for d in "${DIRS[@]}"; do
  if [[ "$(basename "$d")" == "v0" ]]; then
    PRIMARY="$d"
    break
  fi
done

ARCHIVE="$PRIMARY/blocks"
mkdir -p "$ARCHIVE" "$PRIMARY/checkpoints"

echo "primary=$PRIMARY"
echo "merging blocks from: ${DIRS[*]}"

merged=0
for d in "${DIRS[@]}"; do
  [[ -d "$d/blocks" ]] || continue
  for f in "$d/blocks"/*.json; do
    [[ -f "$f" ]] || continue
    base=$(basename "$f")
    [[ "$base" == "MANIFEST.json" ]] && continue
    dest="$ARCHIVE/$base"
    if [[ ! -f "$dest" ]]; then
      cp "$f" "$dest"
      merged=$((merged + 1))
    elif [[ "$f" -nt "$dest" ]]; then
      cp "$f" "$dest"
    fi
  done
done

# Symlink/copy archive into sibling validators so all can serve BlocksResponse
for d in "${DIRS[@]}"; do
  [[ "$d" == "$PRIMARY" ]] && continue
  mkdir -p "$d/blocks"
  for f in "$ARCHIVE"/*.json; do
    [[ -f "$f" ]] || continue
    base=$(basename "$f")
    [[ "$base" == "MANIFEST.json" ]] && continue
    if [[ ! -f "$d/blocks/$base" ]]; then
      cp "$f" "$d/blocks/$base" 2>/dev/null || true
    fi
  done
done

python3 - <<PY
import json, os
from pathlib import Path

primary = Path("$PRIMARY")
archive = primary / "blocks"
state_path = primary / "chain_state.json"
state = json.loads(state_path.read_text())
applied = state.get("applied") or []
height = int(state.get("height") or 0)
tip = state.get("tip_hash")
if isinstance(tip, list):
    tip_hex = bytes(tip).hex()
else:
    tip_hex = str(tip or "")

present = sorted(
    int(p.stem)
    for p in archive.glob("*.json")
    if p.stem.isdigit()
)
first = present[0] if present else None
last = present[-1] if present else None
gaps = []
if present:
    s = set(present)
    for h in range(present[0], present[-1] + 1):
        if h not in s:
            gaps.append(h)

manifest = {
    "chain_id": state.get("chain_id"),
    "tip_height": height,
    "tip_hash_hex": tip_hex if len(tip_hex) == 64 else tip_hex,
    "archive_count": len(present),
    "archive_first": first,
    "archive_last": last,
    "archive_gaps_sample": gaps[:50],
    "archive_gap_count": len(gaps),
    "applied_count": len(applied),
    "note": (
        "Full blocks only exist for heights in this archive. "
        "Heights before archive_first cannot be reconstructed from applied[] summaries. "
        "Observers should use SyncResponse / chain_state checkpoint; "
        "producers catch up via BlocksRequest from archive_first if local tip is just below."
    ),
    "applied_hashes": [
        {"height": a.get("height"), "hash_hex": a.get("hash_hex"), "tx_count": a.get("tx_count")}
        for a in applied
    ],
}
(archive / "MANIFEST.json").write_text(json.dumps(manifest, indent=2) + "\n")

# Tip checkpoint for observers / lab bootstrap
ckpt = {
    "exported_at_unix": __import__("time").time(),
    "height": height,
    "tip_hash_hex": manifest["tip_hash_hex"],
    "chain_id": state.get("chain_id"),
    "state": state,
}
ckpt_path = primary / "checkpoints" / f"height-{height}.json"
ckpt_path.write_text(json.dumps(ckpt) + "\n")
tip_link = primary / "checkpoints" / "tip.json"
tip_link.write_text(json.dumps(ckpt) + "\n")

print(f"archive_count={len(present)} first={first} last={last} gaps={len(gaps)}")
print(f"wrote {archive / 'MANIFEST.json'}")
print(f"wrote {ckpt_path}")
print(f"wrote {tip_link}")
if first is not None and first > 0:
    print(f"WARN: no full blocks for heights 0..{first-1} (pre-archive era)")
PY

# fix merged count print
echo "files_copied_this_run=$merged"
echo "BACKFILL OK  archive=$ARCHIVE"
ls "$ARCHIVE" | tail -5
