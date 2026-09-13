# Finish the loop (this weekend)

The ranking said the missing proof is: offline IOU → mesh ack → later vault withdraw.
Rust still has no `mesh iou` subcommand. These tools close that gap without a ticker.

## Already in the repo

- `crates/proto/src/iou.rs` — AirIou / AirIouAck encode, sign, verify
- `tools/mesh_radio_relay.py` — forwards types 14 / 15 / 16
- Vault `withdraw_hybrid_sol` + `e2e_cashout.ts`
- Path A scripts on `relay-path-a` (`init_relay_devnet.ts`, `post_air_acks.ts`)

## Added on this branch

| Tool | Role |
|---|---|
| `tools/mesh_iou.py` | Sign type 15, write `data/last_iou.json` |
| `tools/witness_iou.py` | Sign type 16 only after a type-15 file exists |
| `tools/iou_burn.py` | `burn_txid = SHA-256(iou_id \\| deposit_seq_le)` |
| `scripts/e2e_product_loop.sh` | Operator checklist |

## Radio hook (one-time)

In `tools/mesh_radio_relay.py`, after the type-14 TCP inject:

```python
from air_ack_log import append_air_ack
append_air_ack(height, block_hash, vidx, sig, pk_hex)
```

`air_ack_log.py` is already on this branch. It does not submit Solana txs.

## Still not done (do not fake it)

- `relay.rs` in `programs-mesh-bridge/.../src/` + `lib.rs` splice + `anchor build`
- `mesh iou` Rust clap command (this Python is the stand-in)
- Two-radio airplane-mode recording
- Second independent operator
- Mainnet vault / RELAY emissions

Public copy stays: PUBLIC TESTNET · tMESH has no cash value · RELAY is not live.
