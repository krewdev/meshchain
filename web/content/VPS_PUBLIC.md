# Public VPS + tunnel + radio multi-hop

## A. Fixed-IP VPS (recommended for stable testnet)

### 1. Provision

Any small VPS (DigitalOcean, Hetzner, Linode, AWS Lightsail):

- Ubuntu 22.04+  
- 1 vCPU / 1–2 GB RAM · 20–40 GB disk  
- Open ports: `8787` (faucet), `8788` (scanner), optionally `9100-9102` (validators)

**Pick:** Hetzner CX22 or DigitalOcean $6 droplet.

### 2. One-shot install (recommended)

On a **fresh Ubuntu VPS as root**:

```bash
# optional domain for HTTPS via Caddy
export MESHCHAIN_DOMAIN=faucet.yourdomain.com

curl -fsSL https://raw.githubusercontent.com/krewdev/meshchain/main/deploy/vps-setup.sh | sudo bash
```

With a local checkout:

```bash
sudo MESHCHAIN_DOMAIN=faucet.yourdomain.com bash deploy/vps-setup.sh
```

What it installs:

- user `meshchain`  
- Rust + release build (`meshchain-node`, scanner, CLI)  
- testnet genesis (3 validators)  
- systemd `meshchain-testnet`  
- faucet `:8787` + scanner `:8788`  
- ufw rules  
- optional Caddy if `MESHCHAIN_DOMAIN` is set  

### 3. Useful commands after install

```bash
systemctl status meshchain-testnet
journalctl -u meshchain-testnet -n 50
tail -f /opt/meshchain/data/host/logs/faucet.log
curl -s http://127.0.0.1:8787/info
curl -s http://127.0.0.1:8788/api/v1/status
```

Env file: `/etc/meshchain/testnet.env` (see `deploy/testnet.env.example`).

### 4. Publish endpoints

1. DNS **A record**: `faucet.yourdomain.com` → VPS public IP  
2. Wait for TLS (Caddy) if domain set  
3. Update site faucet default + `testnet/network.json`:

```json
"endpoints": {
  "faucet_api": "https://faucet.yourdomain.com",
  "faucet_ui": "https://meshchain-sigma.vercel.app/faucet/",
  "scanner_api": "https://faucet.yourdomain.com/api/"
}
```

4. On the Vercel faucet page, set **Faucet API URL** to your domain.

### 5. Firewall (if not using the script)

```bash
sudo ufw allow OpenSSH
sudo ufw allow 8787/tcp
sudo ufw allow 8788/tcp
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
