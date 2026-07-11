//! MeshChain transport: LoRa-fit framing and backends (sim + Meshtastic stdio bridge).

pub mod frag;
pub mod frame;
pub mod meshtastic;
pub mod sim;

pub use frag::{fragment_bytes, session_id_from_hash, FragAssembler};
pub use frame::{
    block_fits_air, decode_block, decode_block_ack, decode_block_hint, decode_frame, decode_tip,
    decode_tx, encode_block, encode_block_ack, encode_block_for_air, encode_block_hint, encode_frame,
    encode_tip, encode_tx, tx_fits_air, BlockAckPayload, BlockHintPayload, Frame, MsgType,
    TipPayload, AIR_MAX_TXS_PER_BLOCK, FRAME_MAGIC, MAX_PAYLOAD,
};
pub use meshtastic::MeshtasticStdioTransport;
pub use sim::SimTransport;
