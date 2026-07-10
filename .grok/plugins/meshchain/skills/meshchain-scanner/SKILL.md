---
name: meshchain-scanner
description: Run and extend the MeshChain blockchain scanner (explorer API + UI)
---

# MeshChain scanner

## Run

```bash
cargo build -p meshchain-scanner
./target/debug/meshchain-scanner --data-dir ./data --listen 0.0.0.0:8787 --auth open
# UI: http://HOST:8787/
```

Or: `mesh scanner --listen 0.0.0.0:8787 --auth open`

## Auth

| Mode | Flag | Behavior |
|------|------|----------|
| Open | `--auth open` | Public internet (default) |
| Mesh 2FA | `--auth mesh2fa` | Require signed mesh challenge (scaffolded) |

## API

- `GET /api/v1/status`
- `GET /api/v1/blocks`
- `GET /api/v1/accounts`
- `GET /api/v1/search?q=`
- `GET /api/v1/auth/challenge`
- `POST /api/v1/auth/verify`

## Source

`crates/scanner/` — std HTTP server, reads `chain_state.json`.
