#!/usr/bin/env bash
# Health check for the public MeshChain seed.
set -euo pipefail
IP="${SEED_IP:-34.172.103.125}"
HTTPS_SCANNER="${HTTPS_SCANNER:-https://${IP}.sslip.io}"
HTTPS_FAUCET="${HTTPS_FAUCET:-https://faucet.${IP}.sslip.io}"

ok=0
check() {
  local name="$1" url="$2"
  local code
  code=$(curl -sk --max-time 8 -o /tmp/mesh_status_body -w "%{http_code}" "$url" 2>/dev/null || echo 000)
  if [[ "$code" == "200" ]]; then
    echo "OK  $name  $url"
    head -c 120 /tmp/mesh_status_body 2>/dev/null; echo
  else
    echo "FAIL $name  $url  (http $code)"
    ok=1
  fi
}

echo "MeshChain public seed status"
echo "============================"
check "faucet HTTP"  "http://${IP}:8787/info"
check "scanner HTTP" "http://${IP}:8788/api/v1/status"
check "faucet HTTPS" "${HTTPS_FAUCET}/info"
check "scanner HTTPS" "${HTTPS_SCANNER}/api/v1/status"
check "chain_state"  "${HTTPS_SCANNER}/api/v1/chain_state"

for p in 9100 9101 9102; do
  if nc -z -w 2 "$IP" "$p" 2>/dev/null; then
    echo "OK  peer :$p"
  else
    echo "FAIL peer :$p"
    ok=1
  fi
done

if [[ -f /tmp/mesh_status_body ]]; then
  python3 - <<'PY' 2>/dev/null || true
import json
try:
  d=json.load(open("/tmp/mesh_status_body"))
  if "height" in d:
    print(f"height={d.get('height')} accounts={d.get('account_count')} supply_tmesh={d.get('total_supply_tmesh')}")
except Exception:
  pass
PY
fi

exit $ok
