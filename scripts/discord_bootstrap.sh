#!/usr/bin/env bash
# Launch / bootstrap MeshChain Discord via bot token.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -z "${DISCORD_BOT_TOKEN:-}" ]]; then
  if [[ -f "$ROOT/.env.discord" ]]; then
    # shellcheck disable=SC1091
    source "$ROOT/.env.discord"
  fi
fi

if [[ -z "${DISCORD_BOT_TOKEN:-}" ]]; then
  cat <<'EOF'
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 MeshChain Discord bootstrap
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

No DISCORD_BOT_TOKEN found.

1) Open: https://discord.com/developers/applications
2) New Application → name: MeshChain
3) Left sidebar → Bot → Add Bot
4) Reset Token → Copy
5) Run:

   export DISCORD_BOT_TOKEN='YOUR_TOKEN_HERE'
   ./scripts/discord_bootstrap.sh

Optional: put the token in .env.discord (gitignored):

   echo "export DISCORD_BOT_TOKEN='...'" > .env.discord

The script creates the server, roles, channels, welcome posts, and invite.
EOF
  open "https://discord.com/developers/applications" 2>/dev/null || true
  open "https://discord.com/app" 2>/dev/null || true
  exit 2
fi

python3 "$ROOT/scripts/discord_bootstrap.py"
echo
if [[ -f "$ROOT/data/discord_server.json" ]]; then
  python3 - <<'PY'
import json
from pathlib import Path
p = Path("data/discord_server.json")
d = json.loads(p.read_text())
print("INVITE:", d.get("invite"))
print("GUILD: ", d.get("guild_id"))
PY
  open "$(python3 -c 'import json;print(json.load(open("data/discord_server.json"))["invite"])')" 2>/dev/null || true
fi
