#!/usr/bin/env bash
# Coordinator: push a new genesis to the GCE seed and restart validators (TESTNET RESET).
#
# Usage:
#   ./scripts/restack_public_seed.sh testnet/published/genesis.json
#
# Env:
#   PROJECT ZONE NAME  (defaults match lab seed)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GENESIS="${1:-}"
if [[ -z "$GENESIS" || ! -f "$GENESIS" ]]; then
  echo "Usage: $0 path/to/genesis.json"
  exit 1
fi

PROJECT="${PROJECT:-xai-ipc-sim-2026}"
ZONE="${ZONE:-us-central1-a}"
NAME="${NAME:-meshchain-testnet}"

echo "WARNING: This resets chain_state on the seed host (testnet wipe)."
echo "  project=$PROJECT zone=$ZONE instance=$NAME"
echo "  genesis=$GENESIS"
if [[ "${ASSUME_YES:-0}" != "1" ]]; then
  read -r -p "Continue? [y/N] " a
  case "$a" in y|Y|yes|YES) ;; *) echo aborted; exit 1 ;; esac
fi

gcloud compute scp "$GENESIS" "$NAME:/tmp/genesis.next.json" \
  --zone="$ZONE" --project="$PROJECT" --quiet

gcloud compute ssh "$NAME" --zone="$ZONE" --project="$PROJECT" --quiet --command='
set -euo pipefail
HOST=/opt/meshchain/data/host
sudo -u meshchain bash -lc "
  set -e
  # stop
  if [[ -f $HOST/host.pids ]]; then
    while read -r p; do kill \$p 2>/dev/null || true; done < $HOST/host.pids || true
  fi
  sleep 1
  for port in 9100 9101 9102 8787 8788; do fuser -k \${port}/tcp 2>/dev/null || true; done
  sleep 1
  cp /tmp/genesis.next.json $HOST/genesis.json
  for i in 0 1 2; do
    mkdir -p $HOST/v\$i/keys
    cp $HOST/genesis.json $HOST/v\$i/
    # drop old tip — testnet restack
    rm -f $HOST/v\$i/chain_state.json
  done
  rm -f $HOST/v0/chain_state.json
  echo genesis installed
  python3 - <<PY
import json
g=json.load(open(\"$HOST/genesis.json\"))
print(\"validators\", len(g[\"validators\"]))
for i,v in enumerate(g[\"validators\"]):
    print(i, v[:20]+\"...\")
PY
"
# restart public bind (3 local producers — extra genesis seats need extra hosts)
sudo -u meshchain /opt/meshchain/deploy/remote-bind-public.sh
'

# Publish locally
cp "$GENESIS" "$ROOT/testnet/published/genesis.json"
cp "$GENESIS" "$ROOT/testnet/genesis.public.json"
mkdir -p "$ROOT/web/testnet/published"
cp "$GENESIS" "$ROOT/web/testnet/published/genesis.json"
echo "Published genesis to testnet/published/ and web/"
echo "Commit + push when ready so join-public users get the new set."
