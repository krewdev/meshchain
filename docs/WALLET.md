# Browser wallet

**TESTNET ONLY.** Keys in the browser are spendable. Download the JSON before you drip.

## Happy path (net)

1. Open https://meshchain-sigma.vercel.app/wallet/
2. **Create wallet** → save `mesh-wallet.json`
3. **Claim faucet** (sends your pubkey so the minter can create the account)
4. Wait for the inclusion line (balance / nonce change on the scanner)
5. **Send** to a mesh name (`MXXXXX-XXXXX-XXX`) or 16-char hex short id

Same panel is embedded on the homepage.

| Path | What it talks to |
|------|------------------|
| **Net** | `https://faucet.34.172.103.125.sslip.io` drip + `POST /submit`, scanner `GET /api/v1/accounts/…` and `POST /api/v1/submit` |
| **Radio** | local `http://127.0.0.1:9299` (`/health`, `/balance`, `/submit`) from `./scripts/start_radio_relay.sh` |

## Radio

The phone/browser does **not** speak LoRa. Radio mode is:

```text
browser  →  127.0.0.1:9299  →  mesh_radio_relay  →  TCP :9100  (+ optional LoRa)
```

```bash
./scripts/start_radio_relay.sh          # mock air + HTTP :9299
MESH_RADIO_PORT=/dev/ttyUSB0 ./scripts/start_radio_relay.sh
```

The Radio tab probes `/health`. If the relay is down, Send is disabled and the start command is shown.

## Signing

Browser signatures must match `TxBody::sign_bytes()` in `crates/proto/src/tx.rs`:

```text
bincode( (PROTOCOL_VERSION:u8 = 1, TxBody) )   // bincode 1.3 fixint LE
ed25519 detached over those bytes
```

Frozen fixture: `cargo test -p meshchain-proto browser_wallet_sign_vectors`  
JS copy: `web/wallet/wallet.js` `SIGN_VECTORS` / `verifySignVectors()`.

```bash
node scripts/check_browser_sign_vectors.mjs
```

## Hygiene

- Confirm + download on create
- Optional **Wrap passphrase** (PBKDF2-SHA256 + AES-GCM). Wrapped JSON has no raw `secret_hex`
- Multiple keys in `localStorage` (`meshchain.wallets.v2`)
- Receive QR of the mesh name
- Activity from `GET /api/v1/accounts/{id}/activity` (archived blocks only)

CLI wallets still work and use the same JSON (`secret_hex` + `public_hex`):

```bash
mesh new-wallet --name me.json --publish
mesh faucet-drip --wallet me.json
mesh send MXXXXX-XXXXX-XXX 1 --wallet me.json --submit 34.172.103.125:9100
```

## Seed deploy (needed for live Net send)

On `34.172.103.125` after this commit:

```bash
# pull main
# copy services/faucet/faucet_server.py  (POST /submit)
# rebuild scanner  (POST /api/v1/submit + GET …/activity)
export MESH_MINT_PEER=127.0.0.1:9100
# restart faucet + scanner + validators
./scripts/seed_health.sh
```

`seed_health.sh` now checks faucet `/submit` and scanner `/api/v1/submit` (expect structured 400, not 404).
