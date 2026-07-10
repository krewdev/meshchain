//! Privacy helpers — domain-separated hashes, stealth receive tags, safe redeem hints.
//!
//! Goals:
//! - Never put raw bank/exchange account numbers on the mesh
//! - Prefer one-time receive identifiers
//! - Hash destinations so casual mesh observers don't see unlock addresses

use crate::crypto::hash_bytes;
use serde::{Deserialize, Serialize};

/// Hash a redeem destination for Burn.redeem_hint (32 bytes).
/// chain_tag examples: b"sol", b"btc", b"usdc"
pub fn redeem_hint(chain_tag: &[u8], destination: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(32 + chain_tag.len() + destination.len());
    buf.extend_from_slice(b"meshchain-redeem-v1");
    buf.extend_from_slice(&(chain_tag.len() as u32).to_le_bytes());
    buf.extend_from_slice(chain_tag);
    buf.extend_from_slice(&(destination.len() as u32).to_le_bytes());
    buf.extend_from_slice(destination);
    hash_bytes(&buf)
}

/// Derive a one-time "stealth" short id for receiving without reusing a static address.
/// `root_short` = permanent account short id; `index` = payment counter; `noise` = 16+ random bytes.
pub fn stealth_short_id(root_short: &[u8; 8], index: u64, noise: &[u8]) -> [u8; 8] {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(b"meshchain-stealth-v1");
    buf.extend_from_slice(root_short);
    buf.extend_from_slice(&index.to_le_bytes());
    buf.extend_from_slice(noise);
    let h = hash_bytes(&buf);
    let mut out = [0u8; 8];
    out.copy_from_slice(&h[..8]);
    out
}

/// Constant-time-ish equality for short ids (avoid trivial early exit on first byte).
pub fn short_id_eq(a: &[u8; 8], b: &[u8; 8]) -> bool {
    let mut v = 0u8;
    for i in 0..8 {
        v |= a[i] ^ b[i];
    }
    v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    /// Never log full pubkeys or seeds.
    pub redact_secrets: bool,
    /// Prefer stealth receive ids for new payments.
    pub prefer_stealth_receive: bool,
    /// Burns must use hashed redeem_hint only (enforced in wallet CLI).
    pub hash_redeem_destinations: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            redact_secrets: true,
            prefer_stealth_receive: true,
            hash_redeem_destinations: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redeem_hint_stable_and_domain_separated() {
        let a = redeem_hint(b"sol", b"Dest111");
        let b = redeem_hint(b"sol", b"Dest111");
        let c = redeem_hint(b"btc", b"Dest111");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn stealth_changes_with_index() {
        let root = [1u8; 8];
        let s0 = stealth_short_id(&root, 0, b"noise-aaaa-bbbb");
        let s1 = stealth_short_id(&root, 1, b"noise-aaaa-bbbb");
        assert_ne!(s0, s1);
    }
}
