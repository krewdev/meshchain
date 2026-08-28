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

export const SIGN_VECTORS = {
  secret_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  public_hex: "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8",
  short_id_hex: "56475aa75463474c",
  transfer_sign_hex:
    "01000000000700000056475aa75463474c222222222222222240420f00000000000000000000000000",
  register_sign_hex:
    "01010000000000000003a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8",
  transfer_sig_hex:
    "0dad9d9f90f16403ced48ddb8acc1fe493a01c756d1775ab3ee0ac01cbc62d0ae9220c1ed8803abec4208cf6dba3f7260da3be35a98ec6c692914ce832441803",
  register_sig_hex:
    "6765269be48083e33de6a5103b250b344ef813940b11ab66306b005bbbac0007142107564999ca4be218d580100c7b0bdb81f7efcca5aa6bbc436ffa253e510d",
};

/** Compare JS bincode + ed25519 against frozen Rust vectors. */
export function verifySignVectors() {
  const file = {
    secret_hex: SIGN_VECTORS.secret_hex,
    public_hex: SIGN_VECTORS.public_hex,
  };
  const pk = hexToBytes(file.public_hex);
  const from = hexToBytes(SIGN_VECTORS.short_id_hex);
  const to = new Uint8Array(8).fill(0x22);
  const t = signBytesTransfer(7, from, to, 1_000_000, 0);
  const r = signBytesRegister(0, pk);
  const out = {
    transfer_bytes: bytesToHex(t) === SIGN_VECTORS.transfer_sign_hex,
    register_bytes: bytesToHex(r) === SIGN_VECTORS.register_sign_hex,
    transfer_sig: false,
    register_sig: false,
  };
  try {
    const ts = signDetached(t, file);
    const rs = signDetached(r, file);
    out.transfer_sig = bytesToHex(ts) === SIGN_VECTORS.transfer_sig_hex;
    out.register_sig = bytesToHex(rs) === SIGN_VECTORS.register_sig_hex;
  } catch {
    /* nacl missing */
  }
  out.ok = out.transfer_bytes && out.register_bytes && out.transfer_sig && out.register_sig;
  return out;
}

const LIST_KEY = "meshchain.wallets.v2";

function listState() {
  try {
    const raw = localStorage.getItem(LIST_KEY);
    if (raw) {
      const j = JSON.parse(raw);
      if (Array.isArray(j.wallets)) return j;
    }
  } catch {
    /* ignore */
  }
  const one = loadStoredWalletPlain();
  if (one) {
    return { active: one.public_hex, wallets: [one] };
  }
  return { active: null, wallets: [] };
}

function saveList(state) {
  localStorage.setItem(LIST_KEY, JSON.stringify(state));
}

function loadStoredWalletPlain() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const j = JSON.parse(raw);
    if (!j?.public_hex) return null;
    return j;
  } catch {
    return null;
  }
}

export function listWallets() {
  return listState().wallets.map((w) => ({
    public_hex: w.public_hex,
    label: w.label || "",
    wrapped: Boolean(w.wrapped),
    created: w.created || null,
  }));
}

export function loadStoredWallet() {
  const st = listState();
  const w =
    st.wallets.find((x) => x.public_hex === st.active) || st.wallets[st.wallets.length - 1] || null;
  if (!w) return loadStoredWalletPlain();
  if (w.secret_hex && w.public_hex) return w;
  if (w.wrapped) return { ...w, locked: true };
  return w;
}

export function storeWallet(file, opts = {}) {
  const st = listState();
  const rec = {
    public_hex: file.public_hex,
    secret_hex: file.wrapped ? undefined : file.secret_hex,
    wrapped: file.wrapped || undefined,
    label: opts.label || file.label || "",
    created: file.created || new Date().toISOString(),
  };
  const i = st.wallets.findIndex((w) => w.public_hex === rec.public_hex);
  if (i >= 0) st.wallets[i] = { ...st.wallets[i], ...rec };
  else st.wallets.push(rec);
  st.active = rec.public_hex;
  saveList(st);
  localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({
      secret_hex: rec.secret_hex,
      public_hex: rec.public_hex,
      wrapped: rec.wrapped,
    })
  );
}

export function setActiveWallet(publicHex) {
  const st = listState();
  st.active = publicHex;
  saveList(st);
  const w = st.wallets.find((x) => x.public_hex === publicHex);
  if (w) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(w));
  }
}

export function clearStoredWallet() {
  const st = listState();
  const active = st.active;
  st.wallets = st.wallets.filter((w) => w.public_hex !== active);
  st.active = st.wallets[0]?.public_hex || null;
  saveList(st);
  if (st.active) {
    const w = st.wallets[0];
    localStorage.setItem(STORAGE_KEY, JSON.stringify(w));
  } else {
    localStorage.removeItem(STORAGE_KEY);
  }
}

export function downloadWallet(file, name = "mesh-wallet.json") {
  const payload = file.wrapped
    ? {
        public_hex: file.public_hex,
        wrapped: file.wrapped,
        note: "Passphrase-wrapped MeshChain testnet key. Unlock in /wallet/.",
      }
    : { secret_hex: file.secret_hex, public_hex: file.public_hex };
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  URL.revokeObjectURL(a.href);
}

async function deriveAesKey(pass, salt) {
  const enc = new TextEncoder().encode(pass);
  const base = await crypto.subtle.importKey("raw", enc, "PBKDF2", false, ["deriveKey"]);
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 120_000, hash: "SHA-256" },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
}

export async function wrapSecret(file, passphrase) {
  if (!passphrase || passphrase.length < 6) throw new Error("passphrase must be at least 6 characters");
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveAesKey(passphrase, salt);
  const pt = new TextEncoder().encode(JSON.stringify({ secret_hex: file.secret_hex, public_hex: file.public_hex }));
  const ct = new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, pt));
  return {
    public_hex: file.public_hex,
    wrapped: {
      v: 1,
      alg: "pbkdf2-sha256-aes-gcm",
      salt: bytesToHex(salt),
      iv: bytesToHex(iv),
      ct: bytesToHex(ct),
    },
  };
}

export async function unwrapSecret(file, passphrase) {
  if (file.secret_hex && file.public_hex) return { secret_hex: file.secret_hex, public_hex: file.public_hex };
  const w = file.wrapped;
  if (!w) throw new Error("wallet is not wrapped");
  const key = await deriveAesKey(passphrase, hexToBytes(w.salt));
  const pt = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: hexToBytes(w.iv) },
    key,
    hexToBytes(w.ct)
  );
  const j = JSON.parse(new TextDecoder().decode(pt));
  if (!j.secret_hex) throw new Error("unwrap produced empty secret");
  return { secret_hex: j.secret_hex, public_hex: j.public_hex || file.public_hex };
}

export async function waitForInclusion(scanner, nameOrHex, before, opts = {}) {
  const timeout = opts.timeoutMs ?? 20000;
  const interval = opts.intervalMs ?? 1500;
  const start = Date.now();
  let last = null;
  let lastErr = null;
  while (Date.now() - start < timeout) {
    try {
      const acc = await scannerAccount(scanner, nameOrHex);
      last = acc;
      const nonceOk = before.nonce == null || Number(acc.nonce) > Number(before.nonce);
      const balChanged =
        before.balance == null || Number(acc.balance) !== Number(before.balance);
      if (nonceOk || balChanged) {
        return { included: true, account: acc, waited_ms: Date.now() - start };
      }
    } catch (e) {
      lastErr = e;
    }
    await new Promise((r) => setTimeout(r, interval));
  }
  return {
    included: false,
    account: last,
    error: lastErr ? String(lastErr.message || lastErr) : "timeout",
    waited_ms: Date.now() - start,
  };
}

export async function scannerActivity(base, nameOrHex, limit = 20) {
  const url =
    base.replace(/\/$/, "") +
    "/api/v1/accounts/" +
    encodeURIComponent(nameOrHex) +
    "/activity?limit=" +
    limit;
  const r = await fetch(url, { cache: "no-store" });
  const j = await parseJson(r);
  if (!r.ok) throw new Error(j.error || "activity unavailable");
  return j;
}

export function scannerAccountUrl(scanner, nameOrHex) {
  return scanner.replace(/\/$/, "") + "/?q=" + encodeURIComponent(nameOrHex);
}

export const RADIO_START_HINT = `./scripts/start_radio_relay.sh
# default HTTP for the browser wallet:
#   MESH_RADIO_HTTP=127.0.0.1:9299
# mock LoRa if no serial radio:
#   (omit MESH_RADIO_PORT)
# real radio:
#   MESH_RADIO_PORT=/dev/ttyUSB0 ./scripts/start_radio_relay.sh`;

