//! Multi-packet fragmentation for PQ signatures and large envelopes.
//!
//! Each fragment fits MAX_PAYLOAD after a small frag header.

use crate::frame::{encode_frame, Frame, FrameError, MsgType, MAX_PAYLOAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Frag header inside payload: session[8] | idx[u16] | total[u16] | chunk...
pub const FRAG_HDR: usize = 8 + 2 + 2;
pub const FRAG_CHUNK: usize = MAX_PAYLOAD - FRAG_HDR;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragSessionMeta {
    pub session_id: [u8; 8],
    pub total: u16,
    pub payload_hash: [u8; 32],
    pub kind: u8, // 1 = PqSigned envelope
}

pub fn fragment_bytes(session_id: [u8; 8], data: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
    if data.is_empty() {
        return Err(FrameError::Codec("empty frag".into()));
    }
    let total = data.chunks(FRAG_CHUNK).count();
    if total > u16::MAX as usize {
        return Err(FrameError::Codec("too many frags".into()));
    }
    let mut frames = Vec::with_capacity(total);
    for (i, chunk) in data.chunks(FRAG_CHUNK).enumerate() {
        let mut payload = Vec::with_capacity(FRAG_HDR + chunk.len());
        payload.extend_from_slice(&session_id);
        payload.extend_from_slice(&(i as u16).to_le_bytes());
        payload.extend_from_slice(&(total as u16).to_le_bytes());
        payload.extend_from_slice(chunk);
        frames.push(encode_frame(MsgType::Frag, &payload)?);
    }
    Ok(frames)
}

pub fn session_id_from_hash(data: &[u8]) -> [u8; 8] {
    let h = Sha256::digest(data);
    let mut id = [0u8; 8];
    id.copy_from_slice(&h[..8]);
    id
}

#[derive(Default)]
pub struct FragAssembler {
    /// session -> (total, chunks)
    pending: HashMap<[u8; 8], (u16, HashMap<u16, Vec<u8>>)>,
}

impl FragAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a decoded FRAG frame payload (not full wire frame).
    /// Returns completed payload when all chunks present.
    pub fn push_payload(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, FrameError> {
        if payload.len() < FRAG_HDR {
            return Err(FrameError::Truncated);
        }
        let mut session = [0u8; 8];
        session.copy_from_slice(&payload[0..8]);
        let idx = u16::from_le_bytes([payload[8], payload[9]]);
        let total = u16::from_le_bytes([payload[10], payload[11]]);
        if total == 0 || idx >= total {
            return Err(FrameError::Codec("bad frag idx/total".into()));
        }
        let chunk = payload[FRAG_HDR..].to_vec();
        let entry = self.pending.entry(session).or_insert_with(|| (total, HashMap::new()));
        if entry.0 != total {
            return Err(FrameError::Codec("frag total mismatch".into()));
        }
        entry.1.insert(idx, chunk);
        if entry.1.len() == total as usize {
            let mut out = Vec::new();
            for i in 0..total {
                let c = entry
                    .1
                    .get(&i)
                    .ok_or_else(|| FrameError::Codec("missing chunk".into()))?;
                out.extend_from_slice(c);
            }
            self.pending.remove(&session);
            return Ok(Some(out));
        }
        Ok(None)
    }

    pub fn push_frame(&mut self, frame: &Frame) -> Result<Option<Vec<u8>>, FrameError> {
        if frame.msg_type != MsgType::Frag {
            return Err(FrameError::Codec("not frag".into()));
        }
        self.push_payload(&frame.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::decode_frame;

    #[test]
    fn frag_roundtrip_3kb() {
        let data: Vec<u8> = (0..3309).map(|i| (i % 256) as u8).collect();
        let sid = session_id_from_hash(&data);
        let frames = fragment_bytes(sid, &data).unwrap();
        assert!(frames.len() > 10);
        let mut asm = FragAssembler::new();
        let mut done = None;
        for f in frames {
            let frame = decode_frame(&f).unwrap();
            if let Some(out) = asm.push_frame(&frame).unwrap() {
                done = Some(out);
            }
        }
        assert_eq!(done.unwrap(), data);
    }
}
