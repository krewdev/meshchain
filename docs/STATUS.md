# MeshChain status (operator truth)

**Updated:** 2026-07-11  
**Network:** `meshchain-testnet-1` (public testnet — **no cash value**, state may wipe)

## Live endpoints

| Service | URL / addr |
|---------|------------|
| Seed peer (TCP gossip) | `34.172.103.125:9100` (+ `:9101`, `:9102`) |
| **Remote observer** (GCE) | `35.192.20.103:9100` — non-PoA full node |
| Scanner | https://34.172.103.125.sslip.io/ |
| Faucet | https://faucet.34.172.103.125.sslip.io/info |
| Site | https://meshchain-sigma.vercel.app |

| Host | Role | Instance |
|------|------|----------|
| `34.172.103.125` | 3× PoA validators + faucet + scanner | `meshchain-testnet` |
| `35.192.20.103` | observer / relay seed | `meshchain-observer` |

Params: [`testnet/network.json`](../testnet/network.json) · [`testnet/seeds.json`](../testnet/seeds.json)

```bash
# Dial both seed and remote observer
mesh observer --peer 34.172.103.125:9100 --peer 35.192.20.103:9100
```

## Integrity posture (Track A+)

| Control | Status |
|---------|--------|
| Signed BlockAcks (ed25519) | yes |
| Leader schedule `height % N` on apply | yes |
| Mint `external_ref` uniqueness | yes |
| Faucet mints via gossip `--peer` | yes (set `MESH_MINT_PEER`) |
| Producers reject untrusted SyncResponse | yes |
| Block-by-block catch-up (`BlocksRequest`) | yes |
| Atomic `chain_state.json` writes | yes |
| Append-only block archive `data/blocks/{h}.json` | yes |
| Equivocation detect (same height, two hashes) | yes |
| Mempool dry-run against state | yes |
| Gossip line size + per-peer rate limit | yes |
| Multi-tx blocks (≤16) | yes |
| `protocol_version` in genesis / Hello | yes (v1) |
| Faucet daily + global rate caps | yes |

## Known limitations

1. **PoA, not open consensus** — seats are coordinator-approved; restack may wipe state.  
2. **TCP gossip first** — Meshtastic path is sidecar/framing; finality is internet PoA today.  
3. **Faucet is a hot minter** — capped, not a treasury; testnet only.  
4. **Solana hybrid vault** — experimental on devnet; not mainnet-ready.  
5. **Short ids are 8 bytes** — full pubkey always bound on register.  
6. **No on-chain validator governance** — `mesh genesis-extend` + restack for set changes.

## Operator quick paths

```bash
# User
mesh join-public && mesh new-wallet --name me.json --publish
mesh faucet-drip --wallet me.json && mesh balance --wallet me.json

# Observer
mesh observer --peer 34.172.103.125:9100

# Producer (after approval)
meshchain-node run --data-dir ./data --validator-index N \
  --listen 0.0.0.0:9100 --peer 34.172.103.125:9100
```

Docs: [RUN_A_NODE](RUN_A_NODE.md) · [MULTI_OPERATOR](MULTI_OPERATOR.md) · [AUDIT_AND_TEST](AUDIT_AND_TEST.md) · [SECURITY_HARDENING](SECURITY_HARDENING.md)

## Deploy checklist (seed)

```bash
cargo build -p mesh -p meshchain-node --release
# install binary + faucet_server.py on host
export MESH_MINT_PEER=127.0.0.1:9100
# unset MESH_ALLOW_OFFLINE_MINT on public hosts
systemctl restart meshchain-testnet   # or host equivalent
./scripts/status_public_seed.sh
```

## Ops: live smoke / observer / archive

```bash
# 1) Public e2e (join → wallet → faucet → balance)
./scripts/test_public_e2e.sh

# 2) External observer (any machine)
mesh join-public
meshchain-node run --data-dir ./data --observer \
  --listen 0.0.0.0:9110 --peer 34.172.103.125:9100

# 3) Merge block archives + tip checkpoint (on seed host data dir)
./scripts/backfill_block_archive.sh /opt/meshchain/data/host
# → data/.../blocks/MANIFEST.json + checkpoints/tip.json
# Full blocks only exist from archive_first (post-integrity deploy).
# Earlier heights: applied[] hashes only; observers use SyncResponse.
```
