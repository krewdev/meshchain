# Bitcoin vault design (extreme cold storage)

Same economic pattern as Solana: **lock BTC on the internet → mint MESH claims on the mesh → burn MESH to unlock BTC**.

Mesh remains the offline truth for the claim. BTC custody is a separate internet-facing module.

## Recommended v1: federated multisig vault

```
User BTC ──► N-of-M multisig vault (cold/watch-only operators)
                    │
                    │  attestation (deposit seen, amount, mesh short id)
                    ▼
              Bridge relayers ──Mint──► MeshChain
                    ▲
                    │  final Burn + PQ auth
              User redeems offline mesh
                    │
                    ▼
              Multisig pays BTC to fresh address
```

| Piece | Choice |
|-------|--------|
| Custody | 2-of-3 or 3-of-5 multisig (HW + geo separated) |
| Deposit detect | Watch-only xpub / electrum / bitcoin core ZMQ |
| Mesh credit | `Mint` with `asset_id = 2` (BTC-claim), `external_ref = txid trunc` |
| Redeem | `Burn` with `asset_id = 2`, `redeem_hint = hash(btc_address)` + PQ if large |
| Double-pay guard | Unique `burn_txid` record (same idea as Solana `WithdrawRecord`) |
| Fees | Taken in BTC at deposit/withdraw; mesh gets net amount |

## Why not “run Bitcoin consensus on LoRa”?

Impossible at useful security. Mesh holds **claims**; Bitcoin holds **settlement**.

## Optional later designs

1. **DLC / adjudicator** — more trust-minimized unlock conditions  
2. **Liquid / Fedimint** — faster pegs for community operators  
3. **On-chain covenants** (when available) — vault script enforces burn proof

## Mesh multi-asset tags

| asset_id | Meaning |
|----------|---------|
| 0 | Generic MESH / test |
| 1 | SOL / SPL vault claim |
| 2 | BTC vault claim |

Balances may stay unified MESH units with **off-mesh policy** that 1 unit = 1 sat or 1 USDC — publish the peg in genesis notes. Future: separate balance maps per `asset_id`.

## Operational cold policy

- BTC multisig keys never on the mesh radio host  
- Mesh cold key (ML-DSA-65) never on the BTC signing machine  
- Redeem requires both: mesh Burn finality **and** multisig ceremony  
- Prefer new BTC receive address every unlock  

## Implementation order

1. Solana vault live (done as program + events)  
2. Relayer Mint/Burn for SOL  
3. BTC watch + multisig coordinator (this doc)  
4. Unified CLI: `mesh cash-out btc …`  
