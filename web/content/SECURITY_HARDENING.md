# Security & privacy hardening (MeshChain)

## Honest framing

**No networked money system is “absolutely bulletproof.”**  
Anyone who claims perfect security or perfect privacy is selling fiction.

MeshChain’s bar is **maximum practical rigor** under real constraints (LoRa airtime, RF physics, Solana/BTC classical L1s):

- **Defense in depth** — many independent locks must all fail  
- **Fail secure** — if unsure, reject  
- **Least privilege** — internet path cannot unlock without mesh proof  
- **Cryptographic authenticity** — spends require keys; large/cold paths use ML-DSA-65  
- **Privacy by design** — minimize linkability; never put seeds or raw bank identifiers on air  
- **Explicit residual risk** — operators and users know what remains  

If a property is not proven, we do not claim it.

---

## Target properties

### Security (integrity & custody)

| Property | Mechanism |
|----------|-----------|
| Only owner spends | ed25519 + optional ML-DSA-65 on body |
| No double-spend | per-account nonces + finality |
| No silent mint | authorized minter set only |
| Vault not freeable via internet alone | **hybrid lock**: mesh_short_id + burn_txid + K attestors |
| Replay unlock | unique burn_txid PDA; deposit amount_unlocked cap |
| Quantum-resistant cold auth | ML-DSA-65 (FIPS 204) for large/vault burns |
| Censorship reduced | multi-validator PoA `ceil(2N/3)` |
| Fail secure hybrid | hybrid forced on production configs; min attestors ≥ 2 |

### Privacy (confidentiality & unlinkability)

| Property | Mechanism | Limit |
|----------|-----------|--------|
| No KYC in protocol | keys only | Social graph still exists |
| Pseudonymous addresses | short ids from key hashes | RF can still localize radios |
| No plaintext redeem address on mesh | `redeem_hint` = hash only | Bridge learns dest when unlocking |
| One-time receive tags | stealth short-id derivation | Optional; user must use it |
| No seeds on mesh/MQTT/logs | policy + code review | Operator mistakes |
| Channel isolation | private Meshtastic channel for funds | PSK compromise → metadata |

**RF is not Tor.** Direction-finding, timing, and traffic volume are residual.

---

## Hybrid dual-control (bullet-resistant vault)

```
Unlock requires ALL:
  [1] deposit.mesh_short_id == provided mesh_short_id
  [2] burn_txid unique + non-zero (mesh Burn happened)
  [3] amount ≤ claim remaining
  [4] ≥ K registered mesh attestors co-sign the Solana tx
  [5] program not paused; vault solvent
```

Internet-only (hacked website, stolen relayer hot key, compromised RPC) → **insufficient**.

---

## Cryptographic suite

| Use | Algorithm | Notes |
|-----|-----------|--------|
| Everyday small mesh tx | ed25519 | Small packets; not long-term PQ |
| Cold / large / vault burn | ML-DSA-65 | Multi-FRAG on LoRa |
| Hashing | SHA-256 with domain separation prefixes | |
| Mesh framing | `MC` magic + typed ports | Reject unknown |
| Vault L1 | Solana program constraints | L1 still classical |

Domain prefixes used in code:

- `meshchain-v2-mldsa65` — PQ context  
- `meshchain-pq-id` — PQ short id  
- `meshchain-redeem-v1` — redeem destination hash  
- `meshchain-stealth-v1` — one-time receive id  

---

## Operational hard rules

1. Cold keys **never** on phones with SIM/5G or always-on cloud backup.  
2. Attestor keys geo-separated; threshold K ≥ 2; never all on one VPS.  
3. Meshtastic funds channel PSK ≠ custody; rotate if leaked.  
4. Power radio only for intentional sessions.  
5. Prefer fresh Solana/BTC destination on every unlock.  
6. No balance/identity publishing to public MQTT by default.  
7. Audit `set_attestors` / authority like root.  

---

## Residual risks (must accept or mitigate off-protocol)

| Risk | Severity | Mitigation |
|------|----------|------------|
| ≥K attestors collude | Critical | Diversify operators; social monitoring |
| ≥2/3 mesh validators censor/halt | High | Diverse validators; user exits while healthy |
| RF traffic analysis | Medium | Rare sessions; cover traffic sparingly |
| Solana/BTC L1 quantum break (future) | High long-term | Migrate vaults when L1s upgrade; minimize time locked |
| User loses cold seed | Critical | Metal backup; optional social recovery (out of band) |
| Implementation bugs | Critical | Tests, review, formal audit before mainnet value |
| Supply-chain / malware on build host | Critical | Reproducible builds; air-gapped sign |

---

## Before real value

- [ ] Independent security review of vault + ledger  
- [ ] Hybrid attestors ≥ 3 with documented ops  
- [ ] PQ required for all vault-related burns  
- [ ] Private mesh channel + RF hygiene guide  
- [ ] Incident response: pause bridge, freeze mint  
- [ ] No mainnet without bug bounty / staged limits  

**Bottom line:** We engineer for **hostile** internet and **curious** RF adversaries. We do **not** claim magical invulnerability.
