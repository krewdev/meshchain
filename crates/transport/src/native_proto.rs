//! Pure-Rust native protobuf formatting for Meshtastic wire protocol (`ToRadio` and `FromRadio`).
//!
//! Enables direct interaction with Meshtastic hardware (Serial, TCP, BLE) without requiring `python3`
//! or external C/Python protobuf libraries.
//!
//! Serial frame format:
//! `[0x94, 0xC3, len_msb, len_lsb, proto_payload...]`

use thiserror::Error;

pub const START1: u8 = 0x94;
pub const START2: u8 = 0xC3;
pub const DEFAULT_PORTNUM_MESHCHAIN: u32 = 265;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("Truncated buffer")]
    Truncated,
    #[error("Invalid serial framing sync bytes")]
    BadSync,
    #[error("Invalid protobuf wire format: {0}")]
    WireFormat(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshPacketData {
    pub from: u32,
    pub to: u32,
    pub channel: u32,
    pub portnum: u32,
    pub payload: Vec<u8>,
    pub hop_limit: u32,
    pub id: u32,
}

/// Encode a `u64` as protobuf varint bytes.
pub fn encode_varint(mut val: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
            out.push(byte);
        } else {
            out.push(byte);
            break;
        }
    }
}

/// Decode a protobuf varint from slice, returning `(value, bytes_consumed)`.
pub fn decode_varint(buf: &[u8]) -> Result<(u64, usize), ProtoError> {
    let mut result = 0u64;
    let mut shift = 0;
    for (i, &byte) in buf.iter().enumerate() {
        if shift >= 64 {
            return Err(ProtoError::WireFormat("Varint overflow".into()));
        }
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err(ProtoError::Truncated)
}

/// Encode length-delimited field `(tag_key | length_varint | bytes)`
pub fn encode_length_delimited(field_number: u32, data: &[u8], out: &mut Vec<u8>) {
    let key = (field_number << 3) | 2; // Wire type 2: Length-delimited
    encode_varint(key as u64, out);
    encode_varint(data.len() as u64, out);
    out.extend_from_slice(data);
}

/// Encode varint field `(tag_key | varint)` if `val != 0` (or `always` if true)
pub fn encode_varint_field(field_number: u32, val: u64, always: bool, out: &mut Vec<u8>) {
    if val != 0 || always {
        let key = field_number << 3; // Wire type 0: Varint
        encode_varint(key as u64, out);
        encode_varint(val, out);
    }
}

/// Encode `Data` submessage inside a `MeshPacket`.
pub fn encode_data_submsg(portnum: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // Field 1: portnum (varint)
    encode_varint_field(1, portnum as u64, true, &mut out);
    // Field 2: payload (bytes)
    encode_length_delimited(2, payload, &mut out);
    out
}

/// Encode `MeshPacket` submessage inside `ToRadio`.
pub fn encode_mesh_packet(data: &MeshPacketData) -> Vec<u8> {
    let mut out = Vec::new();
    encode_varint_field(1, data.from as u64, false, &mut out);
    encode_varint_field(2, data.to as u64, true, &mut out);
    encode_varint_field(3, data.channel as u64, false, &mut out);

    let data_bytes = encode_data_submsg(data.portnum, &data.payload);
    encode_length_delimited(4, &data_bytes, &mut out);

    encode_varint_field(5, data.id as u64, false, &mut out);
    if data.hop_limit != 0 {
        encode_varint_field(10, data.hop_limit as u64, true, &mut out);
    }
    out
}

/// Encode a `ToRadio` protobuf message containing a `packet` (`MeshPacket`, field 1).
pub fn encode_to_radio_packet(data: &MeshPacketData) -> Vec<u8> {
    let mut out = Vec::new();
    let pkt_bytes = encode_mesh_packet(data);
    encode_length_delimited(1, &pkt_bytes, &mut out);
    out
}

/// Wrap raw protobuf payload in standard Meshtastic serial framing `[0x94, 0xC3, len_msb, len_lsb, payload...]`
pub fn encode_serial_frame(proto_bytes: &[u8]) -> Result<Vec<u8>, ProtoError> {
    if proto_bytes.len() > u16::MAX as usize {
        return Err(ProtoError::WireFormat(
            "Payload exceeds 64KB serial limit".into(),
        ));
    }
    let len = proto_bytes.len() as u16;
    let mut out = Vec::with_capacity(4 + proto_bytes.len());
    out.push(START1);
    out.push(START2);
    out.push((len >> 8) as u8);
    out.push((len & 0xFF) as u8);
    out.extend_from_slice(proto_bytes);
    Ok(out)
}

/// Parse serial framing from buffer. Returns `(proto_payload, total_bytes_consumed)`.
pub fn decode_serial_frame(buf: &[u8]) -> Result<(&[u8], usize), ProtoError> {
    if buf.len() < 4 {
        return Err(ProtoError::Truncated);
    }
    if buf[0] != START1 || buf[1] != START2 {
        return Err(ProtoError::BadSync);
    }
    let len = ((buf[2] as usize) << 8) | (buf[3] as usize);
    if buf.len() < 4 + len {
        return Err(ProtoError::Truncated);
    }
    Ok((&buf[4..4 + len], 4 + len))
}

/// Decode a `Data` submessage (`portnum`, `payload`).
fn decode_data_submsg(buf: &[u8]) -> Result<(u32, Vec<u8>), ProtoError> {
    let mut pos = 0;
    let mut portnum = 0u32;
    let mut payload = Vec::new();

    while pos < buf.len() {
        let (key, consumed) = decode_varint(&buf[pos..])?;
        pos += consumed;
        let field_number = (key >> 3) as u32;
        let wire_type = (key & 7) as u8;

        match (field_number, wire_type) {
            (1, 0) => {
                let (val, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                portnum = val as u32;
            }
            (2, 2) => {
                let (len, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                let len = len as usize;
                if pos + len > buf.len() {
                    return Err(ProtoError::Truncated);
                }
                payload = buf[pos..pos + len].to_vec();
                pos += len;
            }
            (_, 0) => {
                let (_, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
            }
            (_, 2) => {
                let (len, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                if pos + len as usize > buf.len() {
                    return Err(ProtoError::Truncated);
                }
                pos += len as usize;
            }
            _ => {
                return Err(ProtoError::WireFormat(format!(
                    "Unsupported wire_type {wire_type} for tag {field_number}"
                )))
            }
        }
    }
    Ok((portnum, payload))
}

/// Decode a `MeshPacket` submessage from wire bytes.
pub fn decode_mesh_packet(buf: &[u8]) -> Result<MeshPacketData, ProtoError> {
    let mut pos = 0;
    let mut data = MeshPacketData {
        from: 0,
        to: 0,
        channel: 0,
        portnum: 0,
        payload: Vec::new(),
        hop_limit: 3,
        id: 0,
    };

    while pos < buf.len() {
        let (key, consumed) = decode_varint(&buf[pos..])?;
        pos += consumed;
        let field_number = (key >> 3) as u32;
        let wire_type = (key & 7) as u8;

        match (field_number, wire_type) {
            (1, 0) => {
                let (val, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                data.from = val as u32;
            }
            (2, 0) => {
                let (val, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                data.to = val as u32;
            }
            (3, 0) => {
                let (val, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                data.channel = val as u32;
            }
            (4, 2) => {
                let (len, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                let len = len as usize;
                if pos + len > buf.len() {
                    return Err(ProtoError::Truncated);
                }
                let (pnum, pbuf) = decode_data_submsg(&buf[pos..pos + len])?;
                data.portnum = pnum;
                data.payload = pbuf;
                pos += len;
            }
            (5, 0) => {
                let (val, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                data.id = val as u32;
            }
            (10, 0) => {
                let (val, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                data.hop_limit = val as u32;
            }
            (_, 0) => {
                let (_, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
            }
            (_, 2) => {
                let (len, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                if pos + len as usize > buf.len() {
                    return Err(ProtoError::Truncated);
                }
                pos += len as usize;
            }
            _ => {
                return Err(ProtoError::WireFormat(format!(
                    "Unsupported wire_type {wire_type} for tag {field_number}"
                )))
            }
        }
    }
    Ok(data)
}

/// Decode a `FromRadio` protobuf message containing `packet` (`MeshPacket`, field 2).
pub fn decode_from_radio_packet(buf: &[u8]) -> Result<Option<MeshPacketData>, ProtoError> {
    let mut pos = 0;
    while pos < buf.len() {
        let (key, consumed) = decode_varint(&buf[pos..])?;
        pos += consumed;
        let field_number = (key >> 3) as u32;
        let wire_type = (key & 7) as u8;

        match (field_number, wire_type) {
            (2, 2) => {
                // MeshPacket is field 2 inside FromRadio
                let (len, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                let len = len as usize;
                if pos + len > buf.len() {
                    return Err(ProtoError::Truncated);
                }
                let pkt = decode_mesh_packet(&buf[pos..pos + len])?;
                return Ok(Some(pkt));
            }
            (_, 0) => {
                let (_, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
            }
            (_, 2) => {
                let (len, consumed) = decode_varint(&buf[pos..])?;
                pos += consumed;
                if pos + len as usize > buf.len() {
                    return Err(ProtoError::Truncated);
                }
                pos += len as usize;
            }
            _ => {
                return Err(ProtoError::WireFormat(format!(
                    "Unsupported wire_type {wire_type} for tag {field_number}"
                )))
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toradio_protobuf_encoding() {
        let original = MeshPacketData {
            from: 0x11223344,
            to: 0xFFFFFFFF,
            channel: 1,
            portnum: DEFAULT_PORTNUM_MESHCHAIN,
            payload: b"MeshChain Native Protobuf over LoRa".to_vec(),
            hop_limit: 5,
            id: 123456,
        };

        // Encode to ToRadio protobuf
        let toradio_bytes = encode_to_radio_packet(&original);
        assert!(!toradio_bytes.is_empty());

        // Extract the MeshPacket (tag 1 inside ToRadio) and verify roundtrip
        let (key, consumed) = decode_varint(&toradio_bytes).unwrap();
        assert_eq!(key >> 3, 1);
        let (len, consumed2) = decode_varint(&toradio_bytes[consumed..]).unwrap();
        let pkt_bytes = &toradio_bytes[consumed + consumed2..consumed + consumed2 + len as usize];

        let decoded = decode_mesh_packet(pkt_bytes).unwrap();
        assert_eq!(decoded.from, original.from);
        assert_eq!(decoded.to, original.to);
        assert_eq!(decoded.channel, original.channel);
        assert_eq!(decoded.portnum, original.portnum);
        assert_eq!(decoded.payload, original.payload);
        assert_eq!(decoded.hop_limit, original.hop_limit);
        assert_eq!(decoded.id, original.id);
    }

    #[test]
    fn test_serial_framing_roundtrip() {
        let payload = b"native proto bytes test";
        let framed = encode_serial_frame(payload).unwrap();
        assert_eq!(framed[0], START1);
        assert_eq!(framed[1], START2);
        assert_eq!(framed.len(), 4 + payload.len());

        let (decoded, consumed) = decode_serial_frame(&framed).unwrap();
        assert_eq!(decoded, payload);
        assert_eq!(consumed, framed.len());
    }

    #[test]
    fn test_fromradio_protobuf_decoding() {
        let original = MeshPacketData {
            from: 100,
            to: 200,
            channel: 0,
            portnum: 265,
            payload: b"Hello FromRadio".to_vec(),
            hop_limit: 3,
            id: 999,
        };
        let pkt_bytes = encode_mesh_packet(&original);

        // Build a simulated FromRadio (field 2 = MeshPacket)
        let mut from_radio_buf = Vec::new();
        encode_length_delimited(2, &pkt_bytes, &mut from_radio_buf);

        let decoded_opt = decode_from_radio_packet(&from_radio_buf).unwrap();
        assert!(decoded_opt.is_some());
        let decoded = decoded_opt.unwrap();
        assert_eq!(decoded.from, original.from);
        assert_eq!(decoded.to, original.to);
        assert_eq!(decoded.portnum, original.portnum);
        assert_eq!(decoded.payload, original.payload);
    }
}
