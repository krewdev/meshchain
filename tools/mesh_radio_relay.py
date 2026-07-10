#!/usr/bin/env python3
"""
MeshChain radio multi-hop relay.

Bridges TCP validator gossip (line-JSON) ↔ Meshtastic LoRa frames (MC magic).

  TCP peer  <──JSON──>  this relay  <──MC frames──>  Meshtastic mesh  <──> other relays

On each radio hop, other MeshChain relays re-inject JSON to their local validators.

Usage:
  # Mock (no radio) — self-loop for testing
  python3 tools/mesh_radio_relay.py --mock --tcp 127.0.0.1:9100

  # Hardware
  python3 tools/mesh_radio_relay.py --port /dev/ttyUSB0 --tcp 127.0.0.1:9100 --channel-index 0

Env / flags:
  --tcp host:port     local validator gossip port to dial (repeatable)
  --listen host:port  optional: accept TCP from validators (default 127.0.0.1:9199)
  --port / --mock     Meshtastic device or mock loop
"""

from __future__ import annotations

import argparse
import binascii
import hashlib
import json
import socket
import threading
import time
from typing import List, Optional

MAGIC = b"MC"
# Control-ish type for JSON gossip blob (fits FRAG later; small JSON only)
MSG_GOSSIP = 20
MAX_PAYLOAD = 180


def encode_mc(payload: bytes, msg_type: int = MSG_GOSSIP) -> bytes:
    if len(payload) > MAX_PAYLOAD:
        raise ValueError(f"payload too large for single LoRa frame: {len(payload)}")
    return MAGIC + bytes([1, msg_type, len(payload) & 0xFF, (len(payload) >> 8) & 0xFF]) + payload


def decode_mc(frame: bytes) -> Optional[bytes]:
    if len(frame) < 6 or frame[0:2] != MAGIC:
        return None
    ln = frame[4] | (frame[5] << 8)
    if len(frame) < 6 + ln:
        return None
    return frame[6 : 6 + ln]


class TcpMesh:
    def __init__(self):
        self._lock = threading.Lock()
        self._conns: List[socket.socket] = []
        self._on_line = None  # callback(str)

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
            srv.listen(8)
            print(f"[tcp] listen {host}:{port}", flush=True)
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

    def broadcast_line(self, line: str, exclude=None):
        data = (line.strip() + "\n").encode()
        dead = []
        with self._lock:
            for i, s in enumerate(self._conns):
                if s is exclude:
                    continue
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
        self._rx_q: List[bytes] = []
        self._lock = threading.Lock()

    def set_handler(self, fn):
        self._on_frame = fn

    def start(self):
        if self.mock:
            print("[radio] mock mode", flush=True)
            return
        try:
            import meshtastic
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
                    if all(c in "0123456789abcdefABCDEF" for c in text.strip()[:8]):
                        try:
                            raw = binascii.unhexlify(text.strip())
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
        print(f"[radio] up {self.port}", flush=True)

    def send_frame(self, frame: bytes):
        if self.mock:
            print(f"[radio] mock tx {len(frame)}B", flush=True)
            if self._on_frame:
                # simulate air delay
                threading.Timer(0.05, lambda: self._on_frame(frame)).start()
            return
        if hasattr(self.iface, "sendData"):
            self.iface.sendData(frame, channelIndex=self.channel_index, wantAck=False)
        else:
            self.iface.sendText(binascii.hexlify(frame).decode(), channelIndex=self.channel_index)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mock", action="store_true")
    ap.add_argument("--port", default="", help="serial or tcp:host:port")
    ap.add_argument("--channel-index", type=int, default=0)
    ap.add_argument("--tcp", action="append", default=[], help="validator host:port (repeat)")
    ap.add_argument("--listen", default="127.0.0.1:9199")
    args = ap.parse_args()

    if not args.mock and not args.port:
        ap.error("--port required unless --mock")

    tcp = TcpMesh()
    radio = Radio(args.mock, args.port, args.channel_index)

    # de-dupe
    seen = set()
    seen_lock = threading.Lock()

    def remember(key: str) -> bool:
        with seen_lock:
            if key in seen:
                return False
            seen.add(key)
            if len(seen) > 5000:
                seen.clear()
            return True

    def on_tcp_line(line: str, source: str = "tcp"):
        # Only forward compact control/hello/tx summaries over LoRa (size limit)
        try:
            msg = json.loads(line)
        except Exception:
            return
        mtype = msg.get("type", "")
        # Prefer small messages over air
        if mtype not in ("hello", "block_ack", "ping", "tx"):
            # blocks are large — skip full block over single LoRa frame
            if mtype == "block":
                # send height hash only for awareness
                blk = msg.get("block") or {}
                hdr = blk.get("header") if isinstance(blk, dict) else {}
                slim = {
                    "type": "block_hint",
                    "height": (hdr or {}).get("height"),
                    "producer_index": (hdr or {}).get("producer_index"),
                }
                payload = json.dumps(slim, separators=(",", ":")).encode()
            else:
                return
        else:
            # strip heavy fields from tx for air (still may be large — skip if too big)
            payload = json.dumps(msg, separators=(",", ":")).encode()
            if len(payload) > MAX_PAYLOAD:
                print(f"[relay] skip oversize {mtype} {len(payload)}B", flush=True)
                return

        key = hashlib_sha(payload)
        if not remember("air:" + key):
            return
        try:
            frame = encode_mc(payload)
            radio.send_frame(frame)
            print(f"[relay] tcp→radio {mtype or 'block_hint'} {len(payload)}B", flush=True)
        except Exception as e:
            print(f"[relay] radio send fail: {e}", flush=True)

    def on_radio_frame(frame: bytes):
        payload = decode_mc(frame)
        if not payload:
            return
        key = hashlib_sha(payload)
        if not remember("rx:" + key):
            return
        try:
            text = payload.decode("utf-8")
            json.loads(text)  # validate
        except Exception:
            return
        print(f"[relay] radio→tcp {len(payload)}B", flush=True)
        tcp.broadcast_line(text)

    def hashlib_sha(b: bytes) -> str:
        return hashlib.sha256(b).hexdigest()[:16]

    tcp.set_handler(on_tcp_line)
    radio.set_handler(on_radio_frame)
    radio.start()

    host, _, port = args.listen.partition(":")
    tcp.listen(host or "127.0.0.1", int(port or "9199"))
    for peer in args.tcp:
        tcp.connect(peer)

    print("[relay] multi-hop radio bridge running", flush=True)
    print("  channel MeshChain-Testnet-1 recommended", flush=True)
    while True:
        time.sleep(3600)


if __name__ == "__main__":
    main()
