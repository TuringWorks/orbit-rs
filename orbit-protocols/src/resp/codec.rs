//! RESP protocol codec for parsing and encoding

use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use super::types::RespValue;
use crate::error::{ProtocolError, ProtocolResult};

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
        b'+' => parse_simple_string(src),
        b'-' => parse_error(src),
        b':' => parse_integer(src),
        b'$' => parse_bulk_string(src),
        b'*' => parse_array(src),
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
            .map_err(|e| ProtocolError::RespError(format!("Invalid integer: {}", e)))?;
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
            .map_err(|e| ProtocolError::RespError(format!("Invalid bulk string length: {}", e)))?;

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
            .map_err(|e| ProtocolError::RespError(format!("Invalid array count: {}", e)))?;

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
}
