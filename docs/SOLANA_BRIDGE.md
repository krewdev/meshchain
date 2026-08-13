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
4. Multisig minter + withdraw attestation  
5. Hardening + optional ZK deposit pool  
