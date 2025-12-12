//! Concrete implementation of BoltProtocolWriter using PackStream encoding

use crate::protocols::error::ProtocolResult;
use crate::protocols::neo4j::bolt_messages::BoltProtocolWriter;
use bytes::BytesMut;
use serde_json::Value;
use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// Concrete Bolt protocol writer implementation
pub struct BoltWriter {
    encoder: PackStreamEncoder,
}

impl BoltWriter {
    /// Create a new Bolt protocol writer
    pub fn new() -> Self {
        Self {
            encoder: PackStreamEncoder::new(),
        }
    }

    /// Write a chunked message to the stream
    async fn write_chunked<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        data: &[u8],
    ) -> ProtocolResult<()> {
        const MAX_CHUNK_SIZE: usize = 65535;

        let mut offset = 0;
        while offset < data.len() {
            let chunk_size = (data.len() - offset).min(MAX_CHUNK_SIZE);

            // Write chunk size (2 bytes, big-endian)
            stream.write_u16(chunk_size as u16).await?;

            // Write chunk data
            stream.write_all(&data[offset..offset + chunk_size]).await?;

            offset += chunk_size;
        }

        // Write end marker (0x0000)
        stream.write_u16(0).await?;
        stream.flush().await?;

        Ok(())
    }
}

impl Default for BoltWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BoltProtocolWriter for BoltWriter {
    /// Send SUCCESS message
    async fn send_success<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        metadata: HashMap<String, Value>,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();

        // SUCCESS message structure (signature 0x70)
        // Tiny struct with 1 field: metadata map
        buf.extend_from_slice(&[0xB1, 0x70]); // TinyStruct(1) + signature

        // Encode metadata map
        self.encoder.encode_map(&mut buf, &metadata)?;

        self.write_chunked(stream, &buf).await
    }

    /// Send RECORD message
    async fn send_record<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        fields: Vec<Value>,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();

        // RECORD message structure (signature 0x71)
        // Tiny struct with 1 field: list of values
        buf.extend_from_slice(&[0xB1, 0x71]); // TinyStruct(1) + signature

        // Encode fields as list
        self.encoder.encode_list(&mut buf, &fields)?;

        self.write_chunked(stream, &buf).await
    }

    /// Send FAILURE message
    async fn send_failure<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        code: &str,
        message: &str,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();

        // FAILURE message structure (signature 0x7F)
        // Tiny struct with 1 field: metadata map
        buf.extend_from_slice(&[0xB1, 0x7F]); // TinyStruct(1) + signature

        // Create metadata map
        let mut metadata = HashMap::new();
        metadata.insert("code".to_string(), Value::String(code.to_string()));
        metadata.insert("message".to_string(), Value::String(message.to_string()));

        // Encode metadata map
        self.encoder.encode_map(&mut buf, &metadata)?;

        self.write_chunked(stream, &buf).await
    }

    /// Send IGNORED message
    async fn send_ignored<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();

        // IGNORED message structure (signature 0x7E)
        // Tiny struct with 0 fields
        buf.extend_from_slice(&[0xB0, 0x7E]); // TinyStruct(0) + signature

        self.write_chunked(stream, &buf).await
    }
}

/// PackStream encoder for Bolt protocol
pub struct PackStreamEncoder;

impl PackStreamEncoder {
    pub fn new() -> Self {
        Self
    }

    /// Encode a map to PackStream format
    pub fn encode_map(
        &mut self,
        buf: &mut BytesMut,
        map: &HashMap<String, Value>,
    ) -> ProtocolResult<()> {
        let len = map.len();

        // Encode map size
        if len <= 15 {
            buf.extend_from_slice(&[(0xA0 | len as u8)]); // Tiny map
        } else if len <= 255 {
            buf.extend_from_slice(&[0xD8, len as u8]); // Map 8
        } else if len <= 65535 {
            buf.extend_from_slice(&[0xD9, (len >> 8) as u8, len as u8]); // Map 16
        } else {
            buf.extend_from_slice(&[
                0xDA,
                (len >> 24) as u8,
                (len >> 16) as u8,
                (len >> 8) as u8,
                len as u8,
            ]); // Map 32
        }

        // Encode key-value pairs
        for (key, value) in map {
            self.encode_string(buf, key)?;
            self.encode_value(buf, value)?;
        }

        Ok(())
    }

    /// Encode a list to PackStream format
    pub fn encode_list(&mut self, buf: &mut BytesMut, list: &[Value]) -> ProtocolResult<()> {
        let len = list.len();

        // Encode list size
        if len <= 15 {
            buf.extend_from_slice(&[(0x90 | len as u8)]); // Tiny list
        } else if len <= 255 {
            buf.extend_from_slice(&[0xD4, len as u8]); // List 8
        } else if len <= 65535 {
            buf.extend_from_slice(&[0xD5, (len >> 8) as u8, len as u8]); // List 16
        } else {
            buf.extend_from_slice(&[
                0xD6,
                (len >> 24) as u8,
                (len >> 16) as u8,
                (len >> 8) as u8,
                len as u8,
            ]); // List 32
        }

        // Encode values
        for value in list {
            self.encode_value(buf, value)?;
        }

        Ok(())
    }

    /// Encode a string to PackStream format
    fn encode_string(&mut self, buf: &mut BytesMut, s: &str) -> ProtocolResult<()> {
        let bytes = s.as_bytes();
        let len = bytes.len();

        // Encode string size
        if len <= 15 {
            buf.extend_from_slice(&[(0x80 | len as u8)]); // Tiny string
        } else if len <= 255 {
            buf.extend_from_slice(&[0xD0, len as u8]); // String 8
        } else if len <= 65535 {
            buf.extend_from_slice(&[0xD1, (len >> 8) as u8, len as u8]); // String 16
        } else {
            buf.extend_from_slice(&[
                0xD2,
                (len >> 24) as u8,
                (len >> 16) as u8,
                (len >> 8) as u8,
                len as u8,
            ]); // String 32
        }

        buf.extend_from_slice(bytes);
        Ok(())
    }

    /// Encode a JSON value to PackStream format
    fn encode_value(&mut self, buf: &mut BytesMut, value: &Value) -> ProtocolResult<()> {
        match value {
            Value::Null => {
                buf.extend_from_slice(&[0xC0]); // Null
            }
            Value::Bool(b) => {
                buf.extend_from_slice(&[if *b { 0xC3 } else { 0xC2 }]); // Boolean
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    self.encode_integer(buf, i)?;
                } else if let Some(f) = n.as_f64() {
                    self.encode_float(buf, f)?;
                }
            }
            Value::String(s) => {
                self.encode_string(buf, s)?;
            }
            Value::Array(arr) => {
                self.encode_list(buf, arr)?;
            }
            Value::Object(obj) => {
                let map: HashMap<String, Value> =
                    obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                self.encode_map(buf, &map)?;
            }
        }
        Ok(())
    }

    /// Encode an integer to PackStream format
    fn encode_integer(&mut self, buf: &mut BytesMut, i: i64) -> ProtocolResult<()> {
        if i >= -16 && i <= 127 {
            buf.extend_from_slice(&[i as u8]); // Tiny int
        } else if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
            buf.extend_from_slice(&[0xC8, i as u8]); // INT_8
        } else if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
            buf.extend_from_slice(&[0xC9, (i >> 8) as u8, i as u8]); // INT_16
        } else if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
            buf.extend_from_slice(&[
                0xCA,
                (i >> 24) as u8,
                (i >> 16) as u8,
                (i >> 8) as u8,
                i as u8,
            ]); // INT_32
        } else {
            buf.extend_from_slice(&[
                0xCB,
                (i >> 56) as u8,
                (i >> 48) as u8,
                (i >> 40) as u8,
                (i >> 32) as u8,
                (i >> 24) as u8,
                (i >> 16) as u8,
                (i >> 8) as u8,
                i as u8,
            ]); // INT_64
        }
        Ok(())
    }

    /// Encode a float to PackStream format
    fn encode_float(&mut self, buf: &mut BytesMut, f: f64) -> ProtocolResult<()> {
        buf.extend_from_slice(&[0xC1]); // FLOAT_64 marker
        buf.extend_from_slice(&f.to_be_bytes());
        Ok(())
    }
}

impl Default for PackStreamEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_string() {
        let mut encoder = PackStreamEncoder::new();
        let mut buf = BytesMut::new();

        encoder.encode_string(&mut buf, "hello").unwrap();

        // Should be: 0x85 (tiny string, len=5) + "hello"
        assert_eq!(buf[0], 0x85);
        assert_eq!(&buf[1..], b"hello");
    }

    #[test]
    fn test_encode_integer() {
        let mut encoder = PackStreamEncoder::new();
        let mut buf = BytesMut::new();

        encoder.encode_integer(&mut buf, 42).unwrap();

        // Should be: 0x2A (tiny int 42)
        assert_eq!(buf[0], 42);
    }

    #[test]
    fn test_encode_map() {
        let mut encoder = PackStreamEncoder::new();
        let mut buf = BytesMut::new();

        let mut map = HashMap::new();
        map.insert("key".to_string(), Value::String("value".to_string()));

        encoder.encode_map(&mut buf, &map).unwrap();

        // Should start with 0xA1 (tiny map, len=1)
        assert_eq!(buf[0], 0xA1);
    }
}
