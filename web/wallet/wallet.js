/**
 * MeshChain browser wallet — create, register, faucet, send.
 * Keys stay in the browser (localStorage). Testnet only.
 *
 * Sign bytes match crates/proto TxBody::sign_bytes():
 *   bincode( (PROTOCOL_VERSION:u8=1, TxBody) )  with bincode 1.3 fixint LE.
 */
const MESH_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PROTOCOL_VERSION = 1;
const STORAGE_KEY = "meshchain.wallet.v1";

export const DEFAULTS = {
  faucet: "https://faucet.34.172.103.125.sslip.io",
  scanner: "https://34.172.103.125.sslip.io",
  radio: "http://127.0.0.1:9299",
};

export function bytesToHex(bytes) {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function hexToBytes(hex) {
  const h = String(hex || "")
    .replace(/^0x/, "")
    .replace(/\s/g, "")
    .toLowerCase();
  if (h.length % 2) throw new Error("bad hex");
  const out = new Uint8Array(h.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
  return out;
}

export function sha256Bytes(bytes) {
  return crypto.subtle.digest("SHA-256", bytes).then((buf) => new Uint8Array(buf));
}

export function shortIdFromPubkey(pk32) {
  return sha256Bytes(pk32).then((h) => h.slice(0, 8));
}

export function meshNameFromShort(sid8) {
  let bits = 0n;
  for (const b of sid8) bits = (bits << 8n) | BigInt(b);
  bits <<= 1n;
  let enc = "";
  for (let i = 12; i >= 0; i--) {
    const idx = Number((bits >> BigInt(i * 5)) & 0x1fn);
    enc += MESH_ALPHABET[idx];
  }
  return `M${enc.slice(0, 5)}-${enc.slice(5, 10)}-${enc.slice(10, 13)}`;
}

export function parseMeshName(name) {
  let t = String(name || "")
    .trim()
    .toUpperCase()
    .replace(/[^A-Z0-9]/g, "")
    .replace(/[IL]/g, "1")
    .replace(/O/g, "0")
    .replace(/U/g, "V");
  if (t.startsWith("M")) t = t.slice(1);
  if (t.length !== 13) throw new Error("mesh name should look like M4K7X-J9P2Q-R3W");
  let bits = 0n;
  for (const c of t) {
    const idx = MESH_ALPHABET.indexOf(c);
    if (idx < 0) throw new Error("invalid mesh name character: " + c);
    bits = (bits << 5n) | BigInt(idx);
  }
  bits >>= 1n;
  const id = new Uint8Array(8);
  for (let i = 7; i >= 0; i--) {
    id[i] = Number(bits & 0xffn);
    bits >>= 8n;
  }
  return id;
}

export function parseRecipient(s) {
  const raw = String(s || "").trim();
  const compact = raw.replace(/[\s-]/g, "");
  if (compact.length === 16 && /^[0-9a-fA-F]+$/.test(compact)) return hexToBytes(compact);
  return parseMeshName(raw);
}

function u32le(n) {
  const b = new Uint8Array(4);
  const v = n >>> 0;
  b[0] = v & 0xff;
  b[1] = (v >>> 8) & 0xff;
  b[2] = (v >>> 16) & 0xff;
  b[3] = (v >>> 24) & 0xff;
  return b;
}

function u64le(n) {
  const b = new Uint8Array(8);
  let v = BigInt(n);
  for (let i = 0; i < 8; i++) {
    b[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return b;
}

/** bincode 1.3: u8 version + u32 variant + fields */
export function signBytesRegister(nonce, pubkey32) {
  const out = new Uint8Array(1 + 4 + 4 + 32);
  out[0] = PROTOCOL_VERSION;
  out.set(u32le(1), 1); // TxBody::Register
  out.set(u32le(nonce), 5);
  out.set(pubkey32, 9);
  return out;
}

export function signBytesTransfer(nonce, from8, to8, amount, fee) {
  const out = new Uint8Array(1 + 4 + 4 + 8 + 8 + 8 + 8);
  out[0] = PROTOCOL_VERSION;
  out.set(u32le(0), 1); // TxBody::Transfer
  out.set(u32le(nonce), 5);
  out.set(from8, 9);
  out.set(to8, 17);
  out.set(u64le(amount), 25);
  out.set(u64le(fee), 33);
  return out;
}

function requireNacl() {
  if (!globalThis.nacl) {
    throw new Error("tweetnacl not loaded — check the script tag on this page");
  }
  return globalThis.nacl;
}

export function generateWallet() {
  const nacl = requireNacl();
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  const kp = nacl.sign.keyPair.fromSeed(seed);
  return {
    secret_hex: bytesToHex(seed),
    public_hex: bytesToHex(kp.publicKey),
  };
}

export function keyPairFromFile(file) {
  const nacl = requireNacl();
  const seed = hexToBytes(file.secret_hex);
  if (seed.length !== 32) throw new Error("secret must be 32 bytes");
  return nacl.sign.keyPair.fromSeed(seed);
}

export function signDetached(msgBytes, file) {
  const nacl = requireNacl();
  const kp = keyPairFromFile(file);
  return nacl.sign.detached(msgBytes, kp.secretKey);
}

export async function describeWallet(file) {
  const pk = hexToBytes(file.public_hex);
  const sid = await shortIdFromPubkey(pk);
  return {
    public_hex: file.public_hex,
    short_id_hex: bytesToHex(sid),
    mesh_name: meshNameFromShort(sid),
    short_id: sid,
    pubkey: pk,
  };
}

export function buildRegisterTx(file, nonce, info) {
  const msg = signBytesRegister(nonce, info.pubkey);
  const sig = signDetached(msg, file);
  return {
    body: { Register: { nonce, pubkey: Array.from(info.pubkey) } },
    signature: Array.from(sig),
    signer: Array.from(info.pubkey),
    pq_pk: null,
    pq_sig: null,
  };
}

export function buildTransferTx(file, info, to8, amountBase, nonce, feeBase = 0) {
  const msg = signBytesTransfer(nonce, info.short_id, to8, amountBase, feeBase);
  const sig = signDetached(msg, file);
  return {
    body: {
      Transfer: {
        nonce,
        from: Array.from(info.short_id),
        to: Array.from(to8),
        amount: amountBase,
        fee: feeBase,
      },
    },
    signature: Array.from(sig),
    signer: Array.from(info.pubkey),
    pq_pk: null,
    pq_sig: null,
  };
}

export function loadStoredWallet() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const j = JSON.parse(raw);
    if (!j?.secret_hex || !j?.public_hex) return null;
    return j;
  } catch {
    return null;
  }
}

export function storeWallet(file) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(file));
}

export function clearStoredWallet() {
  localStorage.removeItem(STORAGE_KEY);
}

export function downloadWallet(file, name = "mesh-wallet.json") {
  const blob = new Blob([JSON.stringify(file, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  URL.revokeObjectURL(a.href);
}

export function parseAmountTmesh(s) {
  const n = Number(String(s).trim());
  if (!Number.isFinite(n) || n <= 0) throw new Error("amount must be > 0");
  return Math.round(n * 1_000_000);
}

async function parseJson(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(text.slice(0, 200) || "bad response");
  }
}

export async function faucetInfo(base) {
  const r = await fetch(base.replace(/\/$/, "") + "/info", { cache: "no-store" });
  const j = await parseJson(r);
  if (!r.ok) throw new Error(j.error || "faucet offline");
  return j;
}

export async function faucetDrip(base, meshName, publicHex) {
  const r = await fetch(base.replace(/\/$/, "") + "/drip", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ mesh_name: meshName, public_key_hex: publicHex }),
  });
  const j = await parseJson(r);
  if (!r.ok || j.ok === false) throw new Error(j.error || "drip failed");
  return j;
}

export async function scannerAccount(base, nameOrHex) {
  const url = base.replace(/\/$/, "") + "/api/v1/accounts/" + encodeURIComponent(nameOrHex);
  const r = await fetch(url, { cache: "no-store" });
  const j = await parseJson(r);
  if (!r.ok) throw new Error(j.error || "account not found");
  return j;
}

export async function submitTx(endpoints, tx, path) {
  const errors = [];
  const body = JSON.stringify({ type: "tx", tx, path: path || "net" });
  const urls =
    path === "radio"
      ? [endpoints.radio.replace(/\/$/, "") + "/submit", endpoints.faucet.replace(/\/$/, "") + "/submit"]
      : [endpoints.faucet.replace(/\/$/, "") + "/submit", endpoints.scanner.replace(/\/$/, "") + "/api/v1/submit"];

  for (const url of urls) {
    try {
      const r = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });
      const j = await parseJson(r);
      if (r.ok && j.ok !== false) return { ...j, via: url };
      errors.push((j.error || r.status) + " @ " + url);
    } catch (e) {
      errors.push(String(e.message || e) + " @ " + url);
    }
  }
  throw new Error("submit failed: " + errors.join(" · "));
}

export async function radioBalance(radioBase, nameOrHex) {
  const url =
    radioBase.replace(/\/$/, "") + "/balance?q=" + encodeURIComponent(nameOrHex);
  const r = await fetch(url, { cache: "no-store" });
  const j = await parseJson(r);
  if (!r.ok || j.ok === false) throw new Error(j.error || "radio relay unreachable");
  return j;
}

export async function radioHealth(radioBase) {
  const r = await fetch(radioBase.replace(/\/$/, "") + "/health", { cache: "no-store" });
  return parseJson(r);
}
