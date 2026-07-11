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
pub const HEADER_LEN: usize = 6;
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
    /// JSON control / bridge hint (relayer legacy)
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
            10 => Some(Self::Control),
            20 => Some(Self::GossipJson),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub msg_type: MsgType,
    pub payload: Vec<u8>,
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

pub fn decode_frame(data: &[u8]) -> Result<Frame, FrameError> {
    if data.len() < HEADER_LEN {
        return Err(FrameError::Truncated);
    }
    if &data[0..2] != FRAME_MAGIC {
        return Err(FrameError::BadMagic);
    }
    if data[2] != FRAME_VERSION {
        return Err(FrameError::BadVersion(data[2]));
    }
    let msg_type = MsgType::from_u8(data[3]).ok_or(FrameError::UnknownType(data[3]))?;
    let len = u16::from_le_bytes([data[4], data[5]]) as usize;
    if data.len() < HEADER_LEN + len {
        return Err(FrameError::Truncated);
    }
    Ok(Frame {
        msg_type,
        payload: data[HEADER_LEN..HEADER_LEN + len].to_vec(),
    })
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
}
