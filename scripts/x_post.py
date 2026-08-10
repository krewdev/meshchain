#!/usr/bin/env python3
"""
Post to X (Twitter) via API v2 using OAuth 1.0a user context.

Setup
-----
1. Create @MeshChain (or your handle) on https://x.com
2. Developer Portal: https://developer.x.com/en/portal/dashboard
   - Create Project + App with **Read and Write**
   - Keys and tokens → generate:
       API Key, API Key Secret
       Access Token, Access Token Secret  (must be Read+Write)
3. Export credentials (never commit):

   export X_API_KEY='...'
   export X_API_SECRET='...'
   export X_ACCESS_TOKEN='...'
   export X_ACCESS_SECRET='...'

   # or: source .env.x

Usage
-----
  python3 scripts/x_post.py --text 'Hello mesh' --dry-run
  python3 scripts/x_post.py --text 'Hello mesh'
  python3 scripts/x_post.py --file marketing/x/queue/day1-01.txt
  python3 scripts/x_post.py --thread marketing/x/queue/day1-thread.json
  python3 scripts/x_post.py --queue marketing/x/queue.json --limit 1
  python3 scripts/x_post.py --queue marketing/x/queue.json --all --dry-run

Pricing note (2026): X API is often pay-per-use for new apps. Check
https://developer.x.com before enabling automation so you are not surprised by charges.
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
from typing import Any, Optional

ROOT = Path(__file__).resolve().parents[1]
API_TWEETS = "https://api.x.com/2/tweets"
UPLOAD_URL = "https://upload.twitter.com/1.1/media/upload.json"
STATE_PATH = ROOT / "data" / "x_post_state.json"
CHUNK = 4 * 1024 * 1024


def load_dotenv_x() -> None:
    env_path = ROOT / ".env.x"
    if not env_path.is_file():
        return
    for line in env_path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        if line.startswith("export "):
            line = line[len("export ") :]
        k, _, v = line.partition("=")
        k, v = k.strip(), v.strip().strip("'").strip('"')
        if k and k not in os.environ:
            os.environ[k] = v


def creds() -> dict[str, str]:
    load_dotenv_x()
    keys = {
        "api_key": os.environ.get("X_API_KEY") or os.environ.get("TWITTER_API_KEY") or "",
        "api_secret": os.environ.get("X_API_SECRET") or os.environ.get("TWITTER_API_SECRET") or "",
        "access_token": os.environ.get("X_ACCESS_TOKEN") or os.environ.get("TWITTER_ACCESS_TOKEN") or "",
        "access_secret": os.environ.get("X_ACCESS_SECRET") or os.environ.get("TWITTER_ACCESS_SECRET") or "",
    }
    missing = [k for k, v in keys.items() if not v]
    if missing:
        raise SystemExit(
            "Missing X credentials: "
            + ", ".join(missing)
            + "\n\nSet X_API_KEY, X_API_SECRET, X_ACCESS_TOKEN, X_ACCESS_SECRET\n"
            "See: marketing/x/README.md"
        )
    return keys


def percent_encode(s: str) -> str:
    return urllib.parse.quote(str(s), safe="~-._")


def oauth_header(method: str, url: str, c: dict[str, str], extra_params: Optional[dict] = None) -> str:
    oauth: dict[str, str] = {
        "oauth_consumer_key": c["api_key"],
        "oauth_nonce": uuid.uuid4().hex,
        "oauth_signature_method": "HMAC-SHA1",
        "oauth_timestamp": str(int(time.time())),
        "oauth_token": c["access_token"],
        "oauth_version": "1.0",
    }
    params = dict(oauth)
    if extra_params:
        params.update(extra_params)
    base_items = sorted((percent_encode(k), percent_encode(v)) for k, v in params.items())
    param_str = "&".join(f"{k}={v}" for k, v in base_items)
    base = "&".join(
        [
            method.upper(),
            percent_encode(url),
            percent_encode(param_str),
        ]
    )
    signing_key = f"{percent_encode(c['api_secret'])}&{percent_encode(c['access_secret'])}"
    sig = hmac.new(signing_key.encode(), base.encode(), hashlib.sha1).digest()
    oauth["oauth_signature"] = base64.b64encode(sig).decode()
    auth = ", ".join(f'{percent_encode(k)}="{percent_encode(v)}"' for k, v in sorted(oauth.items()))
    return f"OAuth {auth}"


def http_json(method: str, url: str, c: dict[str, str], *, form: Optional[dict] = None, query: Optional[dict] = None, timeout: int = 120) -> dict[str, Any]:
    extra: dict[str, str] = {}
    body: Optional[bytes] = None
    headers = {"User-Agent": "MeshChainXPoster/1.0"}
    if query:
        extra.update(query)
    if form is not None:
        extra.update(form)
        body = urllib.parse.urlencode(form).encode()
        headers["Content-Type"] = "application/x-www-form-urlencoded"
    full = url
    if query:
        full = f"{url}?{urllib.parse.urlencode(query)}"
    headers["Authorization"] = oauth_header(method, url, c, extra_params=extra or None)
    req = urllib.request.Request(full, data=body, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode()
            return json.loads(raw) if raw else {}
    except urllib.error.HTTPError as e:
        err = e.read().decode("utf-8", errors="replace")
        raise SystemExit(f"X API HTTP {e.code} {url}: {err}") from e


def upload_media(path: Path, *, dry_run: bool = False) -> Optional[str]:
    if dry_run:
        print(f"[dry-run] would upload media: {path}")
        return "dry-run-media"
    if not path.is_file():
        raise SystemExit(f"media not found: {path}")
    blob = path.read_bytes()
    c = creds()
    suffix = path.suffix.lower()
    if suffix in {".mp4", ".mov", ".m4v"}:
        media_type = "video/mp4"
        category = "tweet_video"
    elif suffix in {".gif"}:
        media_type = "image/gif"
        category = "tweet_gif"
    elif suffix in {".png"}:
        media_type = "image/png"
        category = "tweet_image"
    else:
        media_type = "image/jpeg"
        category = "tweet_image"

    print(f"upload INIT {path.name} ({len(blob)} bytes, {media_type})")
    init = http_json(
        "POST",
        UPLOAD_URL,
        c,
        form={
            "command": "INIT",
            "total_bytes": str(len(blob)),
            "media_type": media_type,
            "media_category": category,
        },
    )
    media_id = str(init.get("media_id_string") or init.get("media_id") or "")
    if not media_id:
        raise SystemExit(f"INIT returned no media_id: {init}")

    for i in range(0, len(blob), CHUNK):
        chunk = blob[i : i + CHUNK]
        idx = i // CHUNK
        boundary = f"----mesh{uuid.uuid4().hex}"
        parts: list[bytes] = []

        def field(name: str, value: str) -> None:
            parts.append(
                (
                    f"--{boundary}\r\n"
                    f'Content-Disposition: form-data; name="{name}"\r\n\r\n'
                    f"{value}\r\n"
                ).encode()
            )

        field("command", "APPEND")
        field("media_id", media_id)
        field("segment_index", str(idx))
        parts.append(
            (
                f"--{boundary}\r\n"
                f'Content-Disposition: form-data; name="media"; filename="{path.name}"\r\n'
                f"Content-Type: application/octet-stream\r\n\r\n"
            ).encode()
        )
        parts.append(chunk)
        parts.append(b"\r\n")
        parts.append(f"--{boundary}--\r\n".encode())
        body = b"".join(parts)
        req = urllib.request.Request(
            UPLOAD_URL,
            data=body,
            method="POST",
            headers={
                "Authorization": oauth_header("POST", UPLOAD_URL, c),
                "Content-Type": f"multipart/form-data; boundary={boundary}",
                "User-Agent": "MeshChainXPoster/1.0",
            },
        )
        try:
            with urllib.request.urlopen(req, timeout=180) as resp:
                resp.read()
        except urllib.error.HTTPError as e:
            err = e.read().decode("utf-8", errors="replace")
            raise SystemExit(f"APPEND HTTP {e.code}: {err}") from e
        print(f"  APPEND segment {idx} ({len(chunk)} bytes)")

    fin = http_json("POST", UPLOAD_URL, c, form={"command": "FINALIZE", "media_id": media_id})
    print(f"FINALIZE {json.dumps(fin)[:400]}")
    info = fin.get("processing_info") or {}
    while info.get("state") in {"pending", "in_progress"}:
        wait = int(info.get("check_after_secs") or 2)
        print(f"  processing {info.get('state')} wait {wait}s")
        time.sleep(wait)
        st = http_json("GET", UPLOAD_URL, c, query={"command": "STATUS", "media_id": media_id})
        info = st.get("processing_info") or {}
        print(f"  STATUS {info}")
        if info.get("state") == "failed":
            raise SystemExit(f"media processing failed: {st}")
    print(f"uploaded media_id={media_id}")
    return media_id


def post_tweet(
    text: str,
    *,
    reply_to: Optional[str] = None,
    media_ids: Optional[list[str]] = None,
    dry_run: bool = False,
) -> dict[str, Any]:
    text = text.strip()
    if not text:
        raise SystemExit("empty tweet text")
    if len(text) > 280:
        # soft warn — X Blue allows longer; still show length
        print(f"warning: text length {len(text)} > 280 (needs longer posts / Premium)", file=sys.stderr)

    body: dict[str, Any] = {"text": text}
    if reply_to:
        body["reply"] = {"in_reply_to_tweet_id": reply_to}
    if media_ids:
        body["media"] = {"media_ids": media_ids}

    if dry_run:
        print("[dry-run] would post:")
        print(text)
        if media_ids:
            print(f"[dry-run] media_ids={media_ids}")
        print("---")
        return {"data": {"id": "dry-run", "text": text}}

    c = creds()
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        API_TWEETS,
        data=data,
        method="POST",
        headers={
            "Authorization": oauth_header("POST", API_TWEETS, c),
            "Content-Type": "application/json",
            "User-Agent": "MeshChainXPoster/1.0",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        err = e.read().decode("utf-8", errors="replace")
        raise SystemExit(f"X API HTTP {e.code}: {err}") from e


def load_state() -> dict:
    if STATE_PATH.is_file():
        return json.loads(STATE_PATH.read_text())
    return {"posted_ids": [], "last_post_at": None}


def save_state(state: dict) -> None:
    STATE_PATH.parent.mkdir(parents=True, exist_ok=True)
    STATE_PATH.write_text(json.dumps(state, indent=2) + "\n")


def post_thread(posts: list[str], *, dry_run: bool = False) -> list[str]:
    ids: list[str] = []
    parent: Optional[str] = None
    for i, text in enumerate(posts):
        r = post_tweet(text, reply_to=parent, dry_run=dry_run)
        tid = r.get("data", {}).get("id", "")
        ids.append(tid)
        print(f"  [{i+1}/{len(posts)}] id={tid}")
        if not dry_run:
            parent = tid
            time.sleep(2)  # be gentle
    return ids


def run_queue(path: Path, *, limit: int, all_posts: bool, dry_run: bool) -> None:
    queue = json.loads(path.read_text())
    items = queue.get("posts") or queue
    if not isinstance(items, list):
        raise SystemExit("queue must be {posts:[...]} or a list")

    state = load_state()
    posted = set(state.get("posted_ids") or [])
    n = 0
    for item in items:
        if not all_posts and n >= limit:
            break
        pid = item.get("id") or item.get("slug")
        if pid and pid in posted and not item.get("repost"):
            print(f"skip already posted: {pid}")
            continue
        kind = item.get("type", "tweet")
        print(f"\n→ {pid or '(no id)'} ({kind})")
        if kind == "thread":
            posts = item.get("posts") or item.get("thread") or []
            ids = post_thread(posts, dry_run=dry_run)
        else:
            text = item.get("text") or item.get("body") or ""
            r = post_tweet(text, dry_run=dry_run)
            ids = [r.get("data", {}).get("id", "")]
            print(f"  id={ids[0]}")
        if not dry_run and pid:
            posted.add(pid)
            state["posted_ids"] = sorted(posted)
            state["last_post_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            state.setdefault("history", []).append(
                {"id": pid, "tweet_ids": ids, "at": state["last_post_at"]}
            )
            save_state(state)
        n += 1
    if n == 0:
        print("nothing to post (queue empty or all done)")


def main() -> int:
    ap = argparse.ArgumentParser(description="MeshChain X poster")
    ap.add_argument("--text", help="Post this text once")
    ap.add_argument("--file", type=Path, help="Read text from file")
    ap.add_argument("--thread", type=Path, help="JSON list of strings, or {posts:[...]}")
    ap.add_argument("--queue", type=Path, help="Queue JSON (marketing/x/queue.json)")
    ap.add_argument("--media", type=Path, help="Attach image or mp4 (chunked upload)")
    ap.add_argument("--reply-to", help="Tweet id to reply to")
    ap.add_argument("--limit", type=int, default=1, help="Max queue items to post (default 1)")
    ap.add_argument("--all", action="store_true", help="Post entire remaining queue")
    ap.add_argument("--dry-run", action="store_true", help="Print only, no API call")
    args = ap.parse_args()

    if args.queue:
        run_queue(args.queue, limit=args.limit, all_posts=args.all, dry_run=args.dry_run)
        return 0

    if args.thread:
        raw = json.loads(args.thread.read_text())
        posts = raw if isinstance(raw, list) else (raw.get("posts") or raw.get("thread") or [])
        post_thread(posts, dry_run=args.dry_run)
        return 0

    text = args.text
    if args.file:
        text = args.file.read_text()
    if not text:
        ap.print_help()
        return 2

    media_ids = None
    if args.media:
        mid = upload_media(args.media, dry_run=args.dry_run)
        if mid:
            media_ids = [mid]

    r = post_tweet(text, reply_to=args.reply_to, media_ids=media_ids, dry_run=args.dry_run)
    print(json.dumps(r, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
