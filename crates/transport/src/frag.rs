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
    if total > MAX_CHUNKS as usize {
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

const MAX_SESSIONS: usize = 64;
const MAX_CHUNKS: u16 = 128;
const SESSION_TTL_SECS: u64 = 60;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Default)]
pub struct FragAssembler {
    /// session -> (total, chunks, last_activity_unix_secs)
    pending: HashMap<[u8; 8], (u16, HashMap<u16, Vec<u8>>, u64)>,
}

impl FragAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a decoded FRAG frame payload (not full wire frame).
    /// Returns completed payload when all chunks present.
    pub fn push_payload(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, FrameError> {
        let now = now_secs();
        // Prune expired sessions to prevent memory leaks from abandoned fragments
        self.pending.retain(|_, (_, _, ts)| now.saturating_sub(*ts) < SESSION_TTL_SECS);

        if payload.len() < FRAG_HDR {
            return Err(FrameError::Truncated);
        }
        let mut session = [0u8; 8];
        session.copy_from_slice(&payload[0..8]);
        let idx = u16::from_le_bytes([payload[8], payload[9]]);
        let total = u16::from_le_bytes([payload[10], payload[11]]);
        if total == 0 || total > MAX_CHUNKS || idx >= total {
            return Err(FrameError::Codec("bad frag idx/total or exceeds chunk limit".into()));
        }
        let chunk = payload[FRAG_HDR..].to_vec();

        if !self.pending.contains_key(&session) && self.pending.len() >= MAX_SESSIONS {
            // Evict the oldest session when at maximum capacity
            if let Some(oldest) = self.pending.iter().min_by_key(|(_, (_, _, ts))| *ts).map(|(k, _)| *k) {
                self.pending.remove(&oldest);
            }
        }

        let entry = self.pending.entry(session).or_insert_with(|| (total, HashMap::new(), now));
        entry.2 = now; // update activity timestamp
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

    /// Check for sessions that have timed out and need a `FragNack` selective request.
    /// Updates session timestamp so subsequent checks wait another `timeout_secs` before re-requesting.
    pub fn check_missing_chunks(&mut self, timeout_secs: u64) -> Vec<([u8; 8], Vec<u16>)> {
        let now = now_secs();
        let mut nacks = Vec::new();
        for (session_id, (total, chunks, ts)) in self.pending.iter_mut() {
            if now.saturating_sub(*ts) >= timeout_secs && chunks.len() < *total as usize {
                let mut missing = Vec::new();
                for i in 0..*total {
                    if !chunks.contains_key(&i) {
                        missing.push(i);
                    }
                }
                if !missing.is_empty() {
                    nacks.push((*session_id, missing));
                    *ts = now;
                }
            }
        }
        nacks
    }
}

pub fn encode_frag_nack(session_id: [u8; 8], missing_indices: &[u16]) -> Result<Vec<u8>, FrameError> {
    if missing_indices.len() > MAX_CHUNKS as usize {
        return Err(FrameError::Codec("too many missing indices".into()));
    }
    let mut payload = Vec::with_capacity(8 + 2 + missing_indices.len() * 2);
    payload.extend_from_slice(&session_id);
    let count = missing_indices.len() as u16;
    payload.extend_from_slice(&count.to_le_bytes());
    for idx in missing_indices {
        payload.extend_from_slice(&idx.to_le_bytes());
    }
    encode_frame(MsgType::FragNack, &payload)
}

pub fn decode_frag_nack(payload: &[u8]) -> Result<([u8; 8], Vec<u16>), FrameError> {
    if payload.len() < 10 {
        return Err(FrameError::Truncated);
    }
    let mut session_id = [0u8; 8];
    session_id.copy_from_slice(&payload[0..8]);
    let count = u16::from_le_bytes([payload[8], payload[9]]) as usize;
    if count > MAX_CHUNKS as usize || payload.len() < 10 + count * 2 {
        return Err(FrameError::Codec("bad count or truncated nack".into()));
    }
    let mut missing = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 10 + i * 2;
        let idx = u16::from_le_bytes([payload[offset], payload[offset + 1]]);
        missing.push(idx);
    }
    Ok((session_id, missing))
}

#[derive(Default)]
pub struct FragCache {
    /// session_id -> (unix_secs, vec_of_encoded_frag_frames)
    sessions: HashMap<[u8; 8], (u64, Vec<Vec<u8>>)>,
}

impl FragCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store the fragmented wire frames of a session for potential retransmission.
    pub fn insert(&mut self, session_id: [u8; 8], frames: Vec<Vec<u8>>) {
        let now = now_secs();
        self.sessions.retain(|_, (ts, _)| now.saturating_sub(*ts) < SESSION_TTL_SECS);
        if self.sessions.len() >= MAX_SESSIONS {
            if let Some(oldest) = self.sessions.iter().min_by_key(|(_, (ts, _))| *ts).map(|(k, _)| *k) {
                self.sessions.remove(&oldest);
            }
        }
        self.sessions.insert(session_id, (now, frames));
    }

    /// Given a `FragNack` session ID and missing chunk indices, return the corresponding wire frames to retransmit.
    pub fn handle_nack(&self, session_id: &[u8; 8], missing_indices: &[u16]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if let Some((_, frames)) = self.sessions.get(session_id) {
            for &idx in missing_indices {
                if let Some(frame) = frames.get(idx as usize) {
                    out.push(frame.clone());
                }
            }
        }
        out
    }

    pub fn prune(&mut self) {
        let now = now_secs();
        self.sessions.retain(|_, (ts, _)| now.saturating_sub(*ts) < SESSION_TTL_SECS);
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

    #[test]
    fn test_frag_limits_and_eviction() {
        let mut asm = FragAssembler::new();
        let mut bad_payload = vec![0u8; FRAG_HDR + 10];
        bad_payload[10] = (MAX_CHUNKS as u8).saturating_add(1);
        bad_payload[11] = 0;
        assert!(asm.push_payload(&bad_payload).is_err());

        for i in 0..MAX_SESSIONS + 5 {
            let mut payload = vec![0u8; FRAG_HDR + 5];
            payload[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            payload[8] = 0; payload[9] = 0;
            payload[10] = 2; payload[11] = 0;
            asm.push_payload(&payload).unwrap();
        }
        assert!(asm.pending.len() <= MAX_SESSIONS);
    }

    #[test]
    fn test_frag_nack_and_cache() {
        let data: Vec<u8> = (0..800).map(|i| (i % 256) as u8).collect();
        let sid = session_id_from_hash(&data);
        let frames = fragment_bytes(sid, &data).unwrap();
        assert!(frames.len() >= 4);

        let mut cache = FragCache::new();
        cache.insert(sid, frames.clone());

        // Simulate receiving only chunk 0, 1, 3 (dropping chunk 2 over the air)
        let mut asm = FragAssembler::new();
        for (i, f_bytes) in frames.iter().enumerate() {
            if i == 2 {
                continue; // drop chunk 2
            }
            let frame = decode_frame(f_bytes).unwrap();
            let res = asm.push_frame(&frame).unwrap();
            assert!(res.is_none());
        }

        // Check missing chunks with 0s timeout (for test)
        let nacks = asm.check_missing_chunks(0);
        assert_eq!(nacks.len(), 1);
        assert_eq!(nacks[0].0, sid);
        assert_eq!(nacks[0].1, vec![2]);

        // Encode and decode FragNack
        let nack_frame_bytes = encode_frag_nack(nacks[0].0, &nacks[0].1).unwrap();
        let nack_frame = decode_frame(&nack_frame_bytes).unwrap();
        assert_eq!(nack_frame.msg_type, MsgType::FragNack);
        let (req_sid, req_missing) = decode_frag_nack(&nack_frame.payload).unwrap();
        assert_eq!(req_sid, sid);
        assert_eq!(req_missing, vec![2]);

        // Cache serves the retransmission of exact missing wire packet
        let retrans = cache.handle_nack(&req_sid, &req_missing);
        assert_eq!(retrans.len(), 1);
        assert_eq!(retrans[0], frames[2]);

        // Push missing chunk into asm -> assembly completes!
        let frame2 = decode_frame(&retrans[0]).unwrap();
        let final_data = asm.push_frame(&frame2).unwrap();
        assert!(final_data.is_some());
        assert_eq!(final_data.unwrap(), data);
    }
}
