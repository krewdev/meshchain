use crate::crypto::{hash_bytes, PublicKey};
use serde::{Deserialize, Serialize};

pub const ADDRESS_LEN: usize = 32;
pub const SHORT_ID_LEN: usize = 8;

pub type Address = PublicKey;
pub type ShortId = [u8; SHORT_ID_LEN];

/// Crockford base32 — no I/L/O/U (hard to confuse when speaking or typing).
const MESH_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Short id = first 8 bytes of SHA-256(pubkey). Collision checked at REGISTER.
pub fn short_id(pubkey: &PublicKey) -> ShortId {
    let h = hash_bytes(pubkey);
    let mut id = [0u8; SHORT_ID_LEN];
    id.copy_from_slice(&h[..SHORT_ID_LEN]);
    id
}

pub fn short_id_hex(id: &ShortId) -> String {
    hex::encode(id)
}

pub fn parse_short_id_hex(s: &str) -> Result<ShortId, String> {
    let bytes = hex::decode(s.trim()).map_err(|e| e.to_string())?;
    if bytes.len() != SHORT_ID_LEN {
        return Err(format!(
            "short id must be {} bytes ({} hex chars), got {}",
            SHORT_ID_LEN,
            SHORT_ID_LEN * 2,
            bytes.len()
        ));
    }
    let mut id = [0u8; SHORT_ID_LEN];
    id.copy_from_slice(&bytes);
    Ok(id)
}

/// Memorable mesh name from short id.
/// Example: `M4K7X-J9P2Q-R3W` (prefix M + Crockford base32 of 8 unique bytes).
///
/// Unique 1:1 with the 8-byte short id (secure: derived from key hash, not pickable).
pub fn mesh_name(id: &ShortId) -> String {
    let enc = base32_encode(id);
    // 13 chars → M + 5-5-3 with dashes for speakability
    format!("M{}-{}-{}", &enc[0..5], &enc[5..10], &enc[10..13])
}

pub fn mesh_name_from_pubkey(pk: &PublicKey) -> String {
    mesh_name(&short_id(pk))
}

/// Parse a mesh name (with or without dashes / leading M) back to short id.
pub fn parse_mesh_name(s: &str) -> Result<ShortId, String> {
    let mut t: String = s
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    // Normalize lookalikes
    t = t
        .replace('I', "1")
        .replace('L', "1")
        .replace('O', "0")
        .replace('U', "V");
    if let Some(rest) = t.strip_prefix('M') {
        t = rest.to_string();
    }
    if t.len() != 13 {
        return Err(format!(
            "mesh name should look like M4K7X-J9P2Q-R3W (got {} data chars)",
            t.len()
        ));
    }
    base32_decode(&t)
}

/// Accept either mesh name (`M4K7X-…`) or raw 16-char hex short id.
pub fn parse_recipient(s: &str) -> Result<ShortId, String> {
    let raw = s.trim();
    let compact: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    // Hex short id: exactly 16 hex chars
    if compact.len() == 16 && compact.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_short_id_hex(&compact);
    }
    parse_mesh_name(raw)
}

fn base32_encode(data: &[u8; 8]) -> String {
    // Encode 64 bits as 13 base32 chars (last char uses remaining bits + zeros)
    let mut bits: u128 = 0;
    for &b in data {
        bits = (bits << 8) | b as u128;
    }
    // 64 bits left-aligned into 65 bits for 13*5
    bits <<= 1;
    let mut out = String::with_capacity(13);
    for i in (0..13).rev() {
        let shift = i * 5;
        let idx = ((bits >> shift) & 0x1f) as usize;
        out.push(MESH_ALPHABET[idx] as char);
    }
    out
}

fn base32_decode(s: &str) -> Result<ShortId, String> {
    if s.len() != 13 {
        return Err("expected 13 base32 characters".into());
    }
    let mut bits: u128 = 0;
    for c in s.bytes() {
        let idx = MESH_ALPHABET
            .iter()
            .position(|&a| a == c)
            .ok_or_else(|| format!("invalid character in mesh name: {}", c as char))?;
        bits = (bits << 5) | idx as u128;
    }
    // We shifted 13*5 = 65 bits; drop the trailing padding bit
    bits >>= 1;
    let mut id = [0u8; 8];
    for i in (0..8).rev() {
        id[i] = (bits & 0xff) as u8;
        bits >>= 8;
    }
    Ok(id)
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn mesh_name_roundtrip() {
        let id = short_id(&[9u8; 32]);
        let name = mesh_name(&id);
        assert!(name.starts_with('M'));
        assert!(name.contains('-'));
        let back = parse_mesh_name(&name).unwrap();
        assert_eq!(id, back);
        // without dashes / lowercase
        let compact = name.replace('-', "").to_lowercase();
        assert_eq!(id, parse_mesh_name(&compact).unwrap());
    }

    #[test]
    fn parse_recipient_hex_or_name() {
        let id = [0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89];
        let hex = short_id_hex(&id);
        assert_eq!(parse_recipient(&hex).unwrap(), id);
        let name = mesh_name(&id);
        assert_eq!(parse_recipient(&name).unwrap(), id);
    }
}

pub fn address_hex(addr: &Address) -> String {
    hex::encode(addr)
}

pub fn parse_address_hex(s: &str) -> Result<Address, String> {
    let bytes = hex::decode(s.trim()).map_err(|e| e.to_string())?;
    if bytes.len() != ADDRESS_LEN {
        return Err(format!("address must be 32 bytes, got {}", bytes.len()));
    }
    let mut a = [0u8; ADDRESS_LEN];
    a.copy_from_slice(&bytes);
    Ok(a)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountId {
    pub short: ShortId,
    pub full: Option<Address>,
}

impl AccountId {
    pub fn from_pubkey(pk: &PublicKey) -> Self {
        Self {
            short: short_id(pk),
            full: Some(*pk),
        }
    }
}
