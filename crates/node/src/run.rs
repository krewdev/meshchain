//! Multi-machine validator run loop (TCP gossip + Meshtastic-ready framing notes).

use crate::consensus::{
    leader_index, produce_block, sign_block_ack, verify_block_ack, FinalityTracker,
};
use crate::mempool::Mempool;
use crate::net::{GossipHub, GossipMsg};
use anyhow::{bail, Context, Result};
use meshchain_ledger::genesis::GenesisConfig;
use meshchain_ledger::registry::Registry;
use meshchain_ledger::state::ChainState;
const RELAYER_POLL_INTERVAL: Duration = Duration::from_secs(10);
use meshchain_proto::block::{Block, MAX_TXS_PER_BLOCK};
use meshchain_proto::crypto::{Keypair, PublicKey};
use meshchain_proto::tx::Tx;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct RunConfig {
    pub data_dir: std::path::PathBuf,
    /// Present for producers; ignored when observer.
    pub validator_index: Option<u8>,
    /// Non-producing full node — anyone can run with shared genesis + seeds.
    pub observer: bool,
    pub listen: SocketAddr,
    pub peers: Vec<String>,
    pub slot_ms: u64,
    pub radio_port: Option<String>, // TODO: use this for meshtastic init
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn genesis_validator_set(genesis: &GenesisConfig) -> Result<HashSet<PublicKey>> {
    let mut set = HashSet::new();
    for h in &genesis.validators {
        let b = hex::decode(h).context("bad validator hex in genesis")?;
        if b.len() != 32 {
            bail!("validator pubkey must be 32 bytes");
        }
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&b);
        set.insert(pk);
    }
    Ok(set)
}

fn blocks_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("blocks")
}

fn save_finalized_block(data_dir: &Path, block: &Block) -> Result<()> {
    let dir = blocks_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", block.header.height));
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string(block)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn load_finalized_blocks(data_dir: &Path, from_height: u64, max_blocks: u32) -> Vec<Block> {
    let dir = blocks_dir(data_dir);
    let mut out = Vec::new();
    let max = max_blocks.clamp(1, 128) as u64;
    for h in from_height..from_height.saturating_add(max) {
        let path = dir.join(format!("{h}.json"));
        if !path.exists() {
            break;
        }
        match fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(b) => out.push(b),
            None => break,
        }
    }
    out
}

pub fn run_validator(cfg: RunConfig) -> Result<()> {
    let genesis_path = cfg.data_dir.join("genesis.json");
    let genesis: GenesisConfig = serde_json::from_str(
        &fs::read_to_string(&genesis_path)
            .with_context(|| format!("missing {}", genesis_path.display()))?,
    )?;
    let n = genesis.validators.len();
    let authorized = genesis_validator_set(&genesis)?;
    let protocol_version = genesis.protocol_version;

    let observer = cfg.observer;
    let (keypair, my_index): (Option<Keypair>, Option<u8>) = if observer {
        (None, None)
    } else {
        let idx = cfg
            .validator_index
            .context("--validator-index required unless --observer")?;
        if idx as usize >= n {
            bail!("validator-index {idx} out of range (have {n} validators in genesis)");
        }
        let key_path = cfg
            .data_dir
            .join("keys")
            .join(format!("validator-{idx}.json"));
        let key_file: meshchain_proto::crypto::KeypairFile =
            serde_json::from_str(&fs::read_to_string(&key_path)?)?;
        let keypair = Keypair::from_file(&key_file).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let expected = hex::decode(&genesis.validators[idx as usize])?;
        if keypair.public_key().as_slice() != expected.as_slice() {
            bail!("validator key does not match genesis validators[{idx}]");
        }
        (Some(keypair), Some(idx))
    };

    let state_path = cfg.data_dir.join("chain_state.json");
    let mut state = if state_path.exists() {
        ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?
    } else {
        ChainState::from_genesis(&genesis)?
    };

    if state.chain_id != genesis.chain_id {
        bail!(
            "state chain_id {} != genesis {}",
            state.chain_id,
            genesis.chain_id
        );
    }

    let hub = GossipHub::start(cfg.listen, cfg.peers.clone())?;

    // --- Solana Bridge Relayer Daemon Setup ---
    if my_index.is_some() {
        let relayer_script = PathBuf::from("programs-mesh-bridge/scripts/relayer_daemon.ts");
        if relayer_script.exists() {
            println!("Starting Solana-MeshChain Bridge Relayer daemon...");
            let bridge_dir = PathBuf::from("programs-mesh-bridge");
            thread::spawn(move || loop {
                let res = std::process::Command::new("deno")
                    .arg("run")
                    .arg("--allow-net")
                    .arg("--allow-read")
                    .arg("--allow-write")
                    .arg("scripts/relayer_daemon.ts")
                    .current_dir(&bridge_dir)
                    .output();

                match res {
                    Ok(out) => {
                        if !out.status.success() {
                            let err = String::from_utf8_lossy(&out.stderr);
                            eprintln!("[Relayer] Bridge Daemon error: {}", err);
                        }
                    }
                    Err(e) => {
                        eprintln!("[Relayer] Failed to execute relayer: {}", e);
                    }
                }
                thread::sleep(RELAYER_POLL_INTERVAL);
            });
        }
    }
    let mut finality = FinalityTracker::new();
    let mut mempool = Mempool::new();
    let mut pending: HashMap<String, Block> = HashMap::new();
    // height -> first seen block hash (equivocation detect)
    let mut seen_at_height: HashMap<u64, String> = HashMap::new();

    if observer {
        println!(
            "observer online chain={} listen={} peers={:?} height={} proto={protocol_version}",
            genesis.chain_id, cfg.listen, cfg.peers, state.height
        );
        println!("role: full node (relay only — not in PoA set)");
    } else {
        let idx = my_index.unwrap();
        let kp = keypair.as_ref().unwrap();
        println!(
            "validator {idx} online chain={} listen={} peers={:?} height={} proto={protocol_version}",
            genesis.chain_id, cfg.listen, cfg.peers, state.height
        );
        println!("pubkey {}", hex::encode(kp.public_key()));
    }
    println!("gossip: TCP line-JSON (Tx / Block / BlockAck / Sync* / Blocks*)");
    println!("docs: docs/RUN_A_NODE.md");

    let _ = hub.broadcast(&GossipMsg::Hello {
        validator_index: my_index.unwrap_or(255),
        pubkey_hex: keypair
            .as_ref()
            .map(|k| hex::encode(k.public_key()))
            .unwrap_or_else(|| "observer".into()),
        chain_id: genesis.chain_id.clone(),
        height: state.height,
        protocol_version,
    });
    let mut last_slot = now_secs().saturating_sub(slot_clock_init(&genesis));
    let mut last_sync_req = now_secs().saturating_sub(55);
    let mut last_blocks_req = now_secs().saturating_sub(50);
    let slot_secs = genesis.slot_secs.max(1);

    // Seal genesis (height 0) immediately if we are leader and chain is empty.
    if !observer && state.applied.is_empty() {
        let my_idx = my_index.unwrap();
        if leader_index(0, n) == my_idx {
            if let Some(kp) = keypair.as_ref() {
                match produce_block(&state, kp, my_idx, now_secs(), vec![]) {
                    Ok(block) => {
                        println!("propose height=0 (genesis seal)");
                        let _ = hub.broadcast(&GossipMsg::Block {
                            block: block.clone(),
                        });
                        on_block(
                            &mut state,
                            &mut finality,
                            &mut pending,
                            &mut seen_at_height,
                            &hub,
                            Some(kp),
                            &authorized,
                            block,
                            n,
                            &state_path,
                            &cfg.data_dir,
                        )?;
                    }
                    Err(e) => eprintln!("genesis produce error: {e}"),
                }
            }
        }
    }

    loop {
        while let Some(msg) = hub.try_recv() {
            match msg {
                GossipMsg::Tx { tx } => {
                    if tx.verify().is_ok() && state.can_apply_tx(&tx) {
                        if mempool.insert(tx.clone()) {
                            let _ = hub.broadcast(&GossipMsg::Tx { tx });
                        }
                    }
                }
                GossipMsg::TxAir { tx_bincode_hex } => {
                    // Meshtastic air path: bincode Tx hex → mempool
                    if let Ok(raw) = hex::decode(tx_bincode_hex.trim()) {
                        match Tx::decode(&raw) {
                            Ok(tx) if tx.verify().is_ok() && state.can_apply_tx(&tx) => {
                                let id = tx.txid_hex();
                                if mempool.insert(tx.clone()) {
                                    println!(
                                        "air: accepted Tx {} into mempool",
                                        &id[..16.min(id.len())]
                                    );
                                    let _ = hub.broadcast(&GossipMsg::Tx { tx });
                                }
                            }
                            Ok(_) => eprintln!("air: tx rejected (verify/apply)"),
                            Err(e) => eprintln!("air: bad tx bincode: {e}"),
                        }
                    }
                }
                GossipMsg::BlockAir { block_bincode_hex } => {
                    if let Ok(raw) = hex::decode(block_bincode_hex.trim()) {
                        match Block::decode(&raw) {
                            Ok(block) => {
                                // Air policy: at most 1 tx per block on this path
                                if block.txs.len() > 1 {
                                    eprintln!(
                                        "air: reject multi-tx block ({} txs) — TCP only",
                                        block.txs.len()
                                    );
                                } else {
                                    on_block(
                                        &mut state,
                                        &mut finality,
                                        &mut pending,
                                        &mut seen_at_height,
                                        &hub,
                                        keypair.as_ref(),
                                        &authorized,
                                        block,
                                        n,
                                        &state_path,
                                        &cfg.data_dir,
                                    )?;
                                }
                            }
                            Err(e) => eprintln!("air: bad block bincode: {e}"),
                        }
                    }
                }
                GossipMsg::Tip {
                    chain_id,
                    height,
                    tip_hash_hex,
                } => {
                    if chain_id == state.chain_id && height > state.height {
                        println!(
                            "air tip: peer height={height} tip={} (local {})",
                            &tip_hash_hex[..tip_hash_hex.len().min(16)],
                            state.height
                        );
                        request_blocks_catchup(&hub, &state, &cfg.data_dir);
                    }
                }
                GossipMsg::Block { block } => {
                    on_block(
                        &mut state,
                        &mut finality,
                        &mut pending,
                        &mut seen_at_height,
                        &hub,
                        keypair.as_ref(),
                        &authorized,
                        block,
                        n,
                        &state_path,
                        &cfg.data_dir,
                    )?;
                }
                GossipMsg::BlockAck {
                    block_hash_hex,
                    validator_pubkey_hex,
                    signature_hex,
                    ..
                } => {
                    if let Ok(pk_bytes) = hex::decode(&validator_pubkey_hex) {
                        if pk_bytes.len() == 32 {
                            let mut pk = [0u8; 32];
                            pk.copy_from_slice(&pk_bytes);
                            if authorized.contains(&pk)
                                && verify_block_ack(&pk, &block_hash_hex, &signature_hex)
                            {
                                finality.ack(&block_hash_hex, pk);
                            }
                        }
                    }
                    try_finalize(
                        &mut state,
                        &mut finality,
                        &mut pending,
                        &block_hash_hex,
                        n,
                        &state_path,
                        &cfg.data_dir,
                    )?;
                }
                GossipMsg::Hello {
                    validator_index,
                    height,
                    chain_id,
                    protocol_version: peer_proto,
                    ..
                } => {
                    if peer_proto != 0 && peer_proto != protocol_version {
                        eprintln!(
                            "peer hello v{validator_index} protocol_version={peer_proto} (local {protocol_version}) — may be incompatible"
                        );
                    }
                    println!(
                        "peer hello v{validator_index} height={height} chain={chain_id} peers={}",
                        hub.peer_count()
                    );
                    if chain_id == state.chain_id && height > state.height {
                        request_blocks_catchup(&hub, &state, &cfg.data_dir);
                        last_blocks_req = now_secs();
                        if observer {
                            let _ = hub.broadcast(&GossipMsg::SyncRequest {
                                chain_id: state.chain_id.clone(),
                                have_height: state.height,
                            });
                        }
                    }
                }
                GossipMsg::SyncRequest {
                    chain_id,
                    have_height,
                } => {
                    if chain_id == state.chain_id && state.height > have_height {
                        // Prefer block-range catch-up (safe for producers to re-apply).
                        let blocks =
                            load_finalized_blocks(&cfg.data_dir, have_height.saturating_add(1), 64);
                        if !blocks.is_empty() {
                            let _ = hub.broadcast(&GossipMsg::BlocksResponse {
                                chain_id: state.chain_id.clone(),
                                blocks,
                            });
                        }
                        // Also serve snapshot for observers (apply path is observer-only).
                        if let Ok(state_json) = serde_json::to_string(&state) {
                            if state_json.len() <= crate::sync_validate::MAX_SYNC_JSON_BYTES {
                                let _ = hub.broadcast(&GossipMsg::SyncResponse {
                                    chain_id: state.chain_id.clone(),
                                    height: state.height,
                                    tip_hash_hex: hex::encode(state.tip_hash),
                                    state_json,
                                });
                            }
                        }
                    }
                }
                GossipMsg::SyncResponse {
                    chain_id,
                    height,
                    tip_hash_hex,
                    state_json,
                } => {
                    // Producers never replace live state from gossip snapshots.
                    if !observer {
                        continue;
                    }
                    match crate::sync_validate::accept_sync_snapshot(
                        &state,
                        &chain_id,
                        height,
                        &tip_hash_hex,
                        &state_json,
                    ) {
                        Ok(incoming) => {
                            println!(
                                "sync: applying snapshot {} → {} (catch-up)",
                                state.height, incoming.height
                            );
                            state = incoming;
                            pending.clear();
                            finality = FinalityTracker::new();
                            seen_at_height.clear();
                            if let Err(e) = state.save_json(&state_path) {
                                eprintln!("sync: save failed: {e}");
                            }
                        }
                        Err(e) => {
                            if e != "not ahead of local height" {
                                eprintln!("sync: reject snapshot: {e}");
                            }
                        }
                    }
                }
                GossipMsg::BlocksRequest {
                    chain_id,
                    from_height,
                    max_blocks,
                } => {
                    if chain_id == state.chain_id {
                        let blocks = load_finalized_blocks(&cfg.data_dir, from_height, max_blocks);
                        if !blocks.is_empty() {
                            println!(
                                "blocks: serve {} from height {} (req max={max_blocks})",
                                blocks.len(),
                                from_height
                            );
                            let _ = hub.broadcast(&GossipMsg::BlocksResponse {
                                chain_id: state.chain_id.clone(),
                                blocks,
                            });
                        }
                    }
                }
                GossipMsg::BlocksResponse { chain_id, blocks } => {
                    if chain_id != state.chain_id || blocks.is_empty() {
                        continue;
                    }
                    let mut applied_n = 0u32;
                    for block in blocks {
                        let expected = if state.applied.is_empty() {
                            0
                        } else {
                            state.height + 1
                        };
                        if block.header.height < expected {
                            continue;
                        }
                        if block.header.height > expected {
                            // Gap — stop; will re-request from expected later.
                            break;
                        }
                        if block.verify_producer_sig().is_err() {
                            eprintln!(
                                "blocks: reject height {} bad producer sig",
                                block.header.height
                            );
                            break;
                        }
                        if !authorized.contains(&block.header.producer) {
                            eprintln!(
                                "blocks: reject height {} unknown producer",
                                block.header.height
                            );
                            break;
                        }
                        match state.apply_block(&block) {
                            Ok(()) => {
                                let _ = save_finalized_block(&cfg.data_dir, &block);
                                applied_n += 1;
                            }
                            Err(e) => {
                                eprintln!(
                                    "blocks: apply height {} failed: {e}",
                                    block.header.height
                                );
                                break;
                            }
                        }
                    }
                    if applied_n > 0 {
                        if let Err(e) = state.save_json(&state_path) {
                            eprintln!("blocks: save state failed: {e}");
                        }
                        println!(
                            "blocks: catch-up applied {applied_n} → height={}",
                            state.height
                        );
                        // Clear mempool txs that no longer apply.
                        mempool.retain_valid(&state);
                    }
                }
                GossipMsg::Ping | GossipMsg::Pong => {}
            }
        }

        let now = now_secs();
        // Periodic catch-up for lagging producers and observers.
        if now.saturating_sub(last_blocks_req) >= 15 && hub.peer_count() > 0 {
            last_blocks_req = now;
            request_blocks_catchup(&hub, &state, &cfg.data_dir);
        }
        if now.saturating_sub(last_sync_req) >= 60
            || (observer
                && now.saturating_sub(last_sync_req) >= 5
                && hub.peer_count() > 0
                && state.height == 0)
        {
            last_sync_req = now;
            if hub.peer_count() > 0 {
                let _ = hub.broadcast(&GossipMsg::SyncRequest {
                    chain_id: state.chain_id.clone(),
                    have_height: state.height,
                });
                if observer {
                    println!(
                        "sync: requested snapshot (local height={}, peers={})",
                        state.height,
                        hub.peer_count()
                    );
                }
            }
        }

        // Drop stale mempool entries that no longer apply.
        mempool.enforce_limits(&state, 256);

        let slot_due = now >= last_slot + slot_secs;
        let mempool_boost = !mempool.is_empty();
        let best_fee = mempool.best_fee();

        if !observer && (slot_due || mempool_boost) {
            let next_height = if state.applied.is_empty() {
                0
            } else {
                state.height + 1
            };
            let leader = leader_index(next_height, n);
            let my_idx = my_index.unwrap();
            let kp = keypair.as_ref().unwrap();
            if leader == my_idx {
                let already_applied = state.applied.iter().any(|a| a.height == next_height);
                let already_pending = pending.values().any(|b| b.header.height == next_height);
                if !already_applied && !already_pending {
                    let mut txs = vec![];
                    if next_height > 0 {
                        // Pack highest-fee txs that still apply (multi-tx blocks).
                        txs = mempool.take_applicable_fee_txs(&state, MAX_TXS_PER_BLOCK);
                        if !txs.is_empty() {
                            let fees: u64 = txs.iter().map(|t| t.priority_fee()).sum();
                            if fees > 0 {
                                println!(
                                    "priority include {} tx(s) total_fee={fees} base units",
                                    txs.len()
                                );
                            } else if !slot_due {
                                println!("fast include {} tx(s) (mempool non-empty)", txs.len());
                            }
                        }
                    }
                    let should_produce = !txs.is_empty() || next_height == 0;
                    if should_produce {
                        last_slot = now;
                        match produce_block(&state, kp, my_idx, now, txs) {
                            Ok(block) => {
                                println!(
                                    "propose height={} txs={} peers={} best_fee={best_fee}",
                                    block.header.height,
                                    block.header.tx_count,
                                    hub.peer_count()
                                );
                                let _ = hub.broadcast(&GossipMsg::Block {
                                    block: block.clone(),
                                });
                                on_block(
                                    &mut state,
                                    &mut finality,
                                    &mut pending,
                                    &mut seen_at_height,
                                    &hub,
                                    Some(kp),
                                    &authorized,
                                    block,
                                    n,
                                    &state_path,
                                    &cfg.data_dir,
                                )?;
                            }
                            Err(e) => eprintln!("produce error: {e}"),
                        }
                    } else if slot_due {
                        last_slot = now;
                    }
                }
            } else if slot_due {
                last_slot = now;
            }
        } else if observer && slot_due {
            last_slot = now;
        }

        // Bound finality + equivocation maps
        finality.prune_if_oversized(2_000);
        if seen_at_height.len() > 2_000 {
            let cutoff = state.height.saturating_sub(500);
            seen_at_height.retain(|h, _| *h >= cutoff);
        }

        thread::sleep(Duration::from_millis(cfg.slot_ms.clamp(50, 500)));
    }
}

/// Pretend last slot was long enough ago that the first real slot is due soon (1s grace).
fn slot_clock_init(genesis: &GenesisConfig) -> u64 {
    genesis.slot_secs.saturating_sub(1).max(0)
}

fn request_blocks_catchup(hub: &GossipHub, state: &ChainState, _data_dir: &Path) {
    let from = if state.applied.is_empty() {
        0
    } else {
        state.height.saturating_add(1)
    };
    let _ = hub.broadcast(&GossipMsg::BlocksRequest {
        chain_id: state.chain_id.clone(),
        from_height: from,
        max_blocks: 64,
    });
}

fn on_block(
    state: &mut ChainState,
    finality: &mut FinalityTracker,
    pending: &mut HashMap<String, Block>,
    seen_at_height: &mut HashMap<u64, String>,
    hub: &GossipHub,
    me: Option<&Keypair>,
    authorized: &HashSet<PublicKey>,
    block: Block,
    n_validators: usize,
    state_path: &Path,
    data_dir: &Path,
) -> Result<()> {
    if block.verify_producer_sig().is_err() {
        return Ok(());
    }
    if !authorized.contains(&block.header.producer) {
        return Ok(());
    }
    let hash_hex = block.hash_hex();
    let h = block.header.height;

    // Equivocation: two different blocks at same height from the schedule.
    if let Some(prev) = seen_at_height.get(&h) {
        if prev != &hash_hex {
            eprintln!(
                "EQUIVOCATION height={h}: already saw {prev}, now {hash_hex} — ignoring second"
            );
            return Ok(());
        }
    } else {
        seen_at_height.insert(h, hash_hex.clone());
    }

    if pending.contains_key(&hash_hex) || state.applied.iter().any(|a| a.hash_hex == hash_hex) {
        // still ack below
    } else {
        pending.insert(hash_hex.clone(), block.clone());
        let _ = hub.broadcast(&GossipMsg::Block {
            block: block.clone(),
        });
    }

    // Producer signature on the block counts as their ACK (documented).
    if authorized.contains(&block.header.producer) {
        finality.ack(&hash_hex, block.header.producer);
    }
    if let Some(kp) = me {
        if authorized.contains(&kp.public_key()) {
            finality.ack(&hash_hex, kp.public_key());
            let _ = hub.broadcast(&GossipMsg::BlockAck {
                height: block.header.height,
                block_hash_hex: hash_hex.clone(),
                validator_pubkey_hex: hex::encode(kp.public_key()),
                signature_hex: sign_block_ack(kp, &hash_hex),
            });
        }
    }

    try_finalize(
        state,
        finality,
        pending,
        &hash_hex,
        n_validators,
        state_path,
        data_dir,
    )
}

fn try_finalize(
    state: &mut ChainState,
    finality: &mut FinalityTracker,
    pending: &mut HashMap<String, Block>,
    hash_hex: &str,
    n_validators: usize,
    state_path: &Path,
    data_dir: &Path,
) -> Result<()> {
    if !finality.is_final(hash_hex, n_validators) {
        return Ok(());
    }
    let Some(block) = pending.get(hash_hex).cloned() else {
        return Ok(());
    };
    let expected = if state.applied.is_empty() {
        0
    } else {
        state.height + 1
    };
    if block.header.height != expected {
        // Keep in pending for when we catch up — do NOT drop.
        if block.header.height > expected {
            // Future block: leave pending; block catch-up fills the gap.
            return Ok(());
        }
        // Stale: drop
        pending.remove(hash_hex);
        return Ok(());
    }
    let block = pending.remove(hash_hex).unwrap();
    match state.apply_block(&block) {
        Ok(()) => {
            let _ = save_finalized_block(data_dir, &block);
            state.save_json(state_path).ok();

            // Dump the current registry (Name -> Address mappings) for the TS Relayer to consume
            let registry = Registry::from_state(state);
            let r_path = data_dir.join("registry.json");
            std::fs::write(&r_path, serde_json::to_string_pretty(&registry).unwrap()).ok();
            println!(
                "finalized height={} acks>=threshold supply={}",
                state.height, state.total_supply
            );
            // Prune finality entries for this hash; try next pending at tip+1.
            finality.retain_hashes(|h| h != hash_hex);
            // Attempt finalize any pending that is now next.
            let next_candidates: Vec<String> = pending
                .iter()
                .filter(|(_, b)| {
                    let exp = state.height + 1;
                    b.header.height == exp
                })
                .map(|(h, _)| h.clone())
                .collect();
            for h in next_candidates {
                try_finalize(
                    state,
                    finality,
                    pending,
                    &h,
                    n_validators,
                    state_path,
                    data_dir,
                )?;
            }
        }
        Err(e) => eprintln!("apply_block failed: {e}"),
    }
    Ok(())
}

/// Submit a signed tx JSON file to a validator peer.
pub fn submit_tx_file(tx_path: &Path, peer: &str) -> Result<()> {
    let raw = fs::read_to_string(tx_path)?;
    let tx: Tx = serde_json::from_str(&raw)?;
    tx.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let msg = GossipMsg::Tx { tx };
    let line = serde_json::to_string(&msg)? + "\n";
    let mut stream =
        std::net::TcpStream::connect(peer).with_context(|| format!("connect {peer}"))?;
    use std::io::Write;
    stream.write_all(line.as_bytes())?;
    stream.flush()?;
    println!("submitted tx to {peer}");
    Ok(())
}

/// Submit via Meshtastic air path: send `tx_air` (bincode hex) + standard `tx` + MCHEX frame.
pub fn submit_tx_air(tx_path: &Path, peer: &str) -> Result<()> {
    let raw = fs::read_to_string(tx_path)?;
    let tx: Tx = serde_json::from_str(&raw)?;
    tx.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let bin = tx.encode().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let frame = meshchain_transport::encode_tx(&tx).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let line_air = serde_json::to_string(&GossipMsg::TxAir {
        tx_bincode_hex: hex::encode(&bin),
    })? + "\n";
    let line_tx = serde_json::to_string(&GossipMsg::Tx { tx })? + "\n";
    let mchex = format!("MCHEX {}\n", hex::encode(&frame));
    let mut stream =
        std::net::TcpStream::connect(peer).with_context(|| format!("connect {peer}"))?;
    use std::io::Write;
    stream.write_all(line_air.as_bytes())?;
    stream.write_all(line_tx.as_bytes())?;
    stream.write_all(mchex.as_bytes())?;
    stream.flush()?;
    println!("submitted air+tcp tx to {peer} ({}B MC frame)", frame.len());
    Ok(())
}
