# RELAY — Path A

Work token for MeshChain relays and vault witnesses.
Not tMESH. Not vault cash. Not live on mainnet.

Program stays `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx`.

## Rule

- `post_air_ack` scores a node. Pays nothing.
- Only an existing `WithdrawRecord` can open a `SettleCredit`.
- `claim_settle_fee` pays `floor(withdraw_fee * 7000/10000 / N)` lamports from `sol_vault`.
- `claim_settle` (RELAY mint path) errors `EmissionsDark` until the vault is mainnet and the flag is flipped.
- `init_relay_config` rejects `emissions_enabled = true`.

## This weekend

Do: add `relay.rs`, upgrade the program on **devnet**, run the deposit → AirIou → withdraw_hybrid → open_settle_credit → claim_settle_fee loop.

Do not: mint RELAY on mainnet, tweet a ticker, lock LP, add a transfer hook, pay raw packet count.

## Wire-up

See comments at the top of `programs-mesh-bridge/programs/programs-mesh-bridge/src/relay.rs`.

```
cd programs-mesh-bridge
anchor build
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
ANCHOR_WALLET=~/.config/solana/id.json \
anchor upgrade target/deploy/programs_mesh_bridge.so \
  --program-id CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx \
  --provider.cluster devnet

npx ts-node scripts/init_relay_devnet.ts
```

Public copy until mainnet vault:

> PUBLIC TESTNET · tMESH has no cash value · RELAY is not live
