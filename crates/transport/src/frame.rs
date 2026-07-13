//! Compact mesh frames for Meshtastic (~237 byte payload budget).
//!
//! Wire: MAGIC[2] | ver[1] | msg_type[1] | len[u16 LE] | payload[len]
//! Total header = 6 bytes.
//!
//! **Air policy (v1):**
//! - Everyday `Tx` (ed25519 only) → single LoRa frame when ≤ MAX_PAYLOAD
//! - Full multi-tx blocks stay on **TCP only**
//! - Over air: `Tip` + optional single-tx `Block` + signed `BlockAck`
//! - Internet still used for faucet / scanner / Solana vault

use meshchain_proto::block::Block;
use meshchain_proto::tx::Tx;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FRAME_MAGIC: &[u8; 2] = b"MC";
pub const FRAME_VERSION: u8 = 1;
pub const FRAME_VERSION_V2: u8 = 2;
pub const HEADER_LEN: usize = 6;
pub const HEADER_LEN_V2: usize = 7;
pub const DEFAULT_HOP_LIMIT: u8 = 3;
/// Conservative max payload under Meshtastic LoRa limits.
pub const MAX_PAYLOAD: usize = 200;
/// Max txs sealed into a block when that block is also offered over air.
pub const AIR_MAX_TXS_PER_BLOCK: usize = 1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Tx = 1,
    Block = 2,
    BlockAck = 3,
    SyncReq = 4,
    SyncResp = 5,
    /// Fragment of a large payload (PQ sig / PqSigned envelope).
    Frag = 6,
    /// Chain tip advertisement (height + tip hash) — mesh gossip without full state.
    Tip = 7,
    /// Compact block hint (height + hash) when full block won't fit air.
    BlockHint = 8,
    /// Selective retransmission request (`session_id[8] | count[u16] | idx0[u16]...`).
    FragNack = 9,
    /// Compressed envelope (`inner_type[1] | decomp_len[u16 LE] | deflate_data...`).
    Compressed = 11,
    /// JSON control / bridge hint (relayer)
    Control = 10,
    /// Legacy: full JSON gossip blob (radio relay MSG_GOSSIP=20)
    GossipJson = 20,
}

impl MsgType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Tx),
            2 => Some(Self::Block),
            3 => Some(Self::BlockAck),
            4 => Some(Self::SyncReq),
            5 => Some(Self::SyncResp),
            6 => Some(Self::Frag),
            7 => Some(Self::Tip),
            8 => Some(Self::BlockHint),
            9 => Some(Self::FragNack),
            11 => Some(Self::Compressed),
            10 => Some(Self::Control),
            20 => Some(Self::GossipJson),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
    pub hop_limit: u8,
}

impl Frame {
    pub fn new(msg_type: MsgType, payload: Vec<u8>) -> Self {
        Self {
            msg_type,
            payload,
            hop_limit: DEFAULT_HOP_LIMIT,
        }
    }
}

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("payload too large: {0} > {MAX_PAYLOAD}")]
    TooLarge(usize),
    #[error("bad magic")]
    BadMagic,
    #[error("bad version {0}")]
    BadVersion(u8),
    #[error("unknown msg type {0}")]
    UnknownType(u8),
    #[error("truncated")]
    Truncated,
    #[error("codec: {0}")]
    Codec(String),
}

pub fn encode_frame(msg_type: MsgType, payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(FrameError::TooLarge(payload.len()));
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(FRAME_MAGIC);
    out.push(FRAME_VERSION);
    out.push(msg_type as u8);
    let len = payload.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

pub fn encode_frame_with_hops(msg_type: MsgType, payload: &[u8], hop_limit: u8) -> Result<Vec<u8>, FrameError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(FrameError::TooLarge(payload.len()));
    }
    let mut out = Vec::with_capacity(HEADER_LEN_V2 + payload.len());
    out.extend_from_slice(FRAME_MAGIC);
    out.push(FRAME_VERSION_V2);
    out.push(msg_type as u8);
    out.push(hop_limit);
    let len = payload.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

pub fn decode_frame(data: &[u8]) -> Result<Frame, FrameError> {
    if data.len() < HEADER_LEN {
        return Err(FrameError::Truncated);
    }
    if &data[0..2] != FRAME_MAGIC {
        return Err(FrameError::BadMagic);
    }
    let ver = data[2];
    if ver == FRAME_VERSION {
        let msg_type = MsgType::from_u8(data[3]).ok_or(FrameError::UnknownType(data[3]))?;
        let len = u16::from_le_bytes([data[4], data[5]]) as usize;
        if data.len() < HEADER_LEN + len {
            return Err(FrameError::Truncated);
        }
        Ok(Frame {
            msg_type,
            payload: data[HEADER_LEN..HEADER_LEN + len].to_vec(),
            hop_limit: DEFAULT_HOP_LIMIT,
        })
    } else if ver == FRAME_VERSION_V2 {
        if data.len() < HEADER_LEN_V2 {
            return Err(FrameError::Truncated);
        }
        let msg_type = MsgType::from_u8(data[3]).ok_or(FrameError::UnknownType(data[3]))?;
        let hop_limit = data[4];
        let len = u16::from_le_bytes([data[5], data[6]]) as usize;
        if data.len() < HEADER_LEN_V2 + len {
            return Err(FrameError::Truncated);
        }
        Ok(Frame {
            msg_type,
            payload: data[HEADER_LEN_V2..HEADER_LEN_V2 + len].to_vec(),
            hop_limit,
        })
    } else {
        Err(FrameError::BadVersion(ver))
    }
}

pub fn encode_tx(tx: &Tx) -> Result<Vec<u8>, FrameError> {
    let body = tx
        .encode()
        .map_err(|e| FrameError::Codec(e.to_string()))?;
    encode_frame(MsgType::Tx, &body)
}

pub fn decode_tx(frame: &Frame) -> Result<Tx, FrameError> {
    if frame.msg_type != MsgType::Tx {
        return Err(FrameError::Codec("not a tx frame".into()));
    }
    Tx::decode(&frame.payload).map_err(|e| FrameError::Codec(e.to_string()))
}

pub fn encode_block(block: &Block) -> Result<Vec<u8>, FrameError> {
    let body = block
        .encode()
        .map_err(|e| FrameError::Codec(e.to_string()))?;
    encode_frame(MsgType::Block, &body)
}

pub fn decode_block(frame: &Frame) -> Result<Block, FrameError> {
    if frame.msg_type != MsgType::Block {
        return Err(FrameError::Codec("not a block frame".into()));
    }
    Block::decode(&frame.payload).map_err(|e| FrameError::Codec(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAckPayload {
    pub height: u64,
    pub block_hash_hex: String,
    pub validator_pubkey_hex: String,
    /// ed25519 signature hex (optional for legacy frames)
    #[serde(default)]
    pub signature_hex: String,
}

/// Compact tip for LoRa: nodes learn height without a full SyncResponse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TipPayload {
    pub chain_id: String,
    pub height: u64,
    /// 32-byte tip hash as hex
    pub tip_hash_hex: String,
}

/// When a full block exceeds air MTU — advertise tip only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHintPayload {
    pub height: u64,
    pub hash_hex: String,
    pub producer_index: u8,
    pub tx_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMsg {
    pub kind: String,
    pub body: serde_json::Value,
}

pub fn encode_tip(tip: &TipPayload) -> Result<Vec<u8>, FrameError> {
    let body = bincode::serialize(tip).map_err(|e| FrameError::Codec(e.to_string()))?;
    encode_frame(MsgType::Tip, &body)
}

pub fn decode_tip(frame: &Frame) -> Result<TipPayload, FrameError> {
    if frame.msg_type != MsgType::Tip {
        return Err(FrameError::Codec("not a tip frame".into()));
    }
    bincode::deserialize(&frame.payload).map_err(|e| FrameError::Codec(e.to_string()))
}

pub fn encode_block_hint(hint: &BlockHintPayload) -> Result<Vec<u8>, FrameError> {
    let body = bincode::serialize(hint).map_err(|e| FrameError::Codec(e.to_string()))?;
    encode_frame(MsgType::BlockHint, &body)
}

pub fn decode_block_hint(frame: &Frame) -> Result<BlockHintPayload, FrameError> {
    if frame.msg_type != MsgType::BlockHint {
        return Err(FrameError::Codec("not a block_hint frame".into()));
    }
    bincode::deserialize(&frame.payload).map_err(|e| FrameError::Codec(e.to_string()))
}

/// True if this block is small enough to send as a full Block frame on air.
pub fn block_fits_air(block: &Block) -> bool {
    if block.txs.len() > AIR_MAX_TXS_PER_BLOCK {
        return false;
    }
    match encode_block(block) {
        Ok(b) => b.len() <= HEADER_LEN + MAX_PAYLOAD,
        Err(_) => false,
    }
}

/// Prefer full block on air if 0–1 tx and fits; else block hint.
pub fn encode_block_for_air(block: &Block) -> Result<Vec<u8>, FrameError> {
    if block_fits_air(block) {
        encode_block(block)
    } else {
        encode_block_hint(&BlockHintPayload {
            height: block.header.height,
            hash_hex: block.hash_hex(),
            producer_index: block.header.producer_index,
            tx_count: block.header.tx_count,
        })
    }
}

pub fn encode_block_ack(ack: &BlockAckPayload) -> Result<Vec<u8>, FrameError> {
    let body = bincode::serialize(ack).map_err(|e| FrameError::Codec(e.to_string()))?;
    encode_frame(MsgType::BlockAck, &body)
}

pub fn decode_block_ack(frame: &Frame) -> Result<BlockAckPayload, FrameError> {
    if frame.msg_type != MsgType::BlockAck {
        return Err(FrameError::Codec("not a block_ack frame".into()));
    }
    bincode::deserialize(&frame.payload).map_err(|e| FrameError::Codec(e.to_string()))
}

/// Everyday transfer without PQ usually fits one air frame.
pub fn tx_fits_air(tx: &Tx) -> bool {
    match encode_tx(tx) {
        Ok(b) => b.len() <= HEADER_LEN + MAX_PAYLOAD,
        Err(_) => false,
    }
}

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Encode a frame with LZ77/Deflate compression (`MsgType::Compressed`).
/// Wire payload inside Compressed msg_type: `inner_msg_type[u8] | uncompressed_len[u16 LE] | deflate_bytes...`
pub fn encode_compressed(inner_msg_type: MsgType, inner_payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    if inner_payload.len() > u16::MAX as usize {
        return Err(FrameError::TooLarge(inner_payload.len()));
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(inner_payload)
        .map_err(|e| FrameError::Codec(e.to_string()))?;
    let deflated = encoder.finish().map_err(|e| FrameError::Codec(e.to_string()))?;

    let mut comp_payload = Vec::with_capacity(1 + 2 + deflated.len());
    comp_payload.push(inner_msg_type as u8);
    comp_payload.extend_from_slice(&(inner_payload.len() as u16).to_le_bytes());
    comp_payload.extend_from_slice(&deflated);

    encode_frame(MsgType::Compressed, &comp_payload)
}

/// Decode a compressed frame into the uncompressed `Frame`.
pub fn decode_compressed(frame: &Frame) -> Result<Frame, FrameError> {
    if frame.msg_type != MsgType::Compressed {
        return Err(FrameError::Codec("not a compressed frame".into()));
    }
    if frame.payload.len() < 3 {
        return Err(FrameError::Truncated);
    }
    let inner_type_u8 = frame.payload[0];
    let inner_msg_type = MsgType::from_u8(inner_type_u8)
        .ok_or(FrameError::UnknownType(inner_type_u8))?;
    let expected_len = u16::from_le_bytes([frame.payload[1], frame.payload[2]]) as usize;
    if expected_len > MAX_PAYLOAD * 64 {
        return Err(FrameError::Codec("decompressed payload too large".into()));
    }

    let mut decoder = DeflateDecoder::new(&frame.payload[3..]);
    let mut decompressed = Vec::with_capacity(expected_len);
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| FrameError::Codec(e.to_string()))?;
    if decompressed.len() != expected_len {
        return Err(FrameError::Codec("decompressed length mismatch".into()));
    }

    Ok(Frame {
        msg_type: inner_msg_type,
        payload: decompressed,
        hop_limit: frame.hop_limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use meshchain_proto::address::short_id;
    use meshchain_proto::crypto::Keypair;
    use meshchain_proto::tx::{Tx, TxBody};

    #[test]
    fn roundtrip_tx_frame() {
        let kp = Keypair::generate();
        let from = short_id(&kp.public_key());
        let to = short_id(&Keypair::generate().public_key());
        let tx = Tx::sign(
            TxBody::Transfer {
                nonce: 0,
                from,
                to,
                amount: 42,
                fee: 0,
            },
            &kp,
        )
        .unwrap();
        let bytes = encode_tx(&tx).unwrap();
        assert!(bytes.len() < 220);
        assert!(tx_fits_air(&tx));
        let frame = decode_frame(&bytes).unwrap();
        let tx2 = decode_tx(&frame).unwrap();
        assert_eq!(tx.txid(), tx2.txid());
    }

    #[test]
    fn roundtrip_tip_frame() {
        let tip = TipPayload {
            chain_id: "meshchain-testnet-1".into(),
            height: 42,
            tip_hash_hex: "ab".repeat(32),
        };
        let bytes = encode_tip(&tip).unwrap();
        assert!(bytes.len() < 120);
        let frame = decode_frame(&bytes).unwrap();
        assert_eq!(decode_tip(&frame).unwrap(), tip);
    }

    #[test]
    fn test_compressed_frame_roundtrip() {
        let raw = b"MeshChain Meshtastic Native L1 Layer 1 LoRa radio compression test repeating string repeating string repeating string".to_vec();
        let comp_bytes = encode_compressed(MsgType::Block, &raw).unwrap();
        assert!(comp_bytes.len() < raw.len() + HEADER_LEN);
        let frame = decode_frame(&comp_bytes).unwrap();
        assert_eq!(frame.msg_type, MsgType::Compressed);
        let decomp = decode_compressed(&frame).unwrap();
        assert_eq!(decomp.msg_type, MsgType::Block);
        assert_eq!(decomp.payload, raw);
    }

    #[test]
    fn test_frame_v2_hops() {
        let payload = b"hello hop limit".to_vec();
        let encoded = encode_frame_with_hops(MsgType::Tx, &payload, 5).unwrap();
        assert_eq!(encoded[2], FRAME_VERSION_V2);
        let frame = decode_frame(&encoded).unwrap();
        assert_eq!(frame.hop_limit, 5);
        assert_eq!(frame.payload, payload);
    }
}
