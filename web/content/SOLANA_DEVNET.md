# Solana devnet bridge (testnet)

## Program identity (DEPLOYED)

| Field | Value |
|-------|--------|
| **Program ID** | `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx` |
| **Cluster** | `devnet` |
| **Status** | **Deployed + initialized** |
| **Config PDA** | `8Q4noL3szKSVQwWyfyPP2ieAB2TTTXGmW42i7VT1ozV4` |
| **SOL vault PDA** | `GqDGmwsz8Mw9RfzPXj42tWmscDdmNxcG5qxuaaGBjEGV` |
| **Explorer** | https://explorer.solana.com/address/CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx?cluster=devnet |
| **Hybrid** | enabled, min_attestations=2, fee_bps=30 |

Source: `programs-mesh-bridge/`

## Re-init / update attestors

```bash
cd programs-mesh-bridge
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
ANCHOR_WALLET=~/.config/solana/id.json \
npx ts-node scripts/initialize_devnet.ts
```

## Hybrid unlock reminder

Withdraw needs:

1. Matching `mesh_short_id` from deposit  
2. Unique mesh `burn_txid`  
3. ≥2 attestor co-signers from the public list  
