# Getting started

MeshChain wallets and services run on the [Meshtastic](https://meshtastic.org/) network.

## Install

```bash
git clone https://github.com/krewdev/meshchain.git
cd meshchain
cargo build -p mesh -p meshchain-node
```

## Simple wallet CLI (no UI required)

```bash
./target/debug/mesh setup              # local test network
./target/debug/mesh demo               # practice transfers
./target/debug/mesh new-wallet         # create a wallet
./target/debug/mesh balance --wallet alice.json
./target/debug/mesh new-cold-key       # quantum-safe cold key
./target/debug/mesh how-cold-works
./target/debug/mesh security
```

## Everyday commands

| Command | What it does |
|---------|----------------|
| `mesh new-wallet` | Create a spending wallet |
| `mesh address` | Show your short mesh address |
| `mesh balance` | Show how much MESH you have |
| `mesh send <addr> <amount>` | Sign a payment |
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
