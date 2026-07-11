# Multi-operator MeshChain testnet

**Goal:** more than one organization runs nodes that share the same `chain_id` and genesis.

## Roles

| Role | Who | Power |
|------|-----|--------|
| **Coordinator** | Lab maintainers | Publishes genesis + seeds; restacks set |
| **Producer** | Approved operators | Signs blocks; seat in `genesis.validators` |
| **Observer** | Anyone | Relays + catch-up; no block power |
| **Wallet user** | Anyone | Register / send / faucet |

## Operator apply (you)

```bash
cargo build -p mesh
./target/debug/mesh validator-keygen --name my-validator.json
# Keep my-validator.json secret. Share only public_hex in an issue.
```

Fill `testnet/operator_application.example.json` and open a GitHub issue.

## Coordinator: add seats

```bash
./target/debug/mesh genesis-extend \
  --genesis testnet/published/genesis.json \
  --add <OPERATOR_PUBLIC_HEX> \
  --out testnet/published/genesis.json

# copy to web/ for Vercel
cp testnet/published/genesis.json web/testnet/published/genesis.json

# update seeds.json with new host:port entries
# commit + push; operators pull
```

**Testnet reset:** new validator set ⇒ wipe `chain_state.json` on all hosts (height restarts). Document in release notes.

## Operator: run producer after approval

```bash
# index N matches your position in genesis.validators
mkdir -p data/keys
cp my-validator.json data/keys/validator-N.json
cp testnet/published/genesis.json data/genesis.json
# empty chain_state for restack
rm -f data/chain_state.json

./target/debug/meshchain-node run \
  --data-dir ./data \
  --validator-index N \
  --listen 0.0.0.0:9100 \
  --peer SEED_HOST:9100 \
  --peer OTHER_OP:9100
```

Open **TCP 9100** (or your port) to seed peers.

## Anyone: run observer (no seat)

```bash
./target/debug/mesh join-public
./target/debug/mesh observer --peer 34.172.103.125:9100 --listen 0.0.0.0:9100
```

Observers request catch-up via gossip `SyncRequest` (producers serve snapshots; only observers apply them).

## Integrity notes (post Track A)

- BlockAcks are **ed25519-signed**
- Leader schedule: `producer_index == height % N`
- Mint `external_ref` unique
- Public faucet mints via **gossip** (`--peer`), not offline fork
- Producers do **not** replace state from untrusted SyncResponse

## Restack checklist

- [ ] Freeze traffic / announce window  
- [ ] `genesis-extend` + publish  
- [ ] Update `seeds.json`  
- [ ] All producers stop  
- [ ] Install genesis + keys; delete chain_state  
- [ ] Start producers + verify height advances  
- [ ] Restart faucet with `MESH_MINT_PEER=127.0.0.1:9100`  
- [ ] `./scripts/test_public_e2e.sh`  
