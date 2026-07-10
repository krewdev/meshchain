#!/usr/bin/env bash
# One-shot MeshChain public testnet host setup for Ubuntu/Debian VPS.
#
# Run as root on a fresh VPS:
#   curl -fsSL https://raw.githubusercontent.com/krewdev/meshchain/main/deploy/vps-setup.sh | sudo bash
#
# Or with options:
#   sudo MESHCHAIN_DOMAIN=faucet.example.com bash deploy/vps-setup.sh
#   sudo MESHCHAIN_REPO=https://github.com/krewdev/meshchain.git bash deploy/vps-setup.sh
#   sudo MESHCHAIN_SKIP_CADDY=1 bash deploy/vps-setup.sh
#
# What it does:
#   - creates user meshchain
#   - installs build deps + optional Caddy
#   - installs Rust for meshchain
#   - clones/updates repo to /opt/meshchain
#   - release-builds node + scanner + mesh CLI
#   - bootstraps testnet genesis (3 validators)
#   - installs systemd unit + env file
#   - opens firewall (ufw) for SSH + faucet + scanner
#   - starts the host
#
set -euo pipefail

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run as root: sudo bash deploy/vps-setup.sh"
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive

MESHCHAIN_HOME="${MESHCHAIN_HOME:-/opt/meshchain}"
MESHCHAIN_USER="${MESHCHAIN_USER:-meshchain}"
MESHCHAIN_REPO="${MESHCHAIN_REPO:-https://github.com/krewdev/meshchain.git}"
MESHCHAIN_BRANCH="${MESHCHAIN_BRANCH:-main}"
MESHCHAIN_DOMAIN="${MESHCHAIN_DOMAIN:-}"          # e.g. faucet.example.com
MESHCHAIN_SKIP_CADDY="${MESHCHAIN_SKIP_CADDY:-0}"
FAUCET_PORT="${FAUCET_PORT:-8787}"
SCANNER_PORT="${SCANNER_PORT:-8788}"
FAUCET_AMOUNT="${FAUCET_AMOUNT:-100000000}"
FAUCET_COOLDOWN="${FAUCET_COOLDOWN:-60}"
OPEN_VALIDATOR_PORTS="${OPEN_VALIDATOR_PORTS:-0}"  # set 1 to expose 9100-9102

echo "=============================================="
echo " MeshChain VPS setup"
echo "  home:    $MESHCHAIN_HOME"
echo "  user:    $MESHCHAIN_USER"
echo "  repo:    $MESHCHAIN_REPO ($MESHCHAIN_BRANCH)"
echo "  domain:  ${MESHCHAIN_DOMAIN:-"(none — use IP or tunnel)"}"
echo "=============================================="

# --- packages ---
if command -v apt-get >/dev/null; then
  apt-get update -qq
  apt-get install -y -qq \
    build-essential pkg-config libssl-dev \
    curl git ca-certificates \
    python3 python3-venv \
    ufw
  if [[ "$MESHCHAIN_SKIP_CADDY" != "1" && -n "$MESHCHAIN_DOMAIN" ]]; then
    if ! command -v caddy >/dev/null; then
      echo "Installing Caddy…"
      apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https
      curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
        | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg 2>/dev/null \
        || true
      # fallback: use official static binary if apt repo fails
      if ! command -v caddy >/dev/null; then
        curl -fsSL "https://caddyserver.com/api/download?os=linux&arch=amd64" -o /usr/local/bin/caddy \
          && chmod +x /usr/local/bin/caddy \
          || echo "warn: could not install Caddy automatically"
      fi
    fi
  fi
else
  echo "This script targets Debian/Ubuntu (apt). Install build-essential, git, curl, python3, ufw manually."
fi

# --- user ---
if ! id "$MESHCHAIN_USER" >/dev/null 2>&1; then
  adduser --disabled-password --gecos "MeshChain" "$MESHCHAIN_USER"
fi
mkdir -p "$MESHCHAIN_HOME"
chown -R "$MESHCHAIN_USER:$MESHCHAIN_USER" "$MESHCHAIN_HOME"

# --- rust (as meshchain user) ---
sudo -u "$MESHCHAIN_USER" bash -lc '
  set -e
  if [[ ! -f "$HOME/.cargo/env" ]]; then
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  fi
  source "$HOME/.cargo/env"
  rustup default stable
'

# --- clone / update ---
sudo -u "$MESHCHAIN_USER" bash -lc "
  set -e
  source \"\$HOME/.cargo/env\"
  if [[ -d \"$MESHCHAIN_HOME/.git\" ]]; then
    cd \"$MESHCHAIN_HOME\"
    git fetch origin
    git checkout \"$MESHCHAIN_BRANCH\"
    git pull --ff-only origin \"$MESHCHAIN_BRANCH\" || true
  else
    # empty dir may exist
    if [[ -z \"\$(ls -A \"$MESHCHAIN_HOME\" 2>/dev/null)\" ]]; then
      git clone --branch \"$MESHCHAIN_BRANCH\" \"$MESHCHAIN_REPO\" \"$MESHCHAIN_HOME\"
    else
      cd \"$MESHCHAIN_HOME\"
      if [[ ! -d .git ]]; then
        git init
        git remote add origin \"$MESHCHAIN_REPO\" || true
        git fetch origin
        git checkout -f \"$MESHCHAIN_BRANCH\" || git checkout -b \"$MESHCHAIN_BRANCH\" origin/\"$MESHCHAIN_BRANCH\"
      fi
    fi
  fi
  cd \"$MESHCHAIN_HOME\"
  echo \"Building release binaries (this can take several minutes)…\"
  cargo build -p meshchain-node -p meshchain-scanner -p mesh --release
  ./scripts/host_bootstrap.sh
"

# --- env file ---
mkdir -p /etc/meshchain
cat >/etc/meshchain/testnet.env <<EOF
# MeshChain testnet host environment
MESHCHAIN_HOME=$MESHCHAIN_HOME
MESHCHAIN_BIN=$MESHCHAIN_HOME/target/release/meshchain-node
MESHCHAIN_SCANNER=$MESHCHAIN_HOME/target/release/meshchain-scanner
MESHCHAIN_DATA=$MESHCHAIN_HOME/data/host/v0
FAUCET_PORT=$FAUCET_PORT
FAUCET_AMOUNT=$FAUCET_AMOUNT
FAUCET_COOLDOWN=$FAUCET_COOLDOWN
CORS_ORIGIN=*
MESHCHAIN_DOMAIN=$MESHCHAIN_DOMAIN
SCANNER_PORT=$SCANNER_PORT
EOF
chown root:root /etc/meshchain/testnet.env
chmod 644 /etc/meshchain/testnet.env

# wrap start script to load env
cat >/usr/local/bin/meshchain-testnet-start <<EOF
#!/usr/bin/env bash
set -euo pipefail
set -a
# shellcheck disable=SC1091
source /etc/meshchain/testnet.env
set +a
cd "$MESHCHAIN_HOME"
export MESHCHAIN_BIN="\${MESHCHAIN_BIN}"
export MESHCHAIN_SCANNER="\${MESHCHAIN_SCANNER}"
exec "$MESHCHAIN_HOME/scripts/start_testnet_host.sh"
EOF
chmod +x /usr/local/bin/meshchain-testnet-start

cat >/usr/local/bin/meshchain-testnet-stop <<EOF
#!/usr/bin/env bash
set -euo pipefail
cd "$MESHCHAIN_HOME"
exec "$MESHCHAIN_HOME/scripts/stop_testnet_host.sh"
EOF
chmod +x /usr/local/bin/meshchain-testnet-stop

# --- systemd ---
cat >/etc/systemd/system/meshchain-testnet.service <<EOF
[Unit]
Description=MeshChain public testnet host (3 validators + faucet + scanner)
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
User=$MESHCHAIN_USER
Group=$MESHCHAIN_USER
WorkingDirectory=$MESHCHAIN_HOME
EnvironmentFile=/etc/meshchain/testnet.env
ExecStart=/usr/local/bin/meshchain-testnet-start
ExecStop=/usr/local/bin/meshchain-testnet-stop
TimeoutStartSec=120

[Install]
WantedBy=multi-user.target
EOF

# --- firewall ---
if command -v ufw >/dev/null; then
  ufw allow OpenSSH || true
  ufw allow "${FAUCET_PORT}/tcp" || true
  ufw allow "${SCANNER_PORT}/tcp" || true
  if [[ "$OPEN_VALIDATOR_PORTS" == "1" ]]; then
    ufw allow 9100:9102/tcp || true
  fi
  # don't force enable if user hasn't; try enable noninteractive
  ufw --force enable || true
fi

# --- optional Caddy reverse proxy ---
if [[ -n "$MESHCHAIN_DOMAIN" && "$MESHCHAIN_SKIP_CADDY" != "1" && -x "$(command -v caddy || true)" ]]; then
  mkdir -p /etc/caddy
  cat >/etc/caddy/Caddyfile <<EOF
$MESHCHAIN_DOMAIN {
  handle /api/* {
    reverse_proxy 127.0.0.1:$SCANNER_PORT
  }
  handle {
    reverse_proxy 127.0.0.1:$FAUCET_PORT
  }
}
EOF
  # if only faucet on root is desired for simpler setup, prefer faucet-only:
  # re-write simpler: path-based is fine
  systemctl enable caddy 2>/dev/null || true
  systemctl restart caddy 2>/dev/null || caddy run --config /etc/caddy/Caddyfile &
  echo "Caddy configured for https://$MESHCHAIN_DOMAIN"
fi

# --- start ---
systemctl daemon-reload
systemctl enable meshchain-testnet
systemctl restart meshchain-testnet

sleep 3
echo
echo "=============================================="
echo " MeshChain testnet host installed"
echo "=============================================="
echo "  Service:  systemctl status meshchain-testnet"
echo "  Logs:     $MESHCHAIN_HOME/data/host/logs/"
echo "  Faucet:   http://127.0.0.1:$FAUCET_PORT/info"
echo "  Scanner:  http://127.0.0.1:$SCANNER_PORT/"
if [[ -n "$MESHCHAIN_DOMAIN" ]]; then
  echo "  Public:   https://$MESHCHAIN_DOMAIN  (if DNS + Caddy OK)"
  echo
  echo "Update Vercel faucet default + testnet/network.json with:"
  echo "  faucet_api: https://$MESHCHAIN_DOMAIN"
fi
echo
echo "Point DNS A record for your domain to this VPS IP, then:"
echo "  curl -s https://YOUR_DOMAIN/info"
echo
curl -s "http://127.0.0.1:$FAUCET_PORT/info" || echo "(faucet not ready yet — check logs)"
echo
curl -s "http://127.0.0.1:$SCANNER_PORT/api/v1/status" 2>/dev/null | head -c 200 || true
echo
echo "Done."
