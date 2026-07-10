use crate::crypto::{hash_bytes, PublicKey};
use serde::{Deserialize, Serialize};

pub const ADDRESS_LEN: usize = 32;
pub const SHORT_ID_LEN: usize = 8;

pub type Address = PublicKey;
pub type ShortId = [u8; SHORT_ID_LEN];

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
