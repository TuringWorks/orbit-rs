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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_simple_string_creation() {
        let val = RespValue::simple_string("hello");
        assert_eq!(val, RespValue::SimpleString("hello".to_string()));
        
        let val = RespValue::simple_string(String::from("world"));
        assert_eq!(val, RespValue::SimpleString("world".to_string()));
    }

    #[test]
    fn test_error_creation() {
        let val = RespValue::error("ERR something wrong");
        assert_eq!(val, RespValue::Error("ERR something wrong".to_string()));
    }

    #[test]
    fn test_integer_creation() {
        let val = RespValue::integer(42);
        assert_eq!(val, RespValue::Integer(42));
        
        let val = RespValue::integer(-100);
        assert_eq!(val, RespValue::Integer(-100));
    }

    #[test]
    fn test_bulk_string_creation() {
        let val = RespValue::bulk_string(Bytes::from("hello"));
        assert_eq!(val, RespValue::BulkString(Bytes::from("hello")));
        
        let val = RespValue::bulk_string_from_str("world");
        assert_eq!(val, RespValue::BulkString(Bytes::from("world")));
    }

    #[test]
    fn test_array_creation() {
        let arr = vec![
            RespValue::simple_string("foo"),
            RespValue::integer(123),
            RespValue::bulk_string_from_str("bar"),
        ];
        let val = RespValue::array(arr.clone());
        assert_eq!(val, RespValue::Array(arr));
    }

    #[test]
    fn test_null_creation() {
        let val = RespValue::null();
        assert_eq!(val, RespValue::NullBulkString);
    }

    #[test]
    fn test_ok_response() {
        let val = RespValue::ok();
        assert_eq!(val, RespValue::SimpleString("OK".to_string()));
    }

    #[test]
    fn test_as_string() {
        let val = RespValue::simple_string("hello");
        assert_eq!(val.as_string(), Some("hello".to_string()));
        
        let val = RespValue::bulk_string_from_str("world");
        assert_eq!(val.as_string(), Some("world".to_string()));
        
        let val = RespValue::integer(42);
        assert_eq!(val.as_string(), None);
        
        let val = RespValue::NullBulkString;
        assert_eq!(val.as_string(), None);
        
        // Test invalid UTF-8
        let invalid_bytes = Bytes::from(vec![0xff, 0xfe, 0xfd]);
        let val = RespValue::BulkString(invalid_bytes);
        assert_eq!(val.as_string(), None);
    }

    #[test]
    fn test_as_integer() {
        let val = RespValue::integer(42);
        assert_eq!(val.as_integer(), Some(42));
        
        let val = RespValue::integer(-100);
        assert_eq!(val.as_integer(), Some(-100));
        
        let val = RespValue::simple_string("hello");
        assert_eq!(val.as_integer(), None);
    }

    #[test]
    fn test_as_array() {
        let arr = vec![
            RespValue::simple_string("foo"),
            RespValue::integer(123),
        ];
        let val = RespValue::Array(arr.clone());
        assert_eq!(val.as_array(), Some(&arr));
        
        let val = RespValue::integer(42);
        assert_eq!(val.as_array(), None);
    }

    #[test]
    fn test_is_null() {
        assert!(RespValue::NullBulkString.is_null());
        assert!(RespValue::NullArray.is_null());
        
        assert!(!RespValue::simple_string("hello").is_null());
        assert!(!RespValue::integer(42).is_null());
        assert!(!RespValue::array(vec![]).is_null());
    }

    #[test]
    fn test_serialize_simple_string() {
        let val = RespValue::simple_string("OK");
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("+OK\r\n"));
    }

    #[test]
    fn test_serialize_error() {
        let val = RespValue::error("ERR invalid command");
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("-ERR invalid command\r\n"));
    }

    #[test]
    fn test_serialize_integer() {
        let val = RespValue::integer(1000);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from(":1000\r\n"));
        
        let val = RespValue::integer(-42);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from(":-42\r\n"));
    }

    #[test]
    fn test_serialize_bulk_string() {
        let val = RespValue::bulk_string_from_str("foobar");
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("$6\r\nfoobar\r\n"));
        
        let val = RespValue::bulk_string_from_str("");
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("$0\r\n\r\n"));
    }

    #[test]
    fn test_serialize_null_bulk_string() {
        let val = RespValue::NullBulkString;
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("$-1\r\n"));
    }

    #[test]
    fn test_serialize_array() {
        let val = RespValue::array(vec![
            RespValue::bulk_string_from_str("foo"),
            RespValue::bulk_string_from_str("bar"),
        ]);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"));
        
        // Empty array
        let val = RespValue::array(vec![]);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("*0\r\n"));
    }

    #[test]
    fn test_serialize_null_array() {
        let val = RespValue::NullArray;
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("*-1\r\n"));
    }

    #[test]
    fn test_serialize_complex_array() {
        let val = RespValue::array(vec![
            RespValue::simple_string("OK"),
            RespValue::error("ERR"),
            RespValue::integer(123),
            RespValue::bulk_string_from_str("hello"),
            RespValue::NullBulkString,
            RespValue::array(vec![RespValue::integer(1), RespValue::integer(2)]),
        ]);
        let serialized = val.serialize();
        let expected = "*6\r\n+OK\r\n-ERR\r\n:123\r\n$5\r\nhello\r\n$-1\r\n*2\r\n:1\r\n:2\r\n";
        assert_eq!(serialized, Bytes::from(expected));
    }

    #[test]
    fn test_display_simple_string() {
        let val = RespValue::simple_string("hello");
        assert_eq!(format!("{}", val), "\"hello\"");
    }

    #[test]
    fn test_display_error() {
        let val = RespValue::error("something wrong");
        assert_eq!(format!("{}", val), "ERROR: something wrong");
    }

    #[test]
    fn test_display_integer() {
        let val = RespValue::integer(42);
        assert_eq!(format!("{}", val), "42");
    }

    #[test]
    fn test_display_bulk_string() {
        let val = RespValue::bulk_string_from_str("hello world");
        assert_eq!(format!("{}", val), "\"hello world\"");
        
        // Test binary data display
        let val = RespValue::BulkString(Bytes::from(vec![0xff, 0xfe, 0xfd]));
        assert_eq!(format!("{}", val), "<binary:3 bytes>");
    }

    #[test]
    fn test_display_null_bulk_string() {
        let val = RespValue::NullBulkString;
        assert_eq!(format!("{}", val), "null");
    }

    #[test]
    fn test_display_array() {
        let val = RespValue::array(vec![
            RespValue::integer(1),
            RespValue::simple_string("hello"),
            RespValue::null(),
        ]);
        assert_eq!(format!("{}", val), "[1, \"hello\", null]");
        
        // Empty array
        let val = RespValue::array(vec![]);
        assert_eq!(format!("{}", val), "[]");
    }

    #[test]
    fn test_display_null_array() {
        let val = RespValue::NullArray;
        assert_eq!(format!("{}", val), "null");
    }

    #[test]
    fn test_from_string() {
        let val: RespValue = String::from("hello").into();
        assert_eq!(val, RespValue::BulkString(Bytes::from("hello")));
    }

    #[test]
    fn test_from_str() {
        let val: RespValue = "world".into();
        assert_eq!(val, RespValue::BulkString(Bytes::from("world")));
    }

    #[test]
    fn test_from_i64() {
        let val: RespValue = 42i64.into();
        assert_eq!(val, RespValue::Integer(42));
    }

    #[test]
    fn test_from_bytes() {
        let bytes = Bytes::from("test");
        let val: RespValue = bytes.clone().into();
        assert_eq!(val, RespValue::BulkString(bytes));
    }

    #[test]
    fn test_clone() {
        let original = RespValue::array(vec![
            RespValue::simple_string("test"),
            RespValue::integer(123),
        ]);
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_partial_eq() {
        let val1 = RespValue::simple_string("hello");
        let val2 = RespValue::simple_string("hello");
        let val3 = RespValue::simple_string("world");
        
        assert_eq!(val1, val2);
        assert_ne!(val1, val3);
    }

    #[test]
    fn test_debug() {
        let val = RespValue::simple_string("test");
        let debug_str = format!("{:?}", val);
        assert!(debug_str.contains("SimpleString"));
        assert!(debug_str.contains("test"));
    }
}
