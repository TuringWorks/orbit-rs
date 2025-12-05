//! MySQL packet encoding and decoding
//!
//! Implements the MySQL wire protocol packet format.
//!
//! ## Packet Format
//! ```text
//! +-------------------+------------------+-------------------+
//! | payload_length(3) | sequence_id(1)   | payload           |
//! +-------------------+------------------+-------------------+
//! ```
//!
//! ## References
//! - MySQL Protocol Spec: `specifications/protocols/mysql-mariadb-reference-rust.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/mysql>

use crate::protocols::error::{ProtocolError, ProtocolResult};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Maximum payload size for a single packet (16 MB - 1)
///
/// MySQL protocol limits each packet's payload to 2^24 - 1 bytes.
/// Larger data must be split across multiple packets.
pub const MAX_PACKET_SIZE: usize = 16_777_215; // 2^24 - 1 = 0xFFFFFF

/// Packet header size (3 bytes length + 1 byte sequence ID)
pub const PACKET_HEADER_SIZE: usize = 4;

/// Maximum payload length that fits in the 3-byte length field
pub const MAX_PAYLOAD_LENGTH: u32 = 0xFFFFFF;

/// MySQL packet
///
/// Represents a single MySQL protocol packet with a sequence ID and payload.
/// For payloads larger than [`MAX_PACKET_SIZE`], the data must be split
/// across multiple packets with incrementing sequence IDs.
#[derive(Debug, Clone)]
pub struct MySqlPacket {
    /// Sequence ID (0-255, wraps around)
    pub sequence_id: u8,
    /// Payload data (max [`MAX_PACKET_SIZE`] bytes)
    pub payload: Bytes,
}

impl MySqlPacket {
    /// Create a new packet
    ///
    /// # Panics
    /// Panics if payload exceeds [`MAX_PACKET_SIZE`].
    /// Use [`MySqlPacket::try_new`] for fallible construction.
    pub fn new(sequence_id: u8, payload: Bytes) -> Self {
        assert!(
            payload.len() <= MAX_PACKET_SIZE,
            "Payload exceeds MAX_PACKET_SIZE"
        );
        Self {
            sequence_id,
            payload,
        }
    }

    /// Try to create a new packet, returning an error if payload is too large
    pub fn try_new(sequence_id: u8, payload: Bytes) -> ProtocolResult<Self> {
        if payload.len() > MAX_PACKET_SIZE {
            return Err(ProtocolError::ParseError(format!(
                "Payload size {} exceeds maximum {}",
                payload.len(),
                MAX_PACKET_SIZE
            )));
        }
        Ok(Self {
            sequence_id,
            payload,
        })
    }

    /// Check if this packet requires splitting for the MySQL protocol
    pub fn requires_splitting(&self) -> bool {
        self.payload.len() >= MAX_PACKET_SIZE
    }

    /// Get the total encoded size of this packet (header + payload)
    pub fn encoded_size(&self) -> usize {
        PACKET_HEADER_SIZE + self.payload.len()
    }

    /// Encode packet to bytes
    ///
    /// Format: [length(3)][sequence_id(1)][payload]
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(PACKET_HEADER_SIZE + self.payload.len());

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
    ///
    /// Returns the decoded packet if sufficient data is available.
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.len() < PACKET_HEADER_SIZE {
            return Err(ProtocolError::IncompleteFrame);
        }

        // Read payload length (3 bytes, little-endian)
        let len = buf.get_u8() as u32 | (buf.get_u8() as u32) << 8 | (buf.get_u8() as u32) << 16;

        // Read sequence ID
        let sequence_id = buf.get_u8();

        // Validate length doesn't exceed protocol maximum
        if len > MAX_PAYLOAD_LENGTH {
            return Err(ProtocolError::ParseError(format!(
                "Packet length {} exceeds maximum {}",
                len, MAX_PAYLOAD_LENGTH
            )));
        }

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
    fn test_constants() {
        assert_eq!(MAX_PACKET_SIZE, 16_777_215);
        assert_eq!(MAX_PACKET_SIZE, 0xFFFFFF);
        assert_eq!(PACKET_HEADER_SIZE, 4);
        assert_eq!(MAX_PAYLOAD_LENGTH, 0xFFFFFF);
    }

    #[test]
    fn test_packet_encode_decode() {
        let packet = MySqlPacket::new(1, Bytes::from("hello"));
        let encoded = packet.encode();
        let decoded = MySqlPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.sequence_id, 1);
        assert_eq!(decoded.payload, Bytes::from("hello"));
    }

    #[test]
    fn test_packet_try_new() {
        // Valid packet
        let packet = MySqlPacket::try_new(1, Bytes::from("hello")).unwrap();
        assert_eq!(packet.sequence_id, 1);
        assert_eq!(packet.payload, Bytes::from("hello"));

        // We can't easily test the error case without creating a huge payload
    }

    #[test]
    fn test_packet_encoded_size() {
        let packet = MySqlPacket::new(1, Bytes::from("hello"));
        assert_eq!(packet.encoded_size(), PACKET_HEADER_SIZE + 5); // 4 + 5 = 9
    }

    #[test]
    fn test_packet_requires_splitting() {
        let small_packet = MySqlPacket::new(1, Bytes::from("hello"));
        assert!(!small_packet.requires_splitting());

        // MAX_PACKET_SIZE exactly should require splitting
        // (We can't test this easily without creating a 16MB payload)
    }

    #[test]
    fn test_lenenc_int() {
        let mut buf = BytesMut::new();

        // Test small value (1 byte: 0-250)
        write_lenenc_int(&mut buf, 100);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 100);

        // Test 2-byte value (0xFC prefix: 251-65535)
        buf.clear();
        write_lenenc_int(&mut buf, 300);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 300);

        // Test 3-byte value (0xFD prefix: 65536-16777215)
        buf.clear();
        write_lenenc_int(&mut buf, 100_000);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 100_000);

        // Test 8-byte value (0xFE prefix: >16777215)
        buf.clear();
        write_lenenc_int(&mut buf, 20_000_000);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 20_000_000);
    }

    #[test]
    fn test_lenenc_int_edge_cases() {
        let mut buf = BytesMut::new();

        // Test boundary: 250 (1 byte)
        write_lenenc_int(&mut buf, 250);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 250);

        // Test boundary: 251 (2 bytes)
        buf.clear();
        write_lenenc_int(&mut buf, 251);
        assert_eq!(buf.len(), 3); // 0xFC + 2 bytes

        // Test boundary: 65535 (2 bytes)
        buf.clear();
        write_lenenc_int(&mut buf, 65535);
        let mut read_buf = buf.clone().freeze();
        assert_eq!(read_lenenc_int(&mut read_buf).unwrap(), 65535);

        // Test boundary: 65536 (3 bytes)
        buf.clear();
        write_lenenc_int(&mut buf, 65536);
        assert_eq!(buf.len(), 4); // 0xFD + 3 bytes
    }

    #[test]
    fn test_lenenc_string() {
        let mut buf = BytesMut::new();
        write_lenenc_string(&mut buf, "hello world");

        let mut read_buf = buf.freeze();
        assert_eq!(read_lenenc_string(&mut read_buf).unwrap(), "hello world");
    }

    #[test]
    fn test_lenenc_string_empty() {
        let mut buf = BytesMut::new();
        write_lenenc_string(&mut buf, "");

        let mut read_buf = buf.freeze();
        assert_eq!(read_lenenc_string(&mut read_buf).unwrap(), "");
    }

    #[test]
    fn test_null_string() {
        let mut buf = BytesMut::new();
        write_null_string(&mut buf, "test");

        let mut read_buf = buf.freeze();
        assert_eq!(read_null_string(&mut read_buf).unwrap(), "test");
    }

    #[test]
    fn test_null_string_empty() {
        let mut buf = BytesMut::new();
        write_null_string(&mut buf, "");

        let mut read_buf = buf.freeze();
        assert_eq!(read_null_string(&mut read_buf).unwrap(), "");
    }

    #[test]
    fn test_decode_incomplete_header() {
        let buf = Bytes::from_static(&[0x05, 0x00]); // Only 2 bytes
        let result = MySqlPacket::decode(buf);
        assert!(matches!(result, Err(ProtocolError::IncompleteFrame)));
    }

    #[test]
    fn test_decode_incomplete_payload() {
        let buf = Bytes::from_static(&[0x05, 0x00, 0x00, 0x01, 0x68, 0x65]); // Length=5, but only 2 bytes payload
        let result = MySqlPacket::decode(buf);
        assert!(matches!(result, Err(ProtocolError::IncompleteFrame)));
    }
}
