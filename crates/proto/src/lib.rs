//! MeshChain protocol: addresses, transactions, blocks, hashing, signing.
//! Designed for LoRa-fit payloads (~237 byte Meshtastic limit).

pub mod address;
pub mod block;
pub mod crypto;
pub mod error;
pub mod pq;
pub mod privacy;
pub mod tx;

pub use address::{short_id, Address, ShortId, ADDRESS_LEN, SHORT_ID_LEN};
pub use block::{Block, BlockHeader, BLOCK_HASH_LEN};
pub use crypto::{hash_bytes, hash_hex, Keypair, PublicKey, Signature, SignatureBytes};
pub use error::ProtoError;
pub use pq::{PqKeypair, PqSigned, PQ_PK_LEN, PQ_SIG_LEN, PQ_SK_LEN};
pub use privacy::{redeem_hint, short_id_eq, stealth_short_id, PrivacyPolicy};
pub use tx::{Tx, TxBody, TxType, PROTOCOL_VERSION};
