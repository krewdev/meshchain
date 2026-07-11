//! Compact mesh frames for Meshtastic (~237 byte payload budget).
//!
//! Wire: MAGIC[2] | ver[1] | msg_type[1] | len[u16 LE] | payload[len]
//! Total header = 6 bytes.

use meshchain_proto::block::Block;
use meshchain_proto::tx::Tx;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FRAME_MAGIC: &[u8; 2] = b"MC";
pub const FRAME_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 6;
/// Conservative max payload under Meshtastic LoRa limits.
pub const MAX_PAYLOAD: usize = 200;

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
    /// JSON control / bridge hint (relayer)
    Control = 10,
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
            10 => Some(Self::Control),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMsg {
    pub kind: String,
    pub body: serde_json::Value,
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
        let frame = decode_frame(&bytes).unwrap();
        let tx2 = decode_tx(&frame).unwrap();
        assert_eq!(tx.txid(), tx2.txid());
    }
}
