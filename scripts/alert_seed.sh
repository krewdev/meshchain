#!/usr/bin/env bash
# Local/cron seed check → Discord webhook on failure.
#   DISCORD_WEBHOOK=https://discord.com/api/webhooks/... ./scripts/alert_seed.sh
#   # or cron: */30 * * * * cd ~/meshchain && DISCORD_WEBHOOK=... ./scripts/alert_seed.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
chmod +x scripts/status_public_seed.sh 2>/dev/null || true

set +e
out=$(./scripts/status_public_seed.sh 2>&1)
code=$?
set -e

if [[ $code -eq 0 ]]; then
  echo "OK"
  exit 0
fi

echo "$out"
if [[ -n "${DISCORD_WEBHOOK:-}" ]]; then
  python3 - <<PY
import json, os, urllib.request
text = """$out"""[-1500:]
payload = json.dumps({
  "content": "⚠️ MeshChain seed health FAIL\n```\n" + text + "\n```"
}).encode()
req = urllib.request.Request(
  os.environ["DISCORD_WEBHOOK"],
  data=payload,
  headers={"Content-Type": "application/json"},
)
urllib.request.urlopen(req, timeout=15)
print("notified Discord")
PY
else
  echo "Set DISCORD_WEBHOOK to notify Discord"
fi
exit $code
