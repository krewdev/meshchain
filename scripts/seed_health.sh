#!/usr/bin/env bash
# Extended public-seed health: HTTP/HTTPS, peers, optional relayer unit (on host).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IP="${SEED_IP:-34.172.103.125}"
HTTPS_SCANNER="${HTTPS_SCANNER:-https://${IP}.sslip.io}"
HTTPS_FAUCET="${HTTPS_FAUCET:-https://faucet.${IP}.sslip.io}"

ok=0
fail() { echo "FAIL $*"; ok=1; }
pass() { echo "OK   $*"; }

echo "MeshChain seed health  $(date -u +%Y-%m-%dT%H:%MZ)"
echo "========================================"

# Prefer existing status script for core checks
if [[ -x "$ROOT/scripts/status_public_seed.sh" ]]; then
  if SEED_IP="$IP" "$ROOT/scripts/status_public_seed.sh"; then
    pass "status_public_seed.sh"
  else
    fail "status_public_seed.sh"
  fi
fi

# Tip freshness (height must parse)
H=$(curl -sk --max-time 8 "$HTTPS_SCANNER/api/v1/status" 2>/dev/null \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('height',''))" 2>/dev/null || true)
if [[ -n "$H" && "$H" != "" ]]; then
  pass "tip height=$H"
else
  fail "tip height unreadable"
fi

# Faucet mint path
if curl -sk --max-time 8 "$HTTPS_FAUCET/info" 2>/dev/null | grep -q mint_via; then
  pass "faucet mint_via present"
else
  fail "faucet info incomplete"
fi

# On-host extras (when run on seed)
if [[ -f /opt/meshchain/data/host/v0/chain_state.json ]]; then
  LH=$(python3 -c "import json;print(json.load(open('/opt/meshchain/data/host/v0/chain_state.json')).get('height'))" 2>/dev/null || echo "?")
  pass "local chain_state height=$LH"
  if systemctl is-active --quiet meshchain-relayer 2>/dev/null; then
    pass "systemd meshchain-relayer active"
  else
    fail "systemd meshchain-relayer inactive (or missing)"
  fi
  if systemctl is-active --quiet meshchain-radio-relay 2>/dev/null; then
    pass "systemd meshchain-radio-relay active"
  else
    echo "WARN meshchain-radio-relay inactive (optional)"
  fi
  if [[ -f /opt/meshchain/data/host/relayer_state.json ]]; then
    python3 - <<'PY'
import json
st=json.load(open("/opt/meshchain/data/host/relayer_state.json"))
print("OK   relayer processed=%d deferred=%d" % (
  len(st.get("processedSeqs") or []),
  len(st.get("deferredSeqs") or st.get("deferred") or []),
))
PY
  fi
fi

echo "========================================"
if [[ "$ok" -eq 0 ]]; then
  echo "HEALTH_OK"
else
  echo "HEALTH_FAIL"
fi
exit $ok
