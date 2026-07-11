#!/usr/bin/env bash
# Convenience wrapper: install public genesis/seeds and optionally sync + print next steps.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
MESH="${MESH:-$ROOT/target/debug/mesh}"
if [[ ! -x "$MESH" ]]; then
  echo "Building mesh…"
  cargo build -p mesh -q
  MESH="$ROOT/target/debug/mesh"
fi
exec "$MESH" join-public "$@"
