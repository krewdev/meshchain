"""Append a decoded type-14 AirBlockAck to MESH_RELAY_ACK_LOG.

Call from tools/mesh_radio_relay.py after the existing TCP inject:

    from air_ack_log import append_air_ack
    append_air_ack(height, block_hash, vidx, sig, pk_hex)

Does not talk to Solana. Drain with programs-mesh-bridge/scripts/post_air_acks.ts.
"""
from __future__ import annotations

import json
import os
import time
from pathlib import Path


def append_air_ack(
    height: int,
    block_hash: bytes,
    validator_index: int,
    sig: bytes,
    validator_pubkey_hex: str = "",
) -> None:
    path = Path(os.environ.get("MESH_RELAY_ACK_LOG", "data/air_acks.jsonl"))
    path.parent.mkdir(parents=True, exist_ok=True)
    row = {
        "height": int(height),
        "block_hash_hex": block_hash.hex(),
        "validator_index": int(validator_index),
        "signature_hex": sig.hex(),
        "validator_pubkey_hex": validator_pubkey_hex,
        "ts": time.time(),
    }
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(row, separators=(",", ":")) + "\n")
