# Run a MeshChain node (for everyone)

**Goal:** other people can create wallets, relay traffic, and (with approval) produce blocks — not only the original lab host.

## Live public seed

| | |
|--|--|
| **Host** | Google Cloud `meshchain-testnet` · `us-central1-a` |
| **Public IP** | **`34.172.103.125`** |
| **Faucet (HTTPS)** | https://faucet.34.172.103.125.sslip.io/info |
| **Scanner (HTTPS)** | https://34.172.103.125.sslip.io/ |
| **Faucet (HTTP)** | http://34.172.103.125:8787 |
| **Scanner (HTTP)** | http://34.172.103.125:8788/ |
| **Submit / seed** | `34.172.103.125:9100` (also 9101, 9102) |
| **Genesis** | [`testnet/published/genesis.json`](../testnet/published/genesis.json) |
| **Seeds** | [`testnet/seeds.json`](../testnet/seeds.json) |
| **Catch-up** | Observers request full `chain_state` via gossip `SyncRequest` / `SyncResponse` |

```bash
# Observer
cp testnet/published/genesis.json data/genesis.json
./target/debug/mesh observer --peer 34.172.103.125:9100

# Wallet
./target/debug/mesh new-wallet --name me.json
./target/debug/mesh register --wallet data/keys/me.json --submit 34.172.103.125:9100
# drip: POST http://34.172.103.125:8787/drip with mesh_name + public_key_hex
./target/debug/mesh send <NAME> 1 --wallet data/keys/me.json --submit 34.172.103.125:9100
```

MeshChain testnet uses **Proof of Authority (PoA)**: a fixed list of validator public keys is in `genesis.json`.  
That is different from Bitcoin/Ethereum open mining.

---

## Three ways to “run a node”

| Role | Who | Needs genesis seat? | What you run |
|------|-----|---------------------|--------------|
| **A. Wallet / user** | Anyone | No | `mesh` CLI only |
| **B. Full node (observer)** | Anyone | No (needs shared genesis + seeds) | `meshchain-node run --observer` |
| **C. Validator (producer)** | Operators on the set | **Yes** — pubkey in genesis | `meshchain-node run --validator-index N` |

Also always available:

| Role | Notes |
|------|--------|
| **Private lab** | `mesh testnet-setup` + 3 local validators — **your own** chain, not shared public state |
| **Faucet / scanner host** | Public HTTP services; can be separate from validators |

---

## A. Wallet user (no node required)

```bash
git clone https://github.com/krewdev/meshchain.git
cd meshchain
cargo build -p mesh -p meshchain-node

./target/debug/mesh new-wallet --name my.json
# When a public peer is up:
./target/debug/mesh register --wallet data/keys/my.json --submit SEED_HOST:9100
./target/debug/mesh send <NAME> 1 --wallet data/keys/my.json --submit SEED_HOST:9100
```

You need a **public seed** (faucet/scanner/submit endpoint) published in `testnet/network.json` → `endpoints`.

---

## B. Full node / observer (anyone — recommended open path)

Observers:

- Connect to seed peers over TCP  
- Relay txs / blocks  
- Track `chain_state` (same chain_id)  
- **Do not** produce blocks or cast validator ACKs  

### 1. Get shared genesis

Coordinator publishes (HTTPS or git):

- `genesis.json` (same for everyone on `meshchain-testnet-1`)  
- `testnet/seeds.json` (bootstrap peers)

```bash
mkdir -p ~/mesh-node/data
# download published files into data/
cp /path/to/published/genesis.json ~/mesh-node/data/
```

### 2. Run observer

```bash
cd meshchain
cargo build -p meshchain-node --release

./target/release/meshchain-node run \
  --data-dir ~/mesh-node/data \
  --observer \
  --listen 0.0.0.0:9100 \
  --peer SEED1:9100 \
  --peer SEED2:9100
```

Or:

```bash
./target/debug/mesh --dir ~/mesh-node/data observer \
  --listen 0.0.0.0:9100 \
  --peer SEED1:9100
```

Open **TCP 9100** (or your listen port) if you want others to peer with you.

### 3. Optional: local submit endpoint

Point wallets at your node if you relay:

```bash
mesh send … --submit YOUR_IP:9100
```

---

## C. Become a **validator** (block producer)

Validators are **permissioned** for testnet safety (PoA). Process:

### Step 1 — Generate a key (you keep the secret)

```bash
./target/debug/mesh validator-keygen --name my-validator.json
```

Prints:

- `public_hex` (safe to share)  
- file path (secret — **never** commit or post in Discord)

### Step 2 — Apply to the set

Open a GitHub issue / Discord `#validators` with:

```json
{
  "role": "validator-applicant",
  "chain_id": "meshchain-testnet-1",
  "public_hex": "<from keygen>",
  "operator_name": "your-handle",
  "contact": "discord or email",
  "region": "e.g. us-east",
  "listen": "host.example.com:9100",
  "uptime_plan": "VPS / home / always-on?"
}
```

Template: [`testnet/operator_application.example.json`](../testnet/operator_application.example.json)

### Step 3 — Coordinator publishes new genesis

Maintainers rebuild genesis including your `public_hex`, publish:

- new `genesis.json`  
- your **index** (e.g. validator index `3`)  
- updated `seeds.json`

**Note:** Adding validators is a **set change** — often a coordinated restart / reset on testnet. See reset policy in TESTNET.md.

### Step 4 — You run as producer

```bash
# data/genesis.json = published shared genesis
# data/keys/validator-3.json = YOUR key (rename from my-validator.json)
# index must match your slot in genesis.validators

./target/release/meshchain-node run \
  --data-dir ./data \
  --validator-index 3 \
  --listen 0.0.0.0:9100 \
  --peer seed1.example:9100 \
  --peer seed2.example:9100
```

Or:

```bash
mesh validator --index 3 --listen 0.0.0.0:9100 --peer seed1:9100 --peer seed2:9100
```

---

## D. Private lab (always free, isolated chain)

Anyone can run **their own** 3-validator network without asking anyone:

```bash
mesh testnet-setup
./scripts/run_local_validators.sh
# or: ./validator-automation/mesh-validator start
```

This is great for development. It is **not** the same balances/history as the public host unless you intentionally share that genesis.

---

## Why not “permissionless validators” yet?

| Design | Tradeoff |
|--------|----------|
| **PoA (now)** | Simple, fits testnet + radio; set is known |
| Open PoS / mining | Needs staking, sybil resistance, harder on LoRa |
| Dynamic validator txs | Protocol upgrade (future) |

Roadmap: keep PoA for testnet-1; expand set via applications; later consider on-chain set updates.

---

## Coordinator checklist (maintainers)

1. Publish **stable** `genesis.json` + `seeds.json` on site/GitHub releases  
2. Keep ≥1 public seed with open `9100` (or private VPN for trusted ops)  
3. Document faucet/scanner HTTP URLs in `testnet/network.json`  
4. Accept operator applications; rotate genesis when adding seats  
5. Never put validator **secrets** in git  

---

## Security

- Validator key = **can sign blocks** for that genesis — treat like a production server key  
- Observers: no block power; still sandbox the process  
- Don’t expose faucet mint keys publicly  
- Testnet only — tMESH has no value  

---

## Related docs

- [MULTI_VALIDATOR.md](./MULTI_VALIDATOR.md) — gossip details  
- [TESTNET.md](./TESTNET.md) — parameters  
- [CLOUD.md](./CLOUD.md) — VPS / GCE host  
- [validator-automation](../validator-automation/README.md) — local ops package  
