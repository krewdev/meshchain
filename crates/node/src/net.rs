//! Multi-machine gossip over TCP (line-delimited JSON).
//! Optional bridge to Meshtastic via tools/meshtastic_bridge.py (hex frames).

use anyhow::{Context, Result};
use meshchain_proto::block::Block;
use meshchain_proto::tx::Tx;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Reject oversized gossip lines (DoS).
pub const MAX_GOSSIP_LINE_BYTES: usize = 512 * 1024;
/// Soft cap on dedup cache entries.
const MAX_SEEN_ENTRIES: usize = 50_000;
/// Per-peer messages accepted per rolling second.
const PEER_MSG_RATE_PER_SEC: u32 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GossipMsg {
    Hello {
        validator_index: u8,
        pubkey_hex: String,
        chain_id: String,
        height: u64,
        #[serde(default = "default_protocol_version")]
        protocol_version: u32,
    },
    Tx {
        tx: Tx,
    },
    /// Meshtastic path: bincode-encoded Tx as hex (from radio relay).
    #[serde(rename = "tx_air")]
    TxAir {
        tx_bincode_hex: String,
    },
    /// Meshtastic path: bincode-encoded Block as hex (single-tx air blocks).
    #[serde(rename = "block_air")]
    BlockAir {
        block_bincode_hex: String,
    },
    /// Mesh tip advertisement (from radio / tip gossip).
    #[serde(rename = "tip")]
    Tip {
        chain_id: String,
        height: u64,
        #[serde(default)]
        tip_hash_hex: String,
    },
    Block {
        block: Block,
    },
    BlockAck {
        height: u64,
        block_hash_hex: String,
        validator_pubkey_hex: String,
        /// ed25519 signature hex over ack_message(block_hash_hex)
        #[serde(default)]
        signature_hex: String,
    },
    /// Catch-up: ask peer for full chain_state if we are behind (observers).
    SyncRequest {
        chain_id: String,
        have_height: u64,
    },
    /// Full ledger snapshot (JSON text of ChainState) for observers / lagging nodes.
    SyncResponse {
        chain_id: String,
        height: u64,
        /// Must equal hex(state.tip_hash) — rejects tampered payloads.
        #[serde(default)]
        tip_hash_hex: String,
        state_json: String,
    },
    /// Producer-safe catch-up: request finalized blocks from height (inclusive).
    BlocksRequest {
        chain_id: String,
        from_height: u64,
        #[serde(default = "default_max_blocks")]
        max_blocks: u32,
    },
    /// Finalized blocks in height order (cryptographically verifiable via apply_block).
    BlocksResponse {
        chain_id: String,
        blocks: Vec<Block>,
    },
    Ping,
    Pong,
}

fn default_protocol_version() -> u32 {
    1
}

fn default_max_blocks() -> u32 {
    64
}

pub struct GossipHub {
    peers: Arc<Mutex<Vec<PeerConn>>>,
    inbound: Receiver<GossipMsg>,
    inbound_tx: Sender<GossipMsg>,
    #[allow(dead_code)]
    seen: Arc<Mutex<HashSet<String>>>,
}

#[allow(dead_code)]
struct PeerConn {
    addr: String,
    writer: TcpStream,
}

impl GossipHub {
    pub fn start(listen: SocketAddr, bootstrap_peers: Vec<String>) -> Result<Self> {
        let (inbound_tx, inbound) = mpsc::channel();
        let peers = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let rate = Arc::new(Mutex::new(HashMap::<String, (Instant, u32)>::new()));

        // Accept loop
        let peers_acc = peers.clone();
        let tx_acc = inbound_tx.clone();
        let seen_acc = seen.clone();
        let rate_acc = rate.clone();
        thread::spawn(move || {
            let listener = match TcpListener::bind(listen) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("gossip listen failed on {listen}: {e}");
                    return;
                }
            };
            eprintln!("gossip listening on {listen}");
            for stream in listener.incoming().flatten() {
                let _ = stream.set_nodelay(true);
                let peer_addr = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown".into());
                if let Ok(clone) = stream.try_clone() {
                    peers_acc.lock().unwrap().push(PeerConn {
                        addr: peer_addr.clone(),
                        writer: clone,
                    });
                }
                let tx = tx_acc.clone();
                let seen = seen_acc.clone();
                let rate = rate_acc.clone();
                let rate_key = peer_addr.clone();
                thread::spawn(move || read_peer(stream, tx, seen, rate, rate_key));
            }
        });

        // Dial bootstrap peers (retry a few times)
        let peers_dial = peers.clone();
        let tx_dial = inbound_tx.clone();
        let seen_dial = seen.clone();
        let rate_dial = rate.clone();
        thread::spawn(move || {
            for peer in bootstrap_peers {
                for attempt in 0..30 {
                    match TcpStream::connect(&peer) {
                        Ok(stream) => {
                            let _ = stream.set_nodelay(true);
                            if let Ok(clone) = stream.try_clone() {
                                peers_dial.lock().unwrap().push(PeerConn {
                                    addr: peer.clone(),
                                    writer: clone,
                                });
                            }
                            let tx = tx_dial.clone();
                            let seen = seen_dial.clone();
                            let rate = rate_dial.clone();
                            let rate_key = peer.clone();
                            thread::spawn(move || read_peer(stream, tx, seen, rate, rate_key));
                            eprintln!("gossip connected to {peer}");
                            break;
                        }
                        Err(_) => {
                            if attempt == 0 {
                                eprintln!("gossip waiting for peer {peer}…");
                            }
                            thread::sleep(Duration::from_millis(500));
                        }
                    }
                }
            }
        });

        Ok(Self {
            peers,
            inbound,
            inbound_tx,
            seen,
        })
    }

    pub fn try_recv(&self) -> Option<GossipMsg> {
        self.inbound.try_recv().ok()
    }

    pub fn broadcast(&self, msg: &GossipMsg) -> Result<()> {
        let line = serde_json::to_string(msg)? + "\n";
        if line.len() > MAX_GOSSIP_LINE_BYTES {
            anyhow::bail!("gossip message too large ({} bytes)", line.len());
        }
        let bytes = line.as_bytes();
        let mut dead = Vec::new();
        let mut peers = self.peers.lock().unwrap();
        for (i, p) in peers.iter_mut().enumerate() {
            if p.writer.write_all(bytes).is_err() || p.writer.flush().is_err() {
                dead.push(i);
            }
        }
        for i in dead.into_iter().rev() {
            peers.remove(i);
        }
        let _ = &self.inbound_tx;
        Ok(())
    }

    pub fn peer_count(&self) -> usize {
        self.peers.lock().unwrap().len()
    }

    #[allow(dead_code)]
    pub fn mark_seen(&self, id: &str) -> bool {
        let mut seen = self.seen.lock().unwrap();
        if seen.len() > MAX_SEEN_ENTRIES {
            seen.clear();
        }
        seen.insert(id.to_string())
    }
}

fn rate_allow(rate: &Mutex<HashMap<String, (Instant, u32)>>, key: &str) -> bool {
    let mut map = match rate.lock() {
        Ok(m) => m,
        Err(_) => return true,
    };
    let now = Instant::now();
    let entry = map.entry(key.to_string()).or_insert((now, 0));
    if now.duration_since(entry.0) >= Duration::from_secs(1) {
        *entry = (now, 1);
        return true;
    }
    if entry.1 >= PEER_MSG_RATE_PER_SEC {
        return false;
    }
    entry.1 += 1;
    true
}

fn read_peer(
    stream: TcpStream,
    tx: Sender<GossipMsg>,
    seen: Arc<Mutex<HashSet<String>>>,
    rate: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
    rate_key: String,
) {
    let reader = BufReader::new(stream);
    for line in reader.lines().flatten() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_GOSSIP_LINE_BYTES {
            eprintln!(
                "gossip: drop oversized line ({} bytes) from {rate_key}",
                line.len()
            );
            continue;
        }
        if !rate_allow(&rate, &rate_key) {
            continue;
        }
        match serde_json::from_str::<GossipMsg>(line) {
            Ok(msg) => {
                let id = match &msg {
                    GossipMsg::Tx { tx } => format!("tx:{}", tx.txid_hex()),
                    GossipMsg::TxAir { tx_bincode_hex } => {
                        format!("txair:{}", &tx_bincode_hex[..tx_bincode_hex.len().min(32)])
                    }
                    GossipMsg::BlockAir { block_bincode_hex } => {
                        format!(
                            "blkair:{}",
                            &block_bincode_hex[..block_bincode_hex.len().min(32)]
                        )
                    }
                    GossipMsg::Tip {
                        chain_id,
                        height,
                        tip_hash_hex,
                    } => format!("tip:{chain_id}:{height}:{tip_hash_hex}"),
                    GossipMsg::Block { block } => format!("blk:{}", block.hash_hex()),
                    GossipMsg::BlockAck {
                        block_hash_hex,
                        validator_pubkey_hex,
                        signature_hex,
                        ..
                    } => format!("ack:{block_hash_hex}:{validator_pubkey_hex}:{signature_hex}"),
                    GossipMsg::Hello {
                        pubkey_hex, height, ..
                    } => format!("hello:{pubkey_hex}:{height}"),
                    GossipMsg::SyncRequest {
                        chain_id,
                        have_height,
                    } => format!("syncreq:{chain_id}:{have_height}"),
                    GossipMsg::SyncResponse {
                        chain_id,
                        height,
                        tip_hash_hex,
                        ..
                    } => format!("syncres:{chain_id}:{height}:{tip_hash_hex}"),
                    GossipMsg::BlocksRequest {
                        chain_id,
                        from_height,
                        max_blocks,
                    } => format!("blksreq:{chain_id}:{from_height}:{max_blocks}"),
                    GossipMsg::BlocksResponse { chain_id, blocks } => {
                        let tip = blocks.last().map(|b| b.header.height).unwrap_or(0);
                        format!("blksres:{chain_id}:{tip}:{}", blocks.len())
                    }
                    GossipMsg::Ping => continue,
                    GossipMsg::Pong => continue,
                };
                let mut seen_g = seen.lock().unwrap();
                if seen_g.len() > MAX_SEEN_ENTRIES {
                    seen_g.clear();
                }
                if seen_g.insert(id) {
                    drop(seen_g);
                    let _ = tx.send(msg);
                }
            }
            Err(e) => eprintln!("gossip decode error: {e}"),
        }
    }
}

use meshchain_transport::frag::{decode_frag_nack, encode_frag_nack, FragAssembler, FragCache};
use meshchain_transport::frame::{decode_frame, MsgType};

/// Spawn meshtastic_bridge.py and forward TXHEX/RXHEX as raw frame bytes callbacks.
#[allow(dead_code)]
pub struct MeshRadioBridge {
    pub _marker: (),
}

impl MeshRadioBridge {
    #[allow(dead_code)]
    pub fn spawn_bridge_process(script: &str, port: &str) -> Result<std::process::Child> {
        let child = std::process::Command::new("python3")
            .arg(script)
            .arg("--port")
            .arg(port)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .context("spawn meshtastic_bridge.py")?;
        Ok(child)
    }
}

/// High-level radio network manager that maintains fragment reassembly, retransmission caching (`FragCache`),
/// and selective ARQ (`FragNack`) for LoRa node gossip.
pub struct MeshRadioHub {
    pub assembler: FragAssembler,
    pub cache: FragCache,
}

impl MeshRadioHub {
    pub fn new() -> Self {
        Self {
            assembler: FragAssembler::new(),
            cache: FragCache::new(),
        }
    }

    /// Process raw incoming LoRa wire bytes.
    /// Returns `Ok(Some(completed_payload))` when a multi-chunk session finishes reassembly.
    /// Returns `Ok(None)` if more chunks are needed or if handled internally (e.g. NACK retransmissions generated).
    pub fn process_incoming_raw(
        &mut self,
        raw_frame: &[u8],
    ) -> Result<(Option<Vec<u8>>, Vec<Vec<u8>>)> {
        let mut outbound_frames = Vec::new();
        let frame = match decode_frame(raw_frame) {
            Ok(f) => f,
            Err(_) => return Ok((None, outbound_frames)),
        };

        match frame.msg_type {
            MsgType::Frag => {
                if let Ok(Some(full_payload)) = self.assembler.push_frame(&frame) {
                    return Ok((Some(full_payload), outbound_frames));
                }
            }
            MsgType::FragNack => {
                if let Ok((sid, missing)) = decode_frag_nack(&frame.payload) {
                    let retrans = self.cache.handle_nack(&sid, &missing);
                    outbound_frames.extend(retrans);
                }
            }
            _ => {}
        }
        Ok((None, outbound_frames))
    }

    /// Check for timed-out reassembly sessions (`timeout_secs`) and generate outbound `FragNack` wire frames.
    pub fn check_timeouts_and_generate_nacks(&mut self, timeout_secs: u64) -> Vec<Vec<u8>> {
        let nacks = self.assembler.check_missing_chunks(timeout_secs);
        let mut frames = Vec::with_capacity(nacks.len());
        for (sid, missing) in nacks {
            if let Ok(frame_bytes) = encode_frag_nack(sid, &missing) {
                frames.push(frame_bytes);
            }
        }
        frames
    }

    /// Cache fragmented wire frames before transmitting over the air for future selective NACK recovery.
    pub fn cache_outbound_session(&mut self, session_id: [u8; 8], frames: Vec<Vec<u8>>) {
        self.cache.insert(session_id, frames);
    }

    /// Prune expired sessions from both assembler and cache.
    #[allow(dead_code)]
    pub fn prune(&mut self) {
        self.cache.prune();
    }
}

impl Default for MeshRadioHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meshchain_transport::frag::{fragment_bytes, session_id_from_hash};

    #[test]
    fn test_meshradiohub_nack_retransmission_flow() {
        let mut sender_hub = MeshRadioHub::new();
        let mut receiver_hub = MeshRadioHub::new();

        let data: Vec<u8> = (0..800).map(|i| (i % 256) as u8).collect();
        let sid = session_id_from_hash(&data);
        let frames = fragment_bytes(sid, &data).unwrap();
        assert!(frames.len() >= 2);

        // Sender caches outbound frames
        sender_hub.cache_outbound_session(sid, frames.clone());

        // Receiver gets chunk 0, but drops all other chunks
        let (res, outbound) = receiver_hub.process_incoming_raw(&frames[0]).unwrap();
        assert!(res.is_none());
        assert!(outbound.is_empty());

        // Timeout triggers NACK frame from receiver
        let nack_frames = receiver_hub.check_timeouts_and_generate_nacks(0);
        assert_eq!(nack_frames.len(), 1);

        // Sender receives NACK frame and produces retransmissions
        let (res2, retrans_frames) = sender_hub.process_incoming_raw(&nack_frames[0]).unwrap();
        assert!(res2.is_none());
        assert_eq!(retrans_frames.len(), frames.len() - 1);

        // Receiver processes retransmissions and finishes assembly
        let mut completed = None;
        for retrans in retrans_frames {
            let (res3, _) = receiver_hub.process_incoming_raw(&retrans).unwrap();
            if let Some(data) = res3 {
                completed = Some(data);
            }
        }
        assert_eq!(completed.unwrap(), data);
    }
}
