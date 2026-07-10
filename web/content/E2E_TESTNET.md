# E2E testnet results (live run)

**Date:** 2026-07-10  
**Network:** meshchain-testnet-1 + Solana **devnet**

## Flow proven

```
1. Create mesh wallet  →  mesh name M3SQRT-XTA1Y-ZJ6
2. Deposit 0.05 SOL    →  vault (hybrid-bound to mesh short id)
3. Mint tMESH          →  49.85 tMESH on mesh ledger
4. Burn + PQ cold key  →  vault-linked burn
5. Hybrid withdraw     →  SOL returned (2 attestors co-signed)
```

## Artifacts from the run

| Step | Detail |
|------|--------|
| Mesh name | `M3SQRT-XTA1Y-ZJ6` |
| Hex short | `1e6f8d774a0fbf23` |
| Deposit tx | [5TA1r4on…pGSS](https://explorer.solana.com/tx/5TA1r4onBW2rGxres1cby1XwTjJg4bAH8daB1A4TVxCKTqTvRQnPBUFR7nG2LudWN8QsCVLEWMnJnKrSrTX2pGSS?cluster=devnet) |
| Net locked | 49,850,000 lamports (after 0.30% fee) |
| Minted | 49.85 tMESH |
| Burn txid | `98a7175cad6f2ccd4126623f7892c66791e1b6b11595f42010b4e3435036ef5e` |
| Withdraw tx | [2Rh9XGkg…i8Fh](https://explorer.solana.com/tx/2Rh9XGkgHUwnfk5eDa8LTXyRWYWg8MErTdQQknXo8BpVd8tRxzhCvAXfvz7wjA4sLDAweU2ggav4ppCaLqaLi8Fh?cluster=devnet) |

## Reproduce

```bash
# 1) setup + wallet
cargo build -p mesh -p meshchain-node -p meshchain-wallet
mesh testnet-setup

# 2) deposit + mint
cd programs-mesh-bridge && yarn
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
ANCHOR_WALLET=~/.config/solana/id.json \
npx ts-node scripts/e2e_vault_to_mesh.ts

# 3) cash-out
meshchain-wallet pq-keygen --out ../data/keys/user_e2e_cold.json
meshchain-node burn-for-withdraw \
  --data-dir ../data \
  --wallet ../data/keys/user_e2e.json \
  --cold ../data/keys/user_e2e_cold.json \
  --amount <net_lamports> \
  --dest-sol <your_sol_pubkey> \
  --asset-id 1
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
ANCHOR_WALLET=~/.config/solana/id.json \
npx ts-node scripts/e2e_cashout.ts
```

## What this proves

- Vault deposit binds SOL to a **mesh short id**  
- Relayer can mint matching **tMESH** on the mesh ledger  
- Cash-out needs **mesh burn + 2 attestors** (internet alone fails)  
- Mesh names work for human identity (`M3SQRT-XTA1Y-ZJ6`)  
