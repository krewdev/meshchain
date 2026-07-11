#!/usr/bin/env bash
# Wrapper around mesh genesis-extend for coordinators.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MESH="${MESH:-$ROOT/target/debug/mesh}"
GENESIS="${1:-$ROOT/testnet/published/genesis.json}"
shift || true
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 [genesis.json] --add HEX [--add HEX2 ...]"
  echo "  or:  $0 genesis.json HEX1 HEX2"
  exit 1
fi
ARGS=(genesis-extend --genesis "$GENESIS" --out "$GENESIS")
if [[ "${1:-}" == --add ]]; then
  ARGS+=("$@")
else
  for pk in "$@"; do
    ARGS+=(--add "$pk")
  done
fi
if [[ ! -x "$MESH" ]]; then
  cargo build -p mesh -q
fi
exec "$MESH" "${ARGS[@]}"
