#!/usr/bin/env python3
"""
MeshChain testnet faucet — drip tMESH to a mesh name.

POST /drip  { "mesh_name": "M3SQRT-XTA1Y-ZJ6" }
GET  /health
GET  /info

Requires meshchain-node mint-for-deposit and a local data dir with genesis + keys.
Env:
  MESHCHAIN_DATA   default ./data
  MESHCHAIN_BIN    path to meshchain-node
  FAUCET_AMOUNT    base units (default 100_000_000 = 100 tMESH)
  FAUCET_PORT      default 8787
  FAUCET_COOLDOWN  seconds between drips per name (default 3600)
  CORS_ORIGIN      default *
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

DATA = Path(os.environ.get("MESHCHAIN_DATA", "./data")).resolve()
BIN = os.environ.get(
    "MESHCHAIN_BIN",
    str(Path(__file__).resolve().parents[2] / "target/debug/meshchain-node"),
)
# also try /tmp build
if not Path(BIN).exists():
    alt = Path("/tmp/mc-e2e/debug/meshchain-node")
    if alt.exists():
        BIN = str(alt)

AMOUNT = int(os.environ.get("FAUCET_AMOUNT", str(100 * 1_000_000)))
PORT = int(os.environ.get("FAUCET_PORT", "8787"))
COOLDOWN = int(os.environ.get("FAUCET_COOLDOWN", "3600"))
CORS = os.environ.get("CORS_ORIGIN", "*")
STATE_FILE = DATA / "faucet_state.json"

# Crockford base32 (match Rust)
ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"


def mesh_name_to_short_hex(name: str) -> str:
    t = "".join(c for c in name.upper() if c.isalnum())
    t = t.replace("I", "1").replace("L", "1").replace("O", "0").replace("U", "V")
    if t.startswith("M"):
        t = t[1:]
    if len(t) != 13:
        raise ValueError("mesh name should look like M4K7X-J9P2Q-R3W")
    bits = 0
    for c in t:
        idx = ALPHABET.find(c)
        if idx < 0:
            raise ValueError(f"invalid character {c}")
        bits = (bits << 5) | idx
    bits >>= 1  # drop padding bit
    raw = bits.to_bytes(8, "big")
    return raw.hex()


def load_state() -> dict:
    if STATE_FILE.exists():
        return json.loads(STATE_FILE.read_text())
    return {"last": {}}


def save_state(st: dict) -> None:
    DATA.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(json.dumps(st, indent=2))


def find_or_create_recipient(short_hex: str) -> str:
    """
    Return 32-byte pubkey hex for an account with this short id.
    If unknown, we cannot invent a private key — recipient must exist on chain
    OR we create a placeholder account by minting only if we have mapping.

    For faucet: require user to have registered (exist in chain_state) OR
    accept optional public_key in request to create account + mint.
    """
    state_path = DATA / "chain_state.json"
    if state_path.exists():
        st = json.loads(state_path.read_text())
        acc = st.get("accounts", {}).get(short_hex)
        if acc and acc.get("pubkey"):
            # pubkey stored as array of bytes in some serdes — handle both
            pk = acc["pubkey"]
            if isinstance(pk, list):
                return bytes(pk).hex()
            if isinstance(pk, str):
                return pk
    return ""


def drip(mesh_name: str, public_key_hex: str | None = None) -> dict:
    short_hex = mesh_name_to_short_hex(mesh_name)
    st = load_state()
    now = time.time()
    last = st.get("last", {}).get(short_hex, 0)
    if now - last < COOLDOWN:
        wait = int(COOLDOWN - (now - last))
        raise RuntimeError(f"cooldown: try again in {wait}s")

    to_pubkey = public_key_hex or find_or_create_recipient(short_hex)
    if not to_pubkey:
        raise RuntimeError(
            "unknown mesh name — open the wallet once on testnet first, "
            "or send public_key_hex (64 hex chars) with the request"
        )

    # verify short id matches pubkey if provided
    if public_key_hex:
        pk = bytes.fromhex(public_key_hex)
        if len(pk) != 32:
            raise RuntimeError("public_key_hex must be 32 bytes")
        h = hashlib.sha256(pk).digest()[:8].hex()
        if h != short_hex:
            raise RuntimeError("public_key_hex does not match mesh name")

    ext = hashlib.sha256(f"faucet:{short_hex}:{now}".encode()).hexdigest()[:32]
    cmd = [
        BIN,
        "mint-for-deposit",
        "--data-dir",
        str(DATA),
        "--to-pubkey",
        to_pubkey,
        "--amount",
        str(AMOUNT),
        "--external-ref-hex",
        ext,
        "--validator-index",
        "0",
    ]
    if not Path(BIN).exists():
        raise RuntimeError(f"meshchain-node not found at {BIN}")

    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr or proc.stdout or "mint failed")

    st.setdefault("last", {})[short_hex] = now
    save_state(st)

    return {
        "ok": True,
        "mesh_name": mesh_name.upper() if mesh_name.upper().startswith("M") else "M" + mesh_name,
        "short_id_hex": short_hex,
        "amount_base": AMOUNT,
        "amount_tmesh": AMOUNT / 1_000_000,
        "log": proc.stdout.strip().splitlines()[-5:],
    }


class Handler(BaseHTTPRequestHandler):
    def _cors(self):
        self.send_header("Access-Control-Allow-Origin", CORS)
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")

    def _json(self, code: int, obj: dict):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self._cors()
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_response(204)
        self._cors()
        self.end_headers()

    def do_GET(self):
        path = urlparse(self.path).path
        if path in ("/health", "/"):
            self._json(200, {"ok": True, "service": "meshchain-faucet", "testnet": True})
            return
        if path == "/info":
            self._json(
                200,
                {
                    "chain_hint": "meshchain-testnet-1",
                    "amount_tmesh": AMOUNT / 1_000_000,
                    "cooldown_secs": COOLDOWN,
                    "data_dir": str(DATA),
                    "token": "tMESH (no cash value)",
                },
            )
            return
        self._json(404, {"ok": False, "error": "not found"})

    def do_POST(self):
        path = urlparse(self.path).path
        if path != "/drip":
            self._json(404, {"ok": False, "error": "not found"})
            return
        n = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(n) if n else b"{}"
        try:
            body = json.loads(raw.decode() or "{}")
            name = body.get("mesh_name") or body.get("name") or ""
            pk = body.get("public_key_hex") or body.get("pubkey")
            result = drip(name, pk)
            self._json(200, result)
        except Exception as e:
            self._json(400, {"ok": False, "error": str(e)})

    def log_message(self, fmt, *args):
        print("[faucet]", fmt % args)


def main():
    print(f"MeshChain faucet on :{PORT}")
    print(f"  data={DATA}")
    print(f"  bin={BIN}")
    print(f"  drip={AMOUNT / 1e6} tMESH cooldown={COOLDOWN}s")
    httpd = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    httpd.serve_forever()


if __name__ == "__main__":
    main()
