# Contributing to MeshChain

Thanks for helping build mesh-native wallets and services on [Meshtastic](https://meshtastic.org/).

## Ground rules

1. **Never commit keys** (`data/`, `*-keypair.json`, wallet/cold JSON with secrets).  
2. Prefer **fail-secure** changes: if a check fails, money must not move.  
3. Keep the **`mesh` CLI** wording simple (no jargon-only UX).  
4. Document residual risks when you touch consensus, vaults, or crypto.  
5. This is **community software for Meshtastic**, not an official Meshtastic product — say so in user-facing text.

## Dev loop

```bash
cargo test -p meshchain-proto -p meshchain-transport -p meshchain-ledger
cargo build -p mesh -p meshchain-node
./target/debug/mesh setup && ./target/debug/mesh demo
```

## PR checklist

- [ ] Tests pass for touched crates  
- [ ] No secrets in the diff  
- [ ] Docs updated if behavior changed  
- [ ] Security impact noted in the PR description  
