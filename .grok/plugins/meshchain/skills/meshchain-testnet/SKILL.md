---
name: meshchain-testnet
description: Work on MeshChain public testnet (meshchain-testnet-1) — wallets, validators, vault, e2e flows
---

# MeshChain testnet skill

## Project root

Usually `~/projects/meshchain` or the git repo `krewdev/meshchain`.

## Key facts

| Item | Value |
|------|--------|
| chain_id | `meshchain-testnet-1` |
| Token | **tMESH** (no cash value) |
| Channel | `MeshChain-Testnet-1` |
| Site | https://meshchain-sigma.vercel.app |
| Solana program (devnet) | `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx` |
| GitHub | https://github.com/krewdev/meshchain |

## Common commands

```bash
cd ~/projects/meshchain
cargo build -p mesh -p meshchain-node -p meshchain-scanner

./target/debug/mesh testnet-setup
./target/debug/mesh testnet-info
./target/debug/mesh new-wallet          # shows mesh name MXXXXX-XXXXX-XXX
./target/debug/mesh demo
./target/debug/mesh scanner --listen 0.0.0.0:8787 --auth open
```

## Safety

- Never mainnet deposits into un-audited programs
- tMESH is worthless / wipeable
- Keep cold keys offline

## Docs in repo

- `docs/TESTNET.md`, `docs/SCANNER.md`, `docs/HYBRID_LOCK.md`, `docs/MULTI_VALIDATOR.md`
- `testnet/network.json`, `testnet/attestors.json`
