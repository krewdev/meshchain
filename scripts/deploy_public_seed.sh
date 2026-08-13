#!/usr/bin/env bash
# One-command public seed upgrade (NO chain wipe).
#
# From laptop (gcloud configured):
#   ./scripts/deploy_public_seed.sh
#
# On the seed host itself:
#   ON_HOST=1 ./scripts/deploy_public_seed.sh
#
# Env:
#   PROJECT ZONE NAME   GCE defaults
#   SKIP_BUILD=1        only restart services
#   SKIP_RELAYER=1      leave relayer alone
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="${PROJECT:-xai-ipc-sim-2026}"
ZONE="${ZONE:-us-central1-a}"
NAME="${NAME:-meshchain-testnet}"

remote() {
  if [[ "${ON_HOST:-0}" == "1" ]]; then
    bash -lc "$*"
  else
    gcloud compute ssh "$NAME" --zone="$ZONE" --project="$PROJECT" --quiet --command="$*"
  fi
}

echo "MeshChain public seed deploy (no wipe)"
echo "  project=$PROJECT zone=$ZONE instance=$NAME"
echo "  ON_HOST=${ON_HOST:-0} SKIP_BUILD=${SKIP_BUILD:-0}"

SKIP_BUILD="${SKIP_BUILD:-0}"
SKIP_RELAYER="${SKIP_RELAYER:-0}"
remote "
set -euo pipefail
export SKIP_BUILD=$SKIP_BUILD SKIP_RELAYER=$SKIP_RELAYER
cd /opt/meshchain
echo '== git =='
sudo -u meshchain git -C /opt/meshchain fetch origin main
sudo -u meshchain git -C /opt/meshchain checkout main
sudo -u meshchain git -C /opt/meshchain pull --ff-only origin main
echo git=\$(sudo -u meshchain git -C /opt/meshchain rev-parse --short HEAD)
sudo chmod +x /opt/meshchain/deploy/remote-bind-public.sh /opt/meshchain/scripts/*.sh 2>/dev/null || true

if [[ \"\$SKIP_BUILD\" != \"1\" ]]; then
  echo '== release build =='
  sudo -u meshchain bash -lc '
    export PATH=\"\$HOME/.cargo/bin:/usr/local/cargo/bin:\$PATH\"
    cd /opt/meshchain
    cargo build --release -p mesh -p meshchain-node -p meshchain-scanner
  '
fi

echo '== restart validators (preserve chain_state) =='
sudo -u meshchain bash -lc '
  export EXTRA_PEERS=35.192.20.103:9100
  export MESH_MINT_PEER=127.0.0.1:9100
  unset MESH_ALLOW_OFFLINE_MINT || true
  unset MESH_RELAYER || true
  bash /opt/meshchain/deploy/remote-bind-public.sh
'

if [[ \"\$SKIP_RELAYER\" != \"1\" ]]; then
  echo '== relayer + radio =='
  if [[ -f /etc/systemd/system/meshchain-relayer.service ]]; then
    sudo systemctl restart meshchain-relayer || true
  fi
  if [[ -f /etc/systemd/system/meshchain-radio-relay.service ]]; then
    sudo systemctl restart meshchain-radio-relay || true
  fi
fi

sleep 3
echo '== health =='
curl -sS -m 5 http://127.0.0.1:8788/api/v1/status | head -c 200; echo
curl -sS -m 5 http://127.0.0.1:8787/info | head -c 160; echo
systemctl is-active meshchain-relayer 2>/dev/null || echo 'relayer: n/a'
systemctl is-active meshchain-radio-relay 2>/dev/null || echo 'radio: n/a'
echo DEPLOY_OK
"

echo "Remote deploy finished. Run: ./scripts/status_public_seed.sh"
