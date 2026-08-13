# Hybrid vault path — external review brief

**Audience:** Independent reviewers / auditors  
**Scope:** Solana **devnet** vault + MeshChain **testnet** mint/burn cash-out  
**Date context:** 2026-08 · **Not mainnet**

## System under review

```
User (Solana devnet wallet)
  → deposit_sol(amount, mesh_short_id)  // program CBRQcjk5…3Vkx
  → DepositRecord PDA + DepositEvent
Public seed relayer (systemd)
  → poll BridgeConfig.depositCount + DepositRecord
  → meshchain-node mint-for-deposit --peer 127.0.0.1:9100
Mesh PoA (3 validators)
  → finalizes Mint / Burn
User
  → mesh burn <amt> --dest-sol <pk> --submit seed:9100   // PQ cold key
  → withdraw_hybrid_sol(burn_txid, …) + ≥2 attestors
Vault releases SOL to destination
```

## Trust assumptions

| Component | Trust |
|-----------|--------|
| Solana program | Correct hybrid_enabled / min_attestations / fee logic |
| Relayer host | Can mint tMESH via gossip (hot path); compromise = unjust mint of **tMESH only** |
| PoA validator set | Coordinator-approved; not open consensus |
| Attestors (hybrid withdraw) | ≥2 co-sign; compromise of K-of-N can release vault SOL |
| Mesh short id binding | Deposit locks to 8-byte id; wrong id = user error / griefing |
| PQ cold key | First vault burn binds ML-DSA-65 to account forever |

## Assets at risk (testnet)

- **Devnet SOL** in vault PDA (real-ish test tokens, not mainnet)
- **tMESH** (worthless, wipeable)
- Reputation / operational continuity of public seed

## Priority questions for reviewers

1. **Mint authority:** Can anyone other than authorized minter keys produce valid `Mint` txs accepted by PoA?  
2. **Double mint:** Is `external_ref` uniqueness enforced under concurrent relayers / restarts?  
3. **Double withdraw:** Is `WithdrawRecord` keyed by burn txid sufficient against replay?  
4. **Short id collision / preimage:** Is 8-byte short id binding acceptable for testnet threat model?  
5. **Burn without deposit:** Can burns unlock SOL not matching deposit `mesh_short_id` / remaining net?  
6. **Attestor set change:** Who can rotate attestors on-chain; is there a race?  
7. **Gossip DoS / fee market:** Can mint/burn spam halt liveness?  
8. **Relayer deferred path:** Does retry of deferred seqs allow stuck funds forever without UX?  
9. **Peer burn path:** Can a stale `chain_state` nonce produce a burn that validators reject silently?  
10. **Air path:** Are BalQuery/AirBlockAck spoofable in ways that affect vault (should not — vault is Solana+TCP PoA)?

## Explicit non-goals (this brief)

- Mainnet launch readiness  
- Formal verification of Anchor program  
- RF / Meshtastic physical side channels  
- Social engineering of operators  

## Artifacts to provide reviewers

| Artifact | Location |
|----------|----------|
| Solana program | `programs-mesh-bridge/programs/` |
| IDL | `programs-mesh-bridge/idl/programs_mesh_bridge.json` |
| Relayer | `programs-mesh-bridge/scripts/relayer_daemon.ts` |
| Mint / burn CLI | `crates/node` `mint-for-deposit` / `burn-for-withdraw`; `mesh burn` |
| Ledger apply | `crates/ledger/src/state.rs` Mint/Burn |
| Hybrid docs | `docs/HYBRID_LOCK.md` · `docs/SOLANA_BRIDGE.md` |
| Live e2e notes | deposit → mint → peer burn → withdraw proven on public seed |

## Suggested review order

1. On-chain withdraw_hybrid_* account constraints + remaining_accounts attestors  
2. Mesh `apply_tx` Burn/Mint + `used_external_refs`  
3. Relayer mint command construction (amount, ref, peer)  
4. Operator key layout on seed (who holds what)  

## Residual risks (operator view)

- Relayer is a **hot minter** for tMESH (testnet acceptable; mainnet needs multisig/threshold mint).  
- Attestors are **hot co-signers** for SOL release.  
- Public seed is a single coordination point for PoA seats.  
- Users who deposit before mesh register rely on deferred retry (documented).  

## Contact

GitHub: https://github.com/krewdev/meshchain  
Security: see `docs/SECURITY.md` / `SECURITY_HARDENING.md`
