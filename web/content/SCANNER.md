# MeshChain Scanner (testnet explorer)

## Public URL (Vercel)

**https://meshchain-sigma.vercel.app/scanner/**

Static explorer that loads a published snapshot of `chain_state.json`.  
**Internet-open** for testnet. Mesh 2FA is planned for the self-hosted mode.

### Refresh the public snapshot

```bash
# after producing new blocks / mints locally
./scripts/sync_scanner_snapshot.sh
vercel --prod
# or push to main if Git auto-deploys
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
