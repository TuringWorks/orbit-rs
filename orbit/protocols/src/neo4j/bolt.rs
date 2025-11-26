//! Bolt Protocol Implementation for Neo4j Compatibility
//!
//! Implements the Bolt v4 protocol for communicating with Neo4j clients.
//! References: https://7687.org/bolt/bolt-protocol-message-specification-4.html

use crate::error::{ProtocolError, ProtocolResult};
use bytes::{BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Bolt Protocol Version 4.1
const BOLT_V4_1: u32 = 0x00040100;
/// Bolt Protocol Version 4.0
const BOLT_V4_0: u32 = 0x00040000;
/// Magic preamble for Bolt protocol (0x6060B017)
const BOLT_MAGIC: u32 = 0x6060B017;

/// Bolt Message Types
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum MessageType {
    Hello = 0x01,
    Goodbye = 0x02,
    Reset = 0x0F,
    Run = 0x10,
    Pull = 0x3F,
    Discard = 0x2F,
    Begin = 0x11,
    Commit = 0x12,
    Rollback = 0x13,
    Success = 0x70,
    Failure = 0x7F,
    Ignored = 0x7E,
    Record = 0x71,
}

/// PackStream Value types
#[derive(Debug, Clone)]
pub enum PackStreamValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<PackStreamValue>),
    Map(HashMap<String, PackStreamValue>),
    Bytes(Vec<u8>),
}

impl PackStreamValue {
    /// Convert to string if possible
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PackStreamValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to i64 if possible
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            PackStreamValue::Integer(i) => Some(*i),
            _ => None,
        }
    }
}

/// Bolt Protocol Handler
pub struct BoltProtocol {
    version: u32,
    state: BoltState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BoltState {
    Handshake,
    Connected,
    Ready,
    Streaming,
    Failed,
}

impl BoltProtocol {
    pub fn new() -> Self {
        Self {
            version: 0,
            state: BoltState::Handshake,
        }
    }

    /// Perform Bolt handshake
    pub async fn handshake<S>(&mut self, stream: &mut S) -> ProtocolResult<u32>
    where
        S: AsyncReadExt + AsyncWriteExt + Unpin,
    {
        // 1. Read Magic Preamble (4 bytes)
        let mut magic_buf = [0u8; 4];
        stream
            .read_exact(&mut magic_buf)
            .await
            .map_err(ProtocolError::from)?;

        let magic = u32::from_be_bytes(magic_buf);
        if magic != BOLT_MAGIC {
            return Err(ProtocolError::HandshakeError(format!(
                "Invalid magic preamble: {:x}",
                magic
            )));
        }

        // 2. Read 4 supported versions (16 bytes)
        let mut versions_buf = [0u8; 16];
        stream
            .read_exact(&mut versions_buf)
            .await
            .map_err(ProtocolError::from)?;

        // 3. Select version
        // We support v4.1 and v4.0
        let mut selected_version = 0;
        for i in 0..4 {
            let start = i * 4;
            let ver = u32::from_be_bytes([
                versions_buf[start],
                versions_buf[start + 1],
                versions_buf[start + 2],
                versions_buf[start + 3],
            ]);

            if ver == BOLT_V4_1 || ver == BOLT_V4_0 {
                selected_version = ver;
                break;
            }
        }

        // 4. Send selected version
        stream
            .write_all(&selected_version.to_be_bytes())
            .await
            .map_err(ProtocolError::from)?;

        if selected_version == 0 {
            return Err(ProtocolError::HandshakeError(
                "No supported Bolt version found".to_string(),
            ));
        }

        self.version = selected_version;
        self.state = BoltState::Connected;

        Ok(selected_version)
    }

    /// Read a message from the stream
    pub fn read_message<'a, S>(
        &'a mut self,
        stream: &'a mut S,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProtocolResult<(MessageType, Bytes)>> + Send + 'a>>
    where
        S: AsyncReadExt + Unpin + Send,
    {
        Box::pin(async move {
            // Read chunk header (2 bytes)
            let mut header = [0u8; 2];
            stream
                .read_exact(&mut header)
                .await
                .map_err(ProtocolError::from)?;
            let chunk_size = u16::from_be_bytes(header) as usize;

            if chunk_size == 0 {
                // No-op / Keep-alive - use loop instead of recursion
                loop {
                    let mut next_header = [0u8; 2];
                    stream
                        .read_exact(&mut next_header)
                        .await
                        .map_err(ProtocolError::from)?;
                    let next_size = u16::from_be_bytes(next_header) as usize;
                    if next_size > 0 {
                        // Found actual data, read it
                        let mut data = vec![0u8; next_size];
                        stream
                            .read_exact(&mut data)
                            .await
                            .map_err(ProtocolError::from)?;
                        return self.parse_message_data(data);
                    }
                }
            }

            // Read chunk data
            let mut data = vec![0u8; chunk_size];
            stream
                .read_exact(&mut data)
                .await
                .map_err(ProtocolError::from)?;

            self.parse_message_data(data)
        })
    }

    /// Parse message data into message type and bytes
    fn parse_message_data(&self, data: Vec<u8>) -> ProtocolResult<(MessageType, Bytes)> {
        // Parse PackStream marker
        // For simplicity, assuming single chunk messages for now
        // Byte 0 is marker, Byte 1 is signature (Message Type)
        // Actually, PackStream structure: [Marker][Signature][Fields...]
        // Structure marker is 0xB0 + size (usually 0xB1 for 1 field, etc.)

        if data.len() < 2 {
            return Err(ProtocolError::DecodingError(
                "Message too short".to_string(),
            ));
        }

        let _marker = data[0];
        let signature = data[1];

        let msg_type = match signature {
            0x01 => MessageType::Hello,
            0x02 => MessageType::Goodbye,
            0x10 => MessageType::Run,
            0x3F => MessageType::Pull,
            _ => {
                return Err(ProtocolError::DecodingError(format!(
                    "Unknown message signature: {:x}",
                    signature
                )))
            }
        };

        Ok((msg_type, Bytes::from(data)))
    }

    /// Send a SUCCESS message
    pub async fn send_success<S>(
        &mut self,
        stream: &mut S,
        metadata: &[(&str, &str)],
    ) -> ProtocolResult<()>
    where
        S: AsyncWriteExt + Unpin,
    {
        // Construct simple PackStream SUCCESS message
        // Structure(0xB1), Signature(0x70), Map(...)
        // For MVP, sending empty map or minimal metadata

        let mut buf = BytesMut::new();
        // Structure size 1 (Signature + Map) -> 0xB1? No, Structure is Marker(0xB0 | size)
        // Message is a Structure.
        // SUCCESS is Structure(size=1) -> [0xB1, 0x70, Map]

        buf.extend_from_slice(&[0xB1, 0x70]);

        // Empty Map: 0xA0
        buf.extend_from_slice(&[0xA0]);

        self.write_chunk(stream, &buf).await
    }

    /// Send a FAILURE message
    pub async fn send_failure<S>(
        &mut self,
        stream: &mut S,
        code: &str,
        message: &str,
    ) -> ProtocolResult<()>
    where
        S: AsyncWriteExt + Unpin,
    {
        // FAILURE is Structure(size=1) -> [0xB1, 0x7F, Map]
        // Map contains "code" and "message"

        // For MVP, simplified error
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[0xB1, 0x7F]);
        buf.extend_from_slice(&[0xA0]); // Empty map for now to avoid complex PackStream encoding in this step

        self.write_chunk(stream, &buf).await
    }

    async fn write_chunk<S>(&mut self, stream: &mut S, data: &[u8]) -> ProtocolResult<()>
    where
        S: AsyncWriteExt + Unpin,
    {
        let len = data.len();
        if len > 65535 {
            return Err(ProtocolError::EncodingError(
                "Message too large for single chunk".to_string(),
            ));
        }

        stream
            .write_all(&(len as u16).to_be_bytes())
            .await
            .map_err(ProtocolError::from)?;
        stream.write_all(data).await.map_err(ProtocolError::from)?;
        stream
            .write_all(&[0, 0])
            .await
            .map_err(ProtocolError::from)?; // End of message marker

        Ok(())
    }

    /// Send a RECORD message with fields
    pub async fn send_record<S>(
        &mut self,
        stream: &mut S,
        fields: &[PackStreamValue],
    ) -> ProtocolResult<()>
    where
        S: AsyncWriteExt + Unpin,
    {
        let mut buf = BytesMut::new();

        // RECORD structure: [0xB1, 0x71, List of fields]
        buf.extend_from_slice(&[0xB1, 0x71]);

        // Encode the list of fields
        self.encode_list(&mut buf, fields)?;

        self.write_chunk(stream, &buf).await
    }

    /// Encode a PackStream value
    fn encode_value(&self, buf: &mut BytesMut, value: &PackStreamValue) -> ProtocolResult<()> {
        match value {
            PackStreamValue::Null => {
                buf.put_u8(0xC0);
            }
            PackStreamValue::Boolean(b) => {
                buf.put_u8(if *b { 0xC3 } else { 0xC2 });
            }
            PackStreamValue::Integer(i) => {
                self.encode_integer(buf, *i);
            }
            PackStreamValue::Float(f) => {
                buf.put_u8(0xC1);
                buf.put_f64(*f);
            }
            PackStreamValue::String(s) => {
                self.encode_string(buf, s)?;
            }
            PackStreamValue::List(items) => {
                self.encode_list(buf, items)?;
            }
            PackStreamValue::Map(map) => {
                self.encode_map(buf, map)?;
            }
            PackStreamValue::Bytes(bytes) => {
                let len = bytes.len();
                if len <= 255 {
                    buf.put_u8(0xCC);
                    buf.put_u8(len as u8);
                } else if len <= 65535 {
                    buf.put_u8(0xCD);
                    buf.put_u16(len as u16);
                } else {
                    buf.put_u8(0xCE);
                    buf.put_u32(len as u32);
                }
                buf.extend_from_slice(bytes);
            }
        }
        Ok(())
    }

    fn encode_integer(&self, buf: &mut BytesMut, i: i64) {
        if (-16..=127).contains(&i) {
            // Tiny int
            buf.put_i8(i as i8);
        } else if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
            buf.put_u8(0xC8);
            buf.put_i8(i as i8);
        } else if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
            buf.put_u8(0xC9);
            buf.put_i16(i as i16);
        } else if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
            buf.put_u8(0xCA);
            buf.put_i32(i as i32);
        } else {
            buf.put_u8(0xCB);
            buf.put_i64(i);
        }
    }

    fn encode_string(&self, buf: &mut BytesMut, s: &str) -> ProtocolResult<()> {
        let bytes = s.as_bytes();
        let len = bytes.len();

        if len <= 15 {
            buf.put_u8(0x80 | len as u8);
        } else if len <= 255 {
            buf.put_u8(0xD0);
            buf.put_u8(len as u8);
        } else if len <= 65535 {
            buf.put_u8(0xD1);
            buf.put_u16(len as u16);
        } else if len <= u32::MAX as usize {
            buf.put_u8(0xD2);
            buf.put_u32(len as u32);
        } else {
            return Err(ProtocolError::EncodingError("String too long".to_string()));
        }
        buf.extend_from_slice(bytes);
        Ok(())
    }

    fn encode_list(&self, buf: &mut BytesMut, items: &[PackStreamValue]) -> ProtocolResult<()> {
        let len = items.len();

        if len <= 15 {
            buf.put_u8(0x90 | len as u8);
        } else if len <= 255 {
            buf.put_u8(0xD4);
            buf.put_u8(len as u8);
        } else if len <= 65535 {
            buf.put_u8(0xD5);
            buf.put_u16(len as u16);
        } else if len <= u32::MAX as usize {
            buf.put_u8(0xD6);
            buf.put_u32(len as u32);
        } else {
            return Err(ProtocolError::EncodingError("List too long".to_string()));
        }

        for item in items {
            self.encode_value(buf, item)?;
        }
        Ok(())
    }

    fn encode_map(
        &self,
        buf: &mut BytesMut,
        map: &HashMap<String, PackStreamValue>,
    ) -> ProtocolResult<()> {
        let len = map.len();

        if len <= 15 {
            buf.put_u8(0xA0 | len as u8);
        } else if len <= 255 {
            buf.put_u8(0xD8);
            buf.put_u8(len as u8);
        } else if len <= 65535 {
            buf.put_u8(0xD9);
            buf.put_u16(len as u16);
        } else if len <= u32::MAX as usize {
            buf.put_u8(0xDA);
            buf.put_u32(len as u32);
        } else {
            return Err(ProtocolError::EncodingError("Map too large".to_string()));
        }

        for (key, value) in map {
            self.encode_string(buf, key)?;
            self.encode_value(buf, value)?;
        }
        Ok(())
    }
}

/// PackStream Decoder
pub struct PackStreamDecoder;

impl PackStreamDecoder {
    /// Decode a PackStream value from bytes
    pub fn decode(data: &[u8]) -> ProtocolResult<(PackStreamValue, usize)> {
        if data.is_empty() {
            return Err(ProtocolError::DecodingError("Empty data".to_string()));
        }

        let marker = data[0];
        Self::decode_value(data, 0)
    }

    fn decode_value(data: &[u8], offset: usize) -> ProtocolResult<(PackStreamValue, usize)> {
        if offset >= data.len() {
            return Err(ProtocolError::DecodingError(
                "Unexpected end of data".to_string(),
            ));
        }

        let marker = data[offset];

        match marker {
            // Null
            0xC0 => Ok((PackStreamValue::Null, 1)),

            // Boolean
            0xC2 => Ok((PackStreamValue::Boolean(false), 1)),
            0xC3 => Ok((PackStreamValue::Boolean(true), 1)),

            // Float64
            0xC1 => {
                if offset + 9 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for float".to_string(),
                    ));
                }
                let bytes: [u8; 8] = data[offset + 1..offset + 9]
                    .try_into()
                    .map_err(|_| ProtocolError::DecodingError("Invalid float bytes".to_string()))?;
                Ok((PackStreamValue::Float(f64::from_be_bytes(bytes)), 9))
            }

            // Tiny int (-16 to 127)
            0x00..=0x7F => Ok((PackStreamValue::Integer(marker as i64), 1)),
            0xF0..=0xFF => Ok((PackStreamValue::Integer((marker as i8) as i64), 1)),

            // INT8
            0xC8 => {
                if offset + 2 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for int8".to_string(),
                    ));
                }
                Ok((PackStreamValue::Integer((data[offset + 1] as i8) as i64), 2))
            }

            // INT16
            0xC9 => {
                if offset + 3 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for int16".to_string(),
                    ));
                }
                let bytes: [u8; 2] = data[offset + 1..offset + 3]
                    .try_into()
                    .map_err(|_| ProtocolError::DecodingError("Invalid int16 bytes".to_string()))?;
                Ok((
                    PackStreamValue::Integer(i16::from_be_bytes(bytes) as i64),
                    3,
                ))
            }

            // INT32
            0xCA => {
                if offset + 5 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for int32".to_string(),
                    ));
                }
                let bytes: [u8; 4] = data[offset + 1..offset + 5]
                    .try_into()
                    .map_err(|_| ProtocolError::DecodingError("Invalid int32 bytes".to_string()))?;
                Ok((
                    PackStreamValue::Integer(i32::from_be_bytes(bytes) as i64),
                    5,
                ))
            }

            // INT64
            0xCB => {
                if offset + 9 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for int64".to_string(),
                    ));
                }
                let bytes: [u8; 8] = data[offset + 1..offset + 9]
                    .try_into()
                    .map_err(|_| ProtocolError::DecodingError("Invalid int64 bytes".to_string()))?;
                Ok((PackStreamValue::Integer(i64::from_be_bytes(bytes)), 9))
            }

            // Tiny string (length 0-15)
            0x80..=0x8F => {
                let len = (marker & 0x0F) as usize;
                Self::decode_string_with_len(data, offset + 1, len)
            }

            // String8
            0xD0 => {
                if offset + 2 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for string length".to_string(),
                    ));
                }
                let len = data[offset + 1] as usize;
                Self::decode_string_with_len(data, offset + 2, len)
                    .map(|(v, consumed)| (v, consumed + 1))
            }

            // String16
            0xD1 => {
                if offset + 3 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for string length".to_string(),
                    ));
                }
                let len = u16::from_be_bytes([data[offset + 1], data[offset + 2]]) as usize;
                Self::decode_string_with_len(data, offset + 3, len)
                    .map(|(v, consumed)| (v, consumed + 2))
            }

            // String32
            0xD2 => {
                if offset + 5 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for string length".to_string(),
                    ));
                }
                let len = u32::from_be_bytes([
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                    data[offset + 4],
                ]) as usize;
                Self::decode_string_with_len(data, offset + 5, len)
                    .map(|(v, consumed)| (v, consumed + 4))
            }

            // Tiny list (length 0-15)
            0x90..=0x9F => {
                let len = (marker & 0x0F) as usize;
                Self::decode_list(data, offset + 1, len)
            }

            // List8
            0xD4 => {
                if offset + 2 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for list length".to_string(),
                    ));
                }
                let len = data[offset + 1] as usize;
                Self::decode_list(data, offset + 2, len).map(|(v, consumed)| (v, consumed + 1))
            }

            // Tiny map (length 0-15)
            0xA0..=0xAF => {
                let len = (marker & 0x0F) as usize;
                Self::decode_map(data, offset + 1, len)
            }

            // Map8
            0xD8 => {
                if offset + 2 > data.len() {
                    return Err(ProtocolError::DecodingError(
                        "Not enough data for map length".to_string(),
                    ));
                }
                let len = data[offset + 1] as usize;
                Self::decode_map(data, offset + 2, len).map(|(v, consumed)| (v, consumed + 1))
            }

            // Structure marker (used for messages)
            0xB0..=0xBF => {
                // Structure - the size is in low nibble, next byte is signature
                let _size = (marker & 0x0F) as usize;
                // For now, return the raw data as a list
                Ok((PackStreamValue::Null, 1))
            }

            _ => Err(ProtocolError::DecodingError(format!(
                "Unknown PackStream marker: 0x{:02X}",
                marker
            ))),
        }
    }

    fn decode_string_with_len(
        data: &[u8],
        offset: usize,
        len: usize,
    ) -> ProtocolResult<(PackStreamValue, usize)> {
        if offset + len > data.len() {
            return Err(ProtocolError::DecodingError(
                "Not enough data for string content".to_string(),
            ));
        }
        let s = String::from_utf8(data[offset..offset + len].to_vec())
            .map_err(|_| ProtocolError::DecodingError("Invalid UTF-8 in string".to_string()))?;
        Ok((PackStreamValue::String(s), 1 + len))
    }

    fn decode_list(
        data: &[u8],
        offset: usize,
        count: usize,
    ) -> ProtocolResult<(PackStreamValue, usize)> {
        let mut items = Vec::with_capacity(count);
        let mut consumed = 1; // marker byte
        let mut pos = offset;

        for _ in 0..count {
            let (value, len) = Self::decode_value(data, pos)?;
            items.push(value);
            pos += len;
            consumed += len;
        }

        Ok((PackStreamValue::List(items), consumed))
    }

    fn decode_map(
        data: &[u8],
        offset: usize,
        count: usize,
    ) -> ProtocolResult<(PackStreamValue, usize)> {
        let mut map = HashMap::new();
        let mut consumed = 1; // marker byte
        let mut pos = offset;

        for _ in 0..count {
            // Decode key (must be string)
            let (key_value, key_len) = Self::decode_value(data, pos)?;
            let key = match key_value {
                PackStreamValue::String(s) => s,
                _ => {
                    return Err(ProtocolError::DecodingError(
                        "Map key must be string".to_string(),
                    ))
                }
            };
            pos += key_len;
            consumed += key_len;

            // Decode value
            let (value, val_len) = Self::decode_value(data, pos)?;
            pos += val_len;
            consumed += val_len;

            map.insert(key, value);
        }

        Ok((PackStreamValue::Map(map), consumed))
    }
}

/// Parsed RUN message
#[derive(Debug)]
pub struct RunMessage {
    pub query: String,
    pub parameters: HashMap<String, PackStreamValue>,
}

impl BoltProtocol {
    /// Parse a RUN message from raw data
    pub fn parse_run_message(&self, data: &[u8]) -> ProtocolResult<RunMessage> {
        // RUN message structure:
        // [0xB2 or 0xB3] (Structure marker with 2-3 fields)
        // [0x10] (RUN signature)
        // [String: query]
        // [Map: parameters]
        // [Map: extra] (optional in Bolt 4.x)

        if data.len() < 4 {
            return Err(ProtocolError::DecodingError(
                "RUN message too short".to_string(),
            ));
        }

        let marker = data[0];
        let signature = data[1];

        if signature != 0x10 {
            return Err(ProtocolError::DecodingError(format!(
                "Expected RUN signature 0x10, got 0x{:02X}",
                signature
            )));
        }

        // Decode query string
        let (query_value, query_len) = PackStreamDecoder::decode_value(data, 2)?;
        let query = match query_value {
            PackStreamValue::String(s) => s,
            _ => {
                return Err(ProtocolError::DecodingError(
                    "RUN query must be string".to_string(),
                ))
            }
        };

        // Decode parameters map
        let params_offset = 2 + query_len;
        let parameters = if params_offset < data.len() {
            let (params_value, _) = PackStreamDecoder::decode_value(data, params_offset)?;
            match params_value {
                PackStreamValue::Map(m) => m,
                PackStreamValue::Null => HashMap::new(),
                _ => {
                    return Err(ProtocolError::DecodingError(
                        "RUN parameters must be map".to_string(),
                    ))
                }
            }
        } else {
            HashMap::new()
        };

        Ok(RunMessage { query, parameters })
    }
}
