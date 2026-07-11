#!/usr/bin/env bash
# Install Caddy + HTTPS for faucet/scanner via free sslip.io DNS (no custom domain needed).
# Run on the seed host as root, or via gcloud ssh.
#
# Usage on host:
#   sudo PUBLIC_IP=34.172.103.125 bash deploy/setup-https-sslip.sh
#
set -euo pipefail

PUBLIC_IP="${PUBLIC_IP:-$(curl -fsS -H 'Metadata-Flavor: Google' \
  http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip 2>/dev/null || true)}"
if [[ -z "${PUBLIC_IP}" ]]; then
  PUBLIC_IP="${1:-}"
fi
if [[ -z "${PUBLIC_IP}" ]]; then
  echo "Set PUBLIC_IP=x.x.x.x"
  exit 1
fi

# sslip.io: 34.172.103.125.sslip.io resolves to that IP
HOST_DNS="${HOST_DNS:-${PUBLIC_IP}.sslip.io}"
FAUCET_PORT="${FAUCET_PORT:-8787}"
SCANNER_PORT="${SCANNER_PORT:-8788}"

echo "Installing Caddy for https://${HOST_DNS} …"

if ! command -v caddy >/dev/null 2>&1; then
  apt-get update -qq
  apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl gnupg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg 2>/dev/null || true
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    | tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null 2>/dev/null || true
  apt-get update -qq || true
  apt-get install -y -qq caddy 2>/dev/null || {
    echo "apt caddy failed; installing static binary"
    curl -fsSL "https://caddyserver.com/api/download?os=linux&arch=amd64" -o /usr/local/bin/caddy
    chmod +x /usr/local/bin/caddy
  }
fi

mkdir -p /etc/caddy
cat >/etc/caddy/Caddyfile <<EOF
# MeshChain public seed — auto HTTPS via Let's Encrypt + sslip.io
${HOST_DNS} {
  encode gzip

  # Scanner API under /api/*
  handle /api/* {
    reverse_proxy 127.0.0.1:${SCANNER_PORT}
  }

  # Scanner UI
  handle /scanner* {
    reverse_proxy 127.0.0.1:${SCANNER_PORT}
  }
  handle / {
    reverse_proxy 127.0.0.1:${SCANNER_PORT}
  }

  # Note: faucet also needs a path — mount under /faucet/*
  handle_path /faucet/* {
    reverse_proxy 127.0.0.1:${FAUCET_PORT}
  }
}

# Dedicated faucet host (same IP)
faucet.${HOST_DNS} {
  encode gzip
  reverse_proxy 127.0.0.1:${FAUCET_PORT}
}
EOF

# Open HTTPS if ufw
if command -v ufw >/dev/null; then
  ufw allow 80/tcp || true
  ufw allow 443/tcp || true
fi

# systemd unit for static binary fallback
if ! systemctl list-unit-files | grep -q '^caddy.service'; then
  cat >/etc/systemd/system/caddy.service <<'UNIT'
[Unit]
Description=Caddy
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
ExecReload=/usr/local/bin/caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
Restart=on-failure
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
UNIT
  systemctl daemon-reload
fi

# Prefer distro caddy path if present
if [[ -x /usr/bin/caddy ]]; then
  sed -i 's|/usr/local/bin/caddy|/usr/bin/caddy|g' /etc/systemd/system/caddy.service 2>/dev/null || true
fi

systemctl enable caddy 2>/dev/null || true
systemctl restart caddy || /usr/local/bin/caddy run --config /etc/caddy/Caddyfile --adapter caddyfile &

sleep 2
echo
echo "=============================================="
echo " HTTPS endpoints (may take 30–60s for certs)"
echo "  Scanner:  https://${HOST_DNS}/"
echo "  API:      https://${HOST_DNS}/api/v1/status"
echo "  Faucet:   https://faucet.${HOST_DNS}/info"
echo "  (plain HTTP still on :8787 / :8788)"
echo "=============================================="
curl -sk "https://${HOST_DNS}/api/v1/status" | head -c 200 || echo "(cert pending — retry shortly)"
echo
