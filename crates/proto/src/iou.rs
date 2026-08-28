//! Compact air IOU — vault-bound spend that fits one LoRa frame.
//!
//! This is the product frame. tMESH is a test counter. Settlement is
//! Solana vault USDC/SOL after mesh witnesses co-sign the same iou_id.
//!
//! Layout (sign bytes = everything before sig):
//!   ver:u8=1 | from:8 | dest:32 | amount:u64 LE | nonce:u32 LE
//!   | expiry_unix:u32 LE | deposit_seq:u64 LE
//!   | ed25519 sig:64
//! Total 129 B payload. MeshChain MC header adds 6 B.

use crate::address::{ShortId, SHORT_ID_LEN};
use crate::crypto::{hash_trunc16, Keypair, PublicKey, Signature, SignatureBytes};
use crate::error::ProtoError;
use serde::{Deserialize, Serialize};

pub const AIR_IOU_VERSION: u8 = 1;
pub const AIR_IOU_MSG: u8 = 15;
pub const AIR_IOU_ACK_MSG: u8 = 16;
pub const AIR_IOU_SIGN_LEN: usize = 1 + SHORT_ID_LEN + 32 + 8 + 4 + 4 + 8; // 65
pub const AIR_IOU_LEN: usize = AIR_IOU_SIGN_LEN + 64; // 129
pub const AIR_IOU_ACK_LEN: usize = 1 + 16 + 1 + 64; // 82

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirIou {
    pub from: ShortId,
    pub dest: [u8; 32],
    pub amount: u64,
    pub nonce: u32,
    pub expiry_unix: u32,
    pub deposit_seq: u64,
    pub signature: SignatureBytes,
}

impl AirIou {
    pub fn sign_bytes(
        from: &ShortId,
        dest: &[u8; 32],
        amount: u64,
        nonce: u32,
        expiry_unix: u32,
        deposit_seq: u64,
    ) -> [u8; AIR_IOU_SIGN_LEN] {
        let mut out = [0u8; AIR_IOU_SIGN_LEN];
        out[0] = AIR_IOU_VERSION;
        out[1..9].copy_from_slice(from);
        out[9..41].copy_from_slice(dest);
        out[41..49].copy_from_slice(&amount.to_le_bytes());
        out[49..53].copy_from_slice(&nonce.to_le_bytes());
        out[53..57].copy_from_slice(&expiry_unix.to_le_bytes());
        out[57..65].copy_from_slice(&deposit_seq.to_le_bytes());
        out
    }

    pub fn iou_id(&self) -> [u8; 16] {
        hash_trunc16(&self.sign_prefix())
    }

    fn sign_prefix(&self) -> [u8; AIR_IOU_SIGN_LEN] {
        Self::sign_bytes(
            &self.from,
            &self.dest,
            self.amount,
            self.nonce,
            self.expiry_unix,
            self.deposit_seq,
        )
    }

    pub fn sign(
        from: ShortId,
        dest: [u8; 32],
        amount: u64,
        nonce: u32,
        expiry_unix: u32,
        deposit_seq: u64,
        keypair: &Keypair,
    ) -> Result<Self, ProtoError> {
        if amount == 0 {
            return Err(ProtoError::InvalidTx("iou amount must be > 0".into()));
        }
        let msg = Self::sign_bytes(&from, &dest, amount, nonce, expiry_unix, deposit_seq);
        Ok(Self {
            from,
            dest,
            amount,
            nonce,
            expiry_unix,
            deposit_seq,
            signature: keypair.sign(&msg),
        })
    }

    pub fn verify(&self, spender: &PublicKey) -> Result<(), ProtoError> {
        if crate::address::short_id(spender) != self.from {
            return Err(ProtoError::InvalidTx("iou signer does not match from".into()));
        }
        Signature::verify(spender, &self.sign_prefix(), &self.signature)
    }

    pub fn encode(&self) -> [u8; AIR_IOU_LEN] {
        let mut out = [0u8; AIR_IOU_LEN];
        let prefix = self.sign_prefix();
        out[..AIR_IOU_SIGN_LEN].copy_from_slice(&prefix);
        out[AIR_IOU_SIGN_LEN..].copy_from_slice(self.signature.as_bytes());
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtoError> {
        if buf.len() != AIR_IOU_LEN {
            return Err(ProtoError::Codec(format!(
                "air iou wants {AIR_IOU_LEN} bytes, got {}",
                buf.len()
            )));
        }
        if buf[0] != AIR_IOU_VERSION {
            return Err(ProtoError::Codec("bad air iou version".into()));
        }
        let mut from = [0u8; 8];
        from.copy_from_slice(&buf[1..9]);
        let mut dest = [0u8; 32];
        dest.copy_from_slice(&buf[9..41]);
        let amount = u64::from_le_bytes(buf[41..49].try_into().unwrap());
        let nonce = u32::from_le_bytes(buf[49..53].try_into().unwrap());
        let expiry_unix = u32::from_le_bytes(buf[53..57].try_into().unwrap());
        let deposit_seq = u64::from_le_bytes(buf[57..65].try_into().unwrap());
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&buf[65..129]);
        Ok(Self {
            from,
            dest,
            amount,
            nonce,
            expiry_unix,
            deposit_seq,
            signature: SignatureBytes(sig),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirIouAck {
    pub iou_id: [u8; 16],
    pub witness_index: u8,
    pub signature: SignatureBytes,
}

impl AirIouAck {
    pub fn sign_bytes(iou_id: &[u8; 16], witness_index: u8) -> [u8; 18] {
        let mut out = [0u8; 18];
        out[0] = AIR_IOU_VERSION;
        out[1..17].copy_from_slice(iou_id);
        out[17] = witness_index;
        out
    }

    pub fn sign(iou_id: [u8; 16], witness_index: u8, keypair: &Keypair) -> Self {
        let msg = Self::sign_bytes(&iou_id, witness_index);
        Self {
            iou_id,
            witness_index,
            signature: keypair.sign(&msg),
        }
    }

    pub fn verify(&self, witness: &PublicKey) -> Result<(), ProtoError> {
        Signature::verify(
            witness,
            &Self::sign_bytes(&self.iou_id, self.witness_index),
            &self.signature,
        )
    }

    pub fn encode(&self) -> [u8; AIR_IOU_ACK_LEN] {
        let mut out = [0u8; AIR_IOU_ACK_LEN];
        out[0] = AIR_IOU_VERSION;
        out[1..17].copy_from_slice(&self.iou_id);
        out[17] = self.witness_index;
        out[18..].copy_from_slice(self.signature.as_bytes());
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Self, ProtoError> {
        if buf.len() != AIR_IOU_ACK_LEN {
            return Err(ProtoError::Codec(format!(
                "air iou ack wants {AIR_IOU_ACK_LEN} bytes, got {}",
                buf.len()
            )));
        }
        if buf[0] != AIR_IOU_VERSION {
            return Err(ProtoError::Codec("bad air iou ack version".into()));
        }
        let mut iou_id = [0u8; 16];
        iou_id.copy_from_slice(&buf[1..17]);
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&buf[18..82]);
        Ok(Self {
            iou_id,
            witness_index: buf[17],
            signature: SignatureBytes(sig),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::short_id;

    #[test]
    fn iou_roundtrip_and_size() {
        let kp = Keypair::generate();
        let from = short_id(&kp.public_key());
        let dest = [0x11u8; 32];
        let iou = AirIou::sign(from, dest, 1_000_000, 3, 1_800_000_000, 7, &kp).unwrap();
        iou.verify(&kp.public_key()).unwrap();
        let raw = iou.encode();
        assert_eq!(raw.len(), AIR_IOU_LEN);
        assert!(AIR_IOU_LEN < 200, "must fit one LoRa payload");
        let back = AirIou::decode(&raw).unwrap();
        assert_eq!(back.amount, 1_000_000);
        assert_eq!(back.deposit_seq, 7);
        back.verify(&kp.public_key()).unwrap();

        let ack = AirIouAck::sign(iou.iou_id(), 0, &kp);
        ack.verify(&kp.public_key()).unwrap();
        let araw = ack.encode();
        assert_eq!(araw.len(), AIR_IOU_ACK_LEN);
        AirIouAck::decode(&araw).unwrap();
    }

    #[test]
    fn wrong_key_rejected() {
        let a = Keypair::generate();
        let b = Keypair::generate();
        let iou = AirIou::sign(short_id(&a.public_key()), [2u8; 32], 1, 0, 1, 0, &a).unwrap();
        assert!(iou.verify(&b.public_key()).is_err());
    }
}
