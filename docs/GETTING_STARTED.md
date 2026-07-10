# Getting started

MeshChain wallets and services run on the [Meshtastic](https://meshtastic.org/) network.

## Install

```bash
git clone https://github.com/krewdev/meshchain.git
cd meshchain
cargo build -p mesh -p meshchain-node
```

## Public testnet (recommended)

```bash
./target/debug/mesh testnet-setup      # chain_id = meshchain-testnet-1
./target/debug/mesh testnet-info
./target/debug/mesh demo
```

tMESH has **no cash value**. See [TESTNET.md](TESTNET.md).

## Mesh names

Each wallet gets a **memorable mesh name** derived from its key (unique, not pickable):

```text
MVGQK7-82943-QJC
```

- Starts with **`M`**
- Crockford base32 (no confusing I / O / L)
- 1:1 with the secure 8-byte short id
- Hex id still works for power users

```bash
mesh new-wallet
# → Mesh name: MVGQK7-82943-QJC

mesh send MVGQK7-82943-QJC 5
```

## Simple wallet CLI (no UI required)

```bash
./target/debug/mesh testnet-setup      # public testnet (preferred)
./target/debug/mesh demo               # practice transfers
./target/debug/mesh new-wallet         # create a wallet + mesh name
./target/debug/mesh address            # show mesh name
./target/debug/mesh balance --wallet alice.json
./target/debug/mesh new-cold-key       # quantum-safe cold key
./target/debug/mesh how-cold-works
./target/debug/mesh security
```

## Everyday commands

| Command | What it does |
|---------|----------------|
| `mesh new-wallet` | Create a wallet + show mesh name |
| `mesh address` | Show mesh name + hex id |
| `mesh balance` | Show how much MESH you have |
| `mesh send <name> <amount>` | Pay a mesh name (or hex) |
| `mesh status` | Network height & supply |
| `mesh new-cold-key` | Long-term cold storage key |

## Real Meshtastic radios

1. Flash devices with [Meshtastic firmware](https://meshtastic.org/docs/getting-started/).
2. Use a **private channel** for funds (not public LongFast for real value).
3. Run `tools/meshtastic_bridge.py` with a USB/TCP node.
4. Keep cold keys offline — radio off while holding.

## Safety

Do **not** put large real value on this software without an audit.  
Read [Security hardening](SECURITY_HARDENING.md) and [Hybrid lock](HYBRID_LOCK.md).
