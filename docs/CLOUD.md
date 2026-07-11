# Cloud validators (MeshChain)

Run a **3-validator testnet host** (plus faucet + scanner) on a cloud VM.

## Recommended path: Google Cloud (gcloud)

You already have `gcloud` on the laptop. Project example: `xai-ipc-sim-2026`.

### Cost

| Machine | RAM | Typical use | Ballpark |
|---------|-----|-------------|----------|
| `e2-micro` | 1 GB | **Too small** for compile | avoid |
| `e2-small` | 2 GB | OK for build + run | ~\$10–15/mo |
| `e2-medium` | 4 GB | Comfortable | ~\$25–30/mo |

First boot compiles Rust (~10–25 min). Then idle cost is the VM only.

### One command (from monorepo)

```bash
cd ~/meshchain   # or ~/projects/meshchain

# optional:
# export PROJECT=xai-ipc-sim-2026
# export ZONE=us-central1-a
# export MACHINE=e2-small

./deploy/gce-create.sh
```

This will:

1. Enable **Compute Engine API**
2. Open firewall: **22, 8787, 8788** (validators **9100–9102** stay closed unless `OPEN_VALIDATOR_PORTS=1`)
3. Create Ubuntu 22.04 VM
4. Run `deploy/vps-setup.sh` (build + 3 validators + faucet + scanner + systemd)

### After create

```bash
# follow install log
gcloud compute ssh meshchain-testnet --zone=us-central1-a \
  --command='sudo tail -f /var/log/meshchain-startup.log'

# health
IP=$(gcloud compute instances describe meshchain-testnet \
  --zone=us-central1-a --format='get(networkInterfaces[0].accessConfigs[0].natIP)')
curl -s http://$IP:8787/info
curl -s http://$IP:8788/api/v1/status
```

Scanner UI: `http://$IP:8788/`

### SSH + mesh-validator on the box

```bash
gcloud compute ssh meshchain-testnet --zone=us-central1-a
sudo -u meshchain -i
cd /opt/meshchain
./scripts/stop_testnet_host.sh
./scripts/start_testnet_host.sh
# or clone meshchain-validator and point MESHCHAIN_HOME=/opt/meshchain
```

### Tear down (stop billing)

```bash
gcloud compute instances delete meshchain-testnet --zone=us-central1-a --quiet
```

---

## Alternative: any Ubuntu VPS (Hetzner / DO / Lightsail)

```bash
ssh root@YOUR_IP
curl -fsSL https://raw.githubusercontent.com/krewdev/meshchain/main/deploy/vps-setup.sh | sudo bash
```

See monorepo `docs/VPS_PUBLIC.md`.

---

## Alternative: no VPS (tunnel from laptop)

Expose local host without a cloud VM:

```bash
cd ~/meshchain
./scripts/start_testnet_host.sh
./scripts/start_public_tunnel.sh   # Cloudflare quick tunnel
```

URL changes on restart — fine for demos, not stable public testnet.

---

## Security notes

| Port | Public? | Purpose |
|------|---------|---------|
| 22 | yes (or IAP) | SSH |
| 8787 | yes | Faucet HTTP |
| 8788 | yes | Scanner |
| 9100–9102 | **no** by default | Validator gossip — only open for multi-host peering |

- tMESH is **test only**
- Validator keys live under `/opt/meshchain/data/host/` — back up if you care about that chain history
- Prefer HTTPS + domain (`MESHCHAIN_DOMAIN=…`) for any real public faucet

---

## Multi-region (advanced)

Today’s lab host is **3 validators on one machine**. For true multi-cloud:

1. Share the **same** `genesis.json` + per-host `validator-N.json`
2. Open `9100` between peers
3. `meshchain-node run --validator-index N --peer other:9100 …`

Use `meshchain-validator` repo automation per host once keys are distributed.
