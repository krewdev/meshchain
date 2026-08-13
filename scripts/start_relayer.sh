#!/usr/bin/env bash
# Start Solana → MeshChain deposit relayer (ts-node).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/programs-mesh-bridge"

export MESHCHAIN_ROOT="${MESHCHAIN_ROOT:-$ROOT}"
export MESHCHAIN_DATA="${MESHCHAIN_DATA:-$ROOT/data/host}"
export MESH_MINT_PEER="${MESH_MINT_PEER:-127.0.0.1:9100}"
export ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-https://api.devnet.solana.com}"

if [[ -z "${ANCHOR_WALLET:-}" ]]; then
  if [[ -f "$HOME/.config/solana/id.json" ]]; then
    export ANCHOR_WALLET="$HOME/.config/solana/id.json"
  else
    echo "ANCHOR_WALLET not set and $HOME/.config/solana/id.json missing" >&2
    exit 1
  fi
fi

if [[ ! -f node_modules/@coral-xyz/anchor/package.json ]]; then
  echo "Installing programs-mesh-bridge deps…"
  if command -v yarn >/dev/null 2>&1; then
    yarn install --frozen-lockfile || yarn install
  else
    npm install
  fi
fi

# Ensure ts-node available
if [[ ! -f node_modules/ts-node/package.json ]]; then
  npm install --no-save ts-node typescript @types/node 2>/dev/null \
    || npm install ts-node typescript @types/node
fi

# transpileOnly avoids Anchor Program ctor type friction on mixed versions
export TS_NODE_TRANSPILE_ONLY=1
export TS_NODE_COMPILER_OPTIONS='{"module":"commonjs","esModuleInterop":true,"resolveJsonModule":true}'
exec npx ts-node scripts/relayer_daemon.ts
