use thiserror::Error;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("account not found: {0}")]
    AccountNotFound(String),
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u32, got: u32 },
    #[error("short id collision")]
    ShortIdCollision,
    #[error("already registered")]
    AlreadyRegistered,
    #[error("unauthorized minter")]
    UnauthorizedMinter,
    #[error("this amount needs a quantum-safe (cold) signature — use your cold key")]
    PqRequired,
    #[error("cold key does not match this account")]
    PqKeyMismatch,
    #[error("invalid block height: expected {expected}, got {got}")]
    BadHeight { expected: u64, got: u64 },
    #[error("bad prev hash")]
    BadPrevHash,
    #[error("unknown producer")]
    UnknownProducer,
    #[error("proto: {0}")]
    Proto(String),
    #[error("io: {0}")]
    Io(String),
    #[error("state: {0}")]
    State(String),
}
