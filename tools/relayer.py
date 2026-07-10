#!/usr/bin/env python3
"""
MeshChain bridge relayer — glue between Solana vault events and mesh Mint/Burn.

Plain flow:
  1) User deposits SOL/USDC into the Solana vault program
  2) This relayer sees the deposit and writes a mesh Mint intent
  3) Validators include Mint → user holds MESH offline
  4) User burns MESH (cold key if large)
  5) This relayer calls withdraw on Solana → user gets crypto back

Dev usage:
  python3 tools/relayer.py --data-dir ./data
  python3 tools/relayer.py --watch-solana   # needs solana RPC + IDL later
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path


def write_mint_intent(data: Path, mesh_short_id: str, amount_net: int, external_ref: str) -> Path:
    intent = {
        "kind": "mint",
        "mesh_short_id": mesh_short_id,
        "amount_net": amount_net,
        "external_ref": external_ref,
        "asset_id": 1,
        "note": "Deposit seen on vault → mint this much MESH on the radio network",
    }
    out = data / "pending_mint.json"
    out.write_text(json.dumps(intent, indent=2))
    return out


def write_withdraw_intent(
    data: Path, burn_txid: str, amount: int, dest: str, height: int
) -> Path:
    intent = {
        "kind": "withdraw",
        "burn_txid": burn_txid,
        "amount": amount,
        "destination": dest,
        "mesh_height": height,
        "note": "Mesh burn final → release vault funds to destination",
    }
    out = data / "pending_withdraw.json"
    out.write_text(json.dumps(intent, indent=2))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Bridge helper: vault on internet ↔ MESH on radio mesh"
    )
    ap.add_argument("--data-dir", default="./data")
    ap.add_argument("--rpc", default="http://127.0.0.1:8899", help="Solana RPC")
    ap.add_argument(
        "--demo",
        action="store_true",
        help="Write example mint + withdraw intents (no chain calls)",
    )
    ap.add_argument(
        "--watch-solana",
        action="store_true",
        help="Placeholder: poll for DepositEvent (wire Anchor IDL next)",
    )
    args = ap.parse_args()
    data = Path(args.data_dir)
    data.mkdir(parents=True, exist_ok=True)

    print("MeshChain relayer")
    print(f"  data folder: {data}")
    print(f"  solana rpc:  {args.rpc}")
    print()

    if args.demo or not args.watch_solana:
        mint_path = write_mint_intent(
            data,
            mesh_short_id="aabbccdd11223344",
            amount_net=997_000_000,  # 1 SOL minus 0.3% fee example (lamports policy)
            external_ref="demo_deposit_seq_0",
        )
        wd_path = write_withdraw_intent(
            data,
            burn_txid="be" + "00" * 31,
            amount=500_000_000,
            dest="DestinationSolPubkeyHere",
            height=42,
        )
        print("Wrote example jobs (no money moved):")
        print(f"  {mint_path}")
        print(f"  {wd_path}")
        print()
        print("Next engineering step: sign real Mint txs into meshchain-node,")
        print("and call programs_mesh_bridge.withdraw_sol with burn proof.")
        return 0

    if args.watch_solana:
        print("Watch mode skeleton — connect Anchor client here.")
        print("Looping heartbeat (Ctrl+C to stop)…")
        try:
            while True:
                print(f"  watching {args.rpc} …")
                time.sleep(5)
        except KeyboardInterrupt:
            print("stopped")
        return 0

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
