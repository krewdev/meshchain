use crate::error::ProtoError;
use ed25519_dalek::{
    Signature as DalekSig, Signer, SigningKey, Verifier, VerifyingKey, SignatureError,
};
use rand::rngs::OsRng;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt;

pub type PublicKey = [u8; 32];

/// 64-byte ed25519 signature (custom serde — std doesn't impl for [u8; 64]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureBytes(pub [u8; 64]);

impl SignatureBytes {
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl From<[u8; 64]> for SignatureBytes {
    fn from(value: [u8; 64]) -> Self {
        Self(value)
    }
}

impl Serialize for SignatureBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut seq = serializer.serialize_tuple(64)?;
        for b in &self.0 {
            seq.serialize_element(b)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for SignatureBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SigVisitor;
        impl<'de> Visitor<'de> for SigVisitor {
            type Value = SignatureBytes;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("64-byte signature")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut arr = [0u8; 64];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(SignatureBytes(arr))
            }
        }
        deserializer.deserialize_tuple(64, SigVisitor)
    }
}

#[derive(Clone)]
pub struct Keypair {
    signing: SigningKey,
}

impl Keypair {
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self { signing }
    }

    pub fn from_bytes(secret: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&secret),
        }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    pub fn public_key(&self) -> PublicKey {
        self.signing.verifying_key().to_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> SignatureBytes {
        SignatureBytes(self.signing.sign(message).to_bytes())
    }
}

pub struct Signature;

impl Signature {
    pub fn verify(pubkey: &PublicKey, message: &[u8], sig: &SignatureBytes) -> Result<(), ProtoError> {
        let vk = VerifyingKey::from_bytes(pubkey).map_err(|_| ProtoError::InvalidPublicKey)?;
        let signature = DalekSig::from_bytes(sig.as_bytes());
        vk.verify(message, &signature)
            .map_err(|_: SignatureError| ProtoError::InvalidSignature)
    }
}

/// SHA-256 hash (full 32 bytes).
pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

pub fn hash_hex(data: &[u8]) -> String {
    hex::encode(hash_bytes(data))
}

/// Truncated hash for air-friendly headers (16 bytes).
pub fn hash_trunc16(data: &[u8]) -> [u8; 16] {
    let full = hash_bytes(data);
    let mut t = [0u8; 16];
    t.copy_from_slice(&full[..16]);
    t
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeypairFile {
    pub secret_hex: String,
    pub public_hex: String,
}

impl Keypair {
    pub fn to_file(&self) -> KeypairFile {
        KeypairFile {
            secret_hex: hex::encode(self.to_bytes()),
            public_hex: hex::encode(self.public_key()),
        }
    }

    pub fn from_file(f: &KeypairFile) -> Result<Self, ProtoError> {
        let bytes = hex::decode(&f.secret_hex).map_err(|e| ProtoError::Codec(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(ProtoError::Codec("secret must be 32 bytes".into()));
        }
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        Ok(Self::from_bytes(secret))
    }
}
