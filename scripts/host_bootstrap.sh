#!/usr/bin/env bash
# Prepare always-on testnet host data (shared genesis + 3 validator trees).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== MeshChain testnet host bootstrap =="
cargo build -p meshchain-node -p mesh -p meshchain-scanner --release

BIN="$ROOT/target/release/mesh"
NODE="$ROOT/target/release/meshchain-node"
SCANNER="$ROOT/target/release/meshchain-scanner"
HOST_DATA="$ROOT/data/host"
mkdir -p "$HOST_DATA"

if [[ ! -f "$HOST_DATA/genesis.json" ]]; then
  echo "Creating testnet genesis…"
  "$NODE" init --data-dir "$HOST_DATA" --validators 3 --testnet
else
  echo "Using existing $HOST_DATA/genesis.json"
fi

for i in 0 1 2; do
  d="$HOST_DATA/v$i"
  mkdir -p "$d/keys"
  cp -f "$HOST_DATA/genesis.json" "$d/"
  cp -f "$HOST_DATA"/keys/validator-*.json "$d/keys/" 2>/dev/null || true
  cp -f "$HOST_DATA/testnet_profile.json" "$d/" 2>/dev/null || true
  # shared chain state for faucet mint simplicity: use v0 as ledger authority
done

# Faucet mints against v0 ledger — copy faucet key if present
cp -f "$HOST_DATA"/keys/faucet.json "$HOST_DATA/v0/keys/" 2>/dev/null || true

echo "Bootstrap OK."
echo "  Start host:  ./scripts/start_testnet_host.sh"
echo "  Live API:    ./scripts/start_scanner_live.sh   # Cloudflare tunnel + Vercel live_api"
echo "  Or Docker:   docker compose up -d   (after bootstrap)"
echo "  Faucet:      http://127.0.0.1:8787/info"
echo "  Scanner:     http://127.0.0.1:8788/"
