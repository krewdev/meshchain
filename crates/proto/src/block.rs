use crate::crypto::{hash_bytes, hash_trunc16, Keypair, PublicKey, Signature, SignatureBytes};
use crate::error::ProtoError;
use crate::tx::Tx;
use serde::{Deserialize, Serialize};

pub const BLOCK_HASH_LEN: usize = 32;
pub const AIR_HASH_LEN: usize = 16;

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
    /// v1: 0 or 1 transaction.
    pub txs: Vec<Tx>,
}

impl BlockHeader {
    pub fn sign_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        bincode::serialize(self).map_err(|e| ProtoError::Codec(e.to_string()))
    }
}

impl Block {
    pub fn genesis(slot_time: u64, producer: &Keypair, producer_index: u8) -> Result<Self, ProtoError> {
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
        if txs.len() > 1 {
            return Err(ProtoError::InvalidBlock("v1 allows at most 1 tx per block".into()));
        }
        let tx_root = if txs.is_empty() {
            [0u8; AIR_HASH_LEN]
        } else {
            hash_trunc16(&txs[0].txid())
        };
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
        if self.txs.len() > 1 {
            return Err(ProtoError::InvalidBlock("v1 max 1 tx".into()));
        }
        if let Some(tx) = self.txs.first() {
            let root = hash_trunc16(&tx.txid());
            if root != self.header.tx_root {
                return Err(ProtoError::InvalidBlock("tx_root mismatch".into()));
            }
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
