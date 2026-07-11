#!/usr/bin/env bash
# Start Meshtastic ↔ TCP gossip relay (first-class air path).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

LISTEN="${MESH_RADIO_LISTEN:-127.0.0.1:9199}"
TCP0="${MESH_RADIO_TCP:-127.0.0.1:9100}"
TIP="${MESH_RADIO_TIP_SECS:-30}"
FLAGS=()

if [[ -n "${MESH_RADIO_PORT:-}" ]]; then
  FLAGS+=(--port "$MESH_RADIO_PORT" --channel-index "${MESH_RADIO_CHANNEL:-0}")
else
  FLAGS+=(--mock)
fi

# Extra TCP peers
EXTRA=()
for p in ${MESH_RADIO_EXTRA_TCP:-127.0.0.1:9101 127.0.0.1:9102}; do
  [[ -n "$p" ]] && EXTRA+=(--tcp "$p")
done

exec python3 "$ROOT/tools/mesh_radio_relay.py" \
  "${FLAGS[@]}" \
  --listen "$LISTEN" \
  --tcp "$TCP0" \
  "${EXTRA[@]}" \
  --tip-secs "$TIP"
