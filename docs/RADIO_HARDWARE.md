# Hardware radio runbook (Meshtastic + MeshChain)

**TESTNET ONLY.** Use a private channel. Do not put mainnet balances on LongFast.

## Goal

Run everyday MeshChain traffic (submit, tip, balance, compact ACKs) over real LoRa nodes, with TCP only for multi-tx catch-up and cloud seed finality.

## Hardware

| Item | Notes |
|------|--------|
| 2–3× Meshtastic devices | T-Beam, Heltec, RAK, etc. |
| USB serial or TCP API | One “gateway” radio on the host |
| Host | Laptop or seed VM with `mesh` + relay |

## Channel

1. Open Meshtastic app → create **private** channel  
2. Name: **`MeshChain-Testnet-1`** (must match `testnet/network.json`)  
3. Share PSK only with test peers  
4. Region / modem preset: match your legal region (US/EU/…)

## Host software

```bash
git clone https://github.com/krewdev/meshchain.git && cd meshchain
cargo build -p mesh -p meshchain-node
pip install meshtastic   # if using serial bridge

# Public chain profile
./target/debug/mesh join-public
```

### Radio relay (gateway)

```bash
# Hardware serial example
export MESH_RADIO_FLAGS="--port /dev/ttyUSB0 --channel-index 0"
export MESH_RADIO_TCP=127.0.0.1:9100          # local observer/validator
export MESHCHAIN_DATA=./data                  # chain_state for air balance
./scripts/start_radio_relay.sh
```

Systemd (seed / always-on host):

```bash
sudo cp deploy/meshchain-radio-relay.service /etc/systemd/system/
sudo cp deploy/radio-relay.env.example /etc/meshchain/radio-relay.env
# edit MESH_RADIO_FLAGS / MESHCHAIN_DATA
sudo systemctl enable --now meshchain-radio-relay
```

Mock (no radio, lab only):

```bash
MESH_RADIO_FLAGS=--mock ./scripts/start_radio_relay.sh
```

## Wallet commands over air

```bash
# Air submit payment
mesh send MXXXXX-XXXXX-XXX 1 --wallet me.json --air --relay 127.0.0.1:9199

# Air balance (BalQuery / BalReply — one frame each way)
mesh balance --air --wallet me.json --relay 127.0.0.1:9199
```

## What fits on LoRa

| Frame | Type | Role |
|-------|------|------|
| Tx | 1 | Everyday transfer |
| Block (0–1 tx) | 2 | Small blocks only |
| Tip | 7 | Height gossip |
| BlockHint | 8 | Large block pointer |
| BalQuery / BalReply | 12 / 13 | Offline balance |
| AirBlockAck | 14 | Compact finality ACK (105 B) |

Multi-tx blocks and bulk catch-up stay on **TCP**. See [AIR_FINALITY.md](AIR_FINALITY.md) and [MESHTASTIC.md](MESHTASTIC.md).

## Catch-up (air + TCP hybrid)

Today:

1. Air **Tip** / **BlockHint** advertise height  
2. Lagging nodes issue TCP `BlocksRequest` / `SyncRequest`  
3. Full block archives on disk enable block-by-block catch-up  

Phase 2/3: selective Frag for missing heights over air (not yet default).

## Checklist before field use

- [ ] Private channel + correct region  
- [ ] Relay sees `chain_state` (`--data-dir`)  
- [ ] Local or public peer for finality (`--submit` / seed)  
- [ ] `mesh balance --air` returns a height  
- [ ] Cold keys offline when not spending  

## Troubleshooting

| Symptom | Check |
|---------|--------|
| No bal reply | Relay up? `MESHCHAIN_DATA`? Account registered? |
| Tx not final | TCP peer / seed online? Mempool dry-run? |
| Serial busy | Only one process owns `/dev/ttyUSB*` |
| Wrong mesh | Channel name / PSK / hop limit |

## Related

- [MESHTASTIC.md](MESHTASTIC.md)  
- [AIR_FINALITY.md](AIR_FINALITY.md)  
- [GETTING_STARTED.md](GETTING_STARTED.md)  
- `./scripts/e2e_air_path.sh` · `./scripts/e2e_air_finality.sh`  
