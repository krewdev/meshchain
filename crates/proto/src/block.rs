use crate::crypto::{hash_bytes, hash_trunc16, Keypair, PublicKey, Signature, SignatureBytes};
use crate::error::ProtoError;
use crate::tx::Tx;
use serde::{Deserialize, Serialize};

pub const BLOCK_HASH_LEN: usize = 32;
pub const AIR_HASH_LEN: usize = 16;
/// Max transactions sealed into one block (v1.1; was 1 in early v1).
pub const MAX_TXS_PER_BLOCK: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    pub height: u64,
    pub prev_hash: [u8; BLOCK_HASH_LEN],
    pub slot_time: u64,
    /// Index into genesis validator set.
    pub producer_index: u8,
    pub producer: PublicKey,
    pub tx_count: u8,
    pub tx_root: [u8; AIR_HASH_LEN],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub producer_sig: SignatureBytes,
    /// 0..=MAX_TXS_PER_BLOCK transactions.
    pub txs: Vec<Tx>,
}

/// Deterministic multi-tx root: hash of concatenated 32-byte txids (empty → zeros).
pub fn compute_tx_root(txs: &[Tx]) -> [u8; AIR_HASH_LEN] {
    if txs.is_empty() {
        return [0u8; AIR_HASH_LEN];
    }
    let mut buf = Vec::with_capacity(txs.len() * 32);
    for tx in txs {
        buf.extend_from_slice(&tx.txid());
    }
    hash_trunc16(&buf)
}

impl BlockHeader {
    pub fn sign_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        bincode::serialize(self).map_err(|e| ProtoError::Codec(e.to_string()))
    }
}

impl Block {
    pub fn genesis(
        slot_time: u64,
        producer: &Keypair,
        producer_index: u8,
    ) -> Result<Self, ProtoError> {
        let header = BlockHeader {
            height: 0,
            prev_hash: [0u8; BLOCK_HASH_LEN],
            slot_time,
            producer_index,
            producer: producer.public_key(),
            tx_count: 0,
            tx_root: [0u8; AIR_HASH_LEN],
        };
        Self::seal(header, vec![], producer)
    }

    pub fn new(
        height: u64,
        prev_hash: [u8; BLOCK_HASH_LEN],
        slot_time: u64,
        producer_index: u8,
        producer: &Keypair,
        txs: Vec<Tx>,
    ) -> Result<Self, ProtoError> {
        if txs.len() > MAX_TXS_PER_BLOCK {
            return Err(ProtoError::InvalidBlock(format!(
                "at most {MAX_TXS_PER_BLOCK} txs per block"
            )));
        }
        let tx_root = compute_tx_root(&txs);
        let header = BlockHeader {
            height,
            prev_hash,
            slot_time,
            producer_index,
            producer: producer.public_key(),
            tx_count: txs.len() as u8,
            tx_root,
        };
        Self::seal(header, txs, producer)
    }

    fn seal(header: BlockHeader, txs: Vec<Tx>, producer: &Keypair) -> Result<Self, ProtoError> {
        let msg = header.sign_bytes()?;
        let producer_sig = producer.sign(&msg);
        Ok(Self {
            header,
            producer_sig,
            txs,
        })
    }

    pub fn hash(&self) -> [u8; BLOCK_HASH_LEN] {
        let bytes = bincode::serialize(&self.header).unwrap_or_default();
        hash_bytes(&bytes)
    }

    pub fn hash_hex(&self) -> String {
        hex::encode(self.hash())
    }

    pub fn verify_producer_sig(&self) -> Result<(), ProtoError> {
        let msg = self.header.sign_bytes()?;
        Signature::verify(&self.header.producer, &msg, &self.producer_sig)?;
        if self.txs.len() as u8 != self.header.tx_count {
            return Err(ProtoError::InvalidBlock("tx_count mismatch".into()));
        }
        if self.txs.len() > MAX_TXS_PER_BLOCK {
            return Err(ProtoError::InvalidBlock(format!(
                "max {MAX_TXS_PER_BLOCK} txs"
            )));
        }
        let root = compute_tx_root(&self.txs);
        if root != self.header.tx_root {
            return Err(ProtoError::InvalidBlock("tx_root mismatch".into()));
        }
        for tx in &self.txs {
            tx.verify()?;
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        bincode::serialize(self).map_err(|e| ProtoError::Codec(e.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self, ProtoError> {
        bincode::deserialize(data).map_err(|e| ProtoError::Codec(e.to_string()))
    }
}
