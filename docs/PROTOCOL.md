# MeshChain Protocol v1

Mesh-native ledger for **hold + spend** over Meshtastic (or any low-bandwidth transport).  
Source of truth for MESH balances is the mesh chain — internet is not required to spend.

## Units

- **MESH** base unit integer, 6 decimals  
- `1_000_000` base units = `1.0 MESH`

## Cryptography

- Identities: **ed25519** 32-byte public keys  
- Short id: first 8 bytes of `SHA-256(pubkey)`  
- Tx/block signatures: ed25519  
- Hashes: SHA-256 (air headers may use 16-byte truncations)

## Transaction types

| Type | Signer | Effect |
|------|--------|--------|
| `Transfer` | Sender | Debit `from`, credit `to`, bump sender nonce |
| `Register` | New user | Bind short id ↔ pubkey |
| `Mint` | Authorized minter | Credit `to`, increase supply (bridge on-ramp) |
| `Burn` | Holder | Debit `from`, decrease supply (bridge off-ramp) |

### Transfer (canonical fields)

- `nonce`, `from` short id, `to` short id, `amount`  
- Signer pubkey must hash to `from`

### Mint (bridge on-ramp hook)

- `nonce` (minter account), `to`, `amount`, `external_ref` [16]  
- optional `to_pubkey` — if recipient unknown, creates the account (must hash to `to`)  
- `external_ref` = truncated hash of Solana deposit tx (or vault event); **unique** on-chain  
- Only keys in genesis `minters` ∪ validators

### Burn (bridge off-ramp hook)

- `nonce`, `from`, `amount`, `redeem_hint` [32]  
- `redeem_hint` carries destination info for the bridge (e.g. Solana pubkey hash)  
- Mesh does not verify Solana; bridge watches final burns

## Blocks (v1.1)

- At most **16 txs** per block (`MAX_TXS_PER_BLOCK`)  
- `tx_root` = SHA-256-trunc16 of concatenated 32-byte txids (empty → zeros)  
- Header: height, prev_hash, slot_time, producer_index, producer, tx_count, tx_root  
- Producer signs header  
- Round-robin leader: `height % N_validators`  
- **Finality:** ≥ `ceil(2N/3)` **signed** validator BlockAcks  
- Producer header signature counts as that validator’s ACK  
- Catch-up: finalized blocks archived and served via `BlocksRequest` / `BlocksResponse`  
- Genesis field `protocol_version` (currently `1`) advertised in gossip Hello

## Block rewards

- Each block with `height > 0` mints `block_reward` to producer (inflation)

## Transport (future)

- Meshtastic private channel, framed packets  
- Sim transport = in-process memory (current)
