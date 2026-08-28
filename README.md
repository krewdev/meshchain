# MeshChain

**Hold Solana value that cannot move with internet alone — it needs the mesh.**

MeshChain is the disconnected last mile and the extra lock on a Solana vault: deposit USDC/SOL while you have LTE, sign a bounded IOU on Meshtastic, settle when a gateway plus mesh witnesses appear. A laptop on hotel Wi-Fi cannot drain the vault by itself.

tMESH is a **test counter**. Product settlement is vault USDC/SOL. Not a messenger. Not “Solana tx over LoRa.” See [PRODUCT.md](./docs/PRODUCT.md).

> Built **for** the Meshtastic ecosystem. Not an official Meshtastic Foundation product — community software that runs **on** Meshtastic radios and channels.

**Live site:** [meshchain-sigma.vercel.app](https://meshchain-sigma.vercel.app) · [**Wallet**](https://meshchain-sigma.vercel.app/wallet/) · [Docs](https://meshchain-sigma.vercel.app/docs/) · [**Testnet**](https://meshchain-sigma.vercel.app/docs/?doc=TESTNET) · [Scanner](https://34.172.103.125.sslip.io/) · [Faucet](https://meshchain-sigma.vercel.app/faucet/) · [**Discord**](https://discord.gg/9YXVtXf2yX) · [**Launch post on X**](https://x.com/RealEyedropz/status/2085953042260578638)

[![Discord](https://img.shields.io/badge/Discord-MeshChain-5865F2?logo=discord&logoColor=white)](https://discord.gg/9YXVtXf2yX)
[![X launch](https://img.shields.io/badge/X-Launch_post-000000?logo=x&logoColor=white)](https://x.com/RealEyedropz/status/2085953042260578638)

[Meshtastic](https://meshtastic.org/) · [GitHub](https://github.com/krewdev/meshchain) · [Discord](https://discord.gg/9YXVtXf2yX) · [Day-1 posts](./marketing/copy/day1-posts.md) · [Security](./docs/SECURITY_HARDENING.md) · [Hybrid vault](./docs/HYBRID_LOCK.md) · [Donate](./docs/DONATE.md)

---

## Public testnet (`meshchain-testnet-1`)

**TESTNET ONLY — tMESH has no cash value. State may be wiped.**

| Parameter | Value |
|-----------|--------|
| chain_id | `meshchain-testnet-1` |
| Token | tMESH |
| Channel | `MeshChain-Testnet-1` |
| Solana | **devnet** for bridge experiments |
| Seed peer | `34.172.103.125:9100` |
| Scanner | https://34.172.103.125.sslip.io/ |
| Faucet | https://faucet.34.172.103.125.sslip.io/info |
| Params | [`testnet/network.json`](./testnet/network.json) · [`seeds.json`](./testnet/seeds.json) |

### Join the **shared** public seed (recommended)

```bash
git clone https://github.com/krewdev/meshchain.git && cd meshchain
cargo build -p mesh -p meshchain-node

./target/debug/mesh join-public
./target/debug/mesh new-wallet --name me.json --publish
./target/debug/mesh faucet-drip --wallet me.json
./target/debug/mesh balance --wallet me.json
# optional: offline balance over radio relay
./scripts/start_radio_relay.sh &
./target/debug/mesh balance --air --wallet me.json
```

### Local-only lab (your own chain)

```bash
./target/debug/mesh testnet-setup
./target/debug/mesh demo
./scripts/start_testnet_host.sh   # optional local validators + faucet
```

### What’s live now

| Feature | Docs / how |
|---------|------------|
| Browser wallet (create / faucet / send / radio) | [WALLET](./docs/WALLET.md) · https://meshchain-sigma.vercel.app/wallet/ |
| Public seed + faucet + scanner | [TESTNET](./docs/TESTNET.md) · [STATUS](./docs/STATUS.md) |
| Air balance / air submit | [MESHTASTIC](./docs/MESHTASTIC.md) · `mesh balance --air` |
| Compact AirBlockAck (LoRa finality frames) | [AIR_FINALITY](./docs/AIR_FINALITY.md) · `./scripts/e2e_air_finality.sh` |
| Solana vault deposit → tMESH mint | [SOLANA_BRIDGE](./docs/SOLANA_BRIDGE.md) · systemd `meshchain-relayer` |
| Hybrid dual-control unlock | [HYBRID_LOCK](./docs/HYBRID_LOCK.md) |

Guide: [**Getting started**](./docs/GETTING_STARTED.md) · [TESTNET](./docs/TESTNET.md) · [Run a node](./docs/RUN_A_NODE.md) · [Status](./docs/STATUS.md)

**Live multi-host:** seed `34.172.103.125:9100` · remote observer `35.192.20.103:9100`

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
| Use a simple wallet (no UI app required) | `mesh` CLI · [Getting started](./docs/GETTING_STARTED.md) |
| Run a local test network | `mesh testnet-setup` + `mesh demo` |
| Check balance offline (radio) | `mesh balance --air` + radio relay |
| Send over LoRa path | `mesh send … --air` |
| Cold storage off Wi‑Fi / 5G | `mesh new-cold-key` + [hybrid vault](./docs/HYBRID_LOCK.md) |
| Bridge SOL (devnet) → tMESH | [SOLANA_BRIDGE](./docs/SOLANA_BRIDGE.md) · deposit + relayer |
| Talk over real radios | `tools/mesh_radio_relay.py` + Meshtastic nodes |

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

**Guide:** [docs/MESHTASTIC.md](./docs/MESHTASTIC.md) (air-first submit, tip gossip, relay)

1. Flash devices with [Meshtastic firmware](https://meshtastic.org/docs/getting-started/).  
2. Private channel **MeshChain-Testnet-1** (not public LongFast for funds).  
3. Run local validator + relay, then air-submit:
   ```bash
   ./scripts/start_radio_relay.sh &          # mock unless MESH_RADIO_PORT set
   mesh send <NAME> 1 --wallet me.json --air --relay 127.0.0.1:9199
   ```
4. Everyday spends use compact **MC Tx** frames; multi-tx blocks stay on TCP.  
5. Faucet / scanner / Solana vault still need the internet.

See also [docs/HARDWARE.md](./docs/HARDWARE.md) · [docs/PROTOCOL.md](./docs/PROTOCOL.md).

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
