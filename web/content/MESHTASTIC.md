# Meshtastic path (air-first)

MeshChain is **designed** for Meshtastic. The public cloud testnet still finalizes on **TCP PoA** so faucet/scanner stay reliable. This doc is the **real radio path** (items 1–5).

## Architecture

```
[ mesh CLI --air ] ──MCHEX/tx_air──► [ mesh_radio_relay :9199 ]
                                            │
                         ┌──────────────────┼──────────────────┐
                         ▼                  ▼                  ▼
                   Meshtastic LoRa    TCP :9100–9102      tip gossip
                   (MC frames)        validators         (height/hash)
```

| Path | Use |
|------|-----|
| **Air (LoRa)** | Everyday Transfer (compact MC Tx), tip ads, 0–1 tx blocks if they fit |
| **TCP** | Multi-tx blocks, catch-up, multi-host, signed ACKs bulk |
| **Internet HTTP** | Faucet, scanner, Solana vault only |

## 1. Air-first tx submit

```bash
# Local validator + mock radio (no hardware)
python3 tools/mesh_radio_relay.py --mock --tcp 127.0.0.1:9100 --listen 127.0.0.1:9199 &

# Sign + inject MC frame into relay → validator mempool
mesh send MXXXX-… 1 --wallet me.json --air --relay 127.0.0.1:9199

# Or two-step
mesh send MXXXX-… 1 --wallet me.json          # writes last_payment.json
mesh air-submit --relay 127.0.0.1:9199
```

Under the hood:

1. Tx signed (ed25519)  
2. Encoded as **MC binary frame** (`MsgType::Tx = 1`)  
3. Sent as `tx_air` JSON + `MCHEX <hex>` + normal `tx` for TCP peers  
4. Validator accepts `tx_air` → mempool → leader includes → finality  

## 2. Radio relay as first-class service

```bash
# Lab mock
python3 tools/mesh_radio_relay.py --mock --tcp 127.0.0.1:9100

# Hardware (private channel MeshChain-Testnet-1)
python3 tools/mesh_radio_relay.py \
  --port /dev/ttyUSB0 --channel-index 0 \
  --tcp 127.0.0.1:9100 --listen 127.0.0.1:9199

# systemd (on seed / operator host)
sudo cp deploy/meshchain-radio-relay.service /etc/systemd/system/
sudo cp deploy/radio-relay.env.example /etc/meshchain/radio-relay.env
# edit MESH_RADIO_FLAGS=--port /dev/ttyUSB0 ...
sudo systemctl enable --now meshchain-radio-relay
```

## 3. Air block policy (1 tx)

| Medium | Max txs / block |
|--------|------------------|
| TCP gossip | up to **16** |
| LoRa / air inject | **1** (`AIR_MAX_TXS_PER_BLOCK`) |

Full multi-tx blocks are **not** pushed over LoRa. Relay sends `block_hint` + `tip` instead.  
`block_air` inject is rejected if `tx_count > 1`.

## 4. Mesh tip gossip

Every `MESH_RADIO_TIP_SECS` (default 30s) the relay advertises:

```json
{ "chain_id", "height", "tip_hash_hex" }
```

as an MC **Tip** frame (type 7). Validators log air tips and request block catch-up if behind.

## 5. Internet optional (spend path)

| Action | Needs internet? |
|--------|-----------------|
| Transfer over local validator + radio | **No** (same mesh / LAN) |
| Observer catch-up via LoRa tip + local peer | Prefer local TCP |
| Faucet drip | **Yes** (HTTP) |
| Scanner UI | **Yes** (HTTP) |
| Solana vault deposit/withdraw | **Yes** |

Public seed `34.172.103.125` remains the internet hub for join/faucet.  
**Mesh-native spend** = local `meshchain-node` + `mesh_radio_relay` + radio.

## Frame types (`MC` magic)

| Type | Name | Payload |
|------|------|---------|
| 1 | Tx | bincode Tx |
| 2 | Block | bincode Block (≤1 tx, fits MTU) |
| 3 | BlockAck | ack fields |
| 7 | Tip | height + tip hash |
| 8 | BlockHint | height + hash when block too big |
| 20 | GossipJson | small JSON hello/tx/ack |

Max payload **200** bytes (+ 6-byte header).

## Mock e2e (no hardware)

```bash
cargo build -p mesh -p meshchain-node
./scripts/e2e_air_path.sh
```

## Channel safety

- Use private channel **`MeshChain-Testnet-1`**  
- **Do not** put funds traffic on public LongFast  
- Never send seed phrases over mesh  

## Honesty

- Live **public** multi-host finality is still TCP PoA on GCE.  
- This path makes **Meshtastic first-class** for submit + tip + multi-hop relay.  
- Full offline village finality (validators only on LoRa, no TCP) is the next milestone after this air path is boring.
