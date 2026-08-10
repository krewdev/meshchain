# How the scanner auto-updates

Vercel only hosts **static files** (or short serverless functions).  
The chain keeps changing on your validator, so “auto-update” needs one of these patterns.

---

## Option A — Live API (best, real-time)

The Vercel page **polls a live Rust scanner every 15s** — **no redeploy** when the chain moves.

### One command (this machine)

```bash
# Default: point the site at the stable public seed scanner
./scripts/start_scanner_live.sh

# Lab only (laptop + Cloudflare tunnel):
FORCE_TUNNEL=1 ./scripts/start_scanner_live.sh
```

Then open: **https://meshchain-sigma.vercel.app/scanner/**

| Piece | Port / URL |
|-------|------------|
| Public seed scanner | `https://34.172.103.125.sslip.io` |
| Public faucet | `https://faucet.34.172.103.125.sslip.io` |
| Vercel UI | polls `live_api` from `web/scanner/data/config.json` |

> The public seed already has HTTPS. Do not overwrite `live_api` with an ephemeral trycloudflare URL unless you are running a private lab.

### Manual VPS (fixed domain)

```bash
cargo build -p meshchain-scanner --release
./target/release/meshchain-scanner --data-dir ./data --listen 0.0.0.0:8788 --auth open
# set live_api in config.json to https://scanner.yourdomain.com, deploy once
```

---

## Option B — Snapshot + GitHub Action (good, near-real-time)

The public site reads `/scanner/data/chain_state.json`.  
A workflow refreshes that file and pushes → Vercel redeploys.

### Setup

1. Expose chain state from your host (examples):

```bash
# simple: serve chain_state only
python3 -m http.server 8899 --directory ./data
# then URL is http://YOUR_IP:8899/chain_state.json
```

Or use the scanner: `https://YOUR_HOST:8787` does not serve raw chain_state by default — use a small static file server or:

```bash
# cron on the validator machine every 10 minutes:
./scripts/sync_scanner_snapshot.sh
git add web/scanner/data && git commit -m "chore: snapshot" && git push
```

2. **Optional remote fetch in CI:** add GitHub secret:

| Secret | Value |
|--------|--------|
| `MESHCHAIN_CHAIN_STATE_URL` | URL that returns `chain_state.json` |

Workflow: `.github/workflows/scanner-snapshot.yml`  
Runs every **30 minutes** + on manual dispatch + when `data/chain_state.json` is pushed.

---

## Option C — Manual (what you had before)

```bash
./scripts/sync_scanner_snapshot.sh
vercel --prod
# or git push after commit
```

---

## Comparison

| Method | Freshness | Needs Vercel redeploy? | Needs always-on host? |
|--------|-----------|------------------------|------------------------|
| **A Live API** | ~15s | No (after config once) | Yes |
| **B Snapshot CI** | ~30 min (or cron) | Yes (auto) | Only if CI pulls remote URL |
| **C Manual** | When you run it | Yes | No |

---

## Mesh 2FA later

Keep Vercel UI public for marketing.  
Run operator explorer as:

```bash
meshchain-scanner --auth mesh2fa --listen 0.0.0.0:8787
```

Then only that host enforces mesh signatures; the static Vercel site can stay open or also point `live_api` there.
