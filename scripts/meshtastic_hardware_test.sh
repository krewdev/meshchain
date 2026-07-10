#!/usr/bin/env bash
# Hardware / mock Meshtastic bridge test for MeshChain frames.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== MeshChain Meshtastic bridge test =="

if [[ "${1:-}" == "--hardware" ]]; then
  PORT="${2:-}"
  if [[ -z "$PORT" ]]; then
    echo "Usage: $0 --hardware /dev/cu.usbserial-*   (or COM3 / tcp:host:4403)"
    exit 1
  fi
  MODE=(--port "$PORT")
  echo "Hardware mode: $PORT"
  echo "Create private channel MeshChain-Testnet-1 on both nodes first."
else
  MODE=(--mock)
  echo "Mock mode (no radio) — loops TXHEX → RXHEX"
fi

# Send a tiny MC frame (magic + version + type + len + payload)
# MC | ver=1 | type=10 (control) | len=4 | "ping"
python3 - <<'PY' | python3 tools/meshtastic_bridge.py "${MODE[@]}"
import sys, time, binascii
frame = bytes([0x4d, 0x43, 0x01, 0x0a, 0x04, 0x00]) + b"ping"  # little-endian len=4
hexf = binascii.hexlify(frame).decode()
# wait for OK
# bridge reads stdin; send after short delay via this pipeline is tricky —
print("TXHEX " + hexf, flush=True)
time.sleep(0.5)
print("QUIT", flush=True)
PY

echo
echo "Manual hardware checklist:"
echo "  1. Two Meshtastic nodes, same region, private channel MeshChain-Testnet-1"
echo "  2. Node A USB to host: python3 tools/meshtastic_bridge.py --port <device>"
echo "  3. Validators: ./scripts/start_testnet_host.sh"
echo "  4. Send MC frames / run mesh wallets; TCP still used for finality in lab"
echo "  5. pip install meshtastic  (for --hardware)"
