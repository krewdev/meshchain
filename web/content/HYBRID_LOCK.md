# Hybrid lock: internet vault + Meshtastic identifiers

## Goal

Funds locked in the internet vault **cannot be released by internet actors alone**.  
Release requires **correct Meshtastic-side identifiers and mesh consensus proof**.

```
                    ┌──────────────────────────┐
   deposit SOL      │  Solana vault (locked)   │
   + mesh_short_id ─►│  bound to mesh identity  │
                    └────────────▲─────────────┘
                                 │ unlock ONLY if ALL of:
                                 │  1. mesh_short_id matches lock
                                 │  2. unique mesh burn_txid
                                 │  3. ≥K mesh attestor signatures
                                 │     (validators who saw final Burn)
                    ┌────────────┴─────────────┐
                    │  Meshtastic MeshChain    │
                    │  Burn + cold key (large) │
                    └──────────────────────────┘
```

## Why this matters for extreme cold storage

| Attacker has | Can steal vault? |
|--------------|------------------|
| Only Solana key / compromised website | **No** (no mesh burn + attestors) |
| Only radio channel PSK | **No** (cannot sign Solana withdraw; no vault key) |
| Compromised single relayer | **No** if hybrid needs K attestors |
| Mesh burn for **wrong** short id | **No** (bound identity mismatch) |
| User’s mesh cold key + final burn + K attestors | **Yes** (intended unlock) |

## Dual control (hybrid)

| Side | What is locked / proven |
|------|-------------------------|
| **Internet** | Lamports/tokens in program vault PDA |
| **Mesh** | MESH claim; Burn destroys claim; `burn_txid` + `mesh_short_id` |
| **Attestors** | Mesh validators (or bridge guardians) also hold Solana keys; they sign that the burn is final |

Internet-only path is **disabled** when `hybrid_enabled = true`.

## Identifier binding

At deposit time the user chooses:

- `mesh_short_id` — 8-byte MeshChain address that will **own** the claim  

Unlock must present the **same** `mesh_short_id` plus a burn that mesh attestors confirm for that id.

Optional later: bind `pq_pk_hash` at deposit so only that cold key’s burns are accepted.

## Flow

### Lock (on-ramp)

1. User calls `deposit_sol(amount, mesh_short_id)`.  
2. Vault holds SOL; `DepositRecord` stores binding.  
3. Relayer mints MESH to that short id on the mesh.  

### Unlock (off-ramp)

1. User burns MESH on mesh (PQ if large).  
2. ≥K **mesh attestors** (registered Solana pubkeys of mesh validators) sign the withdraw intent.  
3. Anyone (relayer) submits `withdraw_hybrid(...)` with:
   - `deposit_seq` + matching `mesh_short_id`
   - `burn_txid`, `amount`, `mesh_height`, `destination`
   - K attestor signatures as remaining signers  
4. Program checks bindings + uniqueness → releases SOL.  

## What attestors sign (canonical message)

```
MESHCHAIN-HYBRID-UNLOCK-v1
|burn_txid=<32B>
|mesh_short_id=<8B>
|amount=<u64>
|destination=<pubkey>
|mesh_height=<u64>
|deposit_seq=<u64>
```

(On Solana we verify ed25519 via multiple `Signer` accounts that must be in the attestor set — simpler than raw ed25519 syscalls for v1: **attestors must co-sign the transaction**.)

## Threat notes

- Attestor majority collusion can still unlock (same as mesh validator trust).  
- Choose K and attestor set carefully; geo-separate keys.  
- Mesh channel encryption ≠ unlock authority.  

## CLI (plain English)

```
mesh how-cold-works     # explains hybrid hold
# deposits always name your mesh address
# cash-out needs radio burn + network witnesses
```
