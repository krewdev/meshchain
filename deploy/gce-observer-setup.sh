#!/usr/bin/env bash
# Create or upgrade a GCE remote observer VM for meshchain-testnet-1.
#
# Usage:
#   ASSUME_YES=1 ./deploy/gce-observer-setup.sh
#   NAME=meshchain-observer ZONE=us-central1-a ./deploy/gce-observer-setup.sh
#
# Env:
#   PROJECT ZONE NAME MACHINE DISK_GB
#   SEED_PEERS   default "34.172.103.125:9100 34.172.103.125:9101 34.172.103.125:9102"
#   MESHCHAIN_REPO BRANCH
set -euo pipefail

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
ZONE="${ZONE:-us-central1-a}"
NAME="${NAME:-meshchain-observer}"
MACHINE="${MACHINE:-e2-small}"
DISK_GB="${DISK_GB:-20}"
TAGS="${TAGS:-meshchain-testnet}"
SEED_PEERS="${SEED_PEERS:-34.172.103.125:9100 34.172.103.125:9101 34.172.103.125:9102}"
MESHCHAIN_REPO="${MESHCHAIN_REPO:-https://github.com/krewdev/meshchain.git}"
MESHCHAIN_BRANCH="${MESHCHAIN_BRANCH:-main}"
SEED_NAME="${SEED_NAME:-meshchain-testnet}"

if [[ -z "${PROJECT:-}" || "$PROJECT" == "(unset)" ]]; then
  echo "Set project: gcloud config set project YOUR_PROJECT"
  exit 1
fi

echo "=============================================="
echo " MeshChain GCE observer"
echo "  project: $PROJECT"
echo "  zone:    $ZONE"
echo "  name:    $NAME"
echo "  machine: $MACHINE"
echo "  seeds:   $SEED_PEERS"
echo "=============================================="

if [[ "${ASSUME_YES:-0}" != "1" ]]; then
  read -r -p "Continue? [y/N] " a
  case "$a" in y|Y|yes|YES) ;; *) echo aborted; exit 1 ;; esac
fi

# Firewall (reuse seed tags)
if ! gcloud compute firewall-rules describe meshchain-allow-validators --project="$PROJECT" >/dev/null 2>&1; then
  gcloud compute firewall-rules create meshchain-allow-validators \
    --project="$PROJECT" \
    --allow=tcp:9100-9110 \
    --target-tags="$TAGS" \
    --description="MeshChain gossip" \
    --direction=INGRESS --priority=1000
fi

EXISTS=0
if gcloud compute instances describe "$NAME" --zone="$ZONE" --project="$PROJECT" >/dev/null 2>&1; then
  EXISTS=1
  echo "instance $NAME already exists — will upgrade in place"
else
  echo "creating $NAME…"
  gcloud compute instances create "$NAME" \
    --project="$PROJECT" \
    --zone="$ZONE" \
    --machine-type="$MACHINE" \
    --boot-disk-size="${DISK_GB}GB" \
    --image-family=ubuntu-2204-lts \
    --image-project=ubuntu-os-cloud \
    --tags="$TAGS" \
    --metadata=enable-oslogin=TRUE
fi

echo "waiting for SSH…"
for i in $(seq 1 30); do
  if gcloud compute ssh "$NAME" --zone="$ZONE" --project="$PROJECT" --quiet --command="true" 2>/dev/null; then
    break
  fi
  sleep 5
done

# Prefer copy release binary from seed if present
echo "syncing release binary from seed (if available)…"
gcloud compute scp "${SEED_NAME}:/opt/meshchain/target/release/meshchain-node" \
  /tmp/meshchain-node-obs --zone="$ZONE" --project="$PROJECT" --quiet 2>/dev/null \
  || true
if [[ -f /tmp/meshchain-node-obs ]]; then
  gcloud compute scp /tmp/meshchain-node-obs "${NAME}:/tmp/meshchain-node" \
    --zone="$ZONE" --project="$PROJECT" --quiet
fi

# Tip state bootstrap from seed
gcloud compute scp "${SEED_NAME}:/opt/meshchain/data/host/v0/chain_state.json" \
  /tmp/obs-chain_state.json --zone="$ZONE" --project="$PROJECT" --quiet 2>/dev/null || true
gcloud compute scp "${SEED_NAME}:/opt/meshchain/data/host/v0/genesis.json" \
  /tmp/obs-genesis.json --zone="$ZONE" --project="$PROJECT" --quiet 2>/dev/null || true
[[ -f /tmp/obs-chain_state.json ]] && gcloud compute scp /tmp/obs-chain_state.json "${NAME}:/tmp/chain_state.json" \
  --zone="$ZONE" --project="$PROJECT" --quiet || true
[[ -f /tmp/obs-genesis.json ]] && gcloud compute scp /tmp/obs-genesis.json "${NAME}:/tmp/genesis.json" \
  --zone="$ZONE" --project="$PROJECT" --quiet || true

PEER_FLAGS=""
for p in $SEED_PEERS; do
  PEER_FLAGS+=" --peer $p"
done

gcloud compute ssh "$NAME" --zone="$ZONE" --project="$PROJECT" --quiet --command="
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
sudo apt-get update -qq
sudo apt-get install -y -qq curl ca-certificates
sudo useradd -r -m -d /opt/mesh-observer -s /usr/sbin/nologin meshchain 2>/dev/null || true
sudo mkdir -p /opt/mesh-observer/bin /opt/mesh-observer/data /var/log/meshchain
if [[ -x /tmp/meshchain-node ]]; then
  sudo install -o meshchain -g meshchain -m 755 /tmp/meshchain-node /opt/mesh-observer/bin/meshchain-node
else
  echo 'No prebuilt binary — cloning and building (slow)…'
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev git
  if [[ ! -d /opt/meshchain ]]; then
    sudo git clone --branch $MESHCHAIN_BRANCH $MESHCHAIN_REPO /opt/meshchain
    sudo chown -R meshchain:meshchain /opt/meshchain
  fi
  sudo -u meshchain bash -lc 'curl --proto \"=https\" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
  sudo -u meshchain bash -lc 'source \$HOME/.cargo/env && cd /opt/meshchain && git fetch && git checkout $MESHCHAIN_BRANCH && git pull && cargo build --release -p meshchain-node'
  sudo install -o meshchain -g meshchain -m 755 /opt/meshchain/target/release/meshchain-node /opt/mesh-observer/bin/meshchain-node
fi
if [[ -f /tmp/genesis.json ]]; then
  sudo -u meshchain cp -f /tmp/genesis.json /opt/mesh-observer/data/genesis.json
fi
if [[ -f /tmp/chain_state.json ]]; then
  sudo -u meshchain cp -f /tmp/chain_state.json /opt/mesh-observer/data/chain_state.json
fi
# require genesis
if [[ ! -f /opt/mesh-observer/data/genesis.json ]]; then
  echo 'missing genesis.json — place testnet/published/genesis.json'
  exit 1
fi
sudo tee /etc/systemd/system/meshchain-observer.service >/dev/null <<UNIT
[Unit]
Description=MeshChain public testnet observer
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=meshchain
WorkingDirectory=/opt/mesh-observer
ExecStart=/opt/mesh-observer/bin/meshchain-node run --data-dir /opt/mesh-observer/data --observer --listen 0.0.0.0:9100${PEER_FLAGS} --slot-ms 100
Restart=always
RestartSec=5
StandardOutput=append:/var/log/meshchain/observer.log
StandardError=append:/var/log/meshchain/observer.log

[Install]
WantedBy=multi-user.target
UNIT
sudo systemctl daemon-reload
sudo systemctl enable meshchain-observer
sudo systemctl restart meshchain-observer
sleep 2
systemctl is-active meshchain-observer
python3 -c \"import json;print('height',json.load(open('/opt/mesh-observer/data/chain_state.json')).get('height','?'))\" 2>/dev/null || true
tail -5 /var/log/meshchain/observer.log || true
"

IP=$(gcloud compute instances describe "$NAME" --zone="$ZONE" --project="$PROJECT" \
  --format='get(networkInterfaces[0].accessConfigs[0].natIP)')
echo
echo "OK observer $NAME → $IP:9100"
echo "  Add to testnet/seeds.json if new."
echo "  Seed EXTRA_PEERS should include $IP:9100"
echo "  mesh observer --peer 34.172.103.125:9100 --peer $IP:9100"
