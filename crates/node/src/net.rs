//! Multi-machine gossip over TCP (line-delimited JSON).
//! Optional bridge to Meshtastic via tools/meshtastic_bridge.py (hex frames).

use anyhow::{Context, Result};
use meshchain_proto::block::Block;
use meshchain_proto::tx::Tx;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GossipMsg {
    Hello {
        validator_index: u8,
        pubkey_hex: String,
        chain_id: String,
        height: u64,
    },
    Tx {
        tx: Tx,
    },
    Block {
        block: Block,
    },
    BlockAck {
        height: u64,
        block_hash_hex: String,
        validator_pubkey_hex: String,
    },
    /// Catch-up: ask peer for full chain_state if we are behind.
    SyncRequest {
        chain_id: String,
        have_height: u64,
    },
    /// Full ledger snapshot (JSON text of ChainState) for observers / lagging nodes.
    SyncResponse {
        chain_id: String,
        height: u64,
        state_json: String,
    },
    Ping,
    Pong,
}

pub struct GossipHub {
    peers: Arc<Mutex<Vec<PeerConn>>>,
    inbound: Receiver<GossipMsg>,
    inbound_tx: Sender<GossipMsg>,
    seen: Arc<Mutex<HashSet<String>>>,
}

struct PeerConn {
    addr: String,
    writer: TcpStream,
}

impl GossipHub {
    pub fn start(listen: SocketAddr, bootstrap_peers: Vec<String>) -> Result<Self> {
        let (inbound_tx, inbound) = mpsc::channel();
        let peers = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::new(Mutex::new(HashSet::new()));

        // Accept loop
        let peers_acc = peers.clone();
        let tx_acc = inbound_tx.clone();
        let seen_acc = seen.clone();
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
                thread::spawn(move || read_peer(stream, tx, seen));
            }
        });

        // Dial bootstrap peers (retry a few times)
        let peers_dial = peers.clone();
        let tx_dial = inbound_tx.clone();
        let seen_dial = seen.clone();
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
                            thread::spawn(move || read_peer(stream, tx, seen));
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
        // Also inject locally so single-node still "hears" own messages if needed — no, avoid loops
        let _ = &self.inbound_tx;
        Ok(())
    }

    pub fn peer_count(&self) -> usize {
        self.peers.lock().unwrap().len()
    }

    pub fn mark_seen(&self, id: &str) -> bool {
        self.seen.lock().unwrap().insert(id.to_string())
    }
}

fn read_peer(stream: TcpStream, tx: Sender<GossipMsg>, seen: Arc<Mutex<HashSet<String>>>) {
    let reader = BufReader::new(stream);
    for line in reader.lines().flatten() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<GossipMsg>(line) {
            Ok(msg) => {
                let id = match &msg {
                    GossipMsg::Tx { tx } => format!("tx:{}", tx.txid_hex()),
                    GossipMsg::Block { block } => format!("blk:{}", block.hash_hex()),
                    GossipMsg::BlockAck {
                        block_hash_hex,
                        validator_pubkey_hex,
                        ..
                    } => format!("ack:{block_hash_hex}:{validator_pubkey_hex}"),
                    GossipMsg::Hello {
                        pubkey_hex,
                        height,
                        ..
                    } => format!("hello:{pubkey_hex}:{height}"),
                    GossipMsg::SyncRequest {
                        chain_id,
                        have_height,
                    } => format!("syncreq:{chain_id}:{have_height}"),
                    GossipMsg::SyncResponse {
                        chain_id,
                        height,
                        ..
                    } => format!("syncres:{chain_id}:{height}"),
                    GossipMsg::Ping => continue,
                    GossipMsg::Pong => continue,
                };
                if seen.lock().unwrap().insert(id) {
                    let _ = tx.send(msg);
                }
            }
            Err(e) => eprintln!("gossip decode error: {e}"),
        }
    }
}

/// Spawn meshtastic_bridge.py and forward TXHEX/RXHEX as raw frame bytes callbacks.
pub struct MeshRadioBridge {
    // kept for future bidirectional frame path
    pub _marker: (),
}

impl MeshRadioBridge {
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
