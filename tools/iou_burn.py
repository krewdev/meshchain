#!/usr/bin/env python3
"""burn_txid = SHA-256(iou_id || deposit_seq_le) as specified in docs/IOU.md."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path


def burn_txid(iou_id: bytes, deposit_seq: int) -> bytes:
    if len(iou_id) != 16:
        raise ValueError("iou_id must be 16 bytes")
    return hashlib.sha256(iou_id + struct.pack("<Q", deposit_seq)).digest()


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--iou", default="data/last_iou.json")
    p.add_argument("--iou-id")
    p.add_argument("--deposit-seq", type=int)
    args = p.parse_args()
    if args.iou_id:
        iid = bytes.fromhex(args.iou_id)
        seq = int(args.deposit_seq or 0)
    else:
        rec = json.loads(Path(args.iou).read_text())
        iid = bytes.fromhex(rec["iou_id_hex"])
        seq = int(rec.get("deposit_seq", 0))
    print(burn_txid(iid, seq).hex())


if __name__ == "__main__":
    main()
