# Phase B — Multi-operator MeshChain testnet

How **other people** run nodes against the shared public seed, and how maintainers add **block producers**.

**TESTNET ONLY.** Adding producers usually **resets** chain state (new tip from genesis).

---

## Roles

| Role | Permission | Command |
|------|------------|---------|
| **Wallet user** | Anyone | `mesh join-public` → register → faucet |
| **Observer** | Anyone | `mesh observer --peer SEED:9100` |
| **Producer (validator)** | Listed in `genesis.validators` | `meshchain-node run --validator-index N` |
| **Coordinator** | Maintains published genesis + seeds | this doc + scripts |

Live seed (lab):

| | |
|--|--|
| IP | `34.172.103.125` |
| Peers | `:9100` `:9101` `:9102` |
| Scanner | https://34.172.103.125.sslip.io/ |
| Faucet | https://faucet.34.172.103.125.sslip.io/info |
| Genesis | `testnet/published/genesis.json` |
| Seeds | `testnet/seeds.json` |

---

## Path A — Run an **observer** (no permission)

Anyone, any machine with outbound TCP:

```bash
git clone https://github.com/krewdev/meshchain.git && cd meshchain
cargo build -p meshchain-node -p mesh --release

mkdir -p ~/mesh-observer/data
curl -fsSL https://meshchain-sigma.vercel.app/testnet/published/genesis.json \
  -o ~/mesh-observer/data/genesis.json
# or: cp testnet/published/genesis.json ~/mesh-observer/data/

./target/release/meshchain-node run \
  --data-dir ~/mesh-observer/data \
  --observer \
  --listen 0.0.0.0:9100 \
  --peer 34.172.103.125:9100 \
  --peer 34.172.103.125:9101

# Catch-up: node sends SyncRequest; seed replies with SyncResponse (hardened).
```

CLI wrapper:

```bash
./target/release/mesh --dir ~/mesh-observer/data observer \
  --peer 34.172.103.125:9100
```

Open **TCP 9100** on your host if you want others to peer with you; then open a PR to add your host to `testnet/seeds.json`.

---

## Path B — Become a **producer** (application)

### 1. Operator generates a key (secret stays private)

```bash
cargo build -p mesh
./target/debug/mesh validator-keygen --name my-validator.json
# → prints public_hex  (SAFE to share)
# → file data/keys/my-validator.json  (SECRET)
```

### 2. Apply

Open a GitHub issue titled `validator application` with
[`testnet/operator_application.example.json`](../testnet/operator_application.example.json):

```json
{
  "role": "validator-applicant",
  "chain_id": "meshchain-testnet-1",
  "public_hex": "<64 hex chars>",
  "operator_name": "your-handle",
  "contact": "discord or email",
  "listen": "your.host.or.ip:9100",
  "region": "us-east",
  "uptime_plan": "VPS 24/7"
}
```

### 3. Coordinator extends genesis

```bash
python3 scripts/genesis_add_validators.py \
  --genesis testnet/published/genesis.json \
  --add <PUBLIC_HEX> \
  --out /tmp/genesis.next.json \
  --note "add @handle"

# review
python3 -c "import json;g=json.load(open('/tmp/genesis.next.json'));print(len(g['validators']))"
```

### 4. Coordinated restack (testnet wipe)

**Seed host (lab):**

```bash
ASSUME_YES=1 ./scripts/restack_public_seed.sh /tmp/genesis.next.json
# then commit published genesis
git add testnet/published/genesis.json web/testnet/published/genesis.json
git commit -m "testnet: add validator seat for @handle"
git push
```

**New operator machine:**

```bash
# After restack, genesis has N validators; your index is N-1 (last added)
mkdir -p data/keys
cp testnet/published/genesis.json data/genesis.json
# rename your key to match index, e.g. validator-3.json
cp ~/safe/my-validator.json data/keys/validator-3.json

./target/release/meshchain-node run \
  --data-dir ./data \
  --validator-index 3 \
  --listen 0.0.0.0:9100 \
  --peer 34.172.103.125:9100 \
  --peer 34.172.103.125:9101 \
  --peer 34.172.103.125:9102
```

**Existing seed** keeps indices `0..2` with existing keys under `/opt/meshchain/data/host/keys/`.

Finality: `ceil(2N/3)` — with 4 validators need **3** ACKs.

---

## Path C — Lab multi-host without public restack

Three machines can run a **private** shared genesis (not the public seed):

1. One host runs `mesh testnet-setup` and shares `genesis.json` + each `validator-i.json` securely  
2. Each host runs `meshchain-node run --validator-index i --peer …`  
3. Do **not** publish those keys  

---

## Seeds list (PRs welcome)

Add observers/extra seeds in `testnet/seeds.json`:

```json
{
  "id": "community-obs-1",
  "host": "YOUR_IP",
  "port": 9100,
  "roles": ["observer"],
  "operator": "github-handle",
  "region": "eu"
}
```

---

## Security

| Do | Don’t |
|----|--------|
| Share only `public_hex` | Post `secret_hex` in Discord/git |
| Use SSH/scp for key install | Email keys unencrypted |
| Expect testnet wipes on restack | Assume balances survive genesis bumps |
| Firewall 9100 to known peers if possible | Expose faucet mint key |

---

## Checklist — coordinator

- [ ] Application has valid 64-char `public_hex`  
- [ ] Operator can open TCP listen port  
- [ ] `genesis_add_validators.py` dry-run reviewed  
- [ ] Announce restack time in Discord/GitHub  
- [ ] `restack_public_seed.sh` on lab seed  
- [ ] Publish genesis + push to GitHub/Vercel  
- [ ] Operator confirms peer hellos + height advances  
- [ ] `scripts/status_public_seed.sh` still green  

---

## Related

- [RUN_A_NODE.md](./RUN_A_NODE.md)  
- [MULTI_VALIDATOR.md](./MULTI_VALIDATOR.md)  
- [AUDIT_AND_TEST.md](./AUDIT_AND_TEST.md)  
- [CLOUD.md](./CLOUD.md)  
