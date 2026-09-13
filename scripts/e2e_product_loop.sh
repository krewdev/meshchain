#!/usr/bin/env bash
# PRODUCT.md loop — lab path. Testnet only. Does not mint RELAY.
# Requires: pynacl, a mesh wallet JSON, optional radio relay on :9199,
#           and (for the Solana tail) the existing vault on devnet.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
WALLET=${WALLET:-data/keys/me.json}
DEST=${DEST:?set DEST to the 32-byte hex or base58 Solana destination}
SEQ=${DEPOSIT_SEQ:-0}
AMOUNT=${AMOUNT:-1.00}

echo "== 1. sign AirIou (type 15)"
python3 tools/mesh_iou.py "$AMOUNT" --dest "$DEST" --wallet "$WALLET" --deposit-seq "$SEQ" ${AIR:+--air}

echo "== 2. witness AirIouAck (type 16) — needs a vault attestor wallet"
if [[ -n "${ATTESTOR_WALLET:-}" ]]; then
  python3 tools/witness_iou.py --iou data/last_iou.json --index "${ATTESTOR_INDEX:-0}" --wallet "$ATTESTOR_WALLET" ${AIR:+--air}
else
  echo "skip witness (set ATTESTOR_WALLET to sign type 16)"
fi

echo "== 3. burn_txid"
python3 tools/iou_burn.py --iou data/last_iou.json

echo "== 4. Solana tail (devnet only)"
echo "  After mesh burn + 2 attestors:"
echo "    BURN_TXID=\$(python3 tools/iou_burn.py --iou data/last_iou.json)"
echo "    cd programs-mesh-bridge && npx ts-node scripts/e2e_cashout.ts"
echo "  Path A fee split (after relay.rs is upgraded on devnet):"
echo "    npx ts-node scripts/init_relay_devnet.ts"
echo "    BURN_TXID=... npx ts-node scripts/e2e_relay_path_a.ts   # if present"
echo
echo "claim_settle must fail EmissionsDark. tMESH has no cash value. RELAY is not live."
