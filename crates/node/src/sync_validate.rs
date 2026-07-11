//! Hardened validation of gossip SyncResponse snapshots.

use meshchain_ledger::state::ChainState;

/// Reject multi-megabyte DoS payloads.
pub const MAX_SYNC_JSON_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

/// Validate and parse a sync snapshot. Never rewinds below `current.height`.
pub fn accept_sync_snapshot(
    current: &ChainState,
    chain_id: &str,
    claimed_height: u64,
    tip_hash_hex: &str,
    state_json: &str,
) -> Result<ChainState, String> {
    if chain_id != current.chain_id {
        return Err("chain_id mismatch".into());
    }
    if state_json.len() > MAX_SYNC_JSON_BYTES {
        return Err(format!(
            "snapshot too large ({} > {} bytes)",
            state_json.len(),
            MAX_SYNC_JSON_BYTES
        ));
    }
    if claimed_height <= current.height {
        return Err("not ahead of local height".into());
    }
    let incoming: ChainState =
        serde_json::from_str(state_json).map_err(|e| format!("bad json: {e}"))?;
    if incoming.chain_id != current.chain_id {
        return Err("embedded chain_id mismatch".into());
    }
    if incoming.validators != current.validators {
        return Err("validator set mismatch".into());
    }
    if incoming.height != claimed_height {
        return Err("height field mismatch".into());
    }
    if !tip_hash_hex.is_empty() {
        let expected = hex::encode(incoming.tip_hash);
        if !expected.eq_ignore_ascii_case(tip_hash_hex) {
            return Err("tip_hash_hex mismatch".into());
        }
    }
    // Sanity: applied history should not be empty for height > 0
    if incoming.height > 0 && incoming.applied.is_empty() {
        return Err("missing applied history".into());
    }
    // Never rewind
    if incoming.height < current.height {
        return Err("rewind rejected".into());
    }
    Ok(incoming)
}

#[cfg(test)]
mod tests {
    use super::*;
    use meshchain_ledger::genesis::{GenesisAccount, GenesisConfig, ONE_MESH};
    use meshchain_proto::crypto::Keypair;

    fn sample_state() -> ChainState {
        let v = Keypair::generate();
        let alice = Keypair::generate();
        let genesis = GenesisConfig {
            chain_id: "meshchain-testnet-1".into(),
            validators: vec![hex::encode(v.public_key())],
            block_reward: 0,
            allocations: vec![GenesisAccount {
                public_key_hex: hex::encode(alice.public_key()),
                balance: 10 * ONE_MESH,
            }],
            minters: vec![],
            slot_secs: 1,
            pq_required_above: 1000 * ONE_MESH,
        };
        let mut st = ChainState::from_genesis(&genesis).unwrap();
        let gblock =
            meshchain_proto::block::Block::new(0, [0u8; 32], 1, 0, &v, vec![]).unwrap();
        st.apply_block(&gblock).unwrap();
        st
    }

    #[test]
    fn accepts_higher_valid_snapshot() {
        let current = sample_state();
        let mut ahead = current.clone();
        ahead.height = current.height + 5;
        ahead.tip_hash = [9u8; 32];
        ahead.applied.push(meshchain_ledger::state::AppliedBlock {
            height: ahead.height,
            hash_hex: hex::encode(ahead.tip_hash),
            tx_count: 0,
        });
        let json = serde_json::to_string(&ahead).unwrap();
        let tip = hex::encode(ahead.tip_hash);
        let out = accept_sync_snapshot(
            &current,
            "meshchain-testnet-1",
            ahead.height,
            &tip,
            &json,
        )
        .unwrap();
        assert_eq!(out.height, ahead.height);
    }

    #[test]
    fn rejects_rewind() {
        let current = sample_state();
        let mut lower = current.clone();
        // claim higher in envelope but body lower — height field mismatch path
        lower.height = current.height; // same
        let json = serde_json::to_string(&lower).unwrap();
        let err = accept_sync_snapshot(
            &current,
            "meshchain-testnet-1",
            current.height, // not ahead
            "",
            &json,
        )
        .unwrap_err();
        assert!(err.contains("not ahead"));
    }

    #[test]
    fn rejects_wrong_chain() {
        let current = sample_state();
        let json = serde_json::to_string(&current).unwrap();
        let err =
            accept_sync_snapshot(&current, "other-chain", current.height + 1, "", &json)
                .unwrap_err();
        assert!(err.contains("chain_id"));
    }

    #[test]
    fn rejects_tip_mismatch() {
        let current = sample_state();
        let mut ahead = current.clone();
        ahead.height = current.height + 1;
        ahead.tip_hash = [1u8; 32];
        ahead.applied.push(meshchain_ledger::state::AppliedBlock {
            height: ahead.height,
            hash_hex: "aa".into(),
            tx_count: 0,
        });
        let json = serde_json::to_string(&ahead).unwrap();
        let err = accept_sync_snapshot(
            &current,
            "meshchain-testnet-1",
            ahead.height,
            "deadbeef",
            &json,
        )
        .unwrap_err();
        assert!(err.contains("tip_hash"));
    }

    #[test]
    fn rejects_oversized() {
        let current = sample_state();
        let big = "x".repeat(MAX_SYNC_JSON_BYTES + 1);
        let err = accept_sync_snapshot(
            &current,
            "meshchain-testnet-1",
            current.height + 1,
            "",
            &big,
        )
        .unwrap_err();
        assert!(err.contains("too large"));
    }
}
