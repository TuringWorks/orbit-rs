//! MySQL packet encoding and decoding

use crate::error::{ProtocolError, ProtocolResult};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// MySQL packet
#[derive(Debug, Clone)]
pub struct MySqlPacket {
    /// Sequence ID
    pub sequence_id: u8,
    /// Payload
    pub payload: Bytes,
}

impl MySqlPacket {
    /// Create a new packet
    pub fn new(sequence_id: u8, payload: Bytes) -> Self {
        Self {
            sequence_id,
            payload,
        }
    }

    /// Encode packet to bytes
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(4 + self.payload.len());

        // Write payload length (3 bytes, little-endian)
        let len = self.payload.len() as u32;
        buf.put_u8((len & 0xFF) as u8);
        buf.put_u8(((len >> 8) & 0xFF) as u8);
        buf.put_u8(((len >> 16) & 0xFF) as u8);

        // Write sequence ID
        buf.put_u8(self.sequence_id);

        // Write payload
        buf.put(self.payload.clone());

        buf
    }

    /// Decode packet from bytes
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.len() < 4 {
            return Err(ProtocolError::IncompleteFrame);
        }

        // Read payload length (3 bytes, little-endian)
        let len = buf.get_u8() as u32 | (buf.get_u8() as u32) << 8 | (buf.get_u8() as u32) << 16;

        // Read sequence ID
        let sequence_id = buf.get_u8();

        if buf.len() < len as usize {
            return Err(ProtocolError::IncompleteFrame);
        }

        let payload = buf.copy_to_bytes(len as usize);

        Ok(Self {
            sequence_id,
            payload,
        })
    }
}

/// Write length-encoded integer
pub fn write_lenenc_int(buf: &mut BytesMut, value: u64) {
    if value < 251 {
        buf.put_u8(value as u8);
    } else if value < 65536 {
        buf.put_u8(0xFC);
        buf.put_u16_le(value as u16);
    } else if value < 16777216 {
        buf.put_u8(0xFD);
        buf.put_u8((value & 0xFF) as u8);
        buf.put_u8(((value >> 8) & 0xFF) as u8);
        buf.put_u8(((value >> 16) & 0xFF) as u8);
    } else {
        buf.put_u8(0xFE);
        buf.put_u64_le(value);
    }
}

/// Read length-encoded integer
pub fn read_lenenc_int(buf: &mut Bytes) -> ProtocolResult<u64> {
    if buf.is_empty() {
        return Err(ProtocolError::IncompleteFrame);
    }

    let first = buf.get_u8();
    match first {
        0..=250 => Ok(first as u64),
        0xFC => {
            if buf.len() < 2 {
                return Err(ProtocolError::IncompleteFrame);
            }
            Ok(buf.get_u16_le() as u64)
        }
        0xFD => {
            if buf.len() < 3 {
                return Err(ProtocolError::IncompleteFrame);
            }
            let b1 = buf.get_u8() as u64;
            let b2 = buf.get_u8() as u64;
            let b3 = buf.get_u8() as u64;
            Ok(b1 | (b2 << 8) | (b3 << 16))
        }
        0xFE => {
            if buf.len() < 8 {
                return Err(ProtocolError::IncompleteFrame);
            }
            Ok(buf.get_u64_le())
        }
        _ => Err(ProtocolError::ParseError(format!(
            "Invalid length-encoded integer: {}",
            first
        ))),
    }
}

/// Write length-encoded string
pub fn write_lenenc_string(buf: &mut BytesMut, s: &str) {
    write_lenenc_int(buf, s.len() as u64);
    buf.put(s.as_bytes());
}

/// Read length-encoded string
pub fn read_lenenc_string(buf: &mut Bytes) -> ProtocolResult<String> {
    let len = read_lenenc_int(buf)? as usize;
    if buf.len() < len {
        return Err(ProtocolError::IncompleteFrame);
    }
    let bytes = buf.copy_to_bytes(len);
    String::from_utf8(bytes.to_vec()).map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))
}

/// Write null-terminated string
pub fn write_null_string(buf: &mut BytesMut, s: &str) {
    buf.put(s.as_bytes());
    buf.put_u8(0);
}

/// Read null-terminated string
pub fn read_null_string(buf: &mut Bytes) -> ProtocolResult<String> {
    let mut bytes = Vec::new();
    loop {
        if buf.is_empty() {
            return Err(ProtocolError::IncompleteFrame);
        }
        let byte = buf.get_u8();
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_encode_decode() {
        let packet = MySqlPacket::new(1, Bytes::from("hello"));
        let encoded = packet.encode();
        let decoded = MySqlPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.sequence_id, 1);
        assert_eq!(decoded.payload, Bytes::from("hello"));
    }

    #[test]
    fn test_lenenc_int() {
        let mut buf = BytesMut::new();

        // Test small value
        write_lenenc_int(&mut buf, 100);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 100);

        // Test 2-byte value
        buf.clear();
        write_lenenc_int(&mut buf, 300);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 300);
    }

    #[test]
    fn test_lenenc_string() {
        let mut buf = BytesMut::new();
        write_lenenc_string(&mut buf, "hello world");

        let mut read_buf = buf.freeze();
        assert_eq!(read_lenenc_string(&mut read_buf).unwrap(), "hello world");
    }

    #[test]
    fn test_null_string() {
        let mut buf = BytesMut::new();
        write_null_string(&mut buf, "test");

        let mut read_buf = buf.freeze();
        assert_eq!(read_null_string(&mut read_buf).unwrap(), "test");
    }
}
