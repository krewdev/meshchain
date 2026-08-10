# MeshChain X account — create in 10 minutes

You must create the account yourself (phone/email verification).  
We automate **posting** after you attach API keys.

## 1. Sign up

1. Open https://x.com/i/flow/signup  
2. Use a dedicated email (e.g. meshchain@yourdomain or Gmail)  
3. Complete phone verification if asked  

### Suggested profile

| Field | Value |
|-------|--------|
| **Display name** | MeshChain |
| **Username** | `@MeshChain` (or `@MeshChainHQ` / `@MeshChainMesh` if taken) |
| **Bio** | Money that moves when the internet doesn’t. Off-grid mesh ledger for Meshtastic · public testnet · tMESH has no cash value · not official Meshtastic |
| **Location** | Mesh / LoRa |
| **Website** | https://meshchain-sigma.vercel.app |
| **Avatar** | `marketing/creatives/discord-icon.png` or `web/assets/logo.svg` export |
| **Header** | Screenshot of GEN hero from `marketing/creatives/ads.html` or `gen-hero-banner.jpg` |

### Pinned post (after first thread)

Use Day-1 launch thread post 1 + link to site + Discord.

---

## 2. Developer app (for automation)

1. Go to https://developer.x.com/en/portal/dashboard  
2. Sign in with the **same** MeshChain account (or a team account you control)  
3. **Create Project** → name `MeshChain`  
4. **Create App** → name `MeshChain Poster`  
5. App permissions → **Read and write** (must regenerate tokens after changing)  
6. **Keys and tokens**:
   - API Key + API Key Secret  
   - Access Token + Access Token Secret (user context, Read+Write)  

### Save locally (gitignored)

```bash
cat > ~/meshchain/.env.x <<'EOF'
export X_API_KEY='...'
export X_API_SECRET='...'
export X_ACCESS_TOKEN='...'
export X_ACCESS_SECRET='...'
EOF
```

---

## 3. Cost warning (2026)

X API pricing changes often. New projects often default to **pay-per-use** (writes cost money; posts with links cost more).  

Before automating daily posts:

- Check https://developer.x.com pricing / your project billing  
- Start with `--dry-run` and one manual post  
- Prefer ~1 post/day for Phase 1  

---

## 4. Post

```bash
cd ~/meshchain
source .env.x

# preview launch thread
python3 scripts/x_post.py --queue marketing/x/queue.json --limit 1 --dry-run

# post next item in queue (Day-1 thread)
python3 scripts/x_post.py --queue marketing/x/queue.json --limit 1

# one-off
python3 scripts/x_post.py --text 'Scanner live → https://34.172.103.125.sslip.io/'
```

GitHub Actions: see `.github/workflows/x-post.yml` (needs repo secrets).

---

## 5. Checklist

- [ ] Account created + avatar/header  
- [ ] Bio + website + Discord in bio or pinned  
- [ ] Dev app Read+Write tokens in `.env.x`  
- [ ] Dry-run queue OK  
- [ ] Day-1 thread posted  
- [ ] Tokens **never** committed to git  
```
