//! Post-quantum signatures: **ML-DSA-65 (FIPS 204)**.
//!
//! Signatures are ~3KB and must be sent via multi-packet FRAG frames on Meshtastic.
//! Use this profile for extreme cold-storage mesh wallets.

use crate::error::ProtoError;
use fips204::ml_dsa_65;
use fips204::traits::{SerDes, Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// ML-DSA-65 public key size (bytes).
pub const PQ_PK_LEN: usize = 1952;
/// ML-DSA-65 secret key size (bytes).
pub const PQ_SK_LEN: usize = 4032;
/// ML-DSA-65 signature size (bytes).
pub const PQ_SIG_LEN: usize = 3309;

/// Context string bound into ML-DSA (domain separation for MeshChain).
pub const MESH_PQ_CTX: &[u8] = b"meshchain-v2-mldsa65";

#[derive(Clone)]
pub struct PqKeypair {
    sk_bytes: [u8; PQ_SK_LEN],
    pk_bytes: [u8; PQ_PK_LEN],
}

impl PqKeypair {
    pub fn generate() -> Result<Self, ProtoError> {
        let (pk, sk) =
            ml_dsa_65::try_keygen().map_err(|e| ProtoError::Codec(format!("pq keygen: {e}")))?;
        Ok(Self {
            sk_bytes: sk.into_bytes(),
            pk_bytes: pk.into_bytes(),
        })
    }

    pub fn public_key_bytes(&self) -> &[u8; PQ_PK_LEN] {
        &self.pk_bytes
    }

    pub fn secret_key_bytes(&self) -> &[u8; PQ_SK_LEN] {
        &self.sk_bytes
    }

    pub fn from_bytes(sk: [u8; PQ_SK_LEN], pk: [u8; PQ_PK_LEN]) -> Self {
        Self {
            sk_bytes: sk,
            pk_bytes: pk,
        }
    }

    /// Short id = first 8 bytes of SHA-256(PQ public key).
    pub fn short_id(&self) -> [u8; 8] {
        short_id_from_pq_pk(&self.pk_bytes)
    }

    pub fn sign(&self, message: &[u8]) -> Result<[u8; PQ_SIG_LEN], ProtoError> {
        let sk = ml_dsa_65::PrivateKey::try_from_bytes(self.sk_bytes)
            .map_err(|e| ProtoError::Codec(format!("pq sk: {e}")))?;
        sk.try_sign(message, MESH_PQ_CTX)
            .map_err(|e| ProtoError::Codec(format!("pq sign: {e}")))
    }

    pub fn to_file(&self) -> PqKeypairFile {
        PqKeypairFile {
            scheme: "ml-dsa-65".into(),
            secret_hex: hex::encode(self.sk_bytes),
            public_hex: hex::encode(self.pk_bytes),
        }
    }

    pub fn from_file(f: &PqKeypairFile) -> Result<Self, ProtoError> {
        if f.scheme != "ml-dsa-65" {
            return Err(ProtoError::Codec(format!(
                "unsupported pq scheme {}",
                f.scheme
            )));
        }
        let sk_v = hex::decode(&f.secret_hex).map_err(|e| ProtoError::Codec(e.to_string()))?;
        let pk_v = hex::decode(&f.public_hex).map_err(|e| ProtoError::Codec(e.to_string()))?;
        if sk_v.len() != PQ_SK_LEN || pk_v.len() != PQ_PK_LEN {
            return Err(ProtoError::Codec("pq key length mismatch".into()));
        }
        let mut sk = [0u8; PQ_SK_LEN];
        let mut pk = [0u8; PQ_PK_LEN];
        sk.copy_from_slice(&sk_v);
        pk.copy_from_slice(&pk_v);
        Ok(Self::from_bytes(sk, pk))
    }
}

pub fn short_id_from_pq_pk(pk: &[u8]) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(b"meshchain-pq-id");
    hasher.update(pk);
    let out = hasher.finalize();
    let mut id = [0u8; 8];
    id.copy_from_slice(&out[..8]);
    id
}

pub fn pq_verify(
    pk: &[u8; PQ_PK_LEN],
    message: &[u8],
    sig: &[u8; PQ_SIG_LEN],
) -> Result<(), ProtoError> {
    let public = ml_dsa_65::PublicKey::try_from_bytes(*pk)
        .map_err(|e| ProtoError::Codec(format!("pq pk: {e}")))?;
    if public.verify(message, sig, MESH_PQ_CTX) {
        Ok(())
    } else {
        Err(ProtoError::InvalidSignature)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PqKeypairFile {
    pub scheme: String,
    pub secret_hex: String,
    pub public_hex: String,
}

/// Envelope for a PQ-signed body (body is classical TxBody bincode or arbitrary message).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PqSigned {
    pub body: Vec<u8>,
    pub signer_pk: Vec<u8>, // PQ_PK_LEN
    pub signature: Vec<u8>, // PQ_SIG_LEN
}

impl PqSigned {
    pub fn sign_message(message: &[u8], kp: &PqKeypair) -> Result<Self, ProtoError> {
        let signature = kp.sign(message)?.to_vec();
        Ok(Self {
            body: message.to_vec(),
            signer_pk: kp.public_key_bytes().to_vec(),
            signature,
        })
    }

    pub fn verify(&self) -> Result<(), ProtoError> {
        if self.signer_pk.len() != PQ_PK_LEN || self.signature.len() != PQ_SIG_LEN {
            return Err(ProtoError::Codec("pq signed length".into()));
        }
        let mut pk = [0u8; PQ_PK_LEN];
        let mut sig = [0u8; PQ_SIG_LEN];
        pk.copy_from_slice(&self.signer_pk);
        sig.copy_from_slice(&self.signature);
        pq_verify(&pk, &self.body, &sig)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        bincode::serialize(self).map_err(|e| ProtoError::Codec(e.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self, ProtoError> {
        bincode::deserialize(data).map_err(|e| ProtoError::Codec(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pq_sign_verify() {
        let kp = PqKeypair::generate().unwrap();
        let msg = b"cold-storage-withdraw-v1";
        let sig = kp.sign(msg).unwrap();
        pq_verify(kp.public_key_bytes(), msg, &sig).unwrap();
        let env = PqSigned::sign_message(msg, &kp).unwrap();
        env.verify().unwrap();
        assert_eq!(sig.len(), PQ_SIG_LEN);
    }
}
