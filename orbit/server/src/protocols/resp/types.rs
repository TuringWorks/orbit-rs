//! RESP protocol types
//!
//! Implements both RESP2 and RESP3 protocol types.
//!
//! ## RESP2 Types (Redis 2.0+)
//! - Simple String (+)
//! - Error (-)
//! - Integer (:)
//! - Bulk String ($)
//! - Array (*)
//!
//! ## RESP3 Types (Redis 6.0+)
//! - Null (_)
//! - Boolean (#)
//! - Double (,)

// Complex return types are intentional for RESP protocol completeness
#![allow(clippy::type_complexity)]
//! - Big Number (()
//! - Bulk Error (!)
//! - Verbatim String (=)
//! - Map (%)
//! - Set (~)
//! - Attribute (|)
//! - Push (>)
//!
//! ## References
//! - RESP Protocol Spec: `specifications/protocols/redis-resp-protocol-specification.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/redis>

use bytes::Bytes;
use std::collections::HashMap;
use std::fmt;

/// RESP protocol value types supporting both RESP2 and RESP3
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    // === RESP2 Types ===
    /// Simple string: +OK\r\n
    SimpleString(String),
    /// Error: -Error message\r\n
    Error(String),
    /// Integer: :1000\r\n (64-bit signed)
    Integer(i64),
    /// Bulk string: $6\r\nfoobar\r\n
    BulkString(Bytes),
    /// Null bulk string: $-1\r\n (RESP2 null representation)
    NullBulkString,
    /// Array: *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
    Array(Vec<RespValue>),
    /// Null array: *-1\r\n (RESP2 null array)
    NullArray,

    // === RESP3 Types ===
    /// Null: _\r\n (RESP3 explicit null)
    Null,
    /// Boolean: #t\r\n or #f\r\n
    Boolean(bool),
    /// Double: ,3.14159\r\n (IEEE 754 double-precision)
    Double(f64),
    /// Big number: (3492890328409238509324850943850943825024385\r\n
    BigNumber(String),
    /// Bulk error: !<length>\r\n<error>\r\n (binary-safe error)
    BulkError(Bytes),
    /// Verbatim string: =<length>\r\n<encoding>:<data>\r\n
    VerbatimString { encoding: String, data: Bytes },
    /// Map: %<count>\r\n<key1><value1>...<keyN><valueN>\r\n
    Map(Vec<(RespValue, RespValue)>),
    /// Set: ~<count>\r\n<element1>...<elementN>\r\n
    Set(Vec<RespValue>),
    /// Attribute: |<count>\r\n<key1><value1>...\r\n (out-of-band metadata)
    Attribute(HashMap<String, RespValue>),
    /// Push: ><count>\r\n<element1>...<elementN>\r\n (Pub/Sub push message)
    Push(Vec<RespValue>),
}

/// Type alias for RESP arrays
pub type RespArray = Vec<RespValue>;

impl RespValue {
    // === RESP2 Constructors ===

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

    /// Create null bulk string (RESP2 null)
    pub fn null() -> Self {
        RespValue::NullBulkString
    }

    /// OK response
    pub fn ok() -> Self {
        RespValue::simple_string("OK")
    }

    // === RESP3 Constructors ===

    /// Create RESP3 null value
    pub fn resp3_null() -> Self {
        RespValue::Null
    }

    /// Create a boolean value (RESP3)
    pub fn boolean(b: bool) -> Self {
        RespValue::Boolean(b)
    }

    /// Create a double value (RESP3)
    pub fn double(d: f64) -> Self {
        RespValue::Double(d)
    }

    /// Create a big number from string (RESP3)
    pub fn big_number(s: impl Into<String>) -> Self {
        RespValue::BigNumber(s.into())
    }

    /// Create a bulk error (RESP3)
    pub fn bulk_error(b: impl Into<Bytes>) -> Self {
        RespValue::BulkError(b.into())
    }

    /// Create a verbatim string (RESP3)
    pub fn verbatim_string(encoding: impl Into<String>, data: impl Into<Bytes>) -> Self {
        RespValue::VerbatimString {
            encoding: encoding.into(),
            data: data.into(),
        }
    }

    /// Create a map (RESP3)
    pub fn map(entries: Vec<(RespValue, RespValue)>) -> Self {
        RespValue::Map(entries)
    }

    /// Create a set (RESP3)
    pub fn set(elements: Vec<RespValue>) -> Self {
        RespValue::Set(elements)
    }

    /// Create an attribute (RESP3 out-of-band data)
    pub fn attribute(attrs: HashMap<String, RespValue>) -> Self {
        RespValue::Attribute(attrs)
    }

    /// Create a push message (RESP3 Pub/Sub)
    pub fn push(elements: Vec<RespValue>) -> Self {
        RespValue::Push(elements)
    }

    // === Accessors ===

    /// Extract as string if possible
    pub fn as_string(&self) -> Option<String> {
        match self {
            RespValue::SimpleString(s) => Some(s.clone()),
            RespValue::BulkString(b) => String::from_utf8(b.to_vec()).ok(),
            RespValue::VerbatimString { data, .. } => String::from_utf8(data.to_vec()).ok(),
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

    /// Extract as double if possible (RESP3)
    pub fn as_double(&self) -> Option<f64> {
        match self {
            RespValue::Double(d) => Some(*d),
            RespValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Extract as boolean if possible (RESP3)
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            RespValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract as array if possible
    pub fn as_array(&self) -> Option<&Vec<RespValue>> {
        match self {
            RespValue::Array(arr) => Some(arr),
            RespValue::Set(set) => Some(set),
            RespValue::Push(push) => Some(push),
            _ => None,
        }
    }

    /// Extract as map if possible (RESP3)
    pub fn as_map(&self) -> Option<&Vec<(RespValue, RespValue)>> {
        match self {
            RespValue::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Check if this is a null value (any variant)
    pub fn is_null(&self) -> bool {
        matches!(
            self,
            RespValue::NullBulkString | RespValue::NullArray | RespValue::Null
        )
    }

    /// Check if this is a RESP3 type
    pub fn is_resp3(&self) -> bool {
        matches!(
            self,
            RespValue::Null
                | RespValue::Boolean(_)
                | RespValue::Double(_)
                | RespValue::BigNumber(_)
                | RespValue::BulkError(_)
                | RespValue::VerbatimString { .. }
                | RespValue::Map(_)
                | RespValue::Set(_)
                | RespValue::Attribute(_)
                | RespValue::Push(_)
        )
    }

    /// Serialize to RESP format (RESP2 or RESP3 depending on type)
    pub fn serialize(&self) -> Bytes {
        let mut buf = Vec::new();
        self.write_to(&mut buf);
        Bytes::from(buf)
    }

    fn write_to(&self, buf: &mut Vec<u8>) {
        match self {
            // === RESP2 Types ===
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

            // === RESP3 Types ===
            RespValue::Null => {
                buf.extend_from_slice(b"_\r\n");
            }
            RespValue::Boolean(b) => {
                buf.push(b'#');
                buf.push(if *b { b't' } else { b'f' });
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Double(d) => {
                buf.push(b',');
                // Handle special float values
                if d.is_nan() {
                    buf.extend_from_slice(b"nan");
                } else if d.is_infinite() {
                    if d.is_sign_positive() {
                        buf.extend_from_slice(b"inf");
                    } else {
                        buf.extend_from_slice(b"-inf");
                    }
                } else {
                    buf.extend_from_slice(d.to_string().as_bytes());
                }
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::BigNumber(n) => {
                buf.push(b'(');
                buf.extend_from_slice(n.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::BulkError(bytes) => {
                buf.push(b'!');
                buf.extend_from_slice(bytes.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(bytes);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::VerbatimString { encoding, data } => {
                // Format: =<length>\r\n<encoding>:<data>\r\n
                // Length includes encoding (3 chars) + colon + data
                let total_len = 3 + 1 + data.len();
                buf.push(b'=');
                buf.extend_from_slice(total_len.to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                // Encoding is exactly 3 characters
                let enc_bytes = encoding.as_bytes();
                if enc_bytes.len() >= 3 {
                    buf.extend_from_slice(&enc_bytes[..3]);
                } else {
                    buf.extend_from_slice(enc_bytes);
                    for _ in 0..(3 - enc_bytes.len()) {
                        buf.push(b' ');
                    }
                }
                buf.push(b':');
                buf.extend_from_slice(data);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Map(entries) => {
                buf.push(b'%');
                buf.extend_from_slice(entries.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for (key, value) in entries {
                    key.write_to(buf);
                    value.write_to(buf);
                }
            }
            RespValue::Set(elements) => {
                buf.push(b'~');
                buf.extend_from_slice(elements.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for val in elements {
                    val.write_to(buf);
                }
            }
            RespValue::Attribute(attrs) => {
                buf.push(b'|');
                buf.extend_from_slice(attrs.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for (key, value) in attrs {
                    // Keys are always simple strings in attributes
                    RespValue::SimpleString(key.clone()).write_to(buf);
                    value.write_to(buf);
                }
            }
            RespValue::Push(elements) => {
                buf.push(b'>');
                buf.extend_from_slice(elements.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for val in elements {
                    val.write_to(buf);
                }
            }
        }
    }
}

impl fmt::Display for RespValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // RESP2 types
            RespValue::SimpleString(s) => write!(f, "\"{s}\""),
            RespValue::Error(s) => write!(f, "ERROR: {s}"),
            RespValue::Integer(i) => write!(f, "{i}"),
            RespValue::BulkString(b) => {
                if let Ok(s) = std::str::from_utf8(b) {
                    write!(f, "\"{s}\"")
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
                    write!(f, "{val}")?;
                }
                write!(f, "]")
            }
            RespValue::NullArray => write!(f, "null"),

            // RESP3 types
            RespValue::Null => write!(f, "null"),
            RespValue::Boolean(b) => write!(f, "{b}"),
            RespValue::Double(d) => {
                if d.is_nan() {
                    write!(f, "NaN")
                } else if d.is_infinite() {
                    if d.is_sign_positive() {
                        write!(f, "Infinity")
                    } else {
                        write!(f, "-Infinity")
                    }
                } else {
                    write!(f, "{d}")
                }
            }
            RespValue::BigNumber(n) => write!(f, "{n}"),
            RespValue::BulkError(b) => {
                if let Ok(s) = std::str::from_utf8(b) {
                    write!(f, "ERROR: {s}")
                } else {
                    write!(f, "ERROR: <binary:{} bytes>", b.len())
                }
            }
            RespValue::VerbatimString { encoding, data } => {
                if let Ok(s) = std::str::from_utf8(data) {
                    write!(f, "\"{s}\" ({encoding})")
                } else {
                    write!(f, "<binary:{} bytes> ({encoding})", data.len())
                }
            }
            RespValue::Map(entries) => {
                write!(f, "{{")?;
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{key}: {value}")?;
                }
                write!(f, "}}")
            }
            RespValue::Set(elements) => {
                write!(f, "Set{{")?;
                for (i, val) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{val}")?;
                }
                write!(f, "}}")
            }
            RespValue::Attribute(attrs) => {
                write!(f, "Attr{{")?;
                for (i, (key, value)) in attrs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{key}: {value}")?;
                }
                write!(f, "}}")
            }
            RespValue::Push(elements) => {
                write!(f, "Push[")?;
                for (i, val) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{val}")?;
                }
                write!(f, "]")
            }
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

// RESP3 From implementations

impl From<bool> for RespValue {
    fn from(b: bool) -> Self {
        RespValue::Boolean(b)
    }
}

impl From<f64> for RespValue {
    fn from(d: f64) -> Self {
        RespValue::Double(d)
    }
}

impl From<f32> for RespValue {
    fn from(d: f32) -> Self {
        RespValue::Double(d as f64)
    }
}

impl From<Vec<(RespValue, RespValue)>> for RespValue {
    fn from(entries: Vec<(RespValue, RespValue)>) -> Self {
        RespValue::Map(entries)
    }
}

impl From<HashMap<String, RespValue>> for RespValue {
    fn from(attrs: HashMap<String, RespValue>) -> Self {
        RespValue::Attribute(attrs)
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
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
        let arr = vec![RespValue::simple_string("foo"), RespValue::integer(123)];
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

    // === RESP3 Tests ===

    #[test]
    fn test_resp3_null_creation() {
        let val = RespValue::resp3_null();
        assert_eq!(val, RespValue::Null);
        assert!(val.is_null());
        assert!(val.is_resp3());
    }

    #[test]
    fn test_boolean_creation() {
        let val_true = RespValue::boolean(true);
        let val_false = RespValue::boolean(false);

        assert_eq!(val_true, RespValue::Boolean(true));
        assert_eq!(val_false, RespValue::Boolean(false));
        assert!(val_true.is_resp3());
        assert_eq!(val_true.as_boolean(), Some(true));
        assert_eq!(val_false.as_boolean(), Some(false));
    }

    #[test]
    fn test_double_creation() {
        let val = RespValue::double(3.14159);
        assert_eq!(val, RespValue::Double(3.14159));
        assert!(val.is_resp3());
        assert!((val.as_double().unwrap() - 3.14159).abs() < 0.0001);

        // Test special values
        let inf = RespValue::double(f64::INFINITY);
        let neg_inf = RespValue::double(f64::NEG_INFINITY);
        let nan = RespValue::double(f64::NAN);

        assert!(inf.as_double().unwrap().is_infinite());
        assert!(neg_inf.as_double().unwrap().is_infinite());
        assert!(nan.as_double().unwrap().is_nan());
    }

    #[test]
    fn test_big_number_creation() {
        let big = RespValue::big_number("123456789012345678901234567890");
        assert_eq!(
            big,
            RespValue::BigNumber("123456789012345678901234567890".to_string())
        );
        assert!(big.is_resp3());
    }

    #[test]
    fn test_bulk_error_creation() {
        let err = RespValue::bulk_error(Bytes::from("ERR something went wrong"));
        assert!(matches!(err, RespValue::BulkError(_)));
        assert!(err.is_resp3());
    }

    #[test]
    fn test_verbatim_string_creation() {
        let val = RespValue::verbatim_string("txt", Bytes::from("Hello World"));
        assert!(matches!(val, RespValue::VerbatimString { .. }));
        assert!(val.is_resp3());
        assert_eq!(val.as_string(), Some("Hello World".to_string()));
    }

    #[test]
    fn test_map_creation() {
        let entries = vec![
            (
                RespValue::bulk_string_from_str("key1"),
                RespValue::integer(100),
            ),
            (
                RespValue::bulk_string_from_str("key2"),
                RespValue::simple_string("value"),
            ),
        ];
        let map = RespValue::map(entries.clone());
        assert_eq!(map.as_map(), Some(&entries));
        assert!(map.is_resp3());
    }

    #[test]
    fn test_set_creation() {
        let elements = vec![
            RespValue::integer(1),
            RespValue::integer(2),
            RespValue::integer(3),
        ];
        let set = RespValue::set(elements.clone());
        assert!(matches!(set, RespValue::Set(_)));
        assert!(set.is_resp3());
        // Sets are treated as arrays for as_array()
        assert_eq!(set.as_array(), Some(&elements));
    }

    #[test]
    fn test_push_creation() {
        let elements = vec![
            RespValue::bulk_string_from_str("message"),
            RespValue::bulk_string_from_str("channel"),
            RespValue::bulk_string_from_str("data"),
        ];
        let push = RespValue::push(elements.clone());
        assert!(matches!(push, RespValue::Push(_)));
        assert!(push.is_resp3());
        // Push is treated as array for as_array()
        assert_eq!(push.as_array(), Some(&elements));
    }

    #[test]
    fn test_attribute_creation() {
        let mut attrs = HashMap::new();
        attrs.insert("ttl".to_string(), RespValue::integer(3600));
        attrs.insert("flags".to_string(), RespValue::integer(0));
        let attr = RespValue::attribute(attrs);
        assert!(matches!(attr, RespValue::Attribute(_)));
        assert!(attr.is_resp3());
    }

    #[test]
    fn test_serialize_resp3_null() {
        let val = RespValue::Null;
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("_\r\n"));
    }

    #[test]
    fn test_serialize_boolean() {
        let val_true = RespValue::Boolean(true);
        let val_false = RespValue::Boolean(false);

        assert_eq!(val_true.serialize(), Bytes::from("#t\r\n"));
        assert_eq!(val_false.serialize(), Bytes::from("#f\r\n"));
    }

    #[test]
    fn test_serialize_double() {
        let val = RespValue::double(3.14);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from(",3.14\r\n"));

        // Test special values
        let inf = RespValue::double(f64::INFINITY);
        assert_eq!(inf.serialize(), Bytes::from(",inf\r\n"));

        let neg_inf = RespValue::double(f64::NEG_INFINITY);
        assert_eq!(neg_inf.serialize(), Bytes::from(",-inf\r\n"));

        let nan = RespValue::double(f64::NAN);
        assert_eq!(nan.serialize(), Bytes::from(",nan\r\n"));
    }

    #[test]
    fn test_serialize_big_number() {
        let val = RespValue::big_number("123456789");
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("(123456789\r\n"));
    }

    #[test]
    fn test_serialize_bulk_error() {
        let val = RespValue::bulk_error(Bytes::from("SYNTAX error"));
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("!12\r\nSYNTAX error\r\n"));
    }

    #[test]
    fn test_serialize_verbatim_string() {
        let val = RespValue::verbatim_string("txt", Bytes::from("hello"));
        let serialized = val.serialize();
        // Length = 3 (encoding) + 1 (colon) + 5 (data) = 9
        assert_eq!(serialized, Bytes::from("=9\r\ntxt:hello\r\n"));
    }

    #[test]
    fn test_serialize_map() {
        let entries = vec![(
            RespValue::bulk_string_from_str("key"),
            RespValue::integer(42),
        )];
        let val = RespValue::map(entries);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("%1\r\n$3\r\nkey\r\n:42\r\n"));
    }

    #[test]
    fn test_serialize_set() {
        let val = RespValue::set(vec![RespValue::integer(1), RespValue::integer(2)]);
        let serialized = val.serialize();
        assert_eq!(serialized, Bytes::from("~2\r\n:1\r\n:2\r\n"));
    }

    #[test]
    fn test_serialize_push() {
        let val = RespValue::push(vec![
            RespValue::bulk_string_from_str("pubsub"),
            RespValue::bulk_string_from_str("message"),
        ]);
        let serialized = val.serialize();
        assert_eq!(
            serialized,
            Bytes::from(">2\r\n$6\r\npubsub\r\n$7\r\nmessage\r\n")
        );
    }

    #[test]
    fn test_display_resp3_types() {
        assert_eq!(format!("{}", RespValue::Null), "null");
        assert_eq!(format!("{}", RespValue::Boolean(true)), "true");
        assert_eq!(format!("{}", RespValue::Boolean(false)), "false");
        assert_eq!(format!("{}", RespValue::Double(3.14)), "3.14");
        assert_eq!(format!("{}", RespValue::Double(f64::INFINITY)), "Infinity");
        assert_eq!(format!("{}", RespValue::Double(f64::NAN)), "NaN");
        assert_eq!(
            format!("{}", RespValue::BigNumber("123".to_string())),
            "123"
        );
    }

    #[test]
    fn test_from_bool() {
        let val: RespValue = true.into();
        assert_eq!(val, RespValue::Boolean(true));

        let val: RespValue = false.into();
        assert_eq!(val, RespValue::Boolean(false));
    }

    #[test]
    fn test_from_f64() {
        let val: RespValue = 3.14f64.into();
        assert_eq!(val, RespValue::Double(3.14));
    }

    #[test]
    fn test_from_f32() {
        let val: RespValue = 3.14f32.into();
        assert!(matches!(val, RespValue::Double(_)));
    }

    #[test]
    fn test_is_resp3() {
        // RESP2 types should return false
        assert!(!RespValue::SimpleString("test".to_string()).is_resp3());
        assert!(!RespValue::Integer(42).is_resp3());
        assert!(!RespValue::Array(vec![]).is_resp3());
        assert!(!RespValue::NullBulkString.is_resp3());

        // RESP3 types should return true
        assert!(RespValue::Null.is_resp3());
        assert!(RespValue::Boolean(true).is_resp3());
        assert!(RespValue::Double(1.0).is_resp3());
        assert!(RespValue::BigNumber("1".to_string()).is_resp3());
        assert!(RespValue::Map(vec![]).is_resp3());
        assert!(RespValue::Set(vec![]).is_resp3());
        assert!(RespValue::Push(vec![]).is_resp3());
    }

    #[test]
    fn test_as_double_from_integer() {
        let val = RespValue::Integer(42);
        assert_eq!(val.as_double(), Some(42.0));
    }
}
