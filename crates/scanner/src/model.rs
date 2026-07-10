//! JSON API models for the scanner.

use meshchain_ledger::state::{AppliedBlock, ChainState};
use meshchain_proto::address::{mesh_name, parse_recipient, parse_short_id_hex};
use serde::Serialize;

const DECIMALS: f64 = 1_000_000.0;

#[derive(Serialize)]
pub struct StatusResponse {
    pub ok: bool,
    pub service: &'static str,
    pub auth_mode: String,
    pub chain_id: String,
    pub height: u64,
    pub tip_hash_hex: String,
    pub total_supply: u64,
    pub total_supply_tmesh: f64,
    pub account_count: usize,
    pub block_count: usize,
    pub block_reward: u64,
    pub pq_required_above: u64,
    pub validators: usize,
    pub is_testnet: bool,
    pub warning: &'static str,
    pub uptime_secs: u64,
    pub mesh_2fa: Mesh2faInfo,
}

#[derive(Serialize)]
pub struct Mesh2faInfo {
    pub enforced: bool,
    pub challenge_path: &'static str,
    pub verify_path: &'static str,
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct BlockSummary {
    pub height: u64,
    pub hash_hex: String,
    pub tx_count: u8,
}

#[derive(Serialize)]
pub struct AccountView {
    pub short_id_hex: String,
    pub mesh_name: String,
    pub balance: u64,
    pub balance_tmesh: f64,
    pub nonce: u32,
    pub has_cold_key: bool,
    pub pubkey_hex: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub query: String,
    pub kind: String,
    pub account: Option<AccountView>,
    pub block: Option<BlockSummary>,
    pub message: Option<String>,
}

pub fn tip_hash_hex(state: &ChainState) -> String {
    hex::encode(state.tip_hash)
}

pub fn block_summaries(state: &ChainState, limit: usize) -> Vec<BlockSummary> {
    state
        .applied
        .iter()
        .rev()
        .take(limit)
        .map(|b: &AppliedBlock| BlockSummary {
            height: b.height,
            hash_hex: b.hash_hex.clone(),
            tx_count: b.tx_count,
        })
        .collect()
}

pub fn find_block(state: &ChainState, height: u64) -> Option<BlockSummary> {
    state.applied.iter().find(|b| b.height == height).map(|b| {
        BlockSummary {
            height: b.height,
            hash_hex: b.hash_hex.clone(),
            tx_count: b.tx_count,
        }
    })
}

pub fn account_view(short_hex: &str, state: &ChainState) -> Option<AccountView> {
    let acc = state.accounts.get(short_hex)?;
    let sid = parse_short_id_hex(short_hex).ok()?;
    Some(AccountView {
        short_id_hex: short_hex.to_string(),
        mesh_name: mesh_name(&sid),
        balance: acc.balance,
        balance_tmesh: acc.balance as f64 / DECIMALS,
        nonce: acc.nonce,
        has_cold_key: acc.pq_pk.is_some(),
        pubkey_hex: hex::encode(acc.pubkey),
    })
}

pub fn list_accounts(state: &ChainState, limit: usize, min_balance: u64) -> Vec<AccountView> {
    let mut rows: Vec<_> = state
        .accounts
        .iter()
        .filter(|(_, a)| a.balance >= min_balance)
        .filter_map(|(k, _)| account_view(k, state))
        .collect();
    rows.sort_by(|a, b| b.balance.cmp(&a.balance));
    rows.truncate(limit);
    rows
}

pub fn resolve_account_query(q: &str, state: &ChainState) -> Option<AccountView> {
    let q = q.trim();
    // Try mesh name or hex short id
    if let Ok(sid) = parse_recipient(q) {
        let hex = hex::encode(sid);
        if let Some(v) = account_view(&hex, state) {
            return Some(v);
        }
    }
    // Direct hex key of accounts map
    if let Some(v) = account_view(q, state) {
        return Some(v);
    }
    // Pubkey hex (64 chars) → short id
    if q.len() == 64 {
        if let Ok(bytes) = hex::decode(q) {
            if bytes.len() == 32 {
                let mut pk = [0u8; 32];
                pk.copy_from_slice(&bytes);
                let sid = meshchain_proto::address::short_id(&pk);
                return account_view(&hex::encode(sid), state);
            }
        }
    }
    // Partial mesh name / hex contains
    let q_up = q.to_uppercase().replace('-', "");
    for (k, _) in &state.accounts {
        if let Ok(sid) = parse_short_id_hex(k) {
            let name = mesh_name(&sid).replace('-', "");
            if name.contains(&q_up) || k.contains(&q.to_lowercase()) {
                return account_view(k, state);
            }
        }
    }
    None
}

pub fn search(q: &str, state: &ChainState) -> SearchResult {
    let q = q.trim();
    if q.is_empty() {
        return SearchResult {
            query: q.into(),
            kind: "empty".into(),
            account: None,
            block: None,
            message: Some("enter a mesh name, short id, or block height".into()),
        };
    }
    if let Ok(h) = q.parse::<u64>() {
        if let Some(b) = find_block(state, h) {
            return SearchResult {
                query: q.into(),
                kind: "block".into(),
                account: None,
                block: Some(b),
                message: None,
            };
        }
    }
    if let Some(a) = resolve_account_query(q, state) {
        return SearchResult {
            query: q.into(),
            kind: "account".into(),
            account: Some(a),
            block: None,
            message: None,
        };
    }
    SearchResult {
        query: q.into(),
        kind: "not_found".into(),
        account: None,
        block: None,
        message: Some("no matching account or block".into()),
    }
}
