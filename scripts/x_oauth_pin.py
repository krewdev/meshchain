#!/usr/bin/env python3
"""
Complete X OAuth 1.0a PIN flow and write Access Token to .env.x

  # step 1 — already done if data/x_oauth_request.json exists:
  python3 scripts/x_oauth_pin.py --request

  # step 2 — after browser shows a PIN:
  python3 scripts/x_oauth_pin.py --pin 1234567
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENV_X = ROOT / ".env.x"
REQ_PATH = ROOT / "data" / "x_oauth_request.json"
REQUEST_URL = "https://api.x.com/oauth/request_token"
ACCESS_URL = "https://api.x.com/oauth/access_token"
AUTHORIZE_URL = "https://api.x.com/oauth/authorize"


def load_env() -> None:
    if not ENV_X.is_file():
        return
    for line in ENV_X.read_text().splitlines():
        line = line.strip()
        if line.startswith("export "):
            line = line[7:]
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        os.environ.setdefault(k.strip(), v.strip().strip("'").strip('"'))


def pct(s: str) -> str:
    return urllib.parse.quote(str(s), safe="~-._")


def signed_request(
    method: str,
    url: str,
    *,
    oauth_extra: dict | None = None,
    token: str | None = None,
    token_secret: str = "",
    form: dict | None = None,
) -> dict:
    api_key = os.environ["X_API_KEY"]
    api_secret = os.environ["X_API_SECRET"]
    oauth = {
        "oauth_consumer_key": api_key,
        "oauth_nonce": uuid.uuid4().hex,
        "oauth_signature_method": "HMAC-SHA1",
        "oauth_timestamp": str(int(time.time())),
        "oauth_version": "1.0",
    }
    if token:
        oauth["oauth_token"] = token
    if oauth_extra:
        oauth.update(oauth_extra)
    # form params also in signature base for OAuth 1.0a
    params = dict(oauth)
    if form:
        params.update(form)
    items = sorted((pct(k), pct(v)) for k, v in params.items())
    param_str = "&".join(f"{k}={v}" for k, v in items)
    base = f"{method.upper()}&{pct(url)}&{pct(param_str)}"
    key = f"{pct(api_secret)}&{pct(token_secret)}"
    sig = base64.b64encode(hmac.new(key.encode(), base.encode(), hashlib.sha1).digest()).decode()
    oauth["oauth_signature"] = sig
    auth = "OAuth " + ", ".join(f'{pct(k)}="{pct(v)}"' for k, v in sorted(oauth.items()))
    data = urllib.parse.urlencode(form).encode() if form else b""
    req = urllib.request.Request(
        url,
        data=data if method.upper() != "GET" else None,
        method=method.upper(),
        headers={"Authorization": auth, "Content-Type": "application/x-www-form-urlencoded"},
    )
    try:
        with urllib.request.urlopen(req, timeout=45) as resp:
            body = resp.read().decode()
    except urllib.error.HTTPError as e:
        raise SystemExit(f"HTTP {e.code}: {e.read().decode()[:800]}") from e
    return dict(urllib.parse.parse_qsl(body))


def do_request() -> None:
    load_env()
    if not os.environ.get("X_API_KEY") or not os.environ.get("X_API_SECRET"):
        raise SystemExit("Set X_API_KEY and X_API_SECRET in .env.x first")
    tok = signed_request("POST", REQUEST_URL, oauth_extra={"oauth_callback": "oob"})
    REQ_PATH.parent.mkdir(parents=True, exist_ok=True)
    REQ_PATH.write_text(json.dumps(tok, indent=2) + "\n")
    url = f"{AUTHORIZE_URL}?oauth_token={tok['oauth_token']}"
    print("Open this URL while logged into the MeshChain X account:\n")
    print(url)
    print("\nThen run:\n  python3 scripts/x_oauth_pin.py --pin YOUR_PIN")


def do_pin(pin: str) -> None:
    load_env()
    if not REQ_PATH.is_file():
        raise SystemExit("Missing data/x_oauth_request.json — run --request first")
    req_tok = json.loads(REQ_PATH.read_text())
    access = signed_request(
        "POST",
        ACCESS_URL,
        oauth_extra={"oauth_verifier": pin.strip()},
        token=req_tok["oauth_token"],
        token_secret=req_tok["oauth_token_secret"],
    )
    # access: oauth_token, oauth_token_secret, user_id, screen_name
    screen = access.get("screen_name", "?")
    print(f"Authorized as @{screen} (user_id={access.get('user_id')})")

    # merge into .env.x
    lines = []
    if ENV_X.is_file():
        for line in ENV_X.read_text().splitlines():
            if line.strip().startswith("export X_ACCESS_TOKEN=") or line.strip().startswith(
                "export X_ACCESS_SECRET="
            ):
                continue
            if line.strip().startswith("export X_SCREEN_NAME="):
                continue
            lines.append(line)
    while lines and lines[-1].strip() == "":
        lines.pop()
    lines.append(f"export X_ACCESS_TOKEN='{access['oauth_token']}'")
    lines.append(f"export X_ACCESS_SECRET='{access['oauth_token_secret']}'")
    lines.append(f"export X_SCREEN_NAME='{screen}'")
    ENV_X.write_text("\n".join(lines) + "\n")
    print(f"Wrote access tokens to {ENV_X} (gitignored)")
    print("\nTest post:\n  source .env.x && python3 scripts/x_post.py --text 'MeshChain online' --dry-run")
    print("  source .env.x && python3 scripts/x_post.py --queue marketing/x/queue.json --limit 1")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--request", action="store_true", help="Start PIN flow")
    ap.add_argument("--pin", help="PIN from X authorize page")
    args = ap.parse_args()
    if args.pin:
        do_pin(args.pin)
        return 0
    if args.request:
        do_request()
        return 0
    ap.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
