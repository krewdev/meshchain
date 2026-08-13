# Air finality (LoRa / Meshtastic)

**Status:** Phase 1 shipped (compact ACKs + relay bridge). Public seed still finalizes primarily over **TCP PoA**; air path can now carry the same ACKs when radios (or mock relay) are attached.

**TESTNET ONLY** — tMESH has no cash value.

## Why this exists

Meshtastic LoRa frames are tiny (~200 B payload after MeshChain’s 6-byte header). The old hex/bincode `BlockAck` (type 3) is ~288 B and **never fit** a single frame, so radio finality was silent-fail. Everyday spend still sealed only because validators shared TCP.

## Phase 1 (done)

| Piece | Detail |
|-------|--------|
| **AirBlockAck** (type 14) | Fixed layout: `height[u8 LE×8] \| hash[32] \| validator_index[u8] \| sig[64]` = **105 B** payload |
| **Node** | Producers emit AirBlockAck over `--radio`; RX counts toward `FinalityTracker` (ceil(2N/3)) |
| **Relay** | Maps index→genesis pubkey; TCP ↔ air for BlockAck; still answers BalQuery |
| **Blocks on air** | 0–1 tx via `encode_block_for_air`; multi-tx stay TCP + BlockHint |

```
[validator A] --TCP BlockAck--> [mesh_radio_relay] --AirBlockAck--> LoRa
[validator B] <--TCP BlockAck-- [mesh_radio_relay] <--AirBlockAck-- LoRa
```

Same finality threshold as TCP: **ceil(2N/3)** authorized ACKs (producer sig counts as one).

## Phase 2 (in progress)

1. **Lab e2e (shipped)** — `./scripts/e2e_air_finality.sh`  
   3 validators + mock radio relay; asserts multi-node tip finality, air balance, and AirBlockAck framing/bridge.  
   Full “TCP-off” finality still needs `--radio` on each producer (Phase 3).
2. **Catch-up (hybrid, current)**  
   - Air **Tip** / **BlockHint** → lagging node logs tip  
   - TCP **`BlocksRequest` / `BlocksResponse`** loads archived `data/blocks/{h}.json`  
   - Full state snapshot via `SyncResponse` (observers)  
   Selective Frag over air for missing heights is next.
3. **Hardware runbook** — [RADIO_HARDWARE.md](RADIO_HARDWARE.md)
4. **Rate limits** — per-name BalQuery already limited; add per-peer air ACK storm guard.

```bash
./scripts/e2e_air_finality.sh
./scripts/e2e_air_path.sh
```

## Phase 3 (later)

- Full LoRa-only validator set (no TCP required between producers).
- PQ-heavy paths only via Frag (already framed); vault burns stay cold+PQ.
- Open consensus (beyond coordinator PoA seats).

## Operator commands

```bash
# Lab: mock radio + local validators
./scripts/start_radio_relay.sh &
meshchain-node run --data-dir ./data/v0 --validator-index 0 \
  --listen 0.0.0.0:9100 --peer 127.0.0.1:9101 --radio /dev/null   # or real port

# Balance without internet (relay needs chain_state)
mesh balance --air --wallet me.json

# Air spend path
mesh send MXXXX-… 1 --wallet me.json --air --relay 127.0.0.1:9199
```

Public seed: radio relay on `127.0.0.1:9199` (mock unless `MESH_RADIO_PORT` set). Finality remains TCP between the three cloud producers; air ACKs are ready when a radio-linked operator joins.

## Related

- [MESHTASTIC.md](MESHTASTIC.md) — frames, air balance, relay
- [STATUS.md](STATUS.md) — live endpoints and integrity posture
- [MULTI_VALIDATOR.md](MULTI_VALIDATOR.md) — TCP PoA gossip
