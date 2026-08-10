# Day-1 launch posts (ready to paste)

**Always include:** testnet only · tMESH has no cash value · not official Meshtastic.

**Links**

| Asset | URL |
|-------|-----|
| Site | https://meshchain-sigma.vercel.app |
| Scanner | https://34.172.103.125.sslip.io/ |
| Faucet UI | https://meshchain-sigma.vercel.app/faucet/ |
| Docs / join | https://meshchain-sigma.vercel.app/docs/?doc=TESTNET |
| GitHub | https://github.com/krewdev/meshchain |
| Discord | https://discord.gg/9YXVtXf2yX |

---

## 1) X / Twitter — launch thread

**Post 1 (hook)**

```
Money that moves when the internet doesn’t.

MeshChain is an open-source mesh-native ledger for Meshtastic LoRa.

• Offline settlement path over radio
• Hybrid Solana vaults that refuse internet-only unlocks
• Public testnet live right now

tMESH has no cash value. Not an official Meshtastic product.

🧵
```

**Post 2 (proof)**

```
Proof, not vapor:

🔍 Live scanner → https://34.172.103.125.sslip.io/
💧 Faucet → https://meshchain-sigma.vercel.app/faucet/
📡 Seed peer → 34.172.103.125:9100
📦 Open source → https://github.com/krewdev/meshchain

Watch height, accounts, and validators update in public.
```

**Post 3 (how)**

```
Fastest path (builders):

git clone https://github.com/krewdev/meshchain.git
cargo build -p mesh -p meshchain-node
./target/debug/mesh join-public
./target/debug/mesh new-wallet --name me.json --publish
./target/debug/mesh faucet-drip --wallet me.json

You get a mesh name like M3SQRT-XTA1Y-ZJ6.
```

**Post 4 (why hybrid)**

```
Why “hybrid”?

Large value can sit in a Solana *devnet* vault bound to a mesh identity.
Unlock needs mesh-side proof + multi-attestor co-sign.

Internet alone is designed to fail.
That’s the point for dual-control custody experiments.
```

**Post 5 (CTA)**

```
If you run Meshtastic nodes, privacy tooling, or Solana bridges:

1. Open the scanner
2. Claim faucet tMESH
3. Star / fork / roast us on Issues

Site → https://meshchain-sigma.vercel.app
Discord → https://discord.gg/9YXVtXf2yX

Feedback > hype. Builders only.
```

**Single-tweet alt (if you skip the thread)**

```
Money that moves when the internet doesn’t.

MeshChain: mesh-native ledger for Meshtastic + hybrid Solana vault (devnet).
Public testnet live · scanner + faucet open · tMESH has no cash value.

https://meshchain-sigma.vercel.app
https://github.com/krewdev/meshchain
```

---

## 2) Reddit — r/meshtastic (feedback-seeking)

**Title**

```
[Project] MeshChain — open-source testnet ledger that can settle over Meshtastic (seeking RF / airtime feedback)
```

**Body**

```markdown
Hey all — sharing a builder project and looking for honest Meshtastic feedback (airtime, channel hygiene, FRAG realities).

**What it is**
MeshChain is a mesh-native ledger + CLI wallet toolkit meant to run *with* Meshtastic (optional hybrid vault experiments on Solana **devnet**). Everyday mesh use is designed for offline/LoRa paths; it’s **not** “buy a coin.”

**What it is not**
- Not an official Meshtastic Foundation product  
- **tMESH is testnet-only and has no cash value** (state can be wiped)  
- Not claiming Tor-level privacy on RF — we document residual risk  

**Live proof**
- Site: https://meshchain-sigma.vercel.app  
- Scanner: https://34.172.103.125.sslip.io/  
- Faucet: https://meshchain-sigma.vercel.app/faucet/  
- Repo: https://github.com/krewdev/meshchain  
- Seed: `34.172.103.125:9100` · channel name `MeshChain-Testnet-1` (do **not** put balances on LongFast)

**Why post here**
LoRa airtime is the hard part. If you’ve run dense meshes, I’d love critique on:
1. Frame size / multi-packet FRAG assumptions  
2. Whether a private funds channel + tip gossip is sane  
3. What would make “try this at a field day” less painful  

Happy to answer technical questions. Roast the design if it’s wrong — that’s useful.

*(Mods: not selling anything; pure open-source testnet.)*
```

**Optional second sub (later, not Day-1 spam):** r/privacy or r/selfhosted — same honesty, different angle (“settlement path when ISP dies”).

---

## 3) Show HN

**Title**

```
Show HN: MeshChain – off-grid mesh ledger for Meshtastic + hybrid Solana vault (testnet)
```

**Body**

```markdown
MeshChain is an open-source ledger and wallet CLI for Meshtastic LoRa meshes, plus an optional hybrid vault on Solana **devnet**.

**Problem**
Mesh networks already relay messages off-grid, but value rails are almost always internet-only. When the ISP dies—or you want dual-control custody—there’s little to experiment with that respects RF constraints.

**What we built**
- Mesh-native PoA testnet (`meshchain-testnet-1`) with memorable mesh names  
- `mesh` CLI: join public seed, faucet drip, send, cold keys (ML-DSA-65 path for large moves)  
- Public scanner + faucet endpoints  
- Hybrid vault idea: internet alone cannot unlock; mesh proof + attestors required  

**Live**
- https://meshchain-sigma.vercel.app  
- Scanner: https://34.172.103.125.sslip.io/  
- Repo: https://github.com/krewdev/meshchain  

**Honest limits**
Testnet only — **tMESH has no cash value**. Not official Meshtastic. RF is not Tor. We’re looking for builders to break it and open issues.

Happy to discuss protocol choices, airtime budget, and the hybrid lock model.
```

---

## 4) Short replies (use under any post)

**“Is this a coin?”**

```
Public testnet only. tMESH has no cash value and can be wiped. It’s a builder sandbox for mesh settlement + dual-control vault experiments—not a token sale.
```

**“Official Meshtastic?”**

```
No. Community open-source software that *uses* Meshtastic as transport. Independent of the Meshtastic Foundation.
```

**“How do I try in 5 minutes?”**

```
Open the scanner (no install): https://34.172.103.125.sslip.io/
Claim faucet: https://meshchain-sigma.vercel.app/faucet/
Or CLI: clone → mesh join-public → new-wallet → faucet-drip (docs on the site).
```

---

## 5) Posting checklist

- [ ] Attach a creative from `marketing/creatives/` (hero / square / story)
- [ ] First reply on X = scanner + faucet links again
- [ ] Stay in thread for 2–4 hours and answer every technical question
- [ ] After Discord exists, edit posts / pin invite
- [ ] Do **not** cross-post identical text to 5 subs in one hour (looks like spam)
```
