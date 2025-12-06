//! RESP protocol codec for parsing and encoding
//!
//! Supports both RESP2 (Redis 2.0+) and RESP3 (Redis 6.0+) protocols.
//!
//! ## Type Prefix Bytes
//!
//! ### RESP2 Types
//! - `+` Simple String
//! - `-` Error
//! - `:` Integer
//! - `$` Bulk String
//! - `*` Array
//!
//! ### RESP3 Types
//! - `_` Null
//! - `#` Boolean
//! - `,` Double
//! - `(` Big Number
//! - `!` Bulk Error
//! - `=` Verbatim String
//! - `%` Map
//! - `~` Set
//! - `|` Attribute
//! - `>` Push
//!
//! ## References
//! - Protocol Spec: `specifications/protocols/redis-resp-protocol-specification.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/redis>

use bytes::{Buf, BytesMut};
use std::collections::HashMap;
use tokio_util::codec::{Decoder, Encoder};

use super::types::RespValue;
use crate::protocols::error::{ProtocolError, ProtocolResult};

/// RESP protocol codec
pub struct RespCodec {
    #[allow(dead_code)]
    max_frame_size: usize,
}

impl RespCodec {
    /// Create a new RESP codec
    pub fn new() -> Self {
        Self {
            max_frame_size: 512 * 1024 * 1024, // 512MB default max
        }
    }

    /// Create a codec with custom max frame size
    pub fn with_max_frame_size(max_frame_size: usize) -> Self {
        Self { max_frame_size }
    }
}

impl Default for RespCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for RespCodec {
    type Item = RespValue;
    type Error = ProtocolError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        match parse_resp_value(src)? {
            Some((value, consumed)) => {
                src.advance(consumed);
                Ok(Some(value))
            }
            None => Ok(None), // Need more data
        }
    }
}

impl Encoder<RespValue> for RespCodec {
    type Error = ProtocolError;

    fn encode(&mut self, item: RespValue, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let serialized = item.serialize();
        dst.extend_from_slice(&serialized);
        Ok(())
    }
}

/// Parse a RESP value from bytes
///
/// Returns (RespValue, bytes_consumed) if successful, None if more data is needed
fn parse_resp_value(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if src.is_empty() {
        return Ok(None);
    }

    let first_byte = src[0];
    match first_byte {
        // RESP2 types
        b'+' => parse_simple_string(src),
        b'-' => parse_error(src),
        b':' => parse_integer(src),
        b'$' => parse_bulk_string(src),
        b'*' => parse_array(src),

        // RESP3 types
        b'_' => parse_null(src),
        b'#' => parse_boolean(src),
        b',' => parse_double(src),
        b'(' => parse_big_number(src),
        b'!' => parse_bulk_error(src),
        b'=' => parse_verbatim_string(src),
        b'%' => parse_map(src),
        b'~' => parse_set(src),
        b'|' => parse_attribute(src),
        b'>' => parse_push(src),

        _ => Err(ProtocolError::RespError(format!(
            "Invalid RESP type byte: {}",
            first_byte as char
        ))),
    }
}

/// Parse simple string: +OK\r\n
fn parse_simple_string(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(end_pos) = find_crlf(src, 1) {
        let s = String::from_utf8_lossy(&src[1..end_pos]).to_string();
        Ok(Some((RespValue::SimpleString(s), end_pos + 2)))
    } else {
        Ok(None)
    }
}

/// Parse error: -Error message\r\n
fn parse_error(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(end_pos) = find_crlf(src, 1) {
        let s = String::from_utf8_lossy(&src[1..end_pos]).to_string();
        Ok(Some((RespValue::Error(s), end_pos + 2)))
    } else {
        Ok(None)
    }
}

/// Parse integer: :1000\r\n
fn parse_integer(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(end_pos) = find_crlf(src, 1) {
        let s = String::from_utf8_lossy(&src[1..end_pos]);
        let i = s
            .parse::<i64>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid integer: {e}")))?;
        Ok(Some((RespValue::Integer(i), end_pos + 2)))
    } else {
        Ok(None)
    }
}

/// Parse bulk string: $6\r\nfoobar\r\n or $-1\r\n (null)
fn parse_bulk_string(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(len_end) = find_crlf(src, 1) {
        let len_str = String::from_utf8_lossy(&src[1..len_end]);
        let len = len_str
            .parse::<i64>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid bulk string length: {e}")))?;

        if len == -1 {
            return Ok(Some((RespValue::NullBulkString, len_end + 2)));
        }

        let len = len as usize;
        let total_size = len_end + 2 + len + 2; // $...\r\n + data + \r\n

        if src.len() < total_size {
            return Ok(None); // Need more data
        }

        let data_start = len_end + 2;
        let data_end = data_start + len;
        let data = src[data_start..data_end].to_vec();

        Ok(Some((RespValue::BulkString(data.into()), total_size)))
    } else {
        Ok(None)
    }
}

/// Parse array: *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n or *-1\r\n (null)
fn parse_array(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(count_end) = find_crlf(src, 1) {
        let count_str = String::from_utf8_lossy(&src[1..count_end]);
        let count = count_str
            .parse::<i64>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid array count: {e}")))?;

        if count == -1 {
            return Ok(Some((RespValue::NullArray, count_end + 2)));
        }

        let count = count as usize;
        let mut elements = Vec::with_capacity(count);
        let mut pos = count_end + 2;

        for _ in 0..count {
            if pos >= src.len() {
                return Ok(None); // Need more data
            }

            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);

            match parse_resp_value(&temp_buf)? {
                Some((value, consumed)) => {
                    elements.push(value);
                    pos += consumed;
                }
                None => return Ok(None), // Need more data
            }
        }

        Ok(Some((RespValue::Array(elements), pos)))
    } else {
        Ok(None)
    }
}

// === RESP3 Parsing Functions ===

/// Parse RESP3 null: _\r\n
fn parse_null(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if src.len() >= 3 && src[1] == b'\r' && src[2] == b'\n' {
        Ok(Some((RespValue::Null, 3)))
    } else if src.len() < 3 {
        Ok(None)
    } else {
        Err(ProtocolError::RespError("Invalid null format".to_string()))
    }
}

/// Parse RESP3 boolean: #t\r\n or #f\r\n
fn parse_boolean(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if src.len() >= 4 && src[2] == b'\r' && src[3] == b'\n' {
        match src[1] {
            b't' => Ok(Some((RespValue::Boolean(true), 4))),
            b'f' => Ok(Some((RespValue::Boolean(false), 4))),
            _ => Err(ProtocolError::RespError(format!(
                "Invalid boolean value: {}",
                src[1] as char
            ))),
        }
    } else if src.len() < 4 {
        Ok(None)
    } else {
        Err(ProtocolError::RespError(
            "Invalid boolean format".to_string(),
        ))
    }
}

/// Parse RESP3 double: ,3.14\r\n or ,inf\r\n or ,-inf\r\n or ,nan\r\n
fn parse_double(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(end_pos) = find_crlf(src, 1) {
        let s = String::from_utf8_lossy(&src[1..end_pos]);
        let d = match s.as_ref() {
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            "nan" => f64::NAN,
            _ => s
                .parse::<f64>()
                .map_err(|e| ProtocolError::RespError(format!("Invalid double: {e}")))?,
        };
        Ok(Some((RespValue::Double(d), end_pos + 2)))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 big number: (3492890328409238509324850943850943825024385\r\n
fn parse_big_number(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(end_pos) = find_crlf(src, 1) {
        let s = String::from_utf8_lossy(&src[1..end_pos]).to_string();
        Ok(Some((RespValue::BigNumber(s), end_pos + 2)))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 bulk error: !<length>\r\n<error>\r\n
fn parse_bulk_error(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(len_end) = find_crlf(src, 1) {
        let len_str = String::from_utf8_lossy(&src[1..len_end]);
        let len = len_str
            .parse::<usize>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid bulk error length: {e}")))?;

        let total_size = len_end + 2 + len + 2;
        if src.len() < total_size {
            return Ok(None);
        }

        let data_start = len_end + 2;
        let data_end = data_start + len;
        let data = src[data_start..data_end].to_vec();

        Ok(Some((RespValue::BulkError(data.into()), total_size)))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 verbatim string: =<length>\r\n<encoding>:<data>\r\n
fn parse_verbatim_string(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(len_end) = find_crlf(src, 1) {
        let len_str = String::from_utf8_lossy(&src[1..len_end]);
        let len = len_str.parse::<usize>().map_err(|e| {
            ProtocolError::RespError(format!("Invalid verbatim string length: {e}"))
        })?;

        let total_size = len_end + 2 + len + 2;
        if src.len() < total_size {
            return Ok(None);
        }

        let data_start = len_end + 2;
        let data_end = data_start + len;
        let content = &src[data_start..data_end];

        // Format: <encoding(3)>:<data>
        if len < 4 {
            return Err(ProtocolError::RespError(
                "Verbatim string too short".to_string(),
            ));
        }

        let encoding = String::from_utf8_lossy(&content[0..3]).to_string();
        // Skip the colon at position 3
        let data = content[4..].to_vec();

        Ok(Some((
            RespValue::VerbatimString {
                encoding,
                data: data.into(),
            },
            total_size,
        )))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 map: %<count>\r\n<key1><value1>...<keyN><valueN>\r\n
fn parse_map(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(count_end) = find_crlf(src, 1) {
        let count_str = String::from_utf8_lossy(&src[1..count_end]);
        let count = count_str
            .parse::<usize>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid map count: {e}")))?;

        let mut entries = Vec::with_capacity(count);
        let mut pos = count_end + 2;

        for _ in 0..count {
            // Parse key
            if pos >= src.len() {
                return Ok(None);
            }
            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);
            let (key, key_consumed) = match parse_resp_value(&temp_buf)? {
                Some(result) => result,
                None => return Ok(None),
            };
            pos += key_consumed;

            // Parse value
            if pos >= src.len() {
                return Ok(None);
            }
            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);
            let (value, value_consumed) = match parse_resp_value(&temp_buf)? {
                Some(result) => result,
                None => return Ok(None),
            };
            pos += value_consumed;

            entries.push((key, value));
        }

        Ok(Some((RespValue::Map(entries), pos)))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 set: ~<count>\r\n<element1>...<elementN>\r\n
fn parse_set(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(count_end) = find_crlf(src, 1) {
        let count_str = String::from_utf8_lossy(&src[1..count_end]);
        let count = count_str
            .parse::<usize>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid set count: {e}")))?;

        let mut elements = Vec::with_capacity(count);
        let mut pos = count_end + 2;

        for _ in 0..count {
            if pos >= src.len() {
                return Ok(None);
            }
            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);
            let (value, consumed) = match parse_resp_value(&temp_buf)? {
                Some(result) => result,
                None => return Ok(None),
            };
            elements.push(value);
            pos += consumed;
        }

        Ok(Some((RespValue::Set(elements), pos)))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 attribute: |<count>\r\n<key1><value1>...\r\n
fn parse_attribute(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(count_end) = find_crlf(src, 1) {
        let count_str = String::from_utf8_lossy(&src[1..count_end]);
        let count = count_str
            .parse::<usize>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid attribute count: {e}")))?;

        let mut attrs = HashMap::new();
        let mut pos = count_end + 2;

        for _ in 0..count {
            // Parse key (must be a string type)
            if pos >= src.len() {
                return Ok(None);
            }
            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);
            let (key_val, key_consumed) = match parse_resp_value(&temp_buf)? {
                Some(result) => result,
                None => return Ok(None),
            };
            let key = key_val.as_string().ok_or_else(|| {
                ProtocolError::RespError("Attribute key must be a string".to_string())
            })?;
            pos += key_consumed;

            // Parse value
            if pos >= src.len() {
                return Ok(None);
            }
            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);
            let (value, value_consumed) = match parse_resp_value(&temp_buf)? {
                Some(result) => result,
                None => return Ok(None),
            };
            pos += value_consumed;

            attrs.insert(key, value);
        }

        Ok(Some((RespValue::Attribute(attrs), pos)))
    } else {
        Ok(None)
    }
}

/// Parse RESP3 push: ><count>\r\n<element1>...<elementN>\r\n
fn parse_push(src: &BytesMut) -> ProtocolResult<Option<(RespValue, usize)>> {
    if let Some(count_end) = find_crlf(src, 1) {
        let count_str = String::from_utf8_lossy(&src[1..count_end]);
        let count = count_str
            .parse::<usize>()
            .map_err(|e| ProtocolError::RespError(format!("Invalid push count: {e}")))?;

        let mut elements = Vec::with_capacity(count);
        let mut pos = count_end + 2;

        for _ in 0..count {
            if pos >= src.len() {
                return Ok(None);
            }
            let remaining = &src[pos..];
            let temp_buf = BytesMut::from(remaining);
            let (value, consumed) = match parse_resp_value(&temp_buf)? {
                Some(result) => result,
                None => return Ok(None),
            };
            elements.push(value);
            pos += consumed;
        }

        Ok(Some((RespValue::Push(elements), pos)))
    } else {
        Ok(None)
    }
}

/// Find CRLF position starting from offset
fn find_crlf(buf: &BytesMut, start: usize) -> Option<usize> {
    (start..buf.len().saturating_sub(1)).find(|&i| buf[i] == b'\r' && buf[i + 1] == b'\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let buf = BytesMut::from(&b"+OK\r\n"[..]);
        let (val, consumed) = parse_simple_string(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::SimpleString("OK".to_string()));
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_parse_integer() {
        let buf = BytesMut::from(&b":1000\r\n"[..]);
        let (val, consumed) = parse_integer(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::Integer(1000));
        assert_eq!(consumed, 7);
    }

    #[test]
    fn test_parse_bulk_string() {
        let buf = BytesMut::from(&b"$6\r\nfoobar\r\n"[..]);
        let (val, consumed) = parse_bulk_string(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::BulkString("foobar".as_bytes().into()));
        assert_eq!(consumed, 12);
    }

    #[test]
    fn test_parse_null_bulk_string() {
        let buf = BytesMut::from(&b"$-1\r\n"[..]);
        let (val, consumed) = parse_bulk_string(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::NullBulkString);
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_parse_array() {
        let buf = BytesMut::from(&b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"[..]);
        let (val, consumed) = parse_array(&buf).unwrap().unwrap();
        match val {
            RespValue::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], RespValue::BulkString("foo".as_bytes().into()));
                assert_eq!(arr[1], RespValue::BulkString("bar".as_bytes().into()));
            }
            _ => panic!("Expected array"),
        }
        assert_eq!(consumed, 22);
    }

    #[test]
    fn test_codec_new() {
        let _codec = RespCodec::new();
        let _default_codec = RespCodec::default();
        // Just verify they can be created
    }

    #[test]
    fn test_codec_decode_simple() {
        let mut codec = RespCodec::new();
        let mut buf = BytesMut::from("+OK\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert_eq!(result, Some(RespValue::simple_string("OK")));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_codec_encode() {
        let mut codec = RespCodec::new();
        let mut buf = BytesMut::new();

        // Test encoding simple string
        let value = RespValue::simple_string("Hello");
        codec.encode(value, &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from("+Hello\r\n"));

        // Test encoding integer
        buf.clear();
        let value = RespValue::integer(42);
        codec.encode(value, &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from(":42\r\n"));

        // Test encoding bulk string
        buf.clear();
        let value = RespValue::bulk_string_from_str("world");
        codec.encode(value, &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from("$5\r\nworld\r\n"));
    }

    #[test]
    fn test_codec_decode_incomplete() {
        let mut codec = RespCodec::new();

        // Test incomplete simple string
        let mut buf = BytesMut::from("+OK");
        let result = codec.decode(&mut buf).unwrap();
        assert_eq!(result, None);

        // Complete the message
        buf.extend_from_slice(b"\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert_eq!(result, Some(RespValue::simple_string("OK")));
    }

    #[test]
    fn test_parse_error() {
        let buf = BytesMut::from(&b"-ERR something went wrong\r\n"[..]);
        let (val, consumed) = parse_error(&buf).unwrap().unwrap();
        assert_eq!(
            val,
            RespValue::Error("ERR something went wrong".to_string())
        );
        assert_eq!(consumed, 27);
    }

    #[test]
    fn test_parse_empty_bulk_string() {
        let buf = BytesMut::from(&b"$0\r\n\r\n"[..]);
        let (val, consumed) = parse_bulk_string(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::BulkString("".as_bytes().into()));
        assert_eq!(consumed, 6);
    }

    #[test]
    fn test_parse_empty_array() {
        let buf = BytesMut::from(&b"*0\r\n"[..]);
        let (val, consumed) = parse_array(&buf).unwrap().unwrap();
        match val {
            RespValue::Array(arr) => assert_eq!(arr.len(), 0),
            _ => panic!("Expected array"),
        }
        assert_eq!(consumed, 4);
    }

    #[test]
    fn test_parse_null_array() {
        let buf = BytesMut::from(&b"*-1\r\n"[..]);
        let (val, consumed) = parse_array(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::NullArray);
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_parse_mixed_array() {
        let buf = BytesMut::from(&b"*3\r\n+OK\r\n:42\r\n$5\r\nhello\r\n"[..]);
        let (val, consumed) = parse_array(&buf).unwrap().unwrap();
        match val {
            RespValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], RespValue::SimpleString("OK".to_string()));
                assert_eq!(arr[1], RespValue::Integer(42));
                assert_eq!(arr[2], RespValue::BulkString("hello".as_bytes().into()));
            }
            _ => panic!("Expected array"),
        }
        assert_eq!(consumed, 25);
    }

    #[test]
    fn test_parse_incomplete_bulk_string() {
        let buf = BytesMut::from(&b"$5\r\nhel"[..]);
        let result = parse_bulk_string(&buf).unwrap();
        assert_eq!(result, None); // Need more data
    }

    #[test]
    fn test_parse_incomplete_array() {
        let buf = BytesMut::from(&b"*2\r\n+OK\r\n"[..]);
        let result = parse_array(&buf).unwrap();
        assert_eq!(result, None); // Need more data for second element
    }

    #[test]
    fn test_invalid_type_byte() {
        let buf = BytesMut::from(&b"@invalid\r\n"[..]);
        let result = parse_resp_value(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_integer() {
        let buf = BytesMut::from(&b":not_a_number\r\n"[..]);
        let result = parse_integer(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_bulk_string_length() {
        let buf = BytesMut::from(&b"$invalid\r\n"[..]);
        let result = parse_bulk_string(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_array_count() {
        let buf = BytesMut::from(&b"*invalid\r\n"[..]);
        let result = parse_array(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_crlf() {
        let buf = BytesMut::from(&b"hello\r\nworld"[..]);
        let pos = find_crlf(&buf, 0);
        assert_eq!(pos, Some(5));

        let pos = find_crlf(&buf, 6);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_codec_multiple_decode() {
        let mut codec = RespCodec::new();
        let mut buf = BytesMut::from("+OK\r\n:42\r\n$5\r\nhello\r\n");

        // Decode first message
        let result1 = codec.decode(&mut buf).unwrap();
        assert_eq!(result1, Some(RespValue::simple_string("OK")));

        // Decode second message
        let result2 = codec.decode(&mut buf).unwrap();
        assert_eq!(result2, Some(RespValue::integer(42)));

        // Decode third message
        let result3 = codec.decode(&mut buf).unwrap();
        assert_eq!(result3, Some(RespValue::bulk_string_from_str("hello")));

        // No more data
        let result4 = codec.decode(&mut buf).unwrap();
        assert_eq!(result4, None);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_codec_encode_all_types() {
        let mut codec = RespCodec::new();
        let mut buf = BytesMut::new();

        let test_values = vec![
            (RespValue::simple_string("test"), "+test\r\n"),
            (RespValue::error("ERROR"), "-ERROR\r\n"),
            (RespValue::integer(-123), ":-123\r\n"),
            (RespValue::bulk_string_from_str("data"), "$4\r\ndata\r\n"),
            (RespValue::NullBulkString, "$-1\r\n"),
            (RespValue::array(vec![]), "*0\r\n"),
            (RespValue::NullArray, "*-1\r\n"),
        ];

        for (value, expected) in test_values {
            buf.clear();
            codec.encode(value, &mut buf).unwrap();
            assert_eq!(buf, BytesMut::from(expected));
        }
    }

    // === RESP3 Parsing Tests ===

    #[test]
    fn test_parse_resp3_null() {
        let buf = BytesMut::from(&b"_\r\n"[..]);
        let (val, consumed) = parse_null(&buf).unwrap().unwrap();
        assert_eq!(val, RespValue::Null);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn test_parse_resp3_boolean() {
        let buf_true = BytesMut::from(&b"#t\r\n"[..]);
        let (val, consumed) = parse_boolean(&buf_true).unwrap().unwrap();
        assert_eq!(val, RespValue::Boolean(true));
        assert_eq!(consumed, 4);

        let buf_false = BytesMut::from(&b"#f\r\n"[..]);
        let (val, consumed) = parse_boolean(&buf_false).unwrap().unwrap();
        assert_eq!(val, RespValue::Boolean(false));
        assert_eq!(consumed, 4);
    }

    #[test]
    fn test_parse_resp3_double() {
        let buf = BytesMut::from(&b",3.14159\r\n"[..]);
        let (val, consumed) = parse_double(&buf).unwrap().unwrap();
        if let RespValue::Double(d) = val {
            assert!((d - 3.14159).abs() < 0.0001);
        } else {
            panic!("Expected double");
        }
        assert_eq!(consumed, 10);

        // Test infinity
        let buf_inf = BytesMut::from(&b",inf\r\n"[..]);
        let (val, _) = parse_double(&buf_inf).unwrap().unwrap();
        if let RespValue::Double(d) = val {
            assert!(d.is_infinite() && d.is_sign_positive());
        } else {
            panic!("Expected double");
        }

        // Test negative infinity
        let buf_neg_inf = BytesMut::from(&b",-inf\r\n"[..]);
        let (val, _) = parse_double(&buf_neg_inf).unwrap().unwrap();
        if let RespValue::Double(d) = val {
            assert!(d.is_infinite() && d.is_sign_negative());
        } else {
            panic!("Expected double");
        }

        // Test NaN
        let buf_nan = BytesMut::from(&b",nan\r\n"[..]);
        let (val, _) = parse_double(&buf_nan).unwrap().unwrap();
        if let RespValue::Double(d) = val {
            assert!(d.is_nan());
        } else {
            panic!("Expected double");
        }
    }

    #[test]
    fn test_parse_resp3_big_number() {
        let buf = BytesMut::from(&b"(123456789012345678901234567890\r\n"[..]);
        let (val, consumed) = parse_big_number(&buf).unwrap().unwrap();
        assert_eq!(
            val,
            RespValue::BigNumber("123456789012345678901234567890".to_string())
        );
        assert_eq!(consumed, 33);
    }

    #[test]
    fn test_parse_resp3_bulk_error() {
        let buf = BytesMut::from(&b"!12\r\nSYNTAX error\r\n"[..]);
        let (val, consumed) = parse_bulk_error(&buf).unwrap().unwrap();
        if let RespValue::BulkError(data) = val {
            assert_eq!(data.as_ref(), b"SYNTAX error");
        } else {
            panic!("Expected bulk error");
        }
        assert_eq!(consumed, 19);
    }

    #[test]
    fn test_parse_resp3_verbatim_string() {
        let buf = BytesMut::from(&b"=9\r\ntxt:hello\r\n"[..]);
        let (val, consumed) = parse_verbatim_string(&buf).unwrap().unwrap();
        if let RespValue::VerbatimString { encoding, data } = val {
            assert_eq!(encoding, "txt");
            assert_eq!(data.as_ref(), b"hello");
        } else {
            panic!("Expected verbatim string");
        }
        assert_eq!(consumed, 15);
    }

    #[test]
    fn test_parse_resp3_map() {
        let buf = BytesMut::from(&b"%1\r\n$3\r\nkey\r\n:42\r\n"[..]);
        let (val, consumed) = parse_map(&buf).unwrap().unwrap();
        if let RespValue::Map(entries) = val {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].0, RespValue::BulkString("key".as_bytes().into()));
            assert_eq!(entries[0].1, RespValue::Integer(42));
        } else {
            panic!("Expected map");
        }
        assert_eq!(consumed, 18);
    }

    #[test]
    fn test_parse_resp3_set() {
        let buf = BytesMut::from(&b"~2\r\n:1\r\n:2\r\n"[..]);
        let (val, consumed) = parse_set(&buf).unwrap().unwrap();
        if let RespValue::Set(elements) = val {
            assert_eq!(elements.len(), 2);
            assert_eq!(elements[0], RespValue::Integer(1));
            assert_eq!(elements[1], RespValue::Integer(2));
        } else {
            panic!("Expected set");
        }
        assert_eq!(consumed, 12);
    }

    #[test]
    fn test_parse_resp3_push() {
        // >2\r\n = 4, $6\r\npubsub\r\n = 12, $7\r\nmessage\r\n = 13 => total 29
        let buf = BytesMut::from(&b">2\r\n$6\r\npubsub\r\n$7\r\nmessage\r\n"[..]);
        let (val, consumed) = parse_push(&buf).unwrap().unwrap();
        if let RespValue::Push(elements) = val {
            assert_eq!(elements.len(), 2);
            assert_eq!(
                elements[0],
                RespValue::BulkString("pubsub".as_bytes().into())
            );
            assert_eq!(
                elements[1],
                RespValue::BulkString("message".as_bytes().into())
            );
        } else {
            panic!("Expected push");
        }
        assert_eq!(consumed, 29);
    }

    #[test]
    fn test_codec_decode_resp3_types() {
        let mut codec = RespCodec::new();

        // Test null
        let mut buf = BytesMut::from("_\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert_eq!(result, Some(RespValue::Null));

        // Test boolean
        let mut buf = BytesMut::from("#t\r\n");
        let result = codec.decode(&mut buf).unwrap();
        assert_eq!(result, Some(RespValue::Boolean(true)));

        // Test double
        let mut buf = BytesMut::from(",3.14\r\n");
        let result = codec.decode(&mut buf).unwrap();
        if let Some(RespValue::Double(d)) = result {
            assert!((d - 3.14).abs() < 0.001);
        } else {
            panic!("Expected double");
        }
    }

    #[test]
    fn test_codec_encode_resp3_types() {
        let mut codec = RespCodec::new();
        let mut buf = BytesMut::new();

        // Test null
        codec.encode(RespValue::Null, &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from("_\r\n"));

        // Test boolean true
        buf.clear();
        codec.encode(RespValue::Boolean(true), &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from("#t\r\n"));

        // Test boolean false
        buf.clear();
        codec.encode(RespValue::Boolean(false), &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from("#f\r\n"));

        // Test double
        buf.clear();
        codec.encode(RespValue::Double(3.14), &mut buf).unwrap();
        assert_eq!(buf, BytesMut::from(",3.14\r\n"));

        // Test big number
        buf.clear();
        codec
            .encode(RespValue::BigNumber("123".to_string()), &mut buf)
            .unwrap();
        assert_eq!(buf, BytesMut::from("(123\r\n"));

        // Test set
        buf.clear();
        codec
            .encode(
                RespValue::Set(vec![RespValue::Integer(1), RespValue::Integer(2)]),
                &mut buf,
            )
            .unwrap();
        assert_eq!(buf, BytesMut::from("~2\r\n:1\r\n:2\r\n"));
    }
}
