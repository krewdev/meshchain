# Public VPS + tunnel + radio multi-hop

## A. Fixed-IP VPS (recommended for stable testnet)

### 1. Provision

Any small VPS (DigitalOcean, Hetzner, Linode, AWS Lightsail):

- Ubuntu 22.04+  
- 1 vCPU / 1–2 GB RAM  
- Open ports: `8787` (faucet), optionally `9100-9102` (validators for peers)

### 2. Install

```bash
sudo adduser --disabled-password meshchain
sudo mkdir -p /opt/meshchain && sudo chown meshchain:meshchain /opt/meshchain
sudo -u meshchain bash -lc '
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  source $HOME/.cargo/env
  git clone https://github.com/krewdev/meshchain.git /opt/meshchain
  cd /opt/meshchain
  cargo build -p meshchain-node --release
  ./scripts/host_bootstrap.sh
'
```

### 3. Run as service

```bash
sudo cp /opt/meshchain/deploy/meshchain-testnet.service /etc/systemd/system/
# edit WorkingDirectory if needed
sudo systemctl daemon-reload
sudo systemctl enable --now meshchain-testnet
curl http://127.0.0.1:8787/info
```

### 4. Publish endpoints

Point DNS `faucet.yourdomain.com` → VPS IP (A record).  
Nginx/Caddy reverse proxy with HTTPS optional.

Update `testnet/network.json`:

```json
"endpoints": {
  "faucet_api": "https://faucet.yourdomain.com",
  "faucet_ui": "https://meshchain-sigma.vercel.app/faucet/"
}
```

### 5. Firewall

```bash
sudo ufw allow OpenSSH
sudo ufw allow 8787/tcp
# optional peer gossip:
# sudo ufw allow 9100:9102/tcp
sudo ufw enable
```

---

## B. Quick public tunnel (no VPS)

On a machine already running the host:

```bash
./scripts/start_testnet_host.sh
./scripts/start_public_tunnel.sh
# prints https://xxxx.trycloudflare.com
```

Use that URL as **Faucet API URL** on  
https://meshchain-sigma.vercel.app/faucet/

**Note:** quick tunnels change URL on restart. For a stable link, use a named Cloudflare tunnel or a VPS.

Stop tunnel:

```bash
./scripts/stop_public_tunnel.sh
```

---

## C. Radio multi-hop finality path

Validators keep **TCP gossip** for reliable lab finality.  
`tools/mesh_radio_relay.py` carries **compact** messages over Meshtastic for multi-hop awareness:

```bash
# on host with radio + local validator :9100
python3 tools/mesh_radio_relay.py --port /dev/ttyUSB0 --tcp 127.0.0.1:9100

# mock (no hardware)
python3 tools/mesh_radio_relay.py --mock --tcp 127.0.0.1:9100
```

| Over LoRa | Over TCP |
|-----------|----------|
| hello, block_ack, small tx, block_hint | full blocks, full txs |

Large blocks stay on TCP; radios extend **reach** between sites that each run a validator + relay.

Channel: **MeshChain-Testnet-1** (private).

```bash
./scripts/meshtastic_hardware_test.sh
./scripts/meshtastic_hardware_test.sh --hardware /dev/ttyUSB0
```
