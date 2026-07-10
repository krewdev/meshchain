# Solana devnet bridge (testnet)

## Program identity (reserved)

| Field | Value |
|-------|--------|
| **Program ID** | `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx` |
| **Cluster** | `devnet` |
| **Source** | `programs-mesh-bridge/` |
| **Status** | Built; on-chain deploy requires ~3.3 SOL rent. Faucet rate-limits may delay first deploy. |

Keypair path (local only, not in git):  
`programs-mesh-bridge/target/deploy/programs_mesh_bridge-keypair.json`

## Deploy (when you have enough devnet SOL)

```bash
solana config set --url devnet
solana airdrop 2   # or use https://faucet.solana.com

cd programs-mesh-bridge
anchor build
cp target/sbpf-solana-solana/release/programs_mesh_bridge.so target/deploy/
solana program deploy target/deploy/programs_mesh_bridge.so \
  --program-id target/deploy/programs_mesh_bridge-keypair.json \
  --url devnet
```

Then update `testnet/attestors.json` → `program.program_id` and `program.status` to `deployed`.

## Initialize hybrid config (after deploy)

Use Anchor/TS client or CLI:

- `initialize(fee_bps=30, withdraw_fee_bps=30, min_attestations=2, hybrid_enabled=true)`
- `set_attestors([ ... pubkeys from attestors.json ... ])`

## Hybrid unlock reminder

Withdraw needs:

1. Matching `mesh_short_id` from deposit  
2. Unique mesh `burn_txid`  
3. ≥2 attestor co-signers from the public list  
