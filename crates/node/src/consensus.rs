//! PoA round-robin consensus helpers.

use meshchain_ledger::state::ChainState;
use meshchain_proto::block::Block;
use meshchain_proto::crypto::{Keypair, PublicKey};
use meshchain_proto::tx::Tx;
use std::collections::{HashMap, HashSet};

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
