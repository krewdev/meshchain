# MeshChain Public Testnet (`meshchain-testnet-1`)

**This is a test network.**  
tMESH has **no cash value**. State may be wiped. Never send mainnet crypto expecting a return.

Live parameters: [`testnet/network.json`](../testnet/network.json)  
Site: https://meshchain-sigma.vercel.app/docs/?doc=TESTNET

---

## Goals

1. Let anyone run wallets and practice hold/spend without real money  
2. Publish **stable chain_id + channel name** so experimenters can align  
3. Exercise hybrid vault logic against **Solana devnet** (not mainnet)  
4. Gather feedback before any mainnet discussion  

---

## Network parameters

| Field | Value |
|-------|--------|
| **chain_id** | `meshchain-testnet-1` |
| **Token** | tMESH (6 decimals) |
| **Consensus** | PoA, ≥3 validators, `ceil(2N/3)` finality |
| **Block production** | **Demand-driven** — blocks when mempool has txs; empty idle slots skipped |
| **slot_secs** | 30 (min gap / idle tick — **not** “empty block every 30s”) |
| **Meshtastic channel name** | `MeshChain-Testnet-1` |
| **Frame magic** | `MC` |
| **Solana bridge** | **devnet only** |
| **PQ threshold** | ≥ 100 tMESH needs cold key; vault burns always PQ |

Full JSON: `testnet/network.json`.

> **Cadence:** If the tip looks “stuck,” check the faucet / whether any txs are submitting. Height only advances when there is work (plus genesis). See [PROTOCOL.md](./PROTOCOL.md) → Production cadence.

---

## Quick start (public seed — recommended)

```bash
git clone https://github.com/krewdev/meshchain.git
cd meshchain
cargo build -p mesh -p meshchain-node

./target/debug/mesh join-public
./target/debug/mesh new-wallet --name me.json --publish
./target/debug/mesh faucet-drip --wallet me.json
./target/debug/mesh balance --wallet me.json
./target/debug/mesh send <MESH_NAME> 1 --wallet me.json --submit 34.172.103.125:9100
```

Full happy path: [GETTING_STARTED.md](./GETTING_STARTED.md)

### Local lab only

```bash
./target/debug/mesh testnet-setup
./target/debug/mesh testnet-info
./target/debug/mesh demo
```

---

## Air path (Meshtastic)

| Feature | Command / note |
|---------|----------------|
| Air submit | `mesh send … --air --relay 127.0.0.1:9199` |
| Air balance | `mesh balance --air` (compact BalQuery/BalReply frames) |
| Radio relay | `./scripts/start_radio_relay.sh` or systemd unit |
| Air finality frames | AirBlockAck type 14 — [AIR_FINALITY.md](./AIR_FINALITY.md) |

Channel name: **`MeshChain-Testnet-1`** (private; do not put balances on LongFast).

---

## Solana hybrid vault (devnet)

| Feature | Note |
|---------|------|
| Program | `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx` |
| Deposit → mint | Relayer polls DepositRecord PDAs, mints via `mint-for-deposit --peer` |
| Deferred mints | Retries until mesh short id is registered |
| Docs | [SOLANA_BRIDGE.md](./SOLANA_BRIDGE.md) · [HYBRID_LOCK.md](./HYBRID_LOCK.md) |

---

## Join over real Meshtastic radios (optional)

1. Flash nodes with [Meshtastic](https://meshtastic.org/docs/getting-started/).  
2. Create a **private** channel named **`MeshChain-Testnet-1`**.  
3. Share the channel PSK **only** with people you trust for testing (PSK is not custody, but it exposes traffic).  
4. Run `tools/meshtastic_bridge.py` on a host with a radio.  
5. Keep **mainnet** funds off this channel.

> There is no global single LoRa cloud. “Public testnet” means **shared parameters + software**.  
> Your mesh is still local/regional unless you peer with others who use the same channel.

---

## Faucet (test MESH only)

| Source | How |
|--------|-----|
| Local | `mesh testnet-setup` creates a faucet key with lots of tMESH |
| Demo | `mesh demo` moves sample balances between alice/bob |
| Operator | A human running a faucet wallet can send tMESH to your short address |

**No Solana/ETH/BTC mainnet faucet.** Bridge tests use **Solana devnet** airdrops separately.

---

## Solana side (devnet)

| Item | Value |
|------|--------|
| Cluster | `devnet` |
| Program | `programs-mesh-bridge` (deploy yourself or wait for published program id) |
| Hybrid unlock | mesh short id + burn + attestors |

```bash
# Example (when program is deployed):
solana config set --url devnet
solana airdrop 2
# anchor deploy — from programs-mesh-bridge
```

---

## Roles

| Role | Who | Job |
|------|-----|-----|
| **Wallet user** | Anyone | Hold/send tMESH, try cold keys |
| **Validator** | Operators | Run node software, finalize blocks (local or shared lab) |
| **Attestor** | Operators | Co-sign hybrid unlocks on Solana devnet |
| **Faucet op** | Volunteers | Drip tMESH to testers |

---

## Reset policy

Operators may reset **meshchain-testnet-1** at any time:

- New genesis  
- Balances zeroed  
- Channel PSK rotated  

Assume **nothing persists**.

---

## Roadmap on testnet

1. ✅ Publish chain_id + docs + CLI `testnet-*`  
2. ✅ Multi-machine validator gossip (TCP) + lab script — [MULTI_VALIDATOR.md](./MULTI_VALIDATOR.md)  
3. ✅ Solana **devnet** program id reserved + binary built — [SOLANA_DEVNET.md](../testnet/SOLANA_DEVNET.md)  
4. ✅ Public attestor list — [attestors.json](../testnet/attestors.json)  
5. ⬜ On-chain program deploy (needs ~3.3 devnet SOL when faucet allows)  
6. ⬜ Only after audit + soak: discuss mainnet parameters  

### Solana program (devnet)

| | |
|--|--|
| **Program ID** | `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx` |
| **Status** | Built & ID reserved; on-chain deploy pending faucet rent |

### Attestors

See https://meshchain-sigma.vercel.app/testnet/attestors.json  

---

## Safety checklist

- [ ] I understand tMESH is worthless  
- [ ] I will not deposit mainnet SOL into unknown programs  
- [ ] Cold keys for tests are separate from real savings  
- [ ] I verified donation addresses only on GitHub if donating  

---

## Support

- GitHub: https://github.com/krewdev/meshchain  
- Donate (optional, real chains): [DONATE.md](./DONATE.md)  
