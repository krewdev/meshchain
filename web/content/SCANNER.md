# MeshChain Scanner (testnet explorer)

## Public URL (Vercel)

**https://meshchain-sigma.vercel.app/scanner/**

**Internet-open** for testnet. Mesh 2FA is for self-hosted mode later.

### Auto-update (read this)

See **[SCANNER_AUTO_UPDATE.md](./SCANNER_AUTO_UPDATE.md)** — three ways:

1. **Live API** (best): host `meshchain-scanner`, set `web/scanner/data/config.json` → `live_api`
2. **GitHub Action** snapshot every 30m via secret `MESHCHAIN_CHAIN_STATE_URL`
3. **Manual:** `./scripts/sync_scanner_snapshot.sh && git push`

### Refresh the public snapshot (manual)

```bash
./scripts/sync_scanner_snapshot.sh
git add web/scanner/data && git commit -m "chore: scanner snapshot" && git push
```

### Optional live API

Run the Rust process and point the Vercel UI at it:

```bash
cargo build -p meshchain-scanner
./target/debug/meshchain-scanner --data-dir ./data --listen 0.0.0.0:8787 --auth open
# open:
# https://meshchain-sigma.vercel.app/scanner/?api=https://YOUR_HOST:8787
```

| Mode | Flag / URL | Behavior |
|------|------------|----------|
| Open (now) | `--auth open` or Vercel static | Public browse |
| Mesh 2FA (later) | `--auth mesh2fa` | Require signed mesh challenge |

## Self-hosted API routes

| Path | Description |
|------|-------------|
| `GET /` | Explorer UI |
| `GET /api/v1/status` | Height, supply, auth |
| `GET /api/v1/blocks` | Recent blocks |
| `GET /api/v1/accounts` | Accounts + mesh names |
| `GET /api/v1/search?q=` | Name / hex / height |
| `GET /api/v1/validators` | Validator set |
| `GET /api/v1/auth/challenge` | Mesh 2FA challenge |
| `POST /api/v1/auth/verify` | Verify signature |

## Vercel static files

```
web/scanner/index.html
web/scanner/js/app.js
web/scanner/js/meshname.js
web/scanner/data/chain_state.json   # snapshot
web/scanner/data/meta.json
```

## TESTNET

tMESH has **no cash value**.
