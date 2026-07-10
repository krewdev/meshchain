/** Mesh name encode/decode — matches crates/proto address.rs (Crockford base32). */
const MESH_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

export function shortIdFromBytes(arr) {
  const id = new Uint8Array(8);
  for (let i = 0; i < 8; i++) id[i] = arr[i] & 0xff;
  return id;
}

export function hexToBytes(hex) {
  const h = hex.replace(/^0x/, "").toLowerCase();
  if (h.length % 2) throw new Error("bad hex");
  const out = new Uint8Array(h.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
  return out;
}

export function bytesToHex(bytes) {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Encode 8-byte short id → MXXXXX-XXXXX-XXX */
export function meshNameFromShortHex(shortHex) {
  const data = hexToBytes(shortHex);
  if (data.length !== 8) throw new Error("short id must be 8 bytes");
  let bits = 0n;
  for (const b of data) bits = (bits << 8n) | BigInt(b);
  bits <<= 1n;
  let enc = "";
  for (let i = 12; i >= 0; i--) {
    const shift = BigInt(i * 5);
    const idx = Number((bits >> shift) & 0x1fn);
    enc += MESH_ALPHABET[idx];
  }
  return `M${enc.slice(0, 5)}-${enc.slice(5, 10)}-${enc.slice(10, 13)}`;
}

export function tipHashHex(tip) {
  if (typeof tip === "string") return tip;
  if (Array.isArray(tip)) return bytesToHex(Uint8Array.from(tip));
  return "";
}
