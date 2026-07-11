# How to audit and test MeshChain

**TESTNET ONLY** — tMESH has no cash value. This is how *you* (and reviewers) verify the system.

---

## 1. Layers of assurance

| Layer | What it proves | How |
|-------|----------------|-----|
| **Unit tests** | Crypto, framing, ledger rules | `cargo test` |
| **Local integration** | Multi-validator sim, transfers, PQ | `mesh demo` |
| **Public seed smoke** | Live host health | `./scripts/status_public_seed.sh` |
| **Public e2e** | Join → register → faucet → balance | `./scripts/test_public_e2e.sh` |
| **Bridge e2e** | SOL vault ↔ mesh mint/burn | `docs/E2E_TESTNET.md` |
| **Security review** | Threat model, residual risk | `docs/SECURITY_HARDENING.md` + checklist below |
| **External audit** | Independent third party | Hire firm; use checklist + scope |

---

## 2. Automated tests (run locally)

```bash
cd meshchain

# Full workspace unit/integration tests
cargo test --workspace

# Focused (what CI runs)
cargo test -p meshchain-proto -p meshchain-ledger -p meshchain-transport

# What is covered today
#   proto:  addresses, mesh names, ed25519 tx sign/verify, PQ sign/verify, privacy hashes
#   ledger: priority fee → producer (balance conservation)
#   transport: frame + FRAG roundtrip
```

CI (GitHub Actions): `.github/workflows/ci.yml` on every push/PR to `main`.

```bash
gh run list --limit 5
gh run view --log-failed   # if red
```

---

## 3. Local network demo (no cloud)

```bash
cargo build -p mesh -p meshchain-node
./target/debug/mesh testnet-setup
./target/debug/mesh demo          # transfers, double-spend reject, mint/burn, PQ large send
./target/debug/mesh balance --wallet data/keys/alice.json
./target/debug/mesh balance --wallet data/keys/bob.json
```

`mesh demo` is the best **single-process** integration test: invalid nonce, overspend, bridge mint/burn, cold key.

Multi-validator lab:

```bash
./validator-automation/mesh-validator bootstrap
./validator-automation/mesh-validator start
./validator-automation/mesh-validator health
./target/debug/mesh send <NAME> 1 --wallet data/keys/alice.json --submit 127.0.0.1:9100
```

---

## 4. Public seed tests (live shared network)

### Health only

```bash
./scripts/status_public_seed.sh
# Expect OK on faucet/scanner HTTP+HTTPS and peers :9100-9102
```

### Full public user path

```bash
./scripts/test_public_e2e.sh
# join-public → wallet → register → faucet-drip → balance ≥ 100
```

Manual version:

```bash
./target/debug/mesh join-public
./target/debug/mesh new-wallet --name audit.json --publish
./target/debug/mesh faucet-drip --wallet audit.json
./target/debug/mesh balance --wallet audit.json
./target/debug/mesh send <OTHER> 1 --wallet audit.json --fee 0.01 --submit 34.172.103.125:9100
./target/debug/mesh sync-state
```

### Live endpoints (as of public seed)

| Check | URL |
|-------|-----|
| Scanner | https://34.172.103.125.sslip.io/ |
| API | https://34.172.103.125.sslip.io/api/v1/status |
| Chain state | https://34.172.103.125.sslip.io/api/v1/chain_state |
| Faucet | https://faucet.34.172.103.125.sslip.io/info |
| Peer | `34.172.103.125:9100` |

---

## 5. Solana hybrid bridge e2e

Requires Solana CLI + funded **devnet** wallet. See [E2E_TESTNET.md](./E2E_TESTNET.md).

```bash
# deposit SOL → mint tMESH → burn+PQ → hybrid withdraw
# scripts under programs-mesh-bridge/
```

**Audit focus:** hybrid unlock needs mesh short id + burn + ≥K attestors — internet alone must fail.

---

## 6. Security audit checklist (internal or external)

Use with [SECURITY_HARDENING.md](./SECURITY_HARDENING.md).

### A. Cryptography & ledger

- [ ] ed25519 verify on every Transfer/Register/Mint/Burn  
- [ ] Nonce strictly increases; double-spend rejected  
- [ ] PQ required above threshold and on vault-linked burns  
- [ ] Mint only by minter/validator set  
- [ ] Priority fee: debit sender amount+fee; credit producer fee; supply conserved  
- [ ] Block producer must be in genesis validator set  
- [ ] Finality ACKs only count genesis validators (not observers)  

### B. Networking

- [ ] Observers cannot produce blocks  
- [ ] SyncResponse rejects mismatched chain_id / validator set  
- [ ] Submit/gossip does not accept unsigned or wrong-signer txs  
- [ ] Faucet cooldown + amount caps (no unlimited mint API without minter key)  

### C. Keys & ops

- [ ] Validator secrets never in git (`data/keys` gitignored)  
- [ ] Published artifacts are **genesis + seeds only**  
- [ ] Host binds: public seed uses `0.0.0.0` intentionally; lab can stay localhost  
- [ ] Cloud firewall: only 22, 80, 443, 8787, 8788, 9100–9102 as intended  

### D. Bridge (Solana)

- [ ] Hybrid mode cannot be bypassed by relayer alone  
- [ ] burn_txid unique / amount caps  
- [ ] Attestor set matches published attestors.json  

### E. Residual risks (document, don’t “fix away”)

- [ ] RF / Meshtastic metadata and DF  
- [ ] PoA set compromise (majority of validators)  
- [ ] Solana L1 classical (not PQ)  
- [ ] Operator key loss / machine compromise  
- [ ] Testnet resets wipe balances  

---

## 7. Suggested external audit scope

If hiring a firm, scope roughly:

1. **Rust crates:** `proto`, `ledger`, `node` (consensus/gossip), `scanner` auth  
2. **Solana program:** `programs-mesh-bridge` (hybrid lock, fees, attestors)  
3. **Threat model:** double-spend, unauthorized mint, vault drain, validator impersonation  
4. **Not in scope (unless paid):** full RF side-channel, social engineering, third-party Meshtastic firmware  

Deliverables: findings severity list, PoC for criticals, retest after fixes.

---

## 8. What “passing” means today

| Gate | Pass criteria |
|------|----------------|
| Unit CI | All `cargo test` green on main |
| Public smoke | `status_public_seed.sh` exit 0 |
| Public e2e | `test_public_e2e.sh` exit 0, balance ≥ faucet drip |
| Demo | `mesh demo` completes without panic |
| Security doc | Residual risks acknowledged; no false “bulletproof” claims |

---

## 9. Quick command card

```bash
# Unit
cargo test --workspace

# Local integration
./target/debug/mesh testnet-setup && ./target/debug/mesh demo

# Live public
./scripts/status_public_seed.sh
./scripts/test_public_e2e.sh

# Security posture printout
./target/debug/mesh security
```
