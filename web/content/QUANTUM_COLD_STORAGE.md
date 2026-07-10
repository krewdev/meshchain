# Quantum-safe extreme cold storage on MeshChain

## Use case

1. User locks **real** BTC / SOL / stables in an **internet vault** (bridge contracts / multisig custodians).  
2. Equivalent **MESH claims** are minted on the **Meshtastic ledger**.  
3. User **stores the mesh wallet offline** — no Wi‑Fi, no 5G, no cellular modem — only LoRa mesh when they deliberately power a radio.  
4. Spending mesh funds or redeeming to BTC/SOL requires physical presence on a mesh + explicit signed action (rare for true cold storage).

**Goal:** long-term custody where the *authorization keys* never sit on networks that are continuously reachable (internet / mobile).

## Why classical-only is not enough

| Algorithm | Quantum threat |
|-----------|----------------|
| ed25519 / secp256k1 | Broken by **Shor** (large enough cryptographically relevant QC) |
| SHA-256 / BLAKE3 | Needs larger outputs under **Grover** (usually 256-bit still OK with margin) |
| AES-256 | Generally considered OK with PQ (Grover halves bits) |

**v1 used ed25519 for LoRa size.** That is **not** long-term quantum-safe.  
**v2 crypto profile:** **ML-DSA-65 (FIPS 204)** for mesh authorization; ed25519 optional hybrid only for transition.

## Quantum-safe design (MeshChain v2 profile)

### Identity & spend keys

- **Primary:** ML-DSA-65 keypair generated **air-gapped** on cold machine.  
- Seed / secret never exported over mesh, BLE (except offline companion cable), MQTT, or phone cloud.  
- Short id still `SHA-256(pubkey)[0..8]` (or hash of PQ pubkey) for compact addressing.

### Signatures on LoRa

ML-DSA-65 signatures are **~3 KB** — **do not fit** one Meshtastic packet (~200 B payload).

| Mechanism | Role |
|-----------|------|
| **FRAG frames** | Split PQ signature (and large pubkeys) across N packets with session id + hash |
| **Spend rarity** | Cold storage expects minutes of airtime for one redeem — acceptable |
| **Hybrid mode** (optional) | ed25519 + ML-DSA both required until sunset date of classical |

### Robustness

| Layer | Measure |
|-------|---------|
| Consensus | ≥3 validators, `ceil(2N/3)` finality, disk persistence |
| Transport | Private Meshtastic channel; rate limits; dedupe; session reassembly timeouts |
| Bridge | Unique deposit seq / burn_txid; multi-relayer attestation (M-of-N) before vault release |
| Cold ops | Paper/metal backup of PQ seed; dual-control for large redeems; vault pause switch |
| Supply | 1:1 or oracle-pegged claims; no silent inflation without mint auth |
| RF | No dependence on 5G/Wi‑Fi for holding; radio powered only when needed |

### Extreme cold workflow

```
[Air-gapped PQ wallet]  --(optional USB)--  [Meshtastic radio ON only for session]
        ^                                              |
        |                                              | LoRa mesh (private)
        |                                              v
   keys never leave                          [Validators / gateway]
   internet-connected phone                          |
                                                     | when redeeming only
                                                     v
                                            [Bridge → BTC/SOL vault unlock]
```

**Holding:** radio can stay **off** for months. Balance is on the ledger; keys are offline.  
**Receive:** someone else sends MESH, or bridge mints after vault deposit — user later syncs headers when they briefly radio-on.  
**Redeem to internet crypto:** burn MESH + multi-packet PQ sig → relayers release vault BTC/SOL to a **fresh** address.

## BTC + SOL vaulting

| Asset | Internet side | Mesh side |
|-------|---------------|-----------|
| SOL / SPL stables | Solana `mesh_bridge` vault program | Mint/Burn MESH |
| BTC | Multisig / DLCs / federation vault (Liquid optional later) | Same MESH units or asset-tagged claims |

MESH can be **multi-asset** later (`asset_id` on Mint/Burn). v1 single MESH; cold storage policy maps 1 USDC↔1 MESH or 1 sat claim units.

## What “quantum safe” does **not** mean

- Mesh RF is not invisible; metadata can leak.  
- Validators can still censor if compromised.  
- Bridge vault smart contracts on Solana/BTC still use **classical** chain crypto until those L1s migrate — **vault long-term** should use multisig + watchtowers + eventual PQ migration of custody scripts.  
- True “store forever against nation-state QC” requires operational discipline, not only algorithms.

## Migration path

1. **Now:** ed25519 mesh + vault bridge (works, **not PQ**).  
2. **v2:** ML-DSA-65 mesh spends + FRAG transport; hybrid dual-sign optional.  
3. **v3:** deprecate ed25519; asset-tagged BTC/SOL claims; M-of-N relayers.  
4. **Parallel:** keep vault cold policies (time-locks, multisig) even while L1s are classical.
