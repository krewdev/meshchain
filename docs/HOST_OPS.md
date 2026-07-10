# Always-on testnet host

Run a **public-style lab host**: 3 validators + tMESH faucet.

## Quick start (this machine)

```bash
./scripts/host_bootstrap.sh      # build + genesis
./scripts/start_testnet_host.sh  # validators :9100-9102 + faucet :8787
curl http://127.0.0.1:8787/info
```

Stop:

```bash
./scripts/stop_testnet_host.sh
```

## Faucet

| | |
|--|--|
| Health | `GET http://HOST:8787/health` |
| Info | `GET http://HOST:8787/info` |
| Drip | `POST /drip` `{"mesh_name":"M…","public_key_hex":"…"}` |

Website UI: https://meshchain-sigma.vercel.app/faucet.html  
(Set **Faucet API URL** to your host, e.g. `http://127.0.0.1:8787` or a public tunnel.)

First drip for a new wallet: include `public_key_hex` from `mesh address` (64 hex chars).

## VPS / Pi deploy

1. Clone repo to `/opt/meshchain`  
2. Install Rust, build release  
3. Create user `meshchain`  
4. `./scripts/host_bootstrap.sh`  
5. Install `deploy/meshchain-testnet.service`  
6. Open firewall: `9100-9102/tcp`, `8787/tcp` (or only 8787 publicly)  
7. Optional: reverse proxy + HTTPS for faucet  

```bash
sudo cp deploy/meshchain-testnet.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now meshchain-testnet
```

## Docker

```bash
./scripts/host_bootstrap.sh
docker compose up -d
```

Note: first container start may `apt-get` packages; prefer native `start_testnet_host.sh` on Pi.

## Meshtastic radios

```bash
./scripts/meshtastic_hardware_test.sh          # mock
./scripts/meshtastic_hardware_test.sh --hardware /dev/ttyUSB0
```

Channel: **MeshChain-Testnet-1** (private).
