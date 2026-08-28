//! JSON API models for the scanner.

use meshchain_ledger::state::{AppliedBlock, ChainState};
use meshchain_proto::address::{mesh_name, parse_recipient, parse_short_id_hex};
use meshchain_proto::block::Block;
use serde::Serialize;
use std::fs;
use std::path::Path;

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
    /// Round-robin leader: `height % N` (always present when validators known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_index: Option<u8>,
    /// Unix seconds from archived block header (`data/blocks/{h}.json`), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_time: Option<u64>,
    /// Producer ed25519 pubkey hex from archive or validator set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_pubkey_hex: Option<String>,
    /// True when `slot_time` / producer came from on-disk archive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_archive: Option<bool>,
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

/// Load a finalized block from `{data_dir}/blocks/{height}.json` if archived.
pub fn load_archived_block(data_dir: &Path, height: u64) -> Option<Block> {
    let path = data_dir.join("blocks").join(format!("{height}.json"));
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn leader_index(height: u64, n_validators: usize) -> Option<u8> {
    if n_validators == 0 {
        return None;
    }
    Some((height as usize % n_validators) as u8)
}

fn enrich_block(state: &ChainState, data_dir: &Path, b: &AppliedBlock) -> BlockSummary {
    let n = state.validators.len();
    let schedule_idx = leader_index(b.height, n);

    if let Some(full) = load_archived_block(data_dir, b.height) {
        let pi = full.header.producer_index;
        return BlockSummary {
            height: b.height,
            hash_hex: b.hash_hex.clone(),
            tx_count: b.tx_count,
            producer_index: Some(pi),
            slot_time: Some(full.header.slot_time),
            producer_pubkey_hex: Some(hex::encode(full.header.producer)),
            from_archive: Some(true),
        };
    }

    let producer_pubkey_hex = schedule_idx.and_then(|i| {
        state
            .validators
            .get(i as usize)
            .map(|pk| hex::encode(pk))
    });

    BlockSummary {
        height: b.height,
        hash_hex: b.hash_hex.clone(),
        tx_count: b.tx_count,
        producer_index: schedule_idx,
        slot_time: None,
        producer_pubkey_hex,
        from_archive: Some(false),
    }
}

pub fn block_summaries(state: &ChainState, data_dir: &Path, limit: usize) -> Vec<BlockSummary> {
    state
        .applied
        .iter()
        .rev()
        .take(limit)
        .map(|b: &AppliedBlock| enrich_block(state, data_dir, b))
        .collect()
}

pub fn find_block(state: &ChainState, data_dir: &Path, height: u64) -> Option<BlockSummary> {
    state
        .applied
        .iter()
        .find(|b| b.height == height)
        .map(|b| enrich_block(state, data_dir, b))
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

#[derive(Serialize)]
pub struct ActivityTx {
    pub height: u64,
    pub kind: String,
    pub amount: Option<u64>,
    pub amount_tmesh: Option<f64>,
    pub fee: Option<u64>,
    pub from_hex: Option<String>,
    pub to_hex: Option<String>,
    pub nonce: u32,
}

#[derive(Serialize)]
pub struct ActivityResponse {
    pub short_id_hex: String,
    pub mesh_name: String,
    pub scanned_blocks: usize,
    pub txs: Vec<ActivityTx>,
    pub note: Option<String>,
}

/// Scan archived blocks newest-first for txs touching this short id.
pub fn account_activity(
    q: &str,
    state: &ChainState,
    data_dir: &Path,
    limit: usize,
) -> Option<ActivityResponse> {
    let view = resolve_account_query(q, state)?;
    let sid = parse_short_id_hex(&view.short_id_hex).ok()?;
    let limit = limit.clamp(1, 100);
    let mut txs = Vec::new();
    let mut scanned = 0usize;
    let mut missing_archive = 0usize;
    for b in state.applied.iter().rev() {
        if txs.len() >= limit {
            break;
        }
        scanned += 1;
        let Some(full) = load_archived_block(data_dir, b.height) else {
            missing_archive += 1;
            continue;
        };
        for tx in full.txs {
            let hit = match &tx.body {
                meshchain_proto::tx::TxBody::Transfer { from, to, .. } => {
                    *from == sid || *to == sid
                }
                meshchain_proto::tx::TxBody::Register { pubkey, .. } => {
                    meshchain_proto::address::short_id(pubkey) == sid
                }
                meshchain_proto::tx::TxBody::Mint { to, .. } => *to == sid,
                meshchain_proto::tx::TxBody::Burn { from, .. } => *from == sid,
            };
            if !hit {
                continue;
            }
            let row = match &tx.body {
                meshchain_proto::tx::TxBody::Transfer {
                    nonce,
                    from,
                    to,
                    amount,
                    fee,
                } => ActivityTx {
                    height: b.height,
                    kind: "transfer".into(),
                    amount: Some(*amount),
                    amount_tmesh: Some(*amount as f64 / DECIMALS),
                    fee: Some(*fee),
                    from_hex: Some(hex::encode(from)),
                    to_hex: Some(hex::encode(to)),
                    nonce: *nonce,
                },
                meshchain_proto::tx::TxBody::Register { nonce, .. } => ActivityTx {
                    height: b.height,
                    kind: "register".into(),
                    amount: None,
                    amount_tmesh: None,
                    fee: None,
                    from_hex: None,
                    to_hex: None,
                    nonce: *nonce,
                },
                meshchain_proto::tx::TxBody::Mint {
                    nonce, to, amount, ..
                } => ActivityTx {
                    height: b.height,
                    kind: "mint".into(),
                    amount: Some(*amount),
                    amount_tmesh: Some(*amount as f64 / DECIMALS),
                    fee: None,
                    from_hex: None,
                    to_hex: Some(hex::encode(to)),
                    nonce: *nonce,
                },
                meshchain_proto::tx::TxBody::Burn {
                    nonce,
                    from,
                    amount,
                    ..
                } => ActivityTx {
                    height: b.height,
                    kind: "burn".into(),
                    amount: Some(*amount),
                    amount_tmesh: Some(*amount as f64 / DECIMALS),
                    fee: None,
                    from_hex: Some(hex::encode(from)),
                    to_hex: None,
                    nonce: *nonce,
                },
            };
            txs.push(row);
            if txs.len() >= limit {
                break;
            }
        }
        if scanned >= 200 {
            break;
        }
    }
    let note = if missing_archive > 0 && txs.is_empty() {
        Some(
            "older heights have no archived block bodies; activity starts after archive_first"
                .into(),
        )
    } else {
        None
    };
    Some(ActivityResponse {
        short_id_hex: view.short_id_hex,
        mesh_name: view.mesh_name,
        scanned_blocks: scanned,
        txs,
        note,
    })
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
    for k in state.accounts.keys() {
        if let Ok(sid) = parse_short_id_hex(k) {
            let name = mesh_name(&sid).replace('-', "");
            if name.contains(&q_up) || k.contains(&q.to_lowercase()) {
                return account_view(k, state);
            }
        }
    }
    None
}

pub fn search(q: &str, state: &ChainState, data_dir: &Path) -> SearchResult {
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
        if let Some(b) = find_block(state, data_dir, h) {
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
