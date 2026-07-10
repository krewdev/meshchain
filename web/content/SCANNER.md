# MeshChain Scanner (testnet explorer)

Internet-accessible blockchain scanner for **meshchain-testnet-1**.

## Run

```bash
# Need chain data first
cargo build -p mesh -p meshchain-node -p meshchain-scanner
./target/debug/mesh testnet-setup
./target/debug/mesh demo   # or mint-for-deposit after vault deposit

# Public scanner (bind all interfaces)
./target/debug/meshchain-scanner --data-dir ./data --listen 0.0.0.0:8787 --auth open
```

Open: http://YOUR_IP:8787/

## Auth modes

| Mode | Flag | Behavior |
|------|------|----------|
| **Open** (now) | `--auth open` | Anyone on the internet can browse |
| **Mesh 2FA** (later) | `--auth mesh2fa` | API returns 401 until client signs a mesh challenge |

Challenge endpoints (always available):

- `GET /api/v1/auth/challenge`
- `POST /api/v1/auth/verify` `{ challenge_id, pubkey_hex, signature_hex }`

When enforcing mesh2fa, wire a session cookie after verify.

## API

| Path | Description |
|------|-------------|
| `GET /api/v1/status` | Height, supply, auth mode |
| `GET /api/v1/blocks?limit=50` | Recent blocks |
| `GET /api/v1/blocks/:height` | One block |
| `GET /api/v1/accounts?limit=100` | Accounts by balance |
| `GET /api/v1/accounts/:id` | Mesh name or hex short id |
| `GET /api/v1/search?q=` | Name / hex / height |
| `GET /api/v1/validators` | Validator set |
| `GET /api/v1/network` | network.json metadata |
| `GET /api/v1/auth/mode` | open vs mesh2fa |

## Internet access

1. Run with `--listen 0.0.0.0:8787`
2. Open firewall / cloud security group for TCP 8787
3. Optional: reverse-proxy with TLS (Caddy/nginx) → `scanner.yourdomain.com`

## Data source

Reads `data/chain_state.json` (reloads every few seconds).  
Point `--data-dir` at a live validator’s data directory.

## TESTNET

tMESH has **no cash value**. Do not treat balances as real money.
