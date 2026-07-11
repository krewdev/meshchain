#!/usr/bin/env python3
"""
MeshChain radio multi-hop relay (first-class Meshtastic path).

Bridges:
  TCP validator gossip (line-JSON)  ↔  Meshtastic LoRa (MC binary frames)

Air policy:
  - MsgType Tx=1       binary bincode Tx  (everyday spend)
  - MsgType Block=2    only if small (≤1 tx and fits MTU); else BlockHint=8
  - MsgType BlockAck=3 binary / compact JSON
  - MsgType Tip=7      chain tip (height + tip_hash) — periodic mesh gossip
  - MsgType GossipJson=20  legacy small JSON (hello, block_hint)

Inject path (air-first submit without radio hardware):
  Connect to --listen and send:
    MCHEX <hex of MC frame>\\n
  or standard gossip JSON line.

Usage:
  python3 tools/mesh_radio_relay.py --mock --tcp 127.0.0.1:9100
  python3 tools/mesh_radio_relay.py --port /dev/ttyUSB0 --tcp 127.0.0.1:9100

Env:
  MESH_RADIO_TIP_SECS   tip advertisement interval (default 30)
"""

from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import os
import socket
import struct
import threading
import time
from typing import Any, Dict, List, Optional

MAGIC = b"MC"
FRAME_VER = 1
MSG_TX = 1
MSG_BLOCK = 2
MSG_BLOCK_ACK = 3
MSG_TIP = 7
MSG_BLOCK_HINT = 8
MSG_GOSSIP = 20
MAX_PAYLOAD = 200
HEADER_LEN = 6


def encode_mc(msg_type: int, payload: bytes) -> bytes:
    if len(payload) > MAX_PAYLOAD:
        raise ValueError(f"payload too large for single LoRa frame: {len(payload)}")
    return MAGIC + bytes([FRAME_VER, msg_type]) + struct.pack("<H", len(payload)) + payload


def decode_mc(frame: bytes) -> Optional[tuple]:
    """Return (msg_type, payload) or None."""
    if len(frame) < HEADER_LEN or frame[0:2] != MAGIC:
        return None
    if frame[2] != FRAME_VER:
        return None
    msg_type = frame[3]
    ln = frame[4] | (frame[5] << 8)
    if len(frame) < HEADER_LEN + ln:
        return None
    return msg_type, frame[HEADER_LEN : HEADER_LEN + ln]


def bincode_tip(chain_id: str, height: int, tip_hash_hex: str) -> bytes:
    """Minimal compatible tip: prefer JSON-in-gossip for Python simplicity when bincode hard.
    Air Tip frames from Rust use bincode; we also accept/send JSON tip via MSG_GOSSIP.
    """
    # Use compact JSON under MSG_TIP as utf-8 when length allows (interop with mock).
    # Rust Tip is bincode — radio path from node uses bridge; this relay dual-supports.
    obj = {"chain_id": chain_id, "height": height, "tip_hash_hex": tip_hash_hex}
    return json.dumps(obj, separators=(",", ":")).encode()


class TcpMesh:
    def __init__(self):
        self._lock = threading.Lock()
        self._conns: List[socket.socket] = []
        self._on_line = None

    def set_handler(self, fn):
        self._on_line = fn

    def connect(self, hostport: str):
        host, _, port = hostport.partition(":")

        def loop():
            while True:
                try:
                    s = socket.create_connection((host, int(port)), timeout=10)
                    s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                    with self._lock:
                        self._conns.append(s)
                    print(f"[tcp] connected {hostport}", flush=True)
                    buf = b""
                    while True:
                        chunk = s.recv(4096)
                        if not chunk:
                            break
                        buf += chunk
                        while b"\n" in buf:
                            line, buf = buf.split(b"\n", 1)
                            text = line.decode("utf-8", errors="ignore").strip()
                            if text and self._on_line:
                                self._on_line(text, source="tcp")
                except Exception as e:
                    print(f"[tcp] {hostport} reconnect: {e}", flush=True)
                    time.sleep(2)

        threading.Thread(target=loop, daemon=True).start()

    def listen(self, host: str, port: int):
        def loop():
            srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            srv.bind((host, port))
            srv.listen(16)
            print(f"[tcp] listen {host}:{port}  (JSON gossip or MCHEX frames)", flush=True)
            while True:
                c, addr = srv.accept()
                print(f"[tcp] accept {addr}", flush=True)
                with self._lock:
                    self._conns.append(c)

                def reader(sock=c):
                    buf = b""
                    try:
                        while True:
                            chunk = sock.recv(4096)
                            if not chunk:
                                break
                            buf += chunk
                            while b"\n" in buf:
                                line, buf = buf.split(b"\n", 1)
                                text = line.decode("utf-8", errors="ignore").strip()
                                if text and self._on_line:
                                    self._on_line(text, source="tcp")
                    except Exception:
                        pass

                threading.Thread(target=reader, daemon=True).start()

        threading.Thread(target=loop, daemon=True).start()

    def broadcast_line(self, line: str):
        data = (line.strip() + "\n").encode()
        dead = []
        with self._lock:
            for i, s in enumerate(self._conns):
                try:
                    s.sendall(data)
                except Exception:
                    dead.append(i)
            for i in reversed(dead):
                try:
                    self._conns[i].close()
                except Exception:
                    pass
                self._conns.pop(i)


class Radio:
    def __init__(self, mock: bool, port: str, channel_index: int):
        self.mock = mock
        self.port = port
        self.channel_index = channel_index
        self._on_frame = None
        self.iface = None

    def set_handler(self, fn):
        self._on_frame = fn

    def start(self):
        if self.mock:
            print("[radio] mock mode (loopback air)", flush=True)
            return
        try:
            import meshtastic  # noqa: F401
            import meshtastic.serial_interface
            import meshtastic.tcp_interface
            from pubsub import pub
        except ImportError:
            raise SystemExit("pip install meshtastic")

        def on_receive(packet, interface):  # noqa: ARG001
            try:
                decoded = packet.get("decoded") or {}
                payload = decoded.get("payload")
                if payload is None:
                    text = decoded.get("text") or ""
                    t = text.strip()
                    if t and all(c in "0123456789abcdefABCDEF" for c in t):
                        try:
                            raw = binascii.unhexlify(t)
                        except Exception:
                            return
                    else:
                        return
                else:
                    raw = bytes(payload)
                if raw[:2] == MAGIC and self._on_frame:
                    self._on_frame(raw)
            except Exception as e:
                print(f"[radio] rx error {e}", flush=True)

        pub.subscribe(on_receive, "meshtastic.receive")
        if self.port.startswith("tcp:"):
            rest = self.port[4:]
            host, _, _p = rest.partition(":")
            self.iface = meshtastic.tcp_interface.TCPInterface(hostname=host or "localhost")
        else:
            self.iface = meshtastic.serial_interface.SerialInterface(devPath=self.port)
        print(f"[radio] up {self.port} ch={self.channel_index}", flush=True)

    def send_frame(self, frame: bytes):
        if self.mock:
            print(f"[radio] mock tx {len(frame)}B type={frame[3] if len(frame)>3 else '?'}", flush=True)
            if self._on_frame:
                threading.Timer(0.05, lambda: self._on_frame(frame)).start()
            return
        if hasattr(self.iface, "sendData"):
            self.iface.sendData(frame, channelIndex=self.channel_index, wantAck=False)
        else:
            self.iface.sendText(binascii.hexlify(frame).decode(), channelIndex=self.channel_index)


class State:
    def __init__(self):
        self.chain_id = "meshchain-testnet-1"
        self.height = 0
        self.tip_hash_hex = ""
        self.lock = threading.Lock()

    def update_from_hello(self, msg: dict):
        with self.lock:
            if msg.get("chain_id"):
                self.chain_id = msg["chain_id"]
            h = msg.get("height")
            if isinstance(h, int) and h >= self.height:
                self.height = h

    def update_tip(self, height: int, tip_hash_hex: str = "", chain_id: str = ""):
        with self.lock:
            if chain_id:
                self.chain_id = chain_id
            if height >= self.height:
                self.height = height
                if tip_hash_hex:
                    self.tip_hash_hex = tip_hash_hex

    def snapshot(self):
        with self.lock:
            return self.chain_id, self.height, self.tip_hash_hex


def main():
    ap = argparse.ArgumentParser(description="MeshChain Meshtastic ↔ TCP gossip relay")
    ap.add_argument("--mock", action="store_true", help="loopback radio (no hardware)")
    ap.add_argument("--port", default="", help="serial path or tcp:host:port")
    ap.add_argument("--channel-index", type=int, default=0)
    ap.add_argument("--tcp", action="append", default=[], help="validator host:port (repeat)")
    ap.add_argument(
        "--listen",
        default="127.0.0.1:9199",
        help="accept TCP inject (MCHEX / gossip JSON)",
    )
    ap.add_argument(
        "--tip-secs",
        type=int,
        default=int(os.environ.get("MESH_RADIO_TIP_SECS", "30")),
        help="seconds between tip advertisements over air (0=off)",
    )
    args = ap.parse_args()

    if not args.mock and not args.port:
        ap.error("--port required unless --mock")

    tcp = TcpMesh()
    radio = Radio(args.mock, args.port, args.channel_index)
    st = State()
    seen = set()
    seen_lock = threading.Lock()

    def remember(key: str) -> bool:
        with seen_lock:
            if key in seen:
                return False
            seen.add(key)
            if len(seen) > 8000:
                seen.clear()
            return True

    def sha16(b: bytes) -> str:
        return hashlib.sha256(b).hexdigest()[:16]

    def air_send(msg_type: int, payload: bytes, label: str):
        if len(payload) > MAX_PAYLOAD:
            print(f"[relay] skip oversize {label} {len(payload)}B", flush=True)
            return
        key = sha16(bytes([msg_type]) + payload)
        if not remember("air:" + key):
            return
        try:
            frame = encode_mc(msg_type, payload)
            radio.send_frame(frame)
            print(f"[relay] →air {label} type={msg_type} {len(payload)}B", flush=True)
        except Exception as e:
            print(f"[relay] air send fail: {e}", flush=True)

    def inject_tcp_json(obj: dict):
        line = json.dumps(obj, separators=(",", ":"))
        key = sha16(line.encode())
        if not remember("tcp:" + key):
            return
        print(f"[relay] →tcp {obj.get('type')} {len(line)}B", flush=True)
        tcp.broadcast_line(line)

    def on_tcp_line(line: str, source: str = "tcp"):
        # Air-first inject: MCHEX <hex>
        if line.upper().startswith("MCHEX "):
            hx = line.split(None, 1)[1].strip()
            try:
                raw = binascii.unhexlify(hx)
            except Exception as e:
                print(f"[relay] bad MCHEX: {e}", flush=True)
                return
            on_radio_frame(raw, from_inject=True)
            return

        try:
            msg = json.loads(line)
        except Exception:
            return
        mtype = msg.get("type", "")

        if mtype == "hello":
            st.update_from_hello(msg)
            # hello is small — air as gossip JSON
            payload = json.dumps(msg, separators=(",", ":")).encode()
            if len(payload) <= MAX_PAYLOAD:
                air_send(MSG_GOSSIP, payload, "hello")
            return

        if mtype == "tx":
            # Prefer binary Tx if client also sent raw; else try JSON (often too big)
            tx = msg.get("tx")
            # Always forward on TCP mesh already; for air, try compact JSON first
            payload = json.dumps(msg, separators=(",", ":")).encode()
            if len(payload) <= MAX_PAYLOAD:
                air_send(MSG_GOSSIP, payload, "tx-json")
            else:
                print(
                    f"[relay] tx too large for single LoRa JSON ({len(payload)}B) — "
                    "use MCHEX binary Tx frame (mesh air-submit)",
                    flush=True,
                )
            return

        if mtype == "block":
            # Air policy: never multi-tx full block as JSON; send block_hint only
            blk = msg.get("block") or {}
            hdr = blk.get("header") if isinstance(blk, dict) else {}
            height = (hdr or {}).get("height", 0) or 0
            tx_count = (hdr or {}).get("tx_count", 0) or 0
            # tip update
            st.update_tip(int(height), chain_id=msg.get("chain_id") or st.chain_id)
            hint = {
                "type": "block_hint",
                "height": height,
                "producer_index": (hdr or {}).get("producer_index"),
                "tx_count": tx_count,
            }
            payload = json.dumps(hint, separators=(",", ":")).encode()
            air_send(MSG_GOSSIP, payload, "block_hint")
            # also emit tip
            cid, h, th = st.snapshot()
            tip_payload = bincode_tip(cid, int(height), th or "")
            if len(tip_payload) <= MAX_PAYLOAD:
                air_send(MSG_TIP, tip_payload, "tip")
            return

        if mtype == "block_ack":
            payload = json.dumps(msg, separators=(",", ":")).encode()
            if len(payload) <= MAX_PAYLOAD:
                air_send(MSG_GOSSIP, payload, "block_ack")
            return

        if mtype in ("ping", "pong"):
            return

        # sync_* too large for air — skip
        if mtype.startswith("sync") or mtype.startswith("blocks"):
            return

    def on_radio_frame(frame: bytes, from_inject: bool = False):
        decoded = decode_mc(frame)
        if not decoded:
            return
        msg_type, payload = decoded
        key = sha16(frame)
        if not remember("rx:" + key):
            return
        tag = "inject" if from_inject else "radio"

        if msg_type == MSG_TX:
            # Binary bincode Tx — re-wrap as gossip JSON for validators
            # Validators expect {"type":"tx","tx": <Tx object>}
            # Without bincode decoder in Python we cannot expand raw Tx.
            # Convention: also accept GossipJson that embeds full tx.
            # For binary: forward as hex control for nodes that understand MCHEX,
            # AND try: if payload looks like we need node-side decode.
            # Practical path: mesh air-submit sends BOTH — binary for air multi-hop
            # and JSON to local TCP. When radio receives binary Tx from another hop,
            # send MCHEX line to local node listeners if any; plus attempt JSON
            # via hex envelope.
            envelope = {
                "type": "tx_air",
                "tx_bincode_hex": binascii.hexlify(payload).decode(),
            }
            # Nodes upgraded to accept tx_air will decode; also push raw MCHEX
            # to any local inject path. For standard meshchain-node, use JSON tx
            # when source is inject from mesh CLI (CLI sends JSON to --submit AND
            # MCHEX to relay). Cross-mesh hops need node support for tx_air.
            inject_tcp_json(envelope)
            # Also broadcast MCHEX to peers that speak it
            tcp.broadcast_line("MCHEX " + binascii.hexlify(frame).decode())
            print(f"[{tag}] Tx binary {len(payload)}B → tcp (tx_air + MCHEX)", flush=True)
            return

        if msg_type in (MSG_TIP, MSG_BLOCK_HINT, MSG_GOSSIP, MSG_BLOCK_ACK):
            try:
                text = payload.decode("utf-8")
                obj = json.loads(text)
            except Exception:
                # tip may be bincode from Rust — store raw hex
                if msg_type == MSG_TIP:
                    inject_tcp_json(
                        {
                            "type": "tip_air",
                            "payload_hex": binascii.hexlify(payload).decode(),
                        }
                    )
                return
            if isinstance(obj, dict):
                if obj.get("type") == "hello" or "height" in obj:
                    h = obj.get("height")
                    if isinstance(h, int):
                        st.update_tip(h, obj.get("tip_hash_hex") or "", obj.get("chain_id") or "")
                if obj.get("type") == "block_hint" and isinstance(obj.get("height"), int):
                    st.update_tip(int(obj["height"]))
                # Map block_hint stays as-is; validators ignore unknown types
                if obj.get("type") in ("tx", "block", "block_ack", "hello"):
                    inject_tcp_json(obj)
                elif obj.get("type") == "block_hint":
                    inject_tcp_json(obj)
                else:
                    # tip json
                    if "height" in obj and "tip_hash_hex" in obj:
                        inject_tcp_json({"type": "tip", **obj})
                    else:
                        inject_tcp_json(obj if "type" in obj else {"type": "control", "body": obj})
            return

        if msg_type == MSG_BLOCK:
            # Full binary block — hex envelope for capable nodes
            inject_tcp_json(
                {
                    "type": "block_air",
                    "block_bincode_hex": binascii.hexlify(payload).decode(),
                }
            )
            print(f"[{tag}] Block binary {len(payload)}B → tcp block_air", flush=True)
            return

        print(f"[{tag}] unhandled type={msg_type} {len(payload)}B", flush=True)

    def tip_loop():
        while True:
            secs = args.tip_secs
            if secs <= 0:
                return
            time.sleep(max(5, secs))
            cid, h, th = st.snapshot()
            if h <= 0 and not th:
                continue
            payload = bincode_tip(cid, h, th)
            air_send(MSG_TIP, payload, f"tip-h{h}")

    tcp.set_handler(on_tcp_line)
    radio.set_handler(on_radio_frame)
    radio.start()

    host, _, port = args.listen.partition(":")
    tcp.listen(host or "127.0.0.1", int(port or "9199"))
    for peer in args.tcp:
        tcp.connect(peer)

    threading.Thread(target=tip_loop, daemon=True).start()

    print("[relay] MeshChain radio bridge running", flush=True)
    print("  channel: MeshChain-Testnet-1 (private; not LongFast)", flush=True)
    print("  air-first: mesh air-submit … --relay 127.0.0.1:9199", flush=True)
    print("  internet optional for spend when radio+local validator up", flush=True)
    while True:
        time.sleep(3600)


if __name__ == "__main__":
    main()
