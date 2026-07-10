use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("codec error: {0}")]
    Codec(String),
    #[error("invalid transaction: {0}")]
    InvalidTx(String),
    #[error("invalid block: {0}")]
    InvalidBlock(String),
}
