#!/usr/bin/env python3
"""Turn data/last_iou.json into data/last_burn.json for e2e_cashout.ts.

The cash-out script still expects a mesh Burn record. Product loop uses
AirIou. Same 32-byte withdraw seed:

    burn_txid = SHA-256(iou_id || deposit_seq_le)

Does not submit a Solana tx. Does not mint RELAY.

  python3 tools/iou_to_last_burn.py
  python3 tools/iou_to_last_burn.py --iou data/last_iou.json --out data/last_burn.json
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from iou_burn import burn_txid


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--iou", default="data/last_iou.json")
    p.add_argument("--out", default="data/last_burn.json")
    p.add_argument("--mesh-height", type=int, default=0)
    args = p.parse_args()

    rec = json.loads(Path(args.iou).read_text())
    iid = bytes.fromhex(rec["iou_id_hex"])
    seq = int(rec.get("deposit_seq", 0))
    txid = rec.get("burn_txid_hex") or burn_txid(iid, seq).hex()
    out = {
        "kind": "air_iou",
        "burn_txid_hex": txid,
        "amount": int(rec["amount"]),
        "mesh_height": int(args.mesh_height),
        "mesh_short_id_hex": rec["from_hex"],
        "iou_id_hex": rec["iou_id_hex"],
        "deposit_seq": seq,
        "dest_hex": rec.get("dest_hex", ""),
    }
    dest = Path(args.out)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(out, indent=2) + "\n")
    print(dest)
    print("burn_txid", txid)
    print("mesh_short", out["mesh_short_id_hex"])
    print("amount    ", out["amount"])
    print("next: cd programs-mesh-bridge && npx ts-node scripts/e2e_cashout.ts")


if __name__ == "__main__":
    main()
