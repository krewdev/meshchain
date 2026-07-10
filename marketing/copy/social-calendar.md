# 28-day social calendar — Phase 1

**Primary links**  
Site: https://meshchain-sigma.vercel.app  
Scanner: https://meshchain-sigma.vercel.app/scanner/  
Faucet: https://meshchain-sigma.vercel.app/faucet/  
GitHub: https://github.com/krewdev/meshchain  
Docs: https://meshchain-sigma.vercel.app/docs/

**Footer (append to long posts)**  
`Public testnet · tMESH has no cash value · Not official Meshtastic · Not financial advice`

Creative refs: `marketing/creatives/` (hero / square / story backgrounds + `ads.html`)

---

## Week 1 — Launch

### Day 1 · Mon · Launch thread (X)

**Post 1/5**
> Money that moves when the internet doesn’t.
>
> MeshChain is a privacy-forward ledger for Meshtastic LoRa.
> Hold offline. Cold keys stay air-gapped. Hybrid vaults refuse internet-only unlocks.
>
> Public testnet is live.
> 🧵

**Post 2/5**
> Why mesh money?
>
> Cell towers fail. Borders jam. Always-on rails surveil by default.
> LoRa mesh already moves messages peer-to-peer.
> We added a minimal, auditable value layer on top.

**Post 3/5**
> Hybrid dual-control vault
>
> SOL on Solana devnet binds to your mesh identity.
> Unlock needs mesh proof + burn + multi-attestor co-sign.
> Browser alone is not enough — by design.

**Post 4/5**
> Try it
>
> 🔭 Scanner → meshchain-sigma.vercel.app/scanner/
> 🚰 Faucet → /faucet/
> 📘 Docs → /docs/?doc=GETTING_STARTED
> ⭐ Source → github.com/krewdev/meshchain

**Post 5/5**
> tMESH is testnet-only. No cash value.
> Community software — not an official Meshtastic product.
>
> Builders: `mesh testnet-setup` and tell us what breaks.

**Media:** hero banner + logo

---

### Day 2 · Tue · Scanner proof

> Height is ticking on the public scanner.
>
> Live view of meshchain-testnet-1 — accounts, tip hash, supply.
>
> → meshchain-sigma.vercel.app/scanner/
>
> (Fallback snapshot if the live tunnel sleeps — host is a laptop for now.)

**Media:** screenshot of scanner UI

---

### Day 3 · Wed · Reddit r/meshtastic

**Title:** `[Project] MeshChain — experimental mesh ledger + hybrid vault for Meshtastic (open source, public testnet)`

**Body:**
Hey mesh folks — I built an experimental value layer designed for LoRa/Meshtastic constraints: CLI wallets, memorable mesh names, multi-validator lab, and a hybrid Solana *devnet* vault that requires mesh-side proof to unlock.

This is **not** official Meshtastic. **tMESH has no cash value.** Looking for critique from people who actually run nodes (airtime, reliability, UX).

- Site: …
- Scanner: …
- Repo: …

What would you refuse to trust on RF, and what would you still want?

---

### Day 4 · Thu · Security thread

> Security model in one slide:
>
> 1. Hot keys for small mesh spends  
> 2. ML-DSA-65 cold path for large / vault-linked burns  
> 3. Vault cash-out: mesh burn + ≥2 attestors  
> 4. RF is not Tor — we say so in the docs  
>
> Read: /docs/?doc=SECURITY_HARDENING

---

### Day 5 · Fri · Faucet challenge

> Weekend challenge for builders:
>
> 1. Claim faucet tMESH  
> 2. Create a wallet → get a mesh name like M3SQRT-XTA1Y-ZJ6  
> 3. Reply with your mesh name (not keys!)  
>
> First wave helps us see real faucet + scanner load.
> → /faucet/

---

### Day 6–7 · Weekend · Engage only

Reply to comments; no new promo posts. Collect FAQ for Week 2.

---

## Week 2 — Proof

| Day | Post |
|-----|------|
| Mon | Vault e2e story: deposit → mint tMESH (devnet links) |
| Tue | “What MeshChain is NOT” honesty carousel |
| Wed | Mesh names deep-dive (Crockford base32, say-aloud IDs) |
| Thu | Short video: 30s product trailer (script in CAMPAIGN.md) |
| Fri | HN “Show HN” if not done Week 1 |
| Sat–Sun | Community Q&A |

### “What MeshChain is NOT” bullets

- Not mainnet  
- Not a yield product  
- Not anonymous like Tor  
- Not official Meshtastic  
- Not “unhackable” — open to audit  

---

## Week 3 — Builders

| Day | Post |
|-----|------|
| Mon | Architecture one-pager diagram |
| Tue | Good first issues + CONTRIBUTING |
| Wed | Cross-post r/solana: hybrid vault dual-control angle |
| Thu | Host ops: how the live scanner tunnel works |
| Fri | Contributor spotlight / RT friendly forks |
| Sat–Sun | Office hours style replies |

---

## Week 4 — Momentum

| Day | Post |
|-----|------|
| Mon | 4-week stats recap (height, faucet claims, stars) |
| Tue | Roadmap teaser: mesh 2FA on scanner |
| Wed | Best organic post → $50–100 paid boost |
| Thu | Hardware notes (Meshtastic kits) — link HARDWARE.md |
| Fri | “Clone and run in 10 minutes” guide |
| Sat–Sun | Thank-you + next milestone |

---

## Evergreen posts (reuse anytime)

**A · One-liner**
> Off-grid value on LoRa. Testnet live. → meshchain-sigma.vercel.app

**B · Dual-control**
> If the internet alone can empty the vault, it’s not hybrid.
> MeshChain requires mesh proof + attestors.

**C · CLI flex**
```
$ mesh new-wallet
→ M3SQRT-XTA1Y-ZJ6
$ mesh security
```

**D · Scanner pulse**
> Chain still breathing. Check height: /scanner/
