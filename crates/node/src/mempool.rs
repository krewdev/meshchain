//! Mempool management for validators and observers.
//! Manages transaction insertion, fee-based sorting, and validation filtering.

use meshchain_ledger::state::ChainState;
use meshchain_proto::tx::Tx;
use std::collections::VecDeque;

pub struct Mempool {
    txs: VecDeque<Tx>,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    /// Creates an empty Mempool.
    pub fn new() -> Self {
        Self {
            txs: VecDeque::new(),
        }
    }

    /// Checks if the mempool is empty.
    pub fn is_empty(&self) -> bool {
        self.txs.is_empty()
    }

    /// Returns the length of the mempool.
    pub fn len(&self) -> usize {
        self.txs.len()
    }

    /// Adds a transaction to the mempool if it is not already present.
    pub fn insert(&mut self, tx: Tx) -> bool {
        let id = tx.txid_hex();
        if !self.txs.iter().any(|t| t.txid_hex() == id) {
            self.txs.push_back(tx);
            true
        } else {
            false
        }
    }

    /// Discards transaction items that can no longer be applied to the current state.
    pub fn retain_valid(&mut self, state: &ChainState) {
        self.txs.retain(|t| state.can_apply_tx(t));
    }

    /// Truncates the mempool to a maximum size, keeping valid txs.
    pub fn enforce_limits(&mut self, state: &ChainState, max_size: usize) {
        if self.len() > max_size {
            self.retain_valid(state);
            while self.len() > max_size {
                self.txs.pop_front();
            }
        }
    }

    /// Returns the highest fee among all transactions currently in the mempool.
    pub fn best_fee(&self) -> u64 {
        self.txs.iter().map(|t| t.priority_fee()).max().unwrap_or(0)
    }

    /// Pops the highest-fee applicable transactions up to `max`, simulating sequential application.
    pub fn take_applicable_fee_txs(&mut self, state: &ChainState, max: usize) -> Vec<Tx> {
        let mut trial = state.clone();
        let mut picked = Vec::new();
        while picked.len() < max && !self.is_empty() {
            let mut best_i: Option<usize> = None;
            let mut best_fee = 0u64;
            let mut best_id = String::new();
            for (i, tx) in self.txs.iter().enumerate() {
                if !trial.can_apply_tx(tx) {
                    continue;
                }
                let fee = tx.priority_fee();
                let id = tx.txid_hex();
                if best_i.is_none() || fee > best_fee || (fee == best_fee && id < best_id) {
                    best_i = Some(i);
                    best_fee = fee;
                    best_id = id;
                }
            }
            let Some(i) = best_i else {
                break;
            };
            let tx = self.txs.remove(i).unwrap();
            let _ = trial.apply_tx(&tx);
            picked.push(tx);
        }
        // Drop permanently invalid spam
        self.txs
            .retain(|t| state.can_apply_tx(t) || trial.can_apply_tx(t));
        picked
    }
}
