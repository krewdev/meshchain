#!/usr/bin/env python3
"""
Extend a MeshChain PoA genesis with additional validator public keys.

Does NOT create private keys — operators run:
  mesh validator-keygen --name operator.json
and send public_hex only.

Usage:
  python3 scripts/genesis_add_validators.py \\
    --genesis testnet/published/genesis.json \\
    --add be22b1... \\
    --add deadbeef... \\
    --out /tmp/genesis.next.json

Then coordinators restack seed hosts with the new genesis (testnet reset).
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def valid_pubkey(h: str) -> str:
    h = h.strip().lower().removeprefix("0x")
    if len(h) != 64:
        raise SystemExit(f"public_hex must be 64 hex chars, got len={len(h)}")
    int(h, 16)
    return h


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--genesis", required=True, type=Path)
    ap.add_argument(
        "--add",
        action="append",
        default=[],
        help="Validator public_hex to append (repeatable)",
    )
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument(
        "--note",
        default="",
        help="Optional note written to sidecar .meta.json",
    )
    args = ap.parse_args()

    if not args.add:
        print("error: provide at least one --add PUBLIC_HEX", file=sys.stderr)
        return 1

    g = json.loads(args.genesis.read_text())
    vals = list(g.get("validators") or [])
    before = len(vals)
    for raw in args.add:
        pk = valid_pubkey(raw)
        if pk in vals:
            print(f"skip duplicate {pk[:16]}…")
            continue
        vals.append(pk)
        print(f"added index {len(vals)-1}: {pk}")

    g["validators"] = vals
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(g, indent=2) + "\n")
    print(f"wrote {args.out}  validators {before} → {len(vals)}")
    print("finality threshold ceil(2N/3) =", (2 * len(vals) + 2) // 3)
    print()
    print("NEXT (coordinated testnet restack):")
    print("  1. Operators place keys as data/keys/validator-{i}.json matching indices")
    print("  2. Distribute genesis to all hosts")
    print("  3. Wipe chain_state (testnet reset) OR accept incompatible tip")
    print("  4. Restart all producers with --validator-index i")
    if args.note:
        meta = {
            "source_genesis": str(args.genesis),
            "out": str(args.out),
            "note": args.note,
            "validator_count": len(vals),
        }
        meta_path = args.out.with_suffix(args.out.suffix + ".meta.json")
        meta_path.write_text(json.dumps(meta, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
