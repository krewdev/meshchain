//! PoA round-robin consensus helpers.

use meshchain_ledger::state::ChainState;
use meshchain_proto::block::Block;
use meshchain_proto::crypto::{Keypair, PublicKey, Signature, SignatureBytes};
use meshchain_proto::tx::Tx;
use std::collections::{HashMap, HashSet};

/// Canonical message for BlockAck signatures.
pub fn ack_message(block_hash_hex: &str) -> Vec<u8> {
    format!("MESHCHAIN-BLOCK-ACK-v1\n{block_hash_hex}").into_bytes()
}

pub fn sign_block_ack(kp: &Keypair, block_hash_hex: &str) -> String {
    let sig = kp.sign(&ack_message(block_hash_hex));
    hex::encode(sig.as_bytes())
}

pub fn verify_block_ack(
    pubkey: &PublicKey,
    block_hash_hex: &str,
    signature_hex: &str,
) -> bool {
    let Ok(bytes) = hex::decode(signature_hex.trim()) else {
        return false;
    };
    if bytes.len() != 64 {
        return false;
    }
    let mut arr = [0u8; 64];
    arr.copy_from_slice(&bytes);
    let sig = SignatureBytes(arr);
    Signature::verify(pubkey, &ack_message(block_hash_hex), &sig).is_ok()
}

/// Tracks ACKs for a block hash. Final when acks >= ceil(2N/3).
#[derive(Debug, Default)]
pub struct FinalityTracker {
    /// block_hash_hex -> set of validator pubkeys that ACK'd
    acks: HashMap<String, HashSet<PublicKey>>,
}

impl FinalityTracker {
    pub fn new() -> Self {
        Self {
            acks: HashMap::new(),
        }
    }

    pub fn threshold(n_validators: usize) -> usize {
        if n_validators == 0 {
            return 1;
        }
        // ceil(2N/3)
        (2 * n_validators).div_ceil(3)
    }

    pub fn ack(&mut self, block_hash_hex: &str, validator: PublicKey) {
        self.acks
            .entry(block_hash_hex.to_string())
            .or_default()
            .insert(validator);
    }

    pub fn is_final(&self, block_hash_hex: &str, n_validators: usize) -> bool {
        let count = self
            .acks
            .get(block_hash_hex)
            .map(|s| s.len())
            .unwrap_or(0);
        count >= Self::threshold(n_validators)
    }

    pub fn ack_count(&self, block_hash_hex: &str) -> usize {
        self.acks
            .get(block_hash_hex)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Drop ACK sets for block hashes no longer pending (bound memory).
    pub fn retain_hashes<F>(&mut self, mut keep: F)
    where
        F: FnMut(&str) -> bool,
    {
        self.acks.retain(|h, _| keep(h));
    }

    /// Hard cap: if too many entries, clear finalized-looking bulk (keeps newest by insertion order unavailable — clear all stale via retain first).
    pub fn prune_if_oversized(&mut self, max_entries: usize) {
        if self.acks.len() > max_entries {
            // Keep nothing over the cap: operators re-sync from blocks on disk.
            let overflow = self.acks.len().saturating_sub(max_entries / 2);
            let drop_keys: Vec<String> = self.acks.keys().take(overflow).cloned().collect();
            for k in drop_keys {
                self.acks.remove(&k);
            }
        }
    }
}

/// Who should produce the block at this height (round-robin by height).
pub fn leader_index(height: u64, n_validators: usize) -> u8 {
    if n_validators == 0 {
        return 0;
    }
    (height as usize % n_validators) as u8
}

pub fn produce_block(
    state: &ChainState,
    producer: &Keypair,
    producer_index: u8,
    slot_time: u64,
    txs: Vec<Tx>,
) -> Result<Block, meshchain_proto::ProtoError> {
    let height = if state.applied.is_empty() {
        0
    } else {
        state.height + 1
    };
    let prev = if height == 0 {
        [0u8; 32]
    } else {
        state.tip_hash
    };
    Block::new(height, prev, slot_time, producer_index, producer, txs)
}

#[cfg(test)]
mod ack_tests {
    use super::*;
    use meshchain_proto::crypto::Keypair;

    #[test]
    fn signed_ack_verifies_and_forged_fails() {
        let kp = Keypair::generate();
        let hash = "deadbeefcafe";
        let sig = sign_block_ack(&kp, hash);
        assert!(verify_block_ack(&kp.public_key(), hash, &sig));
        assert!(!verify_block_ack(&kp.public_key(), hash, "00".repeat(64).as_str()));
        let other = Keypair::generate();
        assert!(!verify_block_ack(&other.public_key(), hash, &sig));
    }
}
