# Getting started

**TESTNET ONLY — tMESH has no cash value. State may be wiped.**

MeshChain is a mesh-native ledger for [Meshtastic](https://meshtastic.org/). Everyday spend can stay offline; large vaults optionally hybrid-lock on Solana **devnet**.

## Happy path (public testnet)

One path that should always work on the live seed:

```bash
git clone https://github.com/krewdev/meshchain.git && cd meshchain
cargo build -p mesh -p meshchain-node

# 1) Install public genesis + seeds
./target/debug/mesh join-public

# 2) Wallet + publish (register on-chain when peer is reachable)
./target/debug/mesh new-wallet --name me.json --publish

# 3) Faucet drip (needs internet → public faucet)
./target/debug/mesh faucet-drip --wallet me.json

# 4) Check balance (scanner / local tip)
./target/debug/mesh balance --wallet me.json

# 5) Send tMESH to a friend (mesh name or hex)
./target/debug/mesh send MXXXXX-XXXXX-XXX 1 --wallet me.json --submit 34.172.103.125:9100

# 6) Optional: balance over radio relay (no internet)
./scripts/start_radio_relay.sh &          # mock unless MESH_RADIO_PORT set
./target/debug/mesh balance --air --wallet me.json
```

| Live service | URL |
|--------------|-----|
| Site / docs | https://meshchain-sigma.vercel.app |
| Scanner | https://34.172.103.125.sslip.io/ |
| Faucet | https://meshchain-sigma.vercel.app/faucet/ |
| Seed peer | `34.172.103.125:9100` |

If faucet or peer is down: `./scripts/status_public_seed.sh`

## Install only

```bash
git clone https://github.com/krewdev/meshchain.git
cd meshchain
cargo build -p mesh -p meshchain-node
```

## Mesh names

Each wallet gets a memorable name (unique, not pickable):

```text
MVGQK7-82943-QJC
```

- Starts with **`M`**
- Crockford base32 (no I / O / L confusion)
- 1:1 with the 8-byte short id
- Hex short id still works for power users

```bash
mesh new-wallet
# → Mesh name: MVGQK7-82943-QJC

mesh send MVGQK7-82943-QJC 5 --wallet me.json --submit 34.172.103.125:9100
```

## Everyday commands

| Command | What it does |
|---------|----------------|
| `mesh join-public` | Public genesis + seeds |
| `mesh new-wallet --name me.json --publish` | Create + register |
| `mesh faucet-drip --wallet me.json` | Drip testnet tMESH |
| `mesh balance --wallet me.json` | Balance via scanner/local tip |
| `mesh balance --air` | Balance via radio relay (offline) |
| `mesh send <name> <amount>` | Pay a mesh name (or hex) |
| `mesh status` | Height & supply |
| `mesh new-cold-key` | Long-term PQ cold key |

## Local lab (own chain)

```bash
./target/debug/mesh testnet-setup
./target/debug/mesh demo
./scripts/start_testnet_host.sh   # optional local validators + faucet
```

## Real Meshtastic radios

1. Flash devices with [Meshtastic firmware](https://meshtastic.org/docs/getting-started/).
2. Use a **private channel** for funds (not public LongFast for balances).
3. `tools/mesh_radio_relay.py` or `./scripts/start_radio_relay.sh`.
4. Keep cold keys offline — radio off while holding large value.

Docs: [MESHTASTIC.md](MESHTASTIC.md) · [AIR_FINALITY.md](AIR_FINALITY.md) · [SOLANA_BRIDGE.md](SOLANA_BRIDGE.md)

## Solana hybrid vault (optional, devnet)

Deposit SOL into the vault → relayer mints tMESH. Cash-out needs mesh burn + 2 attestors.

```bash
# Web: deposit guide + mint watch
# https://meshchain-sigma.vercel.app/bridge/

# Ops / e2e helpers
./scripts/e2e_hybrid_roundtrip.sh          # deposit → mint → burn(peer) → withdraw
./scripts/start_relayer.sh                 # or systemd meshchain-relayer on seed

# Peer burn (public seed)
meshchain-node burn-for-withdraw \
  --data-dir ./data --wallet ./data/keys/me.json --cold ./data/keys/cold.json \
  --amount <base_units> --dest-sol <SolPubkey> --asset-id 1 \
  --peer 34.172.103.125:9100
```

See [SOLANA_BRIDGE.md](SOLANA_BRIDGE.md) and [HYBRID_LOCK.md](HYBRID_LOCK.md).

## Seed ops

```bash
./scripts/status_public_seed.sh    # core HTTP/peer checks
./scripts/seed_health.sh           # + relayer unit when on host
./scripts/deploy_public_seed.sh    # pull + release build + restart (no wipe)
```

## Safety

Do **not** put real mainnet value on this software without an audit.  
tMESH is worthless and wipeable. Read [SECURITY_HARDENING.md](SECURITY_HARDENING.md).
