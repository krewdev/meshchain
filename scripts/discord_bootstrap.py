#!/usr/bin/env python3
"""
Bootstrap the MeshChain Discord server via the Discord Bot API.

Creates (or configures) a guild with roles, categories, channels, topics,
welcome/links pins, and a permanent invite.

Prerequisites
-------------
1. https://discord.com/developers/applications → New Application "MeshChain"
2. Bot → Add Bot → Reset Token → copy token
3. Bot → Privileged Gateway Intents: optional (not required for REST setup)
4. OAuth2 → URL Generator is NOT required for create-guild; bot creates the server

Usage
-----
  export DISCORD_BOT_TOKEN='...'
  # optional: reuse an existing empty server the bot already joined
  # export DISCORD_GUILD_ID='123...'
  python3 scripts/discord_bootstrap.py

  # or:
  ./scripts/discord_bootstrap.sh

Writes invite + IDs to: data/discord_server.json
"""
from __future__ import annotations

import base64
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Optional

API = "https://discord.com/api/v10"
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "data" / "discord_server.json"
ICON_CANDIDATES = [
    ROOT / "marketing" / "creatives" / "discord-icon.png",
    ROOT / "marketing" / "creatives" / "logo.jpg",
    ROOT / "web" / "assets" / "logo.jpg",
]

# Discord role colors are integers (0xRRGGBB)
ROLES = [
    ("Admin", 0xED4245, True),
    ("Moderator", 0xE67E22, True),
    ("Core", 0x9B59B6, True),
    ("Validator Ops", 0x3498DB, True),
    ("Builder", 0x2ECC71, False),
    ("Meshtastic", 0x1ABC9C, False),
    ("Member", 0x95A5A6, False),
    ("Muted", 0x2C2F33, False),
]

# category name -> list of (channel_name, topic, staff_only)
STRUCTURE: list[tuple[str, list[tuple[str, str, bool]]]] = [
    (
        "📋 INFO",
        [
            (
                "welcome",
                "Rules + start links. Read-only for @everyone. Accept Community rules to unlock chat.",
                False,
            ),
            (
                "announcements",
                "Releases, testnet wipes, seed IP changes. Staff write only.",
                False,
            ),
            (
                "links",
                "Canonical URLs: site, GitHub, scanner, faucet, docs, seed peer.",
                False,
            ),
            (
                "roles",
                "How to get Builder / Meshtastic / Validator Ops tags.",
                False,
            ),
        ],
    ),
    (
        "💬 COMMUNITY",
        [
            ("general", "Hangout. Keep support questions in #support.", False),
            (
                "introductions",
                "Who you are · hardware · mesh name if you have one (e.g. M3SQRT-XTA1Y-ZJ6).",
                False,
            ),
            (
                "support",
                "Install, wallets, join-public, faucet errors. Include OS + command + error text.",
                False,
            ),
            (
                "showcase",
                "Nodes, field kits, scanner screenshots, air-path demos.",
                False,
            ),
        ],
    ),
    (
        "🛠 BUILD",
        [
            ("dev", "Protocol, Rust crates, PRs, design debate. Prefer GitHub Issues for bugs.", False),
            ("validators", "Cloud hosts, gossip peers, systemd, multi-validator ops.", False),
            (
                "scanner-faucet",
                "HTTP APIs, explorer UI, faucet rate limits, sslip endpoints.",
                False,
            ),
            (
                "bridge-solana",
                "Hybrid vault, devnet program, attestors. DEVNET ONLY — no mainnet deposits.",
                False,
            ),
            (
                "meshtastic-radio",
                "LoRa regions, MeshChain-Testnet-1 channel, bridge.py, airtime budget.",
                False,
            ),
        ],
    ),
    (
        "🧪 TESTNET",
        [
            (
                "testnet-status",
                "Height, incidents, planned resets. Ops posts; community can confirm outages.",
                False,
            ),
            ("faucet-drops", "Faucet chatter / drip confirmations. Don’t spam-claim.", False),
            (
                "feedback",
                "Feature asks, wipe warnings, “this is broken” with repro steps.",
                False,
            ),
        ],
    ),
    (
        "🔒 STAFF",
        [
            ("staff", "Internal coordination. No public leaks of keys.", True),
            ("alerts", "Webhooks from CI / uptime. Keep noise low.", True),
        ],
    ),
]

WELCOME_MD = """# Welcome to MeshChain

**Mesh-native ledger + wallets for Meshtastic** — optional hybrid vaults on Solana **devnet**.

> Community software. **Not** an official Meshtastic Foundation product.
> **Testnet tMESH has no cash value** and may be wiped.

## Start here (2 minutes)
1. **Scanner** (no install): https://34.172.103.125.sslip.io/
2. **Faucet UI**: https://meshchain-sigma.vercel.app/faucet/
3. **Join docs**: https://meshchain-sigma.vercel.app/docs/?doc=TESTNET
4. **GitHub**: https://github.com/krewdev/meshchain

## CLI (builders)
```
git clone https://github.com/krewdev/meshchain.git && cd meshchain
cargo build -p mesh -p meshchain-node
./target/debug/mesh join-public
./target/debug/mesh new-wallet --name me.json --publish
./target/debug/mesh faucet-drip --wallet me.json
```

## Rules (short)
1. Be respectful — no scams, phishing, or “send me your seed.”
2. Never paste **private keys**, validator secrets, or cold keys.
3. Testnet only — **no mainnet deposits** into unaudited programs.
4. No spam / shill unrelated tokens.
5. Don’t post private Meshtastic PSKs.

## Get help
• Install / wallets → #support
• Validators / cloud → #validators
• Radios → #meshtastic-radio
• Protocol → #dev

Introduce yourself in #introductions after you claim faucet tMESH.
"""

LINKS_MD = """## Canonical links

| What | URL |
|------|-----|
| Site | https://meshchain-sigma.vercel.app |
| Docs | https://meshchain-sigma.vercel.app/docs/ |
| Testnet guide | https://meshchain-sigma.vercel.app/docs/?doc=TESTNET |
| Faucet UI | https://meshchain-sigma.vercel.app/faucet/ |
| Live scanner | https://34.172.103.125.sslip.io/ |
| Live faucet API | https://faucet.34.172.103.125.sslip.io/ |
| GitHub | https://github.com/krewdev/meshchain |
| network.json | https://meshchain-sigma.vercel.app/testnet/network.json |

## Network
- **chain_id:** `meshchain-testnet-1`
- **Token:** tMESH (no cash value)
- **Meshtastic channel name:** `MeshChain-Testnet-1` (not LongFast for funds)
- **Public seed:** `34.172.103.125:9100`
- **Observer:** `35.192.20.103:9100`
- **Solana bridge:** **devnet only**
"""

ANNOUNCE_MD = """**MeshChain public testnet is live** (`meshchain-testnet-1`)

- Scanner: https://34.172.103.125.sslip.io/
- Faucet: https://meshchain-sigma.vercel.app/faucet/
- Repo: https://github.com/krewdev/meshchain
- Site: https://meshchain-sigma.vercel.app

Reminder: **tMESH has no cash value**. State may be wiped. Not official Meshtastic.

If you’re running a node or radio bridge, say hi in #introductions and #validators.
"""

ROLES_MD = """React (or ask staff) for tags that help us route questions:

🛠 **Builder** — coding / PRs
📡 **Meshtastic** — radios / LoRa
🖥 **Validator Ops** — running seed / observer hosts

You can hold more than one.
Reaction roles bot (Carl-bot / Dyno) can be wired later.
"""


class DiscordError(RuntimeError):
    pass


def api(
    method: str,
    path: str,
    token: str,
    body: Optional[dict] = None,
    *,
    raw_body: Optional[bytes] = None,
    content_type: Optional[str] = None,
) -> Any:
    url = API + path
    data = raw_body
    headers = {
        "Authorization": f"Bot {token}",
        "User-Agent": "MeshChainBootstrap (https://github.com/krewdev/meshchain, 1.0)",
    }
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json"
    if content_type:
        headers["Content-Type"] = content_type
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            raw = resp.read()
            if resp.status == 204 or not raw:
                return None
            return json.loads(raw.decode("utf-8"))
    except urllib.error.HTTPError as e:
        err = e.read().decode("utf-8", errors="replace")
        # rate limit
        if e.code == 429:
            try:
                retry = float(json.loads(err).get("retry_after", 2))
            except Exception:
                retry = 2.0
            time.sleep(retry + 0.25)
            return api(method, path, token, body, raw_body=raw_body, content_type=content_type)
        raise DiscordError(f"{method} {path} → HTTP {e.code}: {err}") from e


def load_icon_b64() -> Optional[str]:
    for p in ICON_CANDIDATES:
        if p.is_file():
            raw = p.read_bytes()
            mime = "image/png" if p.suffix.lower() == ".png" else "image/jpeg"
            return f"data:{mime};base64," + base64.b64encode(raw).decode("ascii")
    return None


def list_guilds(token: str) -> list[dict]:
    return api("GET", "/users/@me/guilds", token) or []


def bot_invite_url(client_id: str) -> str:
    # Administrator so bootstrap can manage channels/roles
    return (
        f"https://discord.com/api/oauth2/authorize?client_id={client_id}"
        f"&permissions=8&scope=bot%20applications.commands"
    )


def create_or_get_guild(token: str, bot_id: str) -> dict:
    """
    Discord no longer allows bots to POST /guilds (code 20001).
    Flow: user creates empty server → invites bot with Admin → we configure it.
    """
    existing = os.environ.get("DISCORD_GUILD_ID", "").strip()
    if existing:
        g = api("GET", f"/guilds/{existing}", token)
        print(f"Using existing guild: {g['name']} ({g['id']})")
        return g

    guilds = list_guilds(token)
    prefer = os.environ.get("DISCORD_GUILD_NAME", "MeshChain").strip().lower()
    for g in guilds:
        if g.get("name", "").strip().lower() == prefer:
            full = api("GET", f"/guilds/{g['id']}", token)
            print(f"Found guild by name: {full['name']} ({full['id']})")
            return full
    if len(guilds) == 1:
        full = api("GET", f"/guilds/{guilds[0]['id']}", token)
        print(f"Using only guild: {full['name']} ({full['id']})")
        return full
    if len(guilds) > 1:
        print("Bot is in multiple guilds — set DISCORD_GUILD_ID to pick one:")
        for g in guilds:
            print(f"  {g['id']}  {g.get('name')}")
        raise DiscordError("Set DISCORD_GUILD_ID and re-run")

    # Wait for invite
    wait_s = int(os.environ.get("DISCORD_WAIT_SECS", "300"))
    invite = bot_invite_url(bot_id)
    print(
        f"""
Discord blocks bots from creating servers via API.

Do this now (browser should open):
  1. In Discord: create a server named **MeshChain** (or empty template)
  2. Open this invite and add the bot with Administrator:
     {invite}
  3. Select the MeshChain server → Authorize

Waiting up to {wait_s}s for the bot to join a guild…
""".strip()
    )
    deadline = time.time() + wait_s
    while time.time() < deadline:
        time.sleep(3)
        guilds = list_guilds(token)
        if guilds:
            # prefer MeshChain name
            pick = guilds[0]
            for g in guilds:
                if g.get("name", "").strip().lower() == prefer:
                    pick = g
                    break
            full = api("GET", f"/guilds/{pick['id']}", token)
            print(f"Bot joined: {full['name']} ({full['id']})")
            return full
        print("  …still waiting for guild join")
    raise DiscordError(
        f"Timed out. Invite the bot, then re-run:\n  {invite}\n"
        "Or: DISCORD_GUILD_ID=... python3 scripts/discord_bootstrap.py"
    )


def delete_default_channels(token: str, guild_id: str) -> None:
    chans = api("GET", f"/guilds/{guild_id}/channels", token) or []
    for ch in chans:
        # keep nothing from defaults; we rebuild tree
        name = ch.get("name", "")
        try:
            api("DELETE", f"/channels/{ch['id']}", token)
            print(f"  removed default #{name}")
            time.sleep(0.35)
        except DiscordError as e:
            print(f"  skip delete #{name}: {e}")


def create_roles(token: str, guild_id: str) -> dict[str, str]:
    """Return map role name -> id. Bot role is separate."""
    existing = api("GET", f"/guilds/{guild_id}/roles", token) or []
    by_name = {r["name"]: r["id"] for r in existing}
    out: dict[str, str] = {}
    for name, color, hoist in ROLES:
        if name in by_name:
            out[name] = by_name[name]
            print(f"  role exists: {name}")
            continue
        r = api(
            "POST",
            f"/guilds/{guild_id}/roles",
            token,
            {
                "name": name,
                "color": color,
                "hoist": hoist,
                "mentionable": name in ("Builder", "Meshtastic", "Validator Ops", "Core"),
            },
        )
        out[name] = r["id"]
        print(f"  role created: {name}")
        time.sleep(0.4)
    return out


def deny_everyone_send(channel_id: str) -> dict:
    # permission overwrite object for @everyone later when we have everyone id
    return {}


def create_structure(token: str, guild_id: str, role_ids: dict[str, str], everyone_id: str) -> dict[str, str]:
    """Create categories + channels. Returns channel name -> id."""
    chan_ids: dict[str, str] = {}

    staff_allow = [
        role_ids.get("Admin"),
        role_ids.get("Moderator"),
        role_ids.get("Core"),
    ]
    staff_allow = [x for x in staff_allow if x]

    # Permission bits: VIEW_CHANNEL=1024, SEND_MESSAGES=2048, etc.
    VIEW = 1 << 10
    SEND = 1 << 11
    MANAGE_MSG = 1 << 13

    for cat_name, channels in STRUCTURE:
        cat = api(
            "POST",
            f"/guilds/{guild_id}/channels",
            token,
            {"name": cat_name, "type": 4},  # GUILD_CATEGORY
        )
        cat_id = cat["id"]
        print(f"category {cat_name}")
        time.sleep(0.4)

        for ch_name, topic, staff_only in channels:
            overwrites = []
            if staff_only:
                overwrites.append(
                    {
                        "id": everyone_id,
                        "type": 0,
                        "deny": str(VIEW | SEND),
                        "allow": "0",
                    }
                )
                for rid in staff_allow:
                    overwrites.append(
                        {
                            "id": rid,
                            "type": 0,
                            "allow": str(VIEW | SEND | MANAGE_MSG),
                            "deny": "0",
                        }
                    )
            elif ch_name in ("welcome", "announcements", "links"):
                # read-only for @everyone; staff can still post
                overwrites.append(
                    {
                        "id": everyone_id,
                        "type": 0,
                        "allow": str(VIEW),
                        "deny": str(SEND),
                    }
                )
                for rid in staff_allow:
                    overwrites.append(
                        {
                            "id": rid,
                            "type": 0,
                            "allow": str(VIEW | SEND | MANAGE_MSG),
                            "deny": "0",
                        }
                    )

            body: dict[str, Any] = {
                "name": ch_name,
                "type": 0,  # text
                "topic": topic[:1024],
                "parent_id": cat_id,
            }
            if overwrites:
                body["permission_overwrites"] = overwrites

            ch = api("POST", f"/guilds/{guild_id}/channels", token, body)
            chan_ids[ch_name] = ch["id"]
            print(f"  #{ch_name}")
            time.sleep(0.45)

    return chan_ids


def post(token: str, channel_id: str, content: str) -> None:
    # Discord message limit 2000
    if len(content) > 1900:
        parts = []
        buf = ""
        for line in content.splitlines(keepends=True):
            if len(buf) + len(line) > 1900:
                parts.append(buf)
                buf = line
            else:
                buf += line
        if buf:
            parts.append(buf)
    else:
        parts = [content]
    for p in parts:
        api("POST", f"/channels/{channel_id}/messages", token, {"content": p})
        time.sleep(0.4)


def create_invite(token: str, channel_id: str) -> str:
    inv = api(
        "POST",
        f"/channels/{channel_id}/invites",
        token,
        {"max_age": 0, "max_uses": 0, "unique": True},
    )
    code = inv["code"]
    return f"https://discord.gg/{code}"


def main() -> int:
    token = os.environ.get("DISCORD_BOT_TOKEN", "").strip()
    if not token:
        print(
            """
Missing DISCORD_BOT_TOKEN.

Create a bot (2 minutes):
  1. Open https://discord.com/developers/applications
  2. New Application → name "MeshChain"
  3. Bot → Add Bot → Reset Token → Copy
  4. Run:

     export DISCORD_BOT_TOKEN='paste-token-here'
     python3 scripts/discord_bootstrap.py

The bot will CREATE the MeshChain server, channels, roles, pins, and invite.
You can transfer ownership to your user account afterward (Server Settings → Transfer).
""".strip(),
            file=sys.stderr,
        )
        return 2

    me = api("GET", "/users/@me", token)
    print(f"Bot: {me.get('username')}#{me.get('discriminator', '0')} id={me['id']}")

    guild = create_or_get_guild(token, me["id"])
    gid = guild["id"]
    everyone_id = gid  # @everyone role id == guild id

    # Optional icon update
    icon = load_icon_b64()
    if icon:
        try:
            api("PATCH", f"/guilds/{gid}", token, {"icon": icon, "name": "MeshChain"})
            print("Updated guild name/icon → MeshChain")
        except DiscordError as e:
            print(f"icon/name patch note: {e}")

    print("Clearing default channels…")
    delete_default_channels(token, gid)

    print("Creating roles…")
    role_ids = create_roles(token, gid)

    print("Creating channel tree…")
    chan_ids = create_structure(token, gid, role_ids, everyone_id)

    print("Posting pins…")
    if "welcome" in chan_ids:
        post(token, chan_ids["welcome"], WELCOME_MD)
    if "links" in chan_ids:
        post(token, chan_ids["links"], LINKS_MD)
    if "announcements" in chan_ids:
        post(token, chan_ids["announcements"], ANNOUNCE_MD)
    if "roles" in chan_ids:
        post(token, chan_ids["roles"], ROLES_MD)
    if "testnet-status" in chan_ids:
        post(
            token,
            chan_ids["testnet-status"],
            "**Status:** green\n"
            "**Seed:** `34.172.103.125:9100`\n"
            "**Scanner:** https://34.172.103.125.sslip.io/\n"
            "**Note:** tMESH worthless / wipeable — check scanner for live height.",
        )

    invite_channel = chan_ids.get("welcome") or chan_ids.get("general")
    invite = create_invite(token, invite_channel) if invite_channel else ""
    print(f"\nInvite: {invite}\n")

    # system channel → welcome if possible
    try:
        api(
            "PATCH",
            f"/guilds/{gid}",
            token,
            {
                "system_channel_id": chan_ids.get("welcome"),
                "description": (
                    "Hold and move value on Meshtastic mesh — hybrid Solana vaults, "
                    "tMESH testnet. Community software, not official Meshtastic."
                ),
            },
        )
    except DiscordError as e:
        print(f"guild patch note: {e}")

    payload = {
        "guild_id": gid,
        "guild_name": "MeshChain",
        "bot_id": me["id"],
        "bot_username": me.get("username"),
        "invite": invite,
        "channels": chan_ids,
        "roles": role_ids,
        "site": "https://meshchain-sigma.vercel.app",
        "scanner": "https://34.172.103.125.sslip.io/",
        "created_by": "scripts/discord_bootstrap.py",
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"Wrote {OUT}")

    print(
        """
Next:
  1. Open the invite in your browser and join as YOUR user account.
  2. Server Settings → Roles → drag bot role above Admin/Moderator so it can assign roles later.
  3. Server Settings → Transfer Ownership → your user (recommended).
  4. Enable Community (for rules screening) in Server Settings.
  5. Paste the invite into README / day1-posts / site footer.

Done.
""".strip()
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except DiscordError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise SystemExit(1)
