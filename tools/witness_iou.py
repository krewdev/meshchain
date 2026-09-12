#!/usr/bin/env python3
"""Sign a type-16 AirIouAck ONLY after a type-15 AirIou is on disk.

Witness must be vault attestor index N. Key is a MeshChain wallet JSON
(secret_hex / public_hex) or a raw 32-byte seed hex.

  python tools/witness_iou.py --iou data/last_iou.json --index 0 \
      --wallet data/keys/attestor0.json --air

Writes data/last_iou_ack.json. Does not submit Solana. Does not mint RELAY.
"""
from __future__ import annotations

import argparse
import json
import socket
import time
from pathlib import Path

AIR_IOU_ACK_LEN = 82


def _nacl():
    try:
        from nacl.signing import SigningKey  # type: ignore
    except ImportError as e:
        raise SystemExit("pip install pynacl") from e
    return SigningKey


def sign_ack(secret: bytes, iou_id: bytes, index: int) -> dict:
    if len(iou_id) != 16:
        raise SystemExit("iou_id must be 16 bytes")
    SigningKey = _nacl()
    sk = SigningKey(secret)
    prefix = bytes([1]) + iou_id + bytes([index & 0xFF])
    assert len(prefix) == 18
    sig = bytes(sk.sign(prefix).signature)
    body = prefix + sig
    assert len(body) == AIR_IOU_ACK_LEN
    return {
        "ver": 1,
        "iou_id_hex": iou_id.hex(),
        "witness_index": index,
        "signature_hex": sig.hex(),
        "body_hex": body.hex(),
        "validator_pubkey_hex": bytes(sk.verify_key).hex(),
        "ts": time.time(),
    }


def post_air(relay: str, body: bytes) -> None:
    host, port_s = relay.rsplit(":", 1)
    line = json.dumps({"type": "air_iou_ack", "ack_hex": body.hex()}) + "\n"
    with socket.create_connection((host, int(port_s)), timeout=5) as s:
        s.sendall(line.encode())


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--iou", default="data/last_iou.json")
    p.add_argument("--index", type=int, required=True, help="attestor index in vault set")
    p.add_argument("--wallet", required=True)
    p.add_argument("--out", default="data/last_iou_ack.json")
    p.add_argument("--air", action="store_true")
    p.add_argument("--relay", default="127.0.0.1:9199")
    args = p.parse_args()

    iou = json.loads(Path(args.iou).read_text())
    if "iou_id_hex" not in iou or "body_hex" not in iou:
        raise SystemExit("refusing to ack: no type-15 artifact in --iou")
    wallet = json.loads(Path(args.wallet).read_text())
    secret = bytes.fromhex(wallet["secret_hex"])
    rec = sign_ack(secret, bytes.fromhex(iou["iou_id_hex"]), args.index)
    rec["burn_txid_hex"] = iou.get("burn_txid_hex", "")
    rec["deposit_seq"] = iou.get("deposit_seq", 0)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(rec, indent=2))
    print("AirIouAck signed")
    print("  iou_id ", rec["iou_id_hex"])
    print("  index  ", rec["witness_index"])
    print("  file   ", out)
    if args.air:
        post_air(args.relay, bytes.fromhex(rec["body_hex"]))
        print("  posted ", args.relay)
    print("Next: use burn_txid in last_iou.json with e2e_cashout.ts / withdraw_hybrid_sol")


if __name__ == "__main__":
    main()
