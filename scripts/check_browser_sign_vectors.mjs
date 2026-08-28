#!/usr/bin/env node
/**
 * Check JS bincode layout against frozen Rust vectors
 * (crates/proto browser_wallet_sign_vectors).
 */
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";

const src = readFileSync(new URL("../web/wallet/wallet.js", import.meta.url), "utf8");
const vectors = {};
const grab = (name) => {
  const m = src.match(new RegExp(`${name}:\\s*"([0-9a-f]+)"`));
  if (!m) throw new Error("missing " + name);
  return m[1];
};

function u32le(n) {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(n >>> 0);
  return b;
}
function u64le(n) {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(n));
  return b;
}

const PROTOCOL_VERSION = 1;
function signBytesRegister(nonce, pubkey32) {
  return Buffer.concat([Buffer.from([PROTOCOL_VERSION]), u32le(1), u32le(nonce), Buffer.from(pubkey32)]);
}
function signBytesTransfer(nonce, from8, to8, amount, fee) {
  return Buffer.concat([
    Buffer.from([PROTOCOL_VERSION]),
    u32le(0),
    u32le(nonce),
    Buffer.from(from8),
    Buffer.from(to8),
    u64le(amount),
    u64le(fee),
  ]);
}

const public_hex = grab("public_hex");
const short_id_hex = grab("short_id_hex");
const transfer_sign_hex = grab("transfer_sign_hex");
const register_sign_hex = grab("register_sign_hex");

const pk = Buffer.from(public_hex, "hex");
const from = Buffer.from(short_id_hex, "hex");
const to = Buffer.alloc(8, 0x22);
const t = signBytesTransfer(7, from, to, 1_000_000, 0).toString("hex");
const r = signBytesRegister(0, pk).toString("hex");

let failed = 0;
if (t !== transfer_sign_hex) {
  console.error("transfer sign bytes mismatch\n js", t, "\n rust", transfer_sign_hex);
  failed = 1;
} else {
  console.log("OK transfer sign bytes", t.length / 2, "B");
}
if (r !== register_sign_hex) {
  console.error("register sign bytes mismatch\n js", r, "\n rust", register_sign_hex);
  failed = 1;
} else {
  console.log("OK register sign bytes", r.length / 2, "B");
}
process.exit(failed);
