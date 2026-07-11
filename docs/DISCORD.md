# MeshChain Discord — setup guide

Discord servers must be created in the **Discord app** (or discord.com) by a logged-in account.  
This doc is a complete blueprint so the MeshChain community can be stood up in ~10 minutes.

**Official product note:** MeshChain is community software for the Meshtastic ecosystem — **not** an official Meshtastic Foundation product. Say that in the server description and `#welcome`.

---

## 1. Create the server (you do this)

1. Open [Discord](https://discord.com/app) → **+** → **Create My Own** → **For a club or community**.
2. Name: **MeshChain**
3. Upload icon (optional): use repo marketing art or a simple “M” mesh mark.
4. Server settings → **Overview**:
   - **Description:**  
     `Hold and move value on Meshtastic mesh — hybrid Solana vaults, tMESH testnet. Community software, not official Meshtastic.`
   - AFK / system channel: `#system` (create below) or leave default.
5. **Enable Community** (Server Settings → Enable Community) for:
   - Rules screening  
   - Announcement channels  
   - Better discovery later  

---

## 2. Invite link

Server Settings → **Invites** → Create invite:

| Setting | Value |
|---------|--------|
| Channel | `#welcome` |
| Expire | Never |
| Max uses | No limit |

Save the link as:

```text
https://discord.gg/YOUR_CODE
```

Add it to:

- [ ] GitHub README  
- [ ] https://meshchain-sigma.vercel.app  
- [ ] `docs/GETTING_STARTED.md` / site footer  
- [ ] Twitter / X bio  

---

## 3. Roles

Create roles (top → bottom hierarchy). Turn **Display role members separately** on for Staff / Core.

| Role | Color (suggest) | Permissions (high level) |
|------|-----------------|---------------------------|
| **Admin** | Red | Full admin (you) |
| **Moderator** | Orange | Manage messages, timeout, kick |
| **Core** | Purple | Trusted builders / maintainers |
| **Validator Ops** | Blue | People running public testnet hosts |
| **Builder** | Green | Contributors / PRs |
| **Meshtastic** | Teal | Radio / firmware / hardware chat |
| **Member** | Grey | Default after rules accept |
| **Muted** | Dark | No send (mod tool) |

Bot role: place **above** roles the bot must assign.

---

## 4. Channels

### Categories & channels

```text
📋 INFO
  #welcome              — rules + links (read-only for @everyone)
  #announcements        — releases only (Staff write)
  #links                — GitHub, testnet, faucet, scanner, docs
  #roles                — self-assign instructions (or reaction roles)

💬 COMMUNITY
  #general              — hangout
  #introductions        — who you are + mesh name if any
  #support              — “help me run X”
  #showcase             — screenshots, nodes, setups

🛠 BUILD
  #dev                  — protocol / rust / PRs
  #validators           — cloud host, gossip, ops
  #scanner-faucet       — HTTP APIs, explorer
  #bridge-solana        — hybrid vault / devnet
  #meshtastic-radio     — LoRa, channels, bridge.py

🧪 TESTNET
  #testnet-status       — chain height, incidents (ops)
  #faucet-drops         — optional; rate-limit chatter
  #feedback             — wipe warnings, feature asks

🔒 STAFF (private — Admin/Mod/Core only)
  #staff
  #alerts               — webhook from CI / uptime later
```

### `#welcome` template (pin this)

```markdown
# Welcome to MeshChain

**Mesh-native ledger + wallets for Meshtastic** — with optional hybrid vaults on Solana.

> Community software. **Not** an official Meshtastic Foundation product.
> **Testnet tMESH has no cash value** and may be wiped.

## Start here
• Docs: https://meshchain-sigma.vercel.app/docs/
• Testnet: https://meshchain-sigma.vercel.app/docs/?doc=TESTNET
• GitHub: https://github.com/krewdev/meshchain
• Faucet UI: https://meshchain-sigma.vercel.app/faucet/
• Scanner: (your cloud/public URL when live)

## Rules (short)
1. Be respectful — no scams, no phishing, no “send me your seed.”
2. Never paste **private keys**, validator secrets, or cold keys.
3. Testnet only unless maintainers say otherwise — **no mainnet deposits** into unaudited programs.
4. No spam / shill unrelated tokens.
5. Meshtastic channel PSKs: treat as sensitive; don’t drop private PSKs in public channels.

## Get help
• Install / wallets → #support  
• Validators / cloud → #validators  
• Radios → #meshtastic-radio  
• Protocol → #dev  

Accept the rules (Community onboarding) to unlock the rest of the server.
```

### `#links` pin

```markdown
GitHub:     https://github.com/krewdev/meshchain
Validator:  https://github.com/krewdev/meshchain-validator  (if published)
Site:       https://meshchain-sigma.vercel.app
Testnet:    chain_id = meshchain-testnet-1
Channel:    MeshChain-Testnet-1 (private Meshtastic — not LongFast for funds)
```

---

## 5. Recommended bots (optional)

| Bot | Why |
|-----|-----|
| **Carl-bot** or **Dyno** | Reaction roles, moderation, autoresponder |
| **GitHub** (official webhook) | PR/release posts → `#announcements` or `#dev` |
| **Carl** welcome DM | Optional |

### GitHub webhook → Discord

1. Channel → Edit → Integrations → Webhooks → New  
2. GitHub repo → Settings → Webhooks → payload URL = Discord webhook  
3. Content type JSON; events: Releases, Pull requests (or just Releases)

---

## 6. Safety defaults

Server Settings → **Safety Setup**:

- DM scanning: high  
- Explicit media: block  
- **2FA required for moderation**  
- Verification level: **Email verified** (or phone if public/large)  

Community onboarding:

- Require accept rules before talking  
- Rules = the 5 bullets from `#welcome`  

---

## 7. After the server exists — paste into the project

Once you have the invite URL, tell a maintainer (or open a PR) to set:

```markdown
<!-- README.md -->
[Discord](https://discord.gg/YOUR_CODE)
```

Suggested README badge:

```markdown
[![Discord](https://img.shields.io/badge/Discord-MeshChain-5865F2?logo=discord&logoColor=white)](https://discord.gg/YOUR_CODE)
```

---

## 8. Launch checklist

- [ ] Server created + Community enabled  
- [ ] Roles + channel tree  
- [ ] `#welcome` / `#links` pinned  
- [ ] Permanent invite  
- [ ] GitHub webhook (optional)  
- [ ] README + site link updated  
- [ ] You (Admin) + 1 backup Admin 2FA  

---

## What I (the agent) cannot do

- Create the Discord server or own an invite without your Discord login  
- Join Discord as your bot without a **bot token** you create at https://discord.com/developers/applications  

If you create a **bot application** and paste a token into a local env (never commit it), we can later add a small status bot (testnet height → `#testnet-status`). Say if you want that next.
