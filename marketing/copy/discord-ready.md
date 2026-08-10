# Discord — paste-ready copy

Create the server in the Discord app first (`docs/DISCORD.md` has the full setup).  
Then paste these into channel topics / pins.

**Live invite:** https://discord.gg/9YXVtXf2yX  

(Also wired into README / site / Day-1 posts.)

---

## Server overview

| Field | Value |
|-------|--------|
| **Name** | MeshChain |
| **Description** | Hold and move value on Meshtastic mesh — hybrid Solana vaults, tMESH testnet. Community software, not official Meshtastic. |
| **Welcome screen** | Claim faucet → open scanner → introduce your mesh name in #introductions |

---

## Rules (Community onboarding)

```
1. Be respectful — no scams, phishing, or “send me your seed.”
2. Never paste private keys, validator secrets, or cold keys.
3. Testnet only unless maintainers say otherwise — no mainnet deposits into unaudited programs.
4. No spam / shill of unrelated tokens or paid signals.
5. Meshtastic channel PSKs are sensitive — don’t drop private PSKs in public channels.
6. tMESH has no cash value and may be wiped. Not financial advice. Not official Meshtastic.
```

---

## Role colors (hex)

| Role | Color | Note |
|------|-------|------|
| Admin | `#ED4245` | You + backup |
| Moderator | `#E67E22` | Timeout / kick |
| Core | `#9B59B6` | Maintainers |
| Validator Ops | `#3498DB` | Public hosts |
| Builder | `#2ECC71` | Contributors |
| Meshtastic | `#1ABC9C` | Radio folk |
| Member | `#95A5A6` | Default |
| Muted | `#2C2F33` | Mod tool |

---

## Channel topics (paste into channel settings → Topic)

### 📋 INFO

**#welcome**
```
Rules + start links. Read-only for @everyone. Accept Community rules to unlock chat.
```

**#announcements**
```
Releases, testnet wipes, seed IP changes. Staff write only.
```

**#links**
```
Canonical URLs: site, GitHub, scanner, faucet, docs, seed peer.
```

**#roles**
```
How to get Builder / Meshtastic / Validator Ops tags. No free Nitro sales.
```

### 💬 COMMUNITY

**#general**
```
Hangout. Keep support questions in #support so they’re searchable.
```

**#introductions**
```
Who you are · hardware · mesh name if you have one (e.g. M3SQRT-XTA1Y-ZJ6).
```

**#support**
```
Install, wallets, join-public, faucet errors. Include OS + command + error text.
```

**#showcase**
```
Nodes, field kits, scanner screenshots, air-path demos.
```

### 🛠 BUILD

**#dev**
```
Protocol, Rust crates, PRs, design debate. Prefer GitHub Issues for bugs.
```

**#validators**
```
Cloud hosts, gossip peers, systemd, multi-validator ops.
```

**#scanner-faucet**
```
HTTP APIs, explorer UI, faucet rate limits, sslip endpoints.
```

**#bridge-solana**
```
Hybrid vault, devnet program, attestors. DEVNET ONLY — no mainnet deposits.
```

**#meshtastic-radio**
```
LoRa regions, MeshChain-Testnet-1 channel, bridge.py, airtime budget.
```

### 🧪 TESTNET

**#testnet-status**
```
Height, incidents, planned resets. Ops posts; community can confirm outages.
```

**#faucet-drops**
```
Faucet chatter / drip confirmations. Don’t spam-claim.
```

**#feedback**
```
Feature asks, wipe warnings, “this is broken” with repro steps.
```

### 🔒 STAFF (private)

**#staff**
```
Internal coordination. No public leaks of keys or incident details until ready.
```

**#alerts**
```
Webhooks from CI / uptime. Keep noise low.
```

---

## Pinned messages

### #welcome (pin)

```markdown
# Welcome to MeshChain

**Mesh-native ledger + wallets for Meshtastic** — optional hybrid vaults on Solana **devnet**.

> Community software. **Not** an official Meshtastic Foundation product.  
> **Testnet tMESH has no cash value** and may be wiped.

## Start here (2 minutes)
1. **Scanner** (no install): https://34.172.103.125.sslip.io/
2. **Faucet UI**: https://meshchain-sigma.vercel.app/faucet/
3. **Join docs**: https://meshchain-sigma.vercel.app/docs/?doc=TESTNET
4. **GitHub**: https://github.com/krewdev/meshchain

## CLI (builders)
```bash
git clone https://github.com/krewdev/meshchain.git && cd meshchain
cargo build -p mesh -p meshchain-node
./target/debug/mesh join-public
./target/debug/mesh new-wallet --name me.json --publish
./target/debug/mesh faucet-drip --wallet me.json
```

## Rules (short)
1. Be respectful — no scams, no phishing, no “send me your seed.”
2. Never paste **private keys**, validator secrets, or cold keys.
3. Testnet only — **no mainnet deposits** into unaudited programs.
4. No spam / shill unrelated tokens.
5. Don’t post private Meshtastic PSKs.

## Get help
• Install / wallets → #support  
• Validators / cloud → #validators  
• Radios → #meshtastic-radio  
• Protocol → #dev  

Accept the rules to unlock the rest of the server. Introduce yourself in #introductions.
```

### #links (pin)

```markdown
## Canonical links

| What | URL |
|------|-----|
| Site | https://meshchain-sigma.vercel.app |
| Docs | https://meshchain-sigma.vercel.app/docs/ |
| Testnet guide | https://meshchain-sigma.vercel.app/docs/?doc=TESTNET |
| Faucet UI | https://meshchain-sigma.vercel.app/faucet/ |
| Live scanner | https://34.172.103.125.sslip.io/ |
| Live faucet API | https://faucet.34.172.103.125.sslip.io/ |
| GitHub | https://github.com/krewdev/meshchain |
| network.json | https://meshchain-sigma.vercel.app/testnet/network.json |

## Network
- **chain_id:** `meshchain-testnet-1`
- **Token:** tMESH (no cash value)
- **Meshtastic channel name:** `MeshChain-Testnet-1` (not LongFast for funds)
- **Public seed:** `34.172.103.125:9100`
- **Observer:** `35.192.20.103:9100`
- **Solana bridge:** **devnet only**

## Discord invite
https://discord.gg/9YXVtXf2yX
```

### #announcements — first post template

```markdown
**MeshChain public testnet is live** (`meshchain-testnet-1`)

- Scanner: https://34.172.103.125.sslip.io/
- Faucet: https://meshchain-sigma.vercel.app/faucet/
- Repo: https://github.com/krewdev/meshchain

Reminder: **tMESH has no cash value**. State may be wiped. Not official Meshtastic.

If you’re running a node or radio bridge, say hi in #introductions and #validators.
```

### #testnet-status — status line template

```markdown
**Status:** green  
**Seed:** 34.172.103.125:9100  
**Scanner:** https://34.172.103.125.sslip.io/  
**Height:** _(check scanner)_ · **Accounts:** _(check scanner)_  
**Note:** tMESH worthless / wipeable
```

---

## Welcome screen / embed blurb (Server Settings)

```
Money that moves when the internet doesn’t.

Open-source mesh ledger for Meshtastic · public testnet · hybrid Solana vault experiments (devnet).

tMESH has no cash value · not official Meshtastic
```

---

## Reaction roles (optional, Carl-bot / Dyno)

| Emoji | Role |
|-------|------|
| 🛠 | Builder |
| 📡 | Meshtastic |
| 🖥 | Validator Ops |

Message for #roles:

```
React to get a tag (helps us route questions):

🛠 Builder — coding / PRs  
📡 Meshtastic — radios / LoRa  
🖥 Validator Ops — running seed / observer hosts  

You can hold more than one.
```

---

## Checklist after create

- [ ] Community enabled + rules screening  
- [ ] Channels + topics set  
- [ ] #welcome and #links pinned  
- [ ] Permanent invite created  
- [ ] Invite pasted into README, site, day1-posts  
- [ ] GitHub webhook → #dev or #announcements (optional)  
- [ ] Second Admin with 2FA  
```
