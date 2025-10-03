//! RESP protocol types

use bytes::Bytes;
use std::fmt;

/// RESP protocol value types
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    /// Simple string: +OK\r\n
    SimpleString(String),
    /// Error: -Error message\r\n
    Error(String),
    /// Integer: :1000\r\n
    Integer(i64),
    /// Bulk string: $6\r\nfoobar\r\n
    BulkString(Bytes),
    /// Null bulk string: $-1\r\n
    NullBulkString,
    /// Array: *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
    Array(Vec<RespValue>),
    /// Null array: *-1\r\n
    NullArray,
}

/// Type alias for RESP arrays
pub type RespArray = Vec<RespValue>;

impl RespValue {
    /// Create a simple string
    pub fn simple_string(s: impl Into<String>) -> Self {
        RespValue::SimpleString(s.into())
    }

    /// Create an error
    pub fn error(s: impl Into<String>) -> Self {
        RespValue::Error(s.into())
    }

    /// Create an integer
    pub fn integer(i: i64) -> Self {
        RespValue::Integer(i)
    }

    /// Create a bulk string from bytes
    pub fn bulk_string(b: impl Into<Bytes>) -> Self {
        RespValue::BulkString(b.into())
    }

    /// Create a bulk string from a string
    pub fn bulk_string_from_str(s: impl Into<String>) -> Self {
        RespValue::BulkString(Bytes::from(s.into()))
    }

    /// Create an array
    pub fn array(arr: Vec<RespValue>) -> Self {
        RespValue::Array(arr)
    }

    /// Create null bulk string
    pub fn null() -> Self {
        RespValue::NullBulkString
    }

    /// OK response
    pub fn ok() -> Self {
        RespValue::simple_string("OK")
    }

    /// Extract as string if possible
    pub fn as_string(&self) -> Option<String> {
        match self {
            RespValue::SimpleString(s) => Some(s.clone()),
            RespValue::BulkString(b) => String::from_utf8(b.to_vec()).ok(),
            _ => None,
        }
    }

    /// Extract as integer if possible
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            RespValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Extract as array if possible
    pub fn as_array(&self) -> Option<&Vec<RespValue>> {
        match self {
            RespValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Check if this is a null value
    pub fn is_null(&self) -> bool {
        matches!(self, RespValue::NullBulkString | RespValue::NullArray)
    }

    /// Serialize to RESP2 format
    pub fn serialize(&self) -> Bytes {
        let mut buf = Vec::new();
        self.write_to(&mut buf);
        Bytes::from(buf)
    }

    fn write_to(&self, buf: &mut Vec<u8>) {
        match self {
            RespValue::SimpleString(s) => {
                buf.push(b'+');
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Error(s) => {
                buf.push(b'-');
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Integer(i) => {
                buf.push(b':');
                buf.extend_from_slice(i.to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::BulkString(bytes) => {
                buf.push(b'$');
                buf.extend_from_slice(bytes.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(bytes);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::NullBulkString => {
                buf.extend_from_slice(b"$-1\r\n");
            }
            RespValue::Array(arr) => {
                buf.push(b'*');
                buf.extend_from_slice(arr.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for val in arr {
                    val.write_to(buf);
                }
            }
            RespValue::NullArray => {
                buf.extend_from_slice(b"*-1\r\n");
            }
        }
    }
}

impl fmt::Display for RespValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RespValue::SimpleString(s) => write!(f, "\"{}\"", s),
            RespValue::Error(s) => write!(f, "ERROR: {}", s),
            RespValue::Integer(i) => write!(f, "{}", i),
            RespValue::BulkString(b) => {
                if let Ok(s) = std::str::from_utf8(b) {
                    write!(f, "\"{}\"", s)
                } else {
                    write!(f, "<binary:{} bytes>", b.len())
                }
            }
            RespValue::NullBulkString => write!(f, "null"),
            RespValue::Array(arr) => {
                write!(f, "[")?;
                for (i, val) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            RespValue::NullArray => write!(f, "null"),
        }
    }
}

impl From<String> for RespValue {
    fn from(s: String) -> Self {
        RespValue::bulk_string_from_str(s)
    }
}

impl From<&str> for RespValue {
    fn from(s: &str) -> Self {
        RespValue::bulk_string_from_str(s)
    }
}

impl From<i64> for RespValue {
    fn from(i: i64) -> Self {
        RespValue::Integer(i)
    }
}

impl From<Bytes> for RespValue {
    fn from(b: Bytes) -> Self {
        RespValue::BulkString(b)
    }
}

impl From<Vec<RespValue>> for RespValue {
    fn from(arr: Vec<RespValue>) -> Self {
        RespValue::Array(arr)
    }
}
