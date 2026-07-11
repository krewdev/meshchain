#!/usr/bin/env bash
# Create a Google Compute Engine VM running MeshChain testnet validators.
#
# Prerequisites:
#   gcloud auth login
#   gcloud config set project YOUR_PROJECT
#   (billing enabled; Compute Engine API will be enabled by this script)
#
# Usage:
#   ./deploy/gce-create.sh
#   PROJECT=my-proj ZONE=us-central1-a MACHINE=e2-small ./deploy/gce-create.sh
#   OPEN_VALIDATOR_PORTS=1 MESHCHAIN_DOMAIN=faucet.example.com ./deploy/gce-create.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
ZONE="${ZONE:-us-central1-a}"
NAME="${NAME:-meshchain-testnet}"
MACHINE="${MACHINE:-e2-small}"          # 2GB RAM recommended for rustc build
DISK_GB="${DISK_GB:-40}"
IMAGE_FAMILY="${IMAGE_FAMILY:-ubuntu-2204-lts}"
IMAGE_PROJECT="${IMAGE_PROJECT:-ubuntu-os-cloud}"
MESHCHAIN_REPO="${MESHCHAIN_REPO:-https://github.com/krewdev/meshchain.git}"
MESHCHAIN_BRANCH="${MESHCHAIN_BRANCH:-main}"
MESHCHAIN_DOMAIN="${MESHCHAIN_DOMAIN:-}"
OPEN_VALIDATOR_PORTS="${OPEN_VALIDATOR_PORTS:-0}"
TAGS="${TAGS:-meshchain-testnet}"

if [[ -z "${PROJECT:-}" || "$PROJECT" == "(unset)" ]]; then
  echo "Set a GCP project: gcloud config set project YOUR_PROJECT"
  exit 1
fi

echo "=============================================="
echo " MeshChain GCE create"
echo "  project:  $PROJECT"
echo "  zone:     $ZONE"
echo "  name:     $NAME"
echo "  machine:  $MACHINE"
echo "  disk:     ${DISK_GB}GB"
echo "  domain:   ${MESHCHAIN_DOMAIN:-"(none)"}"
echo "=============================================="
echo
echo "This will:"
echo "  1) Enable compute.googleapis.com (if needed)"
echo "  2) Create firewall rules for SSH + faucet:8787 + scanner:8788"
echo "  3) Create VM and run deploy/vps-setup.sh (Rust release build ~10–20 min)"
echo
echo "Approx cost: e2-small ~\$10–15/mo (varies by region). Delete when done:"
echo "  gcloud compute instances delete $NAME --zone=$ZONE --project=$PROJECT"
echo
if [[ "${ASSUME_YES:-0}" != "1" ]]; then
  read -r -p "Continue? [y/N] " ans
  case "$ans" in
    y|Y|yes|YES) ;;
    *) echo "Aborted."; exit 1 ;;
  esac
fi

echo "Enabling Compute Engine API…"
gcloud services enable compute.googleapis.com --project="$PROJECT"

# Firewall
create_fw() {
  local rule="$1"
  shift
  if gcloud compute firewall-rules describe "$rule" --project="$PROJECT" >/dev/null 2>&1; then
    echo "firewall $rule exists"
  else
    gcloud compute firewall-rules create "$rule" --project="$PROJECT" "$@"
  fi
}

create_fw meshchain-allow-ssh \
  --allow=tcp:22 \
  --target-tags="$TAGS" \
  --description="MeshChain SSH" \
  --direction=INGRESS \
  --priority=1000

create_fw meshchain-allow-faucet \
  --allow=tcp:8787 \
  --target-tags="$TAGS" \
  --description="MeshChain faucet" \
  --direction=INGRESS \
  --priority=1000

create_fw meshchain-allow-scanner \
  --allow=tcp:8788 \
  --target-tags="$TAGS" \
  --description="MeshChain scanner" \
  --direction=INGRESS \
  --priority=1000

if [[ "$OPEN_VALIDATOR_PORTS" == "1" ]]; then
  create_fw meshchain-allow-validators \
    --allow=tcp:9100-9102 \
    --target-tags="$TAGS" \
    --description="MeshChain validator gossip (optional)" \
    --direction=INGRESS \
    --priority=1000
fi

STARTUP="$(mktemp)"
cat >"$STARTUP" <<EOF
#!/bin/bash
set -euo pipefail
exec > /var/log/meshchain-startup.log 2>&1
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq curl git ca-certificates

# Wait for network
sleep 5

export MESHCHAIN_REPO="$MESHCHAIN_REPO"
export MESHCHAIN_BRANCH="$MESHCHAIN_BRANCH"
export MESHCHAIN_DOMAIN="$MESHCHAIN_DOMAIN"
export MESHCHAIN_SKIP_CADDY="${MESHCHAIN_DOMAIN:+0}"
export MESHCHAIN_SKIP_CADDY="\${MESHCHAIN_SKIP_CADDY:-1}"
export OPEN_VALIDATOR_PORTS="$OPEN_VALIDATOR_PORTS"

# Prefer cloning then local vps-setup (works even if raw github URL lags)
if [[ ! -d /opt/meshchain/.git ]]; then
  git clone --branch "\$MESHCHAIN_BRANCH" "\$MESHCHAIN_REPO" /opt/meshchain
fi
cd /opt/meshchain
git fetch origin || true
git checkout "\$MESHCHAIN_BRANCH" || true
git pull --ff-only origin "\$MESHCHAIN_BRANCH" || true
bash deploy/vps-setup.sh
echo DONE \$(date -u) >> /var/log/meshchain-startup.log
EOF

# SSH key
SSH_FLAG=()
if [[ -f "$HOME/.ssh/id_ed25519.pub" ]]; then
  SSH_FLAG=(--metadata-from-file=ssh-keys=<(
    echo "meshchain:$(cat "$HOME/.ssh/id_ed25519.pub")"
    echo "$USER:$(cat "$HOME/.ssh/id_ed25519.pub")"
  ))
fi

if gcloud compute instances describe "$NAME" --zone="$ZONE" --project="$PROJECT" >/dev/null 2>&1; then
  echo "Instance $NAME already exists in $ZONE"
else
  echo "Creating instance…"
  gcloud compute instances create "$NAME" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --machine-type="$MACHINE" \
    --boot-disk-size="${DISK_GB}GB" \
    --boot-disk-type=pd-balanced \
    --image-family="$IMAGE_FAMILY" \
    --image-project="$IMAGE_PROJECT" \
    --tags="$TAGS" \
    --metadata=enable-oslogin=TRUE \
    --metadata-from-file=startup-script="$STARTUP" \
    --scopes=https://www.googleapis.com/auth/cloud-platform
fi

rm -f "$STARTUP"

IP="$(gcloud compute instances describe "$NAME" --zone="$ZONE" --project="$PROJECT" \
  --format='get(networkInterfaces[0].accessConfigs[0].natIP)')"

echo
echo "=============================================="
echo " VM created: $NAME"
echo " Public IP:  $IP"
echo "=============================================="
echo
echo "First boot builds Rust release binaries (10–25 minutes)."
echo "Watch setup:"
echo "  gcloud compute ssh $NAME --zone=$ZONE --project=$PROJECT --command='sudo tail -f /var/log/meshchain-startup.log'"
echo
echo "When ready:"
echo "  curl -s http://$IP:8787/info"
echo "  curl -s http://$IP:8788/api/v1/status"
echo "  open http://$IP:8788/   # scanner"
echo
echo "SSH:"
echo "  gcloud compute ssh $NAME --zone=$ZONE --project=$PROJECT"
echo
echo "Point local mesh CLI at cloud (after register on that chain):"
echo "  # validators gossip is private by default; use faucet/scanner HTTP"
echo
echo "Delete when finished:"
echo "  gcloud compute instances delete $NAME --zone=$ZONE --project=$PROJECT --quiet"
echo
