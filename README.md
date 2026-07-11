# MeshChain

**Hold and move value on a [Meshtastic](https://meshtastic.org/) mesh — with optional hybrid vaults on Solana.**

MeshChain is an open-source **mesh-native ledger** and wallet toolkit. Everyday use works offline from the internet over LoRa. Large / long-term storage uses **quantum-resistant (ML-DSA-65)** cold keys. Vaulted SOL/stables on the internet can only be released with **Meshtastic-side identifiers + mesh witnesses** (hybrid lock).

> Built **for** the Meshtastic ecosystem. Not an official Meshtastic Foundation product — community software that runs **on** Meshtastic radios and channels.

**Live site:** [meshchain-sigma.vercel.app](https://meshchain-sigma.vercel.app) · [Docs](https://meshchain-sigma.vercel.app/docs/) · [**Testnet**](https://meshchain-sigma.vercel.app/docs/?doc=TESTNET)

[Meshtastic](https://meshtastic.org/) · [GitHub](https://github.com/krewdev/meshchain) · [Discord setup](./docs/DISCORD.md) · [Security](./docs/SECURITY_HARDENING.md) · [Hybrid vault](./docs/HYBRID_LOCK.md) · [Donate](./docs/DONATE.md)

---

## Public testnet (`meshchain-testnet-1`)

**TESTNET ONLY — tMESH has no cash value. State may be wiped.**

| Parameter | Value |
|-----------|--------|
| chain_id | `meshchain-testnet-1` |
| Token | tMESH |
| Channel | `MeshChain-Testnet-1` |
| Solana | **devnet** for bridge experiments |
| Params | [`testnet/network.json`](./testnet/network.json) |

```bash
cargo build -p mesh -p meshchain-node
./target/debug/mesh testnet-setup
./target/debug/mesh testnet-info
./target/debug/mesh demo
```

Guide: [docs/TESTNET.md](./docs/TESTNET.md) · **[Run a node](./docs/RUN_A_NODE.md)** (wallets, observers, validators)

---

## Donate

Optional support for development:

| Network | Address |
|---------|---------|
| **Solana** | `7EwBb1yboTkT3eZmUWw4zbWMMJC2a5e9rMeGV9EgkPJp` |
| **Ethereum** | `0xCB2d3d03FC47aec6a6DBA7C91010c16a1b9A5ca2` |
| **Bitcoin** | `bc1qzyfy2eqrxx0n2vugjhp4zkzkqcmhth7h5zhgle` |

---

## What you can do

| Goal | How |
|------|-----|
| Use a simple wallet (no UI app required) | `mesh` CLI |
| Run a local test network | `mesh setup` + `mesh demo` |
| Cold storage off Wi‑Fi / 5G | `mesh new-cold-key` + hybrid vault docs |
| Bridge SOL/stables ↔ mesh claims | Solana program in `programs-mesh-bridge/` |
| Talk over real radios | `tools/meshtastic_bridge.py` + Meshtastic nodes |

---

## Quick start (wallets & service)

### Requirements

- Rust (stable)
- Optional: [Meshtastic](https://meshtastic.org/) hardware + `pip install meshtastic`
- Optional: Solana / Anchor for the vault program

### Install & run the simple CLI

```bash
git clone https://github.com/krewdev/meshchain.git
cd meshchain
cargo build -p mesh -p meshchain-node

# Plain-English commands
./target/debug/mesh setup              # create a local test network
./target/debug/mesh demo               # practice transfers + vault hooks
./target/debug/mesh new-wallet         # create a spending wallet
./target/debug/mesh balance --wallet alice.json
./target/debug/mesh new-cold-key       # quantum-safe cold key (keep offline)
./target/debug/mesh how-cold-works
./target/debug/mesh security           # honest security posture
```

### Everyday wallet commands

```text
mesh new-wallet              Create a wallet file
mesh address                 Show your short mesh address
mesh balance                 Show balance
mesh send <address> <amount> Sign a payment
mesh status                  Network height & supply
```

Keys are stored under `./data/keys/` by default. **Never commit key files.** Keep cold keys off phones with cellular when possible.

---

## Architecture (short)

```
[ Wallet / cold key ] ──Meshtastic LoRa──► [ Mesh validators ]
                                                    │
                                         Mint / Burn / finality
                                                    │
[ Solana vault ] ◄── hybrid unlock (mesh id + burn + attestors)
```

- **Mesh = truth** for MESH balances when offline  
- **Solana vault** (optional) locks real assets; unlock needs mesh proof  
- **Not** a replacement for Bitcoin/Solana L1 security models  

---

## Repository layout

| Path | Purpose |
|------|---------|
| `crates/mesh` | Simple user CLI (`mesh`) |
| `crates/wallet` | Advanced wallet CLI |
| `crates/node` | Validator / simulator |
| `crates/proto` | Transactions, PQ crypto, privacy helpers |
| `crates/ledger` | Balances, nonces, PQ policy |
| `crates/transport` | Meshtastic framing + FRAG for large PQ sigs |
| `programs-mesh-bridge` | Solana hybrid vault (Anchor) |
| `tools/` | `meshtastic_bridge.py`, `relayer.py` |
| `docs/` | Protocol, security, hybrid lock, BTC vault design |

---

## Meshtastic network use

1. Flash devices with [Meshtastic firmware](https://meshtastic.org/docs/getting-started/).  
2. Use a **private channel** for funds traffic (not the public LongFast chat channel for real value).  
3. Run `tools/meshtastic_bridge.py` on a host with a USB/TCP-connected node.  
4. Point MeshChain node/relayer at that bridge for TX/BLOCK frames.

See [docs/HARDWARE.md](./docs/HARDWARE.md) and [docs/PROTOCOL.md](./docs/PROTOCOL.md).

---

## Security & privacy

We aim for **maximum practical rigor** (hybrid dual-control, PQ cold auth, fail-secure defaults).  
We do **not** claim perfect anonymity or unbreakable security.

- [SECURITY_HARDENING.md](./docs/SECURITY_HARDENING.md)  
- [HYBRID_LOCK.md](./docs/HYBRID_LOCK.md)  
- [QUANTUM_COLD_STORAGE.md](./docs/QUANTUM_COLD_STORAGE.md)  

**Do not put significant real value on this software without an independent audit.**

---

## Contributing

Issues and PRs welcome. Please:

- Do not commit secrets or `data/keys`  
- Keep the `mesh` CLI language plain and beginner-friendly  
- Document residual risks when changing consensus or vault rules  

---

## License

MIT — see [LICENSE](./LICENSE).

## Links

- [Meshtastic project](https://meshtastic.org/)  
- [Meshtastic docs](https://meshtastic.org/docs/)  
- [Meshtastic GitHub](https://github.com/meshtastic)  
