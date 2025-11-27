//! CQL wire protocol implementation
//!
//! This module implements the Cassandra wire protocol (version 4).

use crate::error::{ProtocolError, ProtocolResult};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;

/// CQL protocol version
pub const PROTOCOL_VERSION: u8 = 4;

/// CQL frame header size (9 bytes)
pub const FRAME_HEADER_SIZE: usize = 9;

/// CQL frame
#[derive(Debug, Clone)]
pub struct CqlFrame {
    /// Protocol version
    pub version: u8,
    /// Flags
    pub flags: u8,
    /// Stream ID
    pub stream: i16,
    /// Opcode
    pub opcode: CqlOpcode,
    /// Frame body
    pub body: Bytes,
}

impl CqlFrame {
    /// Create a new frame
    pub fn new(opcode: CqlOpcode, body: Bytes) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            flags: 0,
            stream: 0,
            opcode,
            body,
        }
    }

    /// Create a response frame
    pub fn response(stream: i16, opcode: CqlOpcode, body: Bytes) -> Self {
        Self {
            version: PROTOCOL_VERSION | 0x80, // Set response bit
            flags: 0,
            stream,
            opcode,
            body,
        }
    }

    /// Encode frame to bytes
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + self.body.len());

        // Header
        buf.put_u8(self.version);
        buf.put_u8(self.flags);
        buf.put_i16(self.stream);
        buf.put_u8(self.opcode as u8);
        buf.put_u32(self.body.len() as u32);

        // Body
        buf.put(self.body.clone());

        buf
    }

    /// Decode frame from bytes
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        if buf.len() < FRAME_HEADER_SIZE {
            return Err(ProtocolError::IncompleteFrame);
        }

        let version = buf.get_u8();
        let flags = buf.get_u8();
        let stream = buf.get_i16();
        let opcode_byte = buf.get_u8();
        let length = buf.get_u32() as usize;

        if buf.len() < length {
            return Err(ProtocolError::IncompleteFrame);
        }

        let opcode = CqlOpcode::from_u8(opcode_byte)?;
        let body = buf.copy_to_bytes(length);

        Ok(Self {
            version,
            flags,
            stream,
            opcode,
            body,
        })
    }

    /// Check if this is a request frame
    pub fn is_request(&self) -> bool {
        self.version & 0x80 == 0
    }

    /// Check if this is a response frame
    pub fn is_response(&self) -> bool {
        self.version & 0x80 != 0
    }
}

/// CQL opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CqlOpcode {
    /// ERROR response
    Error = 0x00,
    /// STARTUP request
    Startup = 0x01,
    /// READY response
    Ready = 0x02,
    /// AUTHENTICATE response
    Authenticate = 0x03,
    /// OPTIONS request
    Options = 0x05,
    /// SUPPORTED response
    Supported = 0x06,
    /// QUERY request
    Query = 0x07,
    /// RESULT response
    Result = 0x08,
    /// PREPARE request
    Prepare = 0x09,
    /// EXECUTE request
    Execute = 0x0A,
    /// REGISTER request
    Register = 0x0B,
    /// EVENT response
    Event = 0x0C,
    /// BATCH request
    Batch = 0x0D,
    /// AUTH_CHALLENGE response
    AuthChallenge = 0x0E,
    /// AUTH_RESPONSE request
    AuthResponse = 0x0F,
    /// AUTH_SUCCESS response
    AuthSuccess = 0x10,
}

impl CqlOpcode {
    /// Convert u8 to opcode
    pub fn from_u8(byte: u8) -> ProtocolResult<Self> {
        match byte {
            0x00 => Ok(CqlOpcode::Error),
            0x01 => Ok(CqlOpcode::Startup),
            0x02 => Ok(CqlOpcode::Ready),
            0x03 => Ok(CqlOpcode::Authenticate),
            0x05 => Ok(CqlOpcode::Options),
            0x06 => Ok(CqlOpcode::Supported),
            0x07 => Ok(CqlOpcode::Query),
            0x08 => Ok(CqlOpcode::Result),
            0x09 => Ok(CqlOpcode::Prepare),
            0x0A => Ok(CqlOpcode::Execute),
            0x0B => Ok(CqlOpcode::Register),
            0x0C => Ok(CqlOpcode::Event),
            0x0D => Ok(CqlOpcode::Batch),
            0x0E => Ok(CqlOpcode::AuthChallenge),
            0x0F => Ok(CqlOpcode::AuthResponse),
            0x10 => Ok(CqlOpcode::AuthSuccess),
            _ => Err(ProtocolError::InvalidOpcode(byte)),
        }
    }
}

/// Consistency level for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ConsistencyLevel {
    /// Any (write only)
    Any = 0x0000,
    /// One replica
    One = 0x0001,
    /// Two replicas
    Two = 0x0002,
    /// Three replicas
    Three = 0x0003,
    /// Quorum (majority)
    Quorum = 0x0004,
    /// All replicas
    All = 0x0005,
    /// Local quorum
    LocalQuorum = 0x0006,
    /// Each quorum
    EachQuorum = 0x0007,
    /// Serial (lightweight transaction)
    Serial = 0x0008,
    /// Local serial
    LocalSerial = 0x0009,
    /// Local one
    LocalOne = 0x000A,
}

impl ConsistencyLevel {
    /// Convert u16 to consistency level
    pub fn from_u16(value: u16) -> ProtocolResult<Self> {
        match value {
            0x0000 => Ok(ConsistencyLevel::Any),
            0x0001 => Ok(ConsistencyLevel::One),
            0x0002 => Ok(ConsistencyLevel::Two),
            0x0003 => Ok(ConsistencyLevel::Three),
            0x0004 => Ok(ConsistencyLevel::Quorum),
            0x0005 => Ok(ConsistencyLevel::All),
            0x0006 => Ok(ConsistencyLevel::LocalQuorum),
            0x0007 => Ok(ConsistencyLevel::EachQuorum),
            0x0008 => Ok(ConsistencyLevel::Serial),
            0x0009 => Ok(ConsistencyLevel::LocalSerial),
            0x000A => Ok(ConsistencyLevel::LocalOne),
            _ => Err(ProtocolError::InvalidConsistencyLevel(value)),
        }
    }
}

impl Default for ConsistencyLevel {
    fn default() -> Self {
        ConsistencyLevel::One
    }
}

/// Query parameters
#[derive(Debug, Clone)]
pub struct QueryParameters {
    /// Consistency level
    pub consistency: ConsistencyLevel,
    /// Values for bound parameters
    pub values: Option<Vec<Bytes>>,
    /// Skip metadata in response
    pub skip_metadata: bool,
    /// Page size
    pub page_size: Option<i32>,
    /// Paging state
    pub paging_state: Option<Bytes>,
    /// Serial consistency
    pub serial_consistency: Option<ConsistencyLevel>,
    /// Default timestamp
    pub default_timestamp: Option<i64>,
}

impl Default for QueryParameters {
    fn default() -> Self {
        Self {
            consistency: ConsistencyLevel::One,
            values: None,
            skip_metadata: false,
            page_size: None,
            paging_state: None,
            serial_consistency: None,
            default_timestamp: None,
        }
    }
}

impl QueryParameters {
    /// Decode query parameters from bytes
    pub fn decode(mut buf: Bytes) -> ProtocolResult<Self> {
        let consistency = ConsistencyLevel::from_u16(buf.get_u16())?;
        let flags = buf.get_u8();

        let values = if flags & 0x01 != 0 {
            let count = buf.get_u16();
            let mut vals = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let len = buf.get_i32();
                if len >= 0 {
                    let value = buf.copy_to_bytes(len as usize);
                    vals.push(value);
                } else {
                    vals.push(Bytes::new());
                }
            }
            Some(vals)
        } else {
            None
        };

        let skip_metadata = flags & 0x02 != 0;

        let page_size = if flags & 0x04 != 0 {
            Some(buf.get_i32())
        } else {
            None
        };

        let paging_state = if flags & 0x08 != 0 {
            let len = buf.get_i32();
            Some(buf.copy_to_bytes(len as usize))
        } else {
            None
        };

        let serial_consistency = if flags & 0x10 != 0 {
            Some(ConsistencyLevel::from_u16(buf.get_u16())?)
        } else {
            None
        };

        let default_timestamp = if flags & 0x20 != 0 {
            Some(buf.get_i64())
        } else {
            None
        };

        Ok(Self {
            consistency,
            values,
            skip_metadata,
            page_size,
            paging_state,
            serial_consistency,
            default_timestamp,
        })
    }
}

/// Result kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ResultKind {
    /// Void result
    Void = 0x0001,
    /// Rows result
    Rows = 0x0002,
    /// Set keyspace result
    SetKeyspace = 0x0003,
    /// Prepared statement result
    Prepared = 0x0004,
    /// Schema change result
    SchemaChange = 0x0005,
}

/// Build a READY response
pub fn build_ready_response(stream: i16) -> CqlFrame {
    CqlFrame::response(stream, CqlOpcode::Ready, Bytes::new())
}

/// Build a SUPPORTED response
pub fn build_supported_response(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();

    // String map of supported options
    let options: HashMap<String, Vec<String>> = [
        ("CQL_VERSION".to_string(), vec!["3.4.5".to_string()]),
        (
            "COMPRESSION".to_string(),
            vec!["snappy".to_string(), "lz4".to_string()],
        ),
    ]
    .iter()
    .cloned()
    .collect();

    // Encode string multimap
    body.put_u16(options.len() as u16);
    for (key, values) in options {
        write_string(&mut body, &key);
        body.put_u16(values.len() as u16);
        for value in values {
            write_string(&mut body, &value);
        }
    }

    CqlFrame::response(stream, CqlOpcode::Supported, body.freeze())
}

/// Build a RESULT response with VOID
pub fn build_void_result(stream: i16) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(ResultKind::Void as i32);
    CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
}

/// CQL error codes (from Cassandra native protocol v4)
pub mod error_codes {
    /// Server error (generic)
    pub const SERVER_ERROR: i32 = 0x0000;
    /// Protocol error
    pub const PROTOCOL_ERROR: i32 = 0x000A;
    /// Bad credentials
    pub const BAD_CREDENTIALS: i32 = 0x0100;
    /// Unavailable exception
    pub const UNAVAILABLE: i32 = 0x1000;
    /// Overloaded
    pub const OVERLOADED: i32 = 0x1100;
    /// Is bootstrapping
    pub const IS_BOOTSTRAPPING: i32 = 0x1200;
    /// Truncate error
    pub const TRUNCATE_ERROR: i32 = 0x1300;
    /// Write timeout
    pub const WRITE_TIMEOUT: i32 = 0x2000;
    /// Read timeout
    pub const READ_TIMEOUT: i32 = 0x2100;
    /// Syntax error
    pub const SYNTAX_ERROR: i32 = 0x2200;
    /// Unauthorized
    pub const UNAUTHORIZED: i32 = 0x2300;
    /// Invalid (invalid query, invalid request, etc.)
    pub const INVALID: i32 = 0x2400;
    /// Config error
    pub const CONFIG_ERROR: i32 = 0x2500;
    /// Already exists (keyspace, table, etc.)
    pub const ALREADY_EXISTS: i32 = 0x2600;
    /// Unprepared (prepared statement not found)
    pub const UNPREPARED: i32 = 0x2700;
}

/// Map ProtocolError to CQL error code
pub fn map_error_to_cql_code(error: &crate::error::ProtocolError) -> i32 {
    use crate::error::ProtocolError;
    use error_codes::*;

    match error {
        ProtocolError::ParseError(_) => SYNTAX_ERROR,
        ProtocolError::CqlError(_) => PROTOCOL_ERROR,
        ProtocolError::PostgresError(msg) => {
            // Check for specific SQL errors
            let msg_lower = msg.to_lowercase();
            if msg_lower.contains("does not exist") || msg_lower.contains("not found") {
                INVALID
            } else if msg_lower.contains("already exists") || msg_lower.contains("duplicate") {
                ALREADY_EXISTS
            } else if msg_lower.contains("syntax") || msg_lower.contains("parse") {
                SYNTAX_ERROR
            } else if msg_lower.contains("unauthorized") || msg_lower.contains("permission") {
                UNAUTHORIZED
            } else if msg_lower.contains("timeout") {
                if msg_lower.contains("write") {
                    WRITE_TIMEOUT
                } else {
                    READ_TIMEOUT
                }
            } else {
                INVALID
            }
        }
        ProtocolError::AuthenticationError(_) => BAD_CREDENTIALS,
        ProtocolError::AuthorizationError(_) => UNAUTHORIZED,
        ProtocolError::InvalidOpcode(_) => PROTOCOL_ERROR,
        ProtocolError::InvalidConsistencyLevel(_) => PROTOCOL_ERROR,
        ProtocolError::IncompleteFrame => PROTOCOL_ERROR,
        ProtocolError::InvalidUtf8(_) => PROTOCOL_ERROR,
        ProtocolError::InvalidStatement(_) => SYNTAX_ERROR,
        ProtocolError::ConnectionError(_) => UNAVAILABLE,
        ProtocolError::ConnectionClosed => UNAVAILABLE,
        _ => SERVER_ERROR,
    }
}

/// Build an ERROR response
pub fn build_error_response(stream: i16, error_code: i32, message: &str) -> CqlFrame {
    let mut body = BytesMut::new();
    body.put_i32(error_code);
    write_string(&mut body, message);
    CqlFrame::response(stream, CqlOpcode::Error, body.freeze())
}

/// Build an ERROR response from a ProtocolError
pub fn build_error_from_protocol_error(
    stream: i16,
    error: &crate::error::ProtocolError,
) -> CqlFrame {
    let error_code = map_error_to_cql_code(error);
    let message = error.to_string();
    build_error_response(stream, error_code, &message)
}

/// Write a CQL string (2-byte length + UTF-8 bytes)
fn write_string(buf: &mut BytesMut, s: &str) {
    buf.put_u16(s.len() as u16);
    buf.put(s.as_bytes());
}

/// Read a CQL string
pub fn read_string(buf: &mut Bytes) -> ProtocolResult<String> {
    let len = buf.get_u16();
    let bytes = buf.copy_to_bytes(len as usize);
    String::from_utf8(bytes.to_vec()).map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))
}

/// Read a CQL string map
pub fn read_string_map(buf: &mut Bytes) -> ProtocolResult<HashMap<String, String>> {
    let count = buf.get_u16();
    let mut map = HashMap::new();
    for _ in 0..count {
        let key = read_string(buf)?;
        let value = read_string(buf)?;
        map.insert(key, value);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let frame = CqlFrame::new(CqlOpcode::Query, Bytes::from("SELECT * FROM users"));
        let encoded = frame.encode();
        let decoded = CqlFrame::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.opcode, CqlOpcode::Query);
        assert_eq!(decoded.body, Bytes::from("SELECT * FROM users"));
    }

    #[test]
    fn test_opcode_conversion() {
        assert_eq!(CqlOpcode::from_u8(0x07).unwrap(), CqlOpcode::Query);
        assert_eq!(CqlOpcode::from_u8(0x08).unwrap(), CqlOpcode::Result);
        assert!(CqlOpcode::from_u8(0xFF).is_err());
    }

    #[test]
    fn test_consistency_level() {
        assert_eq!(
            ConsistencyLevel::from_u16(0x0001).unwrap(),
            ConsistencyLevel::One
        );
        assert_eq!(
            ConsistencyLevel::from_u16(0x0004).unwrap(),
            ConsistencyLevel::Quorum
        );
    }
}
