#!/usr/bin/env python3
"""
MeshChain ↔ Meshtastic stdio bridge.

Line protocol (stdin/stdout):
  TXHEX <hex>     send MeshChain frame as text/data on mesh channel
  QUIT            exit
  RXHEX <hex>     emitted when a MeshChain frame is received (prefix MC)
  LOG <msg>       diagnostics
  OK              ready

Usage:
  python3 tools/meshtastic_bridge.py --port /dev/cu.usbserial-0001
  python3 tools/meshtastic_bridge.py --port tcp:localhost:4403 --channel-index 0
  python3 tools/meshtastic_bridge.py --mock   # no radio; echo TX back as RX (dev)

Testnet channel name: MeshChain-Testnet-1 (create private channel; do not use LongFast for funds)

Requires: pip install meshtastic  (not needed for --mock)

With validators: run meshchain-node on TCP for finality; use this bridge when
you want MC frames on LoRa as well (relay/tooling path).
"""

from __future__ import annotations

import argparse
import binascii
import sys
import threading
import time


def log(msg: str) -> None:
    print(f"LOG {msg}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="", help="Serial path or tcp:host:port")
    ap.add_argument("--channel-index", type=int, default=0)
    ap.add_argument("--mock", action="store_true", help="Loopback without radio")
    args = ap.parse_args()

    iface = None
    rx_queue: list[bytes] = []
    lock = threading.Lock()

    if args.mock:
        log("mock mode — no radio")
    else:
        try:
            import meshtastic
            import meshtastic.serial_interface
            import meshtastic.tcp_interface
            from pubsub import pub
        except ImportError:
            print("ERR install meshtastic: pip install meshtastic", flush=True)
            return 1

        def on_receive(packet, interface):  # noqa: ARG001
            try:
                decoded = packet.get("decoded") or {}
                payload = decoded.get("payload")
                if payload is None:
                    text = decoded.get("text")
                    if text:
                        # allow hex text messages
                        t = text.strip()
                        if t.startswith("MC") or (len(t) >= 4 and all(c in "0123456789abcdefABCDEF" for c in t[:4])):
                            try:
                                raw = binascii.unhexlify(t) if all(c in "0123456789abcdefABCDEF" for c in t) else text.encode()
                            except Exception:
                                raw = text.encode("utf-8", errors="ignore")
                        else:
                            return
                    else:
                        return
                else:
                    raw = bytes(payload) if not isinstance(payload, bytes) else payload

                if len(raw) >= 2 and raw[0:2] == b"MC":
                    with lock:
                        rx_queue.append(raw)
            except Exception as e:
                log(f"on_receive error: {e}")

        pub.subscribe(on_receive, "meshtastic.receive")

        port = args.port
        if port.startswith("tcp:"):
            # tcp:host:port
            rest = port[4:]
            host, _, p = rest.partition(":")
            iface = meshtastic.tcp_interface.TCPInterface(hostname=host or "localhost")
            log(f"tcp interface {host}")
        else:
            if not port:
                print("ERR --port required (or use --mock)", flush=True)
                return 1
            iface = meshtastic.serial_interface.SerialInterface(devPath=port)
            log(f"serial {port}")

    print("OK", flush=True)

    def drain_rx():
        while True:
            with lock:
                batch = list(rx_queue)
                rx_queue.clear()
            for raw in batch:
                print(f"RXHEX {binascii.hexlify(raw).decode()}", flush=True)
            time.sleep(0.05)

    t = threading.Thread(target=drain_rx, daemon=True)
    t.start()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        if line == "QUIT":
            log("quit")
            break
        if line.startswith("TXHEX "):
            hx = line[6:].strip()
            try:
                raw = binascii.unhexlify(hx)
            except Exception as e:
                print(f"ERR bad hex: {e}", flush=True)
                continue
            if args.mock:
                # loopback
                with lock:
                    rx_queue.append(raw)
                log(f"mock tx {len(raw)} bytes")
            else:
                try:
                    # Send as binary-ish text hex for max client compatibility
                    # Prefer sendData if available
                    if hasattr(iface, "sendData"):
                        iface.sendData(
                            raw,
                            channelIndex=args.channel_index,
                            wantAck=False,
                        )
                    else:
                        iface.sendText(
                            binascii.hexlify(raw).decode(),
                            channelIndex=args.channel_index,
                        )
                    log(f"tx {len(raw)} bytes ch={args.channel_index}")
                except Exception as e:
                    print(f"ERR send: {e}", flush=True)
        else:
            print(f"ERR unknown cmd", flush=True)

    if iface is not None:
        try:
            iface.close()
        except Exception:
            pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
