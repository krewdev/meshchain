# Public MeshChain testnet artifacts

**Seed host (Google Cloud):** `34.172.103.125`

| Endpoint | URL |
|----------|-----|
| Faucet | http://34.172.103.125:8787 |
| Scanner | http://34.172.103.125:8788/ |
| Submit / seed peer | `34.172.103.125:9100` (also :9101, :9102) |

| File | Use |
|------|-----|
| `genesis.json` | Shared PoA genesis — put in `data/genesis.json` |
| `seeds.json` | Bootstrap peers for observers/validators |

## Join as observer

```bash
mkdir -p data && cp testnet/published/genesis.json data/genesis.json
cargo build -p meshchain-node -p mesh
./target/debug/mesh observer --peer 34.172.103.125:9100 --listen 0.0.0.0:9100
```

## Wallet

```bash
./target/debug/mesh new-wallet --name me.json
./target/debug/mesh register --wallet data/keys/me.json --submit 34.172.103.125:9100
# faucet drip (needs public_key_hex first time) — see faucet UI
./target/debug/mesh send <NAME> 1 --wallet data/keys/me.json --submit 34.172.103.125:9100
```

## Become a producer

```bash
./target/debug/mesh validator-keygen
# apply with testnet/operator_application.example.json
# maintainers add your public_hex to a new genesis and republish
```

**Secrets:** validator private keys stay on the seed server only — never in this folder.
