# AirIou — mesh-gated spend frame

Product loop: [PRODUCT.md](PRODUCT.md). Hybrid lock: [HYBRID_LOCK.md](HYBRID_LOCK.md).

This is **not** a Solana transaction on LoRa. It is a 129-byte signed IOU bound to a vault `deposit_seq`. L1 moves later via `withdraw_hybrid_*` plus mesh witnesses.

## Wire

MC header is 6 bytes (`MC` | ver | type | len LE). Payload:

### Type 15 `AirIou` — 129 B

```
ver:u8=1 | from:8 | dest:32 | amount:u64 LE | nonce:u32 LE
| expiry_unix:u32 LE | deposit_seq:u64 LE | ed25519:64
```

Sign bytes = first 65 bytes. `iou_id` = SHA-256(sign bytes)[:16].

`from` is MeshChain short id (`SHA-256(ed25519 pk)[:8]`).  
`dest` is the Solana pubkey that receives vault USDC/SOL.

### Type 16 `AirIouAck` — 82 B

```
ver:u8=1 | iou_id:16 | witness_index:u8 | ed25519:64
```

Witness signs `ver | iou_id | witness_index`. Attestor Solana keys must match the vault set.

## CLI

```
mesh new-wallet --name me.json
mesh iou 1.00 --dest <64 hex Solana pubkey> --wallet me.json --deposit-seq 1
mesh iou 1.00 --dest <64 hex> --wallet me.json --deposit-seq 1 --air --relay 127.0.0.1:9199
```

Writes `data/last_iou.json`. Radio relay already forwards types 15/16 as `air_iou` / `air_iou_ack` JSON.

## Settle mapping

`burn_txid` / withdraw uniqueness key = `SHA-256(iou_id || deposit_seq.to_le_bytes())` (32 B) until the program takes `iou_id` directly.

Hotel Wi-Fi without K attestor co-signers cannot complete `withdraw_hybrid_*`.
