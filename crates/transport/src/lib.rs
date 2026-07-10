//! MeshChain transport: LoRa-fit framing and backends (sim + Meshtastic stdio bridge).

pub mod frag;
pub mod frame;
pub mod meshtastic;
pub mod sim;

pub use frag::{fragment_bytes, session_id_from_hash, FragAssembler};
pub use frame::{decode_frame, encode_frame, Frame, MsgType, FRAME_MAGIC};
pub use meshtastic::MeshtasticStdioTransport;
pub use sim::SimTransport;
