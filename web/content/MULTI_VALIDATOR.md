# Multi-machine validators (testnet)

MeshChain validators gossip over **TCP** (line-delimited JSON).  
Optional **Meshtastic** radios carry the same `MC` frames via `tools/meshtastic_bridge.py`.

## Lab: 3 validators on one computer

```bash
cargo build -p mesh -p meshchain-node
./target/debug/mesh testnet-setup
chmod +x scripts/run_local_validators.sh
./scripts/run_local_validators.sh
```

Ports: `9100`, `9101`, `9102`.

### Submit a payment

```bash
# sign a payment first
./target/debug/mesh send <to_short> 1 --wallet alice.json --out ./data/last_payment.json

# inject into gossip
./target/debug/meshchain-node submit-tx --tx ./data/last_payment.json --peer 127.0.0.1:9100
```

## Multi-host

On each machine, share the **same** `genesis.json` and that host’s `validator-N.json`.

```bash
# host A
meshchain-node run --data-dir ./data --validator-index 0 \
  --listen 0.0.0.0:9100 --peer hostB:9100 --peer hostC:9100

# host B
meshchain-node run --data-dir ./data --validator-index 1 \
  --listen 0.0.0.0:9100 --peer hostA:9100 --peer hostC:9100
```

Or via simple CLI:

```bash
mesh validator --index 0 --listen 0.0.0.0:9100 --peer 1.2.3.4:9100
```

## Meshtastic path

1. Private channel `MeshChain-Testnet-1`  
2. `python3 tools/meshtastic_bridge.py --port /dev/ttyUSB0`  
3. Run validators on hosts that also bridge radios (TCP still used for lab finality speed)

## Gossip messages

| Type | Purpose |
|------|---------|
| `hello` | Peer announce |
| `tx` | Mempool share |
| `block` | Proposed block |
| `block_ack` | Finality vote |

Finality: `ceil(2N/3)` ACKs.
