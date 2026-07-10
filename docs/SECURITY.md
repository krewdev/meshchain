# Security (summary)

**Full hardening model:** [SECURITY_HARDENING.md](SECURITY_HARDENING.md)  
**Hybrid vault:** [HYBRID_LOCK.md](HYBRID_LOCK.md)  
**Quantum cold storage:** [QUANTUM_COLD_STORAGE.md](QUANTUM_COLD_STORAGE.md)

## Principles

1. **Fail secure** — reject on any failed check  
2. **Dual control** — internet vault + Meshtastic identity/attestation  
3. **Least exposure** — cold keys offline; radio off while holding  
4. **No false claims** — not “absolute bulletproof”; residual risks listed  

## Guarantees we engineer for

- Only keyholders authorize spends; vault burns need **ML-DSA-65**  
- Unauthorized mint rejected  
- Nonces + finality stop double-spend  
- Vault unlock needs **matching mesh_short_id + burn_txid + K attestors**  
- Hashed redeem destinations on mesh (privacy)  

## Non-guarantees

- RF traffic analysis / direction finding  
- Colluding attestor or validator majorities  
- Bugs before audit  
- Classical L1 quantum risk long-term  

## Key handling

- Never send seeds over mesh, MQTT, or cloud phones  
- Metal backup of cold seed  
- Power Meshtastic only for intentional sessions  
