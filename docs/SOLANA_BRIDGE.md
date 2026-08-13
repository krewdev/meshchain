# Solana ↔ MeshChain Vault Bridge

## User-approved design

> SOL or stables are sent to a **vault** on Solana; the program mints the **equivalent minus a fee** to the user’s preferred **mesh wallet**.  
> Bridging back: **mesh tokens are burned**, then **SOL/stables are released** to the destination Solana wallet.

### Extreme cold storage

After mint, the user **never needs internet/5G** to hold value. Redeem uses mesh + **ML-DSA-65** auth (see QUANTUM_COLD_STORAGE.md).  
BTC follows the same pattern via a separate federation/multisig vault (same Mint/Burn mesh hooks, different internet custody).

This is **Phase v2** relative to mesh consensus. Mesh v1 implements `Mint` / `Burn` hooks so the bridge can plug in without redesigning the ledger.

## Flow

### On-ramp (Solana → Mesh)

```
User wallet (Solana)
    │  deposit SOL or USDC/USDT (SPL)
    ▼
┌─────────────────────────────────────┐
│  MeshBridge program (Solana)        │
│  - Vault token accounts             │
│  - Config: fee_bps, minters, paused │
│  - Deposit event / PDA receipt      │
└─────────────────────────────────────┘
    │  amount_net = amount * (1 - fee_bps/10_000)
    │  emit Deposit { mesh_short_id, amount_net, mint, depositor, seq }
    ▼
Bridge relayer(s) (internet + mesh radio)
    │  observe deposit (RPC)
    │  sign MeshChain Mint{ to: mesh_short_id, amount: amount_net, external_ref: hash(sol_tx) }
    ▼
MeshChain validators finalize Mint
    │
    ▼
User holds MESH on mesh — spend offline, no Solana needed
```

### Off-ramp (Mesh → Solana)

```
User mesh wallet
    │  Burn{ amount, redeem_hint: solana_dest_pubkey }
    ▼
MeshChain finalizes Burn (supply down)
    │
Bridge relayer(s)
    │  observe final Burn + merkle/height proof (v1: trusted multi-sig minters)
    │  call MeshBridge::withdraw(dest, amount, burn_proof)
    ▼
┌─────────────────────────────────────┐
│  Vault releases SOL/stable          │
│  minus optional withdraw fee        │
└─────────────────────────────────────┘
    │
    ▼
Destination Solana wallet
```

## Fee model

| Direction | Fee | Receiver |
|-----------|-----|----------|
| Deposit | `fee_bps` of deposit (e.g. 30 = 0.30%) | Solana treasury PDA |
| Withdraw | optional `withdraw_fee_bps` | Solana treasury PDA |

Mesh receives **net** amount only (`amount_net`). Fees never mint as MESH unless you explicitly choose that policy (default: fees stay on Solana).

## Program accounts (sketch)

```text
Config PDA:
  authority, fee_bps, withdraw_fee_bps, vault_bump,
  mesh_minter_threshold, paused, supported_mints[]

Vault:
  token accounts for wSOL, USDC, USDT, ...

DepositRecord PDA (per seq or per sol_tx):
  depositor, mesh_short_id, amount_gross, amount_net, mint, processed

WithdrawRecord PDA (per mesh burn txid):
  burn_txid, dest, amount, completed  // anti double-release
```

## Security model (federated v1 bridge)

1. **Custody risk:** vault funds controlled by Solana program; upgrade authority must be locked/multisig.  
2. **Mint authority on mesh:** only bridge minter keys (can be same set as mesh validators).  
3. **Double mint:** `external_ref` / deposit PDA unique; relayer and mesh reject duplicates.  
4. **Double withdraw:** `WithdrawRecord` keyed by mesh burn `txid`.  
5. **Equivocation:** require M-of-N relayer attestations before withdraw (recommended).  
6. **Mesh finality:** do not release SOL until burn is in a **final** mesh block.

## What is NOT in mesh v1

- The Solana program binary itself (implement in Anchor in bridge phase)  
- ZK shielded pools (later privacy phase)  
- Trustless light-client verification of mesh headers on Solana (later)

## Mapping to MeshChain txs

| Bridge step | Mesh tx |
|-------------|---------|
| After vault deposit | `Mint { to, amount_net, external_ref }` |
| User exits mesh | `Burn { from, amount, redeem_hint }` |

## Rate / oracle (stables vs SOL)

- **1:1 stables → MESH** (1 USDC = 1 MESH face value) is simplest for “mesh cash.”  
- **SOL → MESH** needs a price oracle or fixed peg policy; start with **stables only**, add SOL with oracle later.

## Implementation order

1. Mesh sim: Mint/Burn working (done in ledger + sim)  
2. Anchor `mesh_bridge` program with deposit/withdraw  
3. Relayer daemon: Solana RPC ↔ meshchain-node API / Meshtastic  

## Public seed relayer ops

**Do not** rely on the embedded `meshchain-node` spawn. It is **off by default** (`MESH_RELAYER=1` to enable lab mode). Run **one** systemd unit:

```bash
# On seed host
sudo cp deploy/meshchain-relayer.service /etc/systemd/system/
sudo cp deploy/relayer.env.example /etc/meshchain/relayer.env
# edit ANCHOR_WALLET path; generate key if needed:
#   sudo -u meshchain solana-keygen new -o /home/meshchain/.config/solana/id.json
sudo chmod 600 /etc/meshchain/relayer.env
sudo systemctl daemon-reload
sudo systemctl enable --now meshchain-relayer
sudo journalctl -u meshchain-relayer -f
# or: tail -f /var/log/meshchain/relayer.log
```

Env (see `deploy/relayer.env.example`):

| Var | Purpose |
|-----|---------|
| `ANCHOR_PROVIDER_URL` | Solana RPC (devnet) |
| `ANCHOR_WALLET` | Hot keypair for Anchor provider |
| `MESHCHAIN_DATA` | Host data dir (`…/data/host`) |
| `MESH_MINT_PEER` | Gossip peer for `mint-for-deposit` (`127.0.0.1:9100`) |

IDL ships at `programs-mesh-bridge/idl/programs_mesh_bridge.json` (no Anchor rebuild required on the seed).

### Fund the relayer wallet (devnet)

Public RPC airdrops often fail. From a funded machine:

```bash
# Seed hot wallet (example — check /etc/meshchain/relayer.env)
solana transfer <RELAYER_PUBKEY> 0.25 \
  --url https://api.devnet.solana.com \
  --allow-unfunded-recipient
solana balance <RELAYER_PUBKEY> --url https://api.devnet.solana.com
```

The relayer only needs SOL if it also **signs deposits** from that key. Listening + minting mesh does **not** spend Solana SOL (mint is signed by mesh validator keys via `mint-for-deposit --peer`).

### Deferred deposits (retry)

If a deposit’s mesh short id is **not registered** on the mesh ledger yet, the relayer:

1. Logs `deferred — register mesh wallet first (will retry)`
2. Stores `seq` in `data/host/relayer_state.json` → `deferredSeqs`
3. **Re-scans every poll** until `chain_state` / registry has the pubkey, then mints

Register first:

```bash
mesh new-wallet --name me.json --publish
# or faucet drip which registers
mesh faucet-drip --wallet me.json
```

Then deposit SOL bound to that wallet’s short id (sha256(pubkey)[:8]).

Manual mint (mesh side only, no Solana deposit):

```bash
meshchain-node mint-for-deposit \
  --data-dir /opt/meshchain/data/host/v0 \
  --to-pubkey <32-byte-hex> \
  --amount 1000000 \
  --external-ref-hex $(openssl rand -hex 16) \
  --validator-index 0 \
  --peer 127.0.0.1:9100
```

Recipient must already be on-chain (register / faucet) so the relayer can resolve short id → full pubkey.

### Hybrid round-trip e2e

```bash
export ANCHOR_WALLET=~/.config/solana/id.json   # funded on devnet
export MESH_MINT_PEER=34.172.103.125:9100       # or 127.0.0.1:9100 on seed
./scripts/e2e_hybrid_roundtrip.sh
# deposit only:
SKIP_WITHDRAW=1 ./scripts/e2e_hybrid_roundtrip.sh
```

### Peer-submitted vault burn (public seed)

```bash
# Plain English (preferred)
mesh new-cold-key --name cold.json
mesh burn 10 --wallet me.json --cold cold.json --dest-sol <YourSolPubkeyBase58>
# defaults --submit to public seed from seeds.json / MESH_SUBMIT

# Equivalent low-level:
meshchain-node burn-for-withdraw \
  --data-dir ./data \
  --wallet ./data/keys/me.json \
  --cold ./data/keys/cold.json \
  --amount 10000000 \
  --dest-sol <YourSolPubkeyBase58> \
  --asset-id 1 \
  --peer 34.172.103.125:9100
# writes last_burn.json → hybrid withdraw
```

Web helper (Phantom deposit + mint watch): https://meshchain-sigma.vercel.app/bridge/  
Security brief for reviewers: [HYBRID_SECURITY_REVIEW.md](HYBRID_SECURITY_REVIEW.md)
4. Multisig minter + withdraw attestation  
5. Hardening + optional ZK deposit pool  
