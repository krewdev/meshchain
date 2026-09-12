#!/usr/bin/env python3
"""Sign a type-15 AirIou. The Rust CLI does not have `mesh iou` yet.

Wallet JSON: {"secret_hex": "..32-byte ed25519 seed..", "public_hex": ".."}

  python tools/mesh_iou.py 1.00 --dest <32-byte hex or Solana base58> \
      --wallet data/keys/me.json --deposit-seq 0

Writes data/last_iou.json. Optional --air posts the 129B frame to the radio relay.
Does not talk to Solana. Does not mint RELAY.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import struct
import time
from pathlib import Path

AIR_IOU_LEN = 129
MESH_DECIMALS = 1_000_000


def _nacl():
    try:
        from nacl.signing import SigningKey  # type: ignore
    except ImportError as e:
        raise SystemExit("pip install pynacl  # needed to sign AirIou") from e
    return SigningKey


def short_id(pubkey: bytes) -> bytes:
    return hashlib.sha256(pubkey).digest()[:8]


def parse_dest(s: str) -> bytes:
    s = s.strip()
    if len(s) == 64 and all(c in "0123456789abcdefABCDEF" for c in s):
        raw = bytes.fromhex(s)
        if len(raw) != 32:
            raise SystemExit("dest hex must be 32 bytes")
        return raw
    try:
        import base58  # type: ignore

        raw = base58.b58decode(s)
    except Exception:
        raise SystemExit("dest must be 32-byte hex or Solana base58 (pip install base58)")
    if len(raw) != 32:
        raise SystemExit(f"decoded dest is {len(raw)} bytes, need 32")
    return raw


def parse_amount(s: str) -> int:
    if "." in s:
        whole, frac = s.split(".", 1)
        frac = (frac + "000000")[:6]
        return int(whole) * MESH_DECIMALS + int(frac)
    return int(s) * MESH_DECIMALS


def sign_iou(secret: bytes, dest: bytes, amount: int, nonce: int, expiry: int, deposit_seq: int) -> dict:
    SigningKey = _nacl()
    sk = SigningKey(secret)
    pk = bytes(sk.verify_key)
    frm = short_id(pk)
    prefix = (
        bytes([1])
        + frm
        + dest
        + struct.pack("<Q", amount)
        + struct.pack("<I", nonce)
        + struct.pack("<I", expiry)
        + struct.pack("<Q", deposit_seq)
    )
    assert len(prefix) == 65
    sig = sk.sign(prefix).signature
    body = prefix + bytes(sig)
    assert len(body) == AIR_IOU_LEN
    iou_id = hashlib.sha256(prefix).digest()[:16]
    burn = hashlib.sha256(iou_id + struct.pack("<Q", deposit_seq)).digest()
    return {
        "ver": 1,
        "from_hex": frm.hex(),
        "dest_hex": dest.hex(),
        "amount": amount,
        "nonce": nonce,
        "expiry_unix": expiry,
        "deposit_seq": deposit_seq,
        "signature_hex": bytes(sig).hex(),
        "iou_id_hex": iou_id.hex(),
        "burn_txid_hex": burn.hex(),
        "body_hex": body.hex(),
        "ts": time.time(),
    }


def post_air(relay: str, body: bytes) -> None:
    host, port_s = relay.rsplit(":", 1)
    line = json.dumps({"type": "air_iou", "iou_bincode_hex": body.hex()}) + "\n"
    with socket.create_connection((host, int(port_s)), timeout=5) as s:
        s.sendall(line.encode())


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("amount", help="decimal MESH/vault units, e.g. 1.00")
    p.add_argument("--dest", required=True, help="Solana dest: 32-byte hex or base58")
    p.add_argument("--wallet", default="data/keys/me.json")
    p.add_argument("--deposit-seq", type=int, default=0)
    p.add_argument("--nonce", type=int, default=1)
    p.add_argument("--ttl", type=int, default=86400, help="seconds until expiry")
    p.add_argument("--out", default="data/last_iou.json")
    p.add_argument("--air", action="store_true")
    p.add_argument("--relay", default="127.0.0.1:9199")
    args = p.parse_args()

    wallet = json.loads(Path(args.wallet).read_text())
    secret = bytes.fromhex(wallet["secret_hex"])
    dest = parse_dest(args.dest)
    amount = parse_amount(args.amount)
    if amount <= 0:
        raise SystemExit("amount must be > 0")
    rec = sign_iou(
        secret,
        dest,
        amount,
        args.nonce,
        int(time.time()) + args.ttl,
        args.deposit_seq,
    )
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(rec, indent=2))
    print("AirIou signed")
    print("  iou_id     ", rec["iou_id_hex"])
    print("  from       ", rec["from_hex"])
    print("  dest       ", rec["dest_hex"])
    print("  amount     ", rec["amount"])
    print("  deposit_seq", rec["deposit_seq"])
    print("  burn_txid  ", rec["burn_txid_hex"])
    print("  file       ", out)
    if args.air:
        post_air(args.relay, bytes.fromhex(rec["body_hex"]))
        print("  posted to  ", args.relay)
    print("Next: python tools/witness_iou.py --iou", out)


if __name__ == "__main__":
    main()
