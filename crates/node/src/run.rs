//! Multi-machine validator run loop (TCP gossip + Meshtastic-ready framing notes).

use crate::consensus::{leader_index, produce_block, FinalityTracker};
use crate::net::{GossipHub, GossipMsg};
use anyhow::{bail, Context, Result};
use meshchain_ledger::genesis::GenesisConfig;
use meshchain_ledger::state::ChainState;
use meshchain_proto::block::Block;
use meshchain_proto::crypto::{Keypair, PublicKey};
use meshchain_proto::tx::Tx;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
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

pub fn run_validator(cfg: RunConfig) -> Result<()> {
    let genesis_path = cfg.data_dir.join("genesis.json");
    let genesis: GenesisConfig = serde_json::from_str(
        &fs::read_to_string(&genesis_path)
            .with_context(|| format!("missing {}", genesis_path.display()))?,
    )?;
    let n = genesis.validators.len();
    let authorized = genesis_validator_set(&genesis)?;

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
    let mut finality = FinalityTracker::new();
    let mut mempool: VecDeque<Tx> = VecDeque::new();
    let mut pending: HashMap<String, Block> = HashMap::new();

    if observer {
        println!(
            "observer online chain={} listen={} peers={:?} height={}",
            genesis.chain_id, cfg.listen, cfg.peers, state.height
        );
        println!("role: full node (relay only — not in PoA set)");
    } else {
        let idx = my_index.unwrap();
        let kp = keypair.as_ref().unwrap();
        println!(
            "validator {idx} online chain={} listen={} peers={:?} height={}",
            genesis.chain_id, cfg.listen, cfg.peers, state.height
        );
        println!("pubkey {}", hex::encode(kp.public_key()));
    }
    println!("gossip: TCP line-JSON (Tx / Block / BlockAck / Sync*)");
    println!("docs: docs/RUN_A_NODE.md");

    let _ = hub.broadcast(&GossipMsg::Hello {
        validator_index: my_index.unwrap_or(255),
        pubkey_hex: keypair
            .as_ref()
            .map(|k| hex::encode(k.public_key()))
            .unwrap_or_else(|| "observer".into()),
        chain_id: genesis.chain_id.clone(),
        height: state.height,
    });
    // Ask seeds for catch-up immediately (observers and lagging producers).
    let _ = hub.broadcast(&GossipMsg::SyncRequest {
        chain_id: genesis.chain_id.clone(),
        have_height: state.height,
    });

    let mut last_slot = now_secs();
    let mut last_sync_req = now_secs();
    let slot_secs = genesis.slot_secs.max(1);

    loop {
        while let Some(msg) = hub.try_recv() {
            match msg {
                GossipMsg::Tx { tx } => {
                    if tx.verify().is_ok() {
                        let id = tx.txid_hex();
                        if !mempool.iter().any(|t| t.txid_hex() == id) {
                            mempool.push_back(tx.clone());
                            let _ = hub.broadcast(&GossipMsg::Tx { tx });
                        }
                    }
                }
                GossipMsg::Block { block } => {
                    on_block(
                        &mut state,
                        &mut finality,
                        &mut pending,
                        &hub,
                        keypair.as_ref(),
                        &authorized,
                        block,
                        n,
                        &state_path,
                    )?;
                }
                GossipMsg::BlockAck {
                    block_hash_hex,
                    validator_pubkey_hex,
                    ..
                } => {
                    // Only genesis validators count toward finality (ignore observer noise).
                    if let Ok(pk_bytes) = hex::decode(&validator_pubkey_hex) {
                        if pk_bytes.len() == 32 {
                            let mut pk = [0u8; 32];
                            pk.copy_from_slice(&pk_bytes);
                            if authorized.contains(&pk) {
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
                    )?;
                }
                GossipMsg::Hello {
                    validator_index,
                    height,
                    chain_id,
                    ..
                } => {
                    println!(
                        "peer hello v{validator_index} height={height} chain={chain_id} peers={}",
                        hub.peer_count()
                    );
                    // Catch-up if peer is ahead
                    if chain_id == state.chain_id && height > state.height {
                        let _ = hub.broadcast(&GossipMsg::SyncRequest {
                            chain_id: state.chain_id.clone(),
                            have_height: state.height,
                        });
                    }
                }
                GossipMsg::SyncRequest {
                    chain_id,
                    have_height,
                } => {
                    if chain_id == state.chain_id && state.height > have_height {
                        if let Ok(state_json) = serde_json::to_string(&state) {
                            let _ = hub.broadcast(&GossipMsg::SyncResponse {
                                chain_id: state.chain_id.clone(),
                                height: state.height,
                                state_json,
                            });
                            println!(
                                "sync: served snapshot height={} to peer (they had {have_height})",
                                state.height
                            );
                        }
                    }
                }
                GossipMsg::SyncResponse {
                    chain_id,
                    height,
                    state_json,
                } => {
                    if chain_id != state.chain_id {
                        continue;
                    }
                    if height <= state.height {
                        continue;
                    }
                    match serde_json::from_str::<ChainState>(&state_json) {
                        Ok(incoming) => {
                            if incoming.chain_id != state.chain_id {
                                continue;
                            }
                            if incoming.validators != state.validators {
                                eprintln!("sync: reject snapshot (validator set mismatch)");
                                continue;
                            }
                            println!(
                                "sync: applying snapshot {} → {} (catch-up)",
                                state.height, incoming.height
                            );
                            state = incoming;
                            pending.clear();
                            finality = FinalityTracker::new();
                            if let Err(e) = state.save_json(&state_path) {
                                eprintln!("sync: save failed: {e}");
                            }
                        }
                        Err(e) => eprintln!("sync: bad snapshot: {e}"),
                    }
                }
                GossipMsg::Ping | GossipMsg::Pong => {}
                // Future: block_hint from radio relay ignored
            }
        }

        let now = now_secs();
        // Observers / lagging nodes re-request sync every 60s if still alone-ish
        if now.saturating_sub(last_sync_req) >= 60 {
            last_sync_req = now;
            let _ = hub.broadcast(&GossipMsg::SyncRequest {
                chain_id: state.chain_id.clone(),
                have_height: state.height,
            });
        }
        let slot_due = now >= last_slot + slot_secs;
        // Fast path: any pending tx can be proposed without waiting full slot_secs.
        // Higher fees still win ordering (MEV-style tip auction).
        let mempool_boost = !mempool.is_empty();
        let best_fee = mempool
            .iter()
            .map(|t| t.priority_fee())
            .max()
            .unwrap_or(0);

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
                        // Highest priority fee first (then txid for deterministic tie-break).
                        if let Some(tx) = take_highest_fee_tx(&mut mempool) {
                            if tx.priority_fee() > 0 {
                                println!(
                                    "priority include fee={} base units (tip → producer)",
                                    tx.priority_fee()
                                );
                            } else if !slot_due {
                                println!("fast include (mempool non-empty, fee=0)");
                            }
                            txs.push(tx);
                        }
                    }
                    // Skip empty blocks when idle (stops block-reward inflation spam).
                    // Still produce empty block at height 0 (genesis seal) if needed.
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
                                    &hub,
                                    Some(kp),
                                    &authorized,
                                    block,
                                    n,
                                    &state_path,
                                )?;
                            }
                            Err(e) => eprintln!("produce error: {e}"),
                        }
                    } else if slot_due {
                        // Idle tick: advance clock without minting empty blocks.
                        last_slot = now;
                    }
                }
            } else if slot_due {
                // Non-leaders still advance their local slot clock so they don't spin.
                last_slot = now;
            }
        } else if observer && slot_due {
            last_slot = now;
        }

        thread::sleep(Duration::from_millis(cfg.slot_ms.clamp(50, 500)));
    }
}

/// Pop the highest-fee tx (MEV priority auction). Ties broken by txid hex (stable).
fn take_highest_fee_tx(mempool: &mut VecDeque<Tx>) -> Option<Tx> {
    if mempool.is_empty() {
        return None;
    }
    let mut best_i = 0usize;
    let mut best_fee = mempool[0].priority_fee();
    let mut best_id = mempool[0].txid_hex();
    for (i, tx) in mempool.iter().enumerate().skip(1) {
        let fee = tx.priority_fee();
        let id = tx.txid_hex();
        if fee > best_fee || (fee == best_fee && id < best_id) {
            best_i = i;
            best_fee = fee;
            best_id = id;
        }
    }
    mempool.remove(best_i)
}

fn on_block(
    state: &mut ChainState,
    finality: &mut FinalityTracker,
    pending: &mut HashMap<String, Block>,
    hub: &GossipHub,
    me: Option<&Keypair>,
    authorized: &HashSet<PublicKey>,
    block: Block,
    n_validators: usize,
    state_path: &Path,
) -> Result<()> {
    if block.verify_producer_sig().is_err() {
        return Ok(());
    }
    // Producer must be a genesis validator.
    if !authorized.contains(&block.header.producer) {
        return Ok(());
    }
    let hash_hex = block.hash_hex();
    if pending.contains_key(&hash_hex)
        || state.applied.iter().any(|a| a.hash_hex == hash_hex)
    {
        // still ack if we haven't
    } else {
        pending.insert(hash_hex.clone(), block.clone());
        let _ = hub.broadcast(&GossipMsg::Block {
            block: block.clone(),
        });
    }

    // Producer ACK counts; only genesis validators emit BlockAck.
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
            });
        }
    }

    try_finalize(state, finality, pending, &hash_hex, n_validators, state_path)
}

fn try_finalize(
    state: &mut ChainState,
    finality: &mut FinalityTracker,
    pending: &mut HashMap<String, Block>,
    hash_hex: &str,
    n_validators: usize,
    state_path: &Path,
) -> Result<()> {
    if !finality.is_final(hash_hex, n_validators) {
        return Ok(());
    }
    let Some(block) = pending.remove(hash_hex) else {
        return Ok(());
    };
    let expected = if state.applied.is_empty() {
        0
    } else {
        state.height + 1
    };
    if block.header.height != expected {
        // put back if future? drop for simplicity
        return Ok(());
    }
    match state.apply_block(&block) {
        Ok(()) => {
            state.save_json(state_path).ok();
            println!(
                "finalized height={} acks>=threshold supply={}",
                state.height, state.total_supply
            );
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
