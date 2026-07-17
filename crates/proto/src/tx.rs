use crate::address::{short_id, ShortId, ADDRESS_LEN, SHORT_ID_LEN};
use crate::crypto::{hash_bytes, hash_trunc16, Keypair, PublicKey, Signature, SignatureBytes};
use crate::error::ProtoError;
use crate::pq::{pq_verify, PqKeypair, PQ_PK_LEN, PQ_SIG_LEN};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxType {
    Transfer = 1,
    Register = 2,
    Mint = 3,
    Burn = 4,
}

impl TxType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Transfer),
            2 => Some(Self::Register),
            3 => Some(Self::Mint),
            4 => Some(Self::Burn),
            _ => None,
        }
    }
}

/// Signed transaction envelope.
///
/// Small spends: ed25519 only.
/// Large / cold spends: also attach ML-DSA-65 (`pq_pk` + `pq_sig`) over `body.sign_bytes()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tx {
    pub body: TxBody,
    pub signature: SignatureBytes,
    /// Full ed25519 pubkey of signer.
    pub signer: PublicKey,
    /// Optional post-quantum public key (ML-DSA-65, 1952 bytes).
    /// Always serialized (bincode needs stable layout; do not skip_serializing_if).
    #[serde(default)]
    pub pq_pk: Option<Vec<u8>>,
    /// Optional post-quantum signature over body.sign_bytes().
    #[serde(default)]
    pub pq_sig: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TxBody {
    /// Everyday payment. Optional `fee` is a priority tip paid to the **block producer**
    /// (MEV-style inclusion preference — higher fee wins the next slot when mempool is contested).
    Transfer {
        nonce: u32,
        from: ShortId,
        to: ShortId,
        amount: u64,
        /// Priority fee in base units (0 = no tip). Paid to block producer on inclusion.
        #[serde(default)]
        fee: u64,
    },
    Register {
        nonce: u32,
        /// Full pubkey being registered (must match signer).
        pubkey: PublicKey,
    },
    /// Authorized minter (validator/bridge) credits `to` with `amount`.
    /// Future Solana vault bridge posts Mint after lock on L1.
    Mint {
        nonce: u32,
        to: ShortId,
        amount: u64,
        /// Optional external ref (e.g. Solana tx sig hash trunc 16).
        external_ref: [u8; 16],
        /// If recipient is unknown, minter may attach full pubkey (must hash to `to`)
        /// so gossip/peer mints can create the account without a prior Register.
        #[serde(default)]
        to_pubkey: Option<PublicKey>,
    },
    /// Holder burns MESH; bridge watches for off-ramp unlock.
    Burn {
        nonce: u32,
        from: ShortId,
        amount: u64,
        /// Destination hint for bridge (e.g. hash of Solana pubkey), not verified on mesh.
        redeem_hint: [u8; 32],
        /// Asset tag: 0 = MESH/generic, 1 = SOL-claim, 2 = BTC-claim (policy on bridge).
        #[serde(default)]
        asset_id: u32,
    },
}

impl TxBody {
    pub fn tx_type(&self) -> TxType {
        match self {
            TxBody::Transfer { .. } => TxType::Transfer,
            TxBody::Register { .. } => TxType::Register,
            TxBody::Mint { .. } => TxType::Mint,
            TxBody::Burn { .. } => TxType::Burn,
        }
    }

    pub fn nonce(&self) -> u32 {
        match self {
            TxBody::Transfer { nonce, .. }
            | TxBody::Register { nonce, .. }
            | TxBody::Mint { nonce, .. }
            | TxBody::Burn { nonce, .. } => *nonce,
        }
    }

    /// Priority tip (base units) for inclusion preference. Only Transfer carries a fee today.
    pub fn priority_fee(&self) -> u64 {
        match self {
            TxBody::Transfer { fee, .. } => *fee,
            _ => 0,
        }
    }

    /// Canonical bytes for signing (deterministic bincode).
    pub fn sign_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        bincode::serialize(&(PROTOCOL_VERSION, self)).map_err(|e| ProtoError::Codec(e.to_string()))
    }
}

impl Tx {
    pub fn sign(body: TxBody, keypair: &Keypair) -> Result<Self, ProtoError> {
        let msg = body.sign_bytes()?;
        let signature = keypair.sign(&msg);
        Ok(Self {
            body,
            signature,
            signer: keypair.public_key(),
            pq_pk: None,
            pq_sig: None,
        })
    }

    /// Sign with ed25519 + ML-DSA-65 (required for large cold-storage spends).
    pub fn sign_with_pq(
        body: TxBody,
        keypair: &Keypair,
        pq: &PqKeypair,
    ) -> Result<Self, ProtoError> {
        let mut tx = Self::sign(body, keypair)?;
        let msg = tx.body.sign_bytes()?;
        let sig = pq.sign(&msg)?;
        tx.pq_pk = Some(pq.public_key_bytes().to_vec());
        tx.pq_sig = Some(sig.to_vec());
        Ok(tx)
    }

    pub fn amount_for_pq_policy(&self) -> Option<u64> {
        match &self.body {
            TxBody::Transfer { amount, .. } | TxBody::Burn { amount, .. } => Some(*amount),
            _ => None,
        }
    }

    /// Priority tip attached to this tx (0 if none).
    pub fn priority_fee(&self) -> u64 {
        self.body.priority_fee()
    }

    pub fn has_pq(&self) -> bool {
        self.pq_pk.is_some() && self.pq_sig.is_some()
    }

    pub fn verify_pq(&self) -> Result<(), ProtoError> {
        let pk = self
            .pq_pk
            .as_ref()
            .ok_or_else(|| ProtoError::InvalidTx("missing pq public key".into()))?;
        let sig = self
            .pq_sig
            .as_ref()
            .ok_or_else(|| ProtoError::InvalidTx("missing pq signature".into()))?;
        if pk.len() != PQ_PK_LEN || sig.len() != PQ_SIG_LEN {
            return Err(ProtoError::InvalidTx("bad pq key/sig size".into()));
        }
        let mut pka = [0u8; PQ_PK_LEN];
        let mut sa = [0u8; PQ_SIG_LEN];
        pka.copy_from_slice(pk);
        sa.copy_from_slice(sig);
        let msg = self.body.sign_bytes()?;
        pq_verify(&pka, &msg, &sa)
    }

    pub fn verify(&self) -> Result<(), ProtoError> {
        let msg = self.body.sign_bytes()?;
        Signature::verify(&self.signer, &msg, &self.signature)?;

        // Structural checks
        match &self.body {
            TxBody::Transfer {
                from, amount, fee, ..
            } => {
                if *amount == 0 {
                    return Err(ProtoError::InvalidTx("amount must be > 0".into()));
                }
                // fee may be 0 (no tip); overflow guard for amount+fee debit
                if amount.checked_add(*fee).is_none() {
                    return Err(ProtoError::InvalidTx("amount+fee overflow".into()));
                }
                if short_id(&self.signer) != *from {
                    return Err(ProtoError::InvalidTx("signer does not match from".into()));
                }
            }
            TxBody::Register { pubkey, .. } => {
                if *pubkey != self.signer {
                    return Err(ProtoError::InvalidTx(
                        "register pubkey must be signer".into(),
                    ));
                }
            }
            TxBody::Mint { amount, .. } => {
                if *amount == 0 {
                    return Err(ProtoError::InvalidTx("mint amount must be > 0".into()));
                }
            }
            TxBody::Burn { from, amount, .. } => {
                if *amount == 0 {
                    return Err(ProtoError::InvalidTx("burn amount must be > 0".into()));
                }
                if short_id(&self.signer) != *from {
                    return Err(ProtoError::InvalidTx("signer does not match from".into()));
                }
            }
        }

        // If PQ material present, it must verify (always).
        if self.has_pq() {
            self.verify_pq()?;
        }
        Ok(())
    }

    pub fn txid(&self) -> [u8; 32] {
        // Exclude optional PQ fields from txid stability for classical-only? Include all for uniqueness.
        let bytes = bincode::serialize(self).unwrap_or_default();
        hash_bytes(&bytes)
    }

    pub fn txid_hex(&self) -> String {
        hex::encode(self.txid())
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        bincode::serialize(self).map_err(|e| ProtoError::Codec(e.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self, ProtoError> {
        bincode::deserialize(data).map_err(|e| ProtoError::Codec(e.to_string()))
    }
}

/// Compact wire estimate helpers (for docs/tests).
pub fn transfer_wire_size_estimate() -> usize {
    // ver + type + nonce + from + to + amount + sig + signer
    1 + 1 + 4 + SHORT_ID_LEN + SHORT_ID_LEN + 8 + 64 + ADDRESS_LEN
}

pub fn mint_external_ref_from_solana_sig(sig_bytes: &[u8]) -> [u8; 16] {
    hash_trunc16(sig_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::short_id;

    #[test]
    fn sign_verify_transfer() {
        let kp = Keypair::generate();
        let from = short_id(&kp.public_key());
        let to = short_id(&Keypair::generate().public_key());
        let body = TxBody::Transfer {
            nonce: 0,
            from,
            to,
            amount: 1_000_000,
            fee: 0,
        };
        let tx = Tx::sign(body, &kp).unwrap();
        tx.verify().unwrap();
        assert_eq!(tx.priority_fee(), 0);

        let body_tip = TxBody::Transfer {
            nonce: 1,
            from,
            to,
            amount: 1_000_000,
            fee: 50_000,
        };
        let tx2 = Tx::sign(body_tip, &kp).unwrap();
        tx2.verify().unwrap();
        assert_eq!(tx2.priority_fee(), 50_000);
        assert!(transfer_wire_size_estimate() < 200);
    }
}
