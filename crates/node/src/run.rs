//! Multi-machine validator run loop (TCP gossip + Meshtastic-ready framing notes).

use crate::consensus::{leader_index, produce_block, FinalityTracker};
use crate::net::{GossipHub, GossipMsg};
use anyhow::{bail, Context, Result};
use meshchain_ledger::genesis::GenesisConfig;
use meshchain_ledger::state::ChainState;
use meshchain_proto::block::Block;
use meshchain_proto::crypto::Keypair;
use meshchain_proto::tx::Tx;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct RunConfig {
    pub data_dir: std::path::PathBuf,
    pub validator_index: u8,
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

pub fn run_validator(cfg: RunConfig) -> Result<()> {
    let genesis_path = cfg.data_dir.join("genesis.json");
    let genesis: GenesisConfig = serde_json::from_str(
        &fs::read_to_string(&genesis_path)
            .with_context(|| format!("missing {}", genesis_path.display()))?,
    )?;
    let n = genesis.validators.len();
    if cfg.validator_index as usize >= n {
        bail!(
            "validator-index {} out of range (have {n} validators in genesis)",
            cfg.validator_index
        );
    }

    let key_path = cfg
        .data_dir
        .join("keys")
        .join(format!("validator-{}.json", cfg.validator_index));
    let key_file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(&key_path)?)?;
    let keypair = Keypair::from_file(&key_file).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let expected = hex::decode(&genesis.validators[cfg.validator_index as usize])?;
    if keypair.public_key().as_slice() != expected.as_slice() {
        bail!(
            "validator key does not match genesis validators[{}]",
            cfg.validator_index
        );
    }

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

    println!(
        "validator {} online chain={} listen={} peers={:?} height={}",
        cfg.validator_index,
        genesis.chain_id,
        cfg.listen,
        cfg.peers,
        state.height
    );
    println!("pubkey {}", hex::encode(keypair.public_key()));
    println!("gossip: TCP line-JSON (Tx / Block / BlockAck)");
    println!("Meshtastic: use tools/meshtastic_bridge.py for MC frames on radios");

    let _ = hub.broadcast(&GossipMsg::Hello {
        validator_index: cfg.validator_index,
        pubkey_hex: hex::encode(keypair.public_key()),
        chain_id: genesis.chain_id.clone(),
        height: state.height,
    });

    let mut last_slot = now_secs();
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
                        &keypair,
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
                    if let Ok(pk_bytes) = hex::decode(&validator_pubkey_hex) {
                        if pk_bytes.len() == 32 {
                            let mut pk = [0u8; 32];
                            pk.copy_from_slice(&pk_bytes);
                            finality.ack(&block_hash_hex, pk);
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
                }
                GossipMsg::Ping | GossipMsg::Pong => {}
            }
        }

        let now = now_secs();
        if now >= last_slot + slot_secs {
            last_slot = now;
            let next_height = if state.applied.is_empty() {
                0
            } else {
                state.height + 1
            };
            let leader = leader_index(next_height, n);
            if leader == cfg.validator_index {
                let already_applied = state.applied.iter().any(|a| a.height == next_height);
                let already_pending = pending.values().any(|b| b.header.height == next_height);
                if !already_applied && !already_pending {
                    let mut txs = vec![];
                    if next_height > 0 {
                        if let Some(tx) = mempool.pop_front() {
                            txs.push(tx);
                        }
                    }
                    match produce_block(&state, &keypair, cfg.validator_index, now, txs) {
                        Ok(block) => {
                            println!(
                                "propose height={} txs={} peers={}",
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
                                &keypair,
                                block,
                                n,
                                &state_path,
                            )?;
                        }
                        Err(e) => eprintln!("produce error: {e}"),
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(cfg.slot_ms.clamp(50, 500)));
    }
}

fn on_block(
    state: &mut ChainState,
    finality: &mut FinalityTracker,
    pending: &mut HashMap<String, Block>,
    hub: &GossipHub,
    me: &Keypair,
    block: Block,
    n_validators: usize,
    state_path: &Path,
) -> Result<()> {
    if block.verify_producer_sig().is_err() {
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

    finality.ack(&hash_hex, me.public_key());
    finality.ack(&hash_hex, block.header.producer);
    let _ = hub.broadcast(&GossipMsg::BlockAck {
        height: block.header.height,
        block_hash_hex: hash_hex.clone(),
        validator_pubkey_hex: hex::encode(me.public_key()),
    });

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
