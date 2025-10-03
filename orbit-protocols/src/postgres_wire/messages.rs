//! PostgreSQL wire protocol message types
//!
//! This module implements all message types used in the PostgreSQL wire protocol.
//! See: https://www.postgresql.org/docs/current/protocol-message-formats.html

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::io::Cursor;

use crate::error::{ProtocolError, ProtocolResult};

// Note: Frontend and backend message types are separate
// because they share some byte values (C, D, E, S)

/// Frontend message type identifiers (client -> server)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FrontendMessageType {
    Query = b'Q' as isize,
    Parse = b'P' as isize,
    Bind = b'B' as isize,
    Execute = b'E' as isize,
    Describe = b'D' as isize,
    Close = b'C' as isize,
    Flush = b'H' as isize,
    Sync = b'S' as isize,
    Terminate = b'X' as isize,
    PasswordMessage = b'p' as isize,
}

/// Backend message type identifiers (server -> client)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BackendMessageType {
    Authentication = b'R' as isize,
    BackendKeyData = b'K' as isize,
    BindComplete = b'2' as isize,
    CloseComplete = b'3' as isize,
    CommandComplete = b'C' as isize,
    DataRow = b'D' as isize,
    EmptyQueryResponse = b'I' as isize,
    ErrorResponse = b'E' as isize,
    NoData = b'n' as isize,
    NoticeResponse = b'N' as isize,
    ParameterDescription = b't' as isize,
    ParameterStatus = b'S' as isize,
    ParseComplete = b'1' as isize,
    ReadyForQuery = b'Z' as isize,
    RowDescription = b'T' as isize,
}

/// Frontend (client) messages
#[derive(Debug, Clone)]
pub enum FrontendMessage {
    /// Startup message (no type byte, length-prefixed)
    Startup {
        protocol_version: i32,
        parameters: HashMap<String, String>,
    },
    /// Simple query
    Query { query: String },
    /// Parse (prepared statement)
    Parse {
        statement_name: String,
        query: String,
        param_types: Vec<i32>,
    },
    /// Bind parameters to prepared statement
    Bind {
        portal: String,
        statement: String,
        param_formats: Vec<i16>,
        params: Vec<Option<Bytes>>,
        result_formats: Vec<i16>,
    },
    /// Execute portal
    Execute {
        portal: String,
        max_rows: i32,
    },
    /// Describe prepared statement or portal
    Describe {
        target: DescribeTarget,
        name: String,
    },
    /// Close prepared statement or portal
    Close {
        target: CloseTarget,
        name: String,
    },
    /// Flush output
    Flush,
    /// Sync (end of extended query)
    Sync,
    /// Terminate connection
    Terminate,
    /// Password message
    Password { password: String },
}

#[derive(Debug, Clone, Copy)]
pub enum DescribeTarget {
    Statement,
    Portal,
}

#[derive(Debug, Clone, Copy)]
pub enum CloseTarget {
    Statement,
    Portal,
}

/// Backend (server) messages
#[derive(Debug, Clone)]
pub enum BackendMessage {
    /// Authentication response
    Authentication(AuthenticationResponse),
    /// Backend key data for cancellation
    BackendKeyData {
        process_id: i32,
        secret_key: i32,
    },
    /// Bind complete
    BindComplete,
    /// Close complete
    CloseComplete,
    /// Command completion
    CommandComplete { tag: String },
    /// Data row
    DataRow { values: Vec<Option<Bytes>> },
    /// Empty query response
    EmptyQueryResponse,
    /// Error response
    ErrorResponse { fields: HashMap<u8, String> },
    /// No data
    NoData,
    /// Notice response
    NoticeResponse { fields: HashMap<u8, String> },
    /// Parameter description
    ParameterDescription { param_types: Vec<i32> },
    /// Parameter status
    ParameterStatus { name: String, value: String },
    /// Parse complete
    ParseComplete,
    /// Ready for query
    ReadyForQuery { status: TransactionStatus },
    /// Row description
    RowDescription { fields: Vec<FieldDescription> },
}

#[derive(Debug, Clone)]
pub enum AuthenticationResponse {
    Ok,
    CleartextPassword,
    MD5Password { salt: [u8; 4] },
    KerberosV5,
    SCMCredential,
    GSS,
    SSPI,
    GSSContinue { data: Bytes },
    SASL { mechanisms: Vec<String> },
    SASLContinue { data: Bytes },
    SASLFinal { data: Bytes },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Idle = b'I' as isize,
    InTransaction = b'T' as isize,
    InFailedTransaction = b'E' as isize,
}

#[derive(Debug, Clone)]
pub struct FieldDescription {
    pub name: String,
    pub table_oid: i32,
    pub column_id: i16,
    pub type_oid: i32,
    pub type_size: i16,
    pub type_modifier: i32,
    pub format: i16,
}

impl FrontendMessage {
    /// Parse a frontend message from bytes
    pub fn parse(buf: &mut BytesMut) -> ProtocolResult<Option<Self>> {
        if buf.len() < 5 {
            return Ok(None); // Need at least type byte + length
        }

        // Check for startup message (no type byte, starts with length)
        if buf.len() >= 4 {
            let mut peek = Cursor::new(&buf[..]);
            let len = peek.get_i32() as usize;
            
            if len >= 8 && buf.len() >= len {
                // Check if this looks like a startup message
                let protocol_version = (&buf[4..8]).get_i32();
                if protocol_version == 196608 || protocol_version == 80877103 {
                    // Valid startup or SSL request
                    return Self::parse_startup(buf);
                }
            }
        }

        let msg_type = buf[0];
        
        // Check if we have enough data for the length
        if buf.len() < 5 {
            return Ok(None);
        }

        let len = {
            let mut cursor = Cursor::new(&buf[1..5]);
            cursor.get_i32() as usize
        };
        
        if buf.len() < 1 + len {
            return Ok(None); // Not enough data yet
        }

        // Copy the message data before advancing buffer
        let msg_data = buf[5..1 + len].to_vec();
        
        // Remove message from buffer
        buf.advance(1 + len);

        // Parse the message data
        let mut cursor = Cursor::new(&msg_data[..]);
        let message = match msg_type {
            b'Q' => Self::parse_query(&mut cursor)?,
            b'P' => Self::parse_parse(&mut cursor)?,
            b'B' => Self::parse_bind(&mut cursor)?,
            b'E' => Self::parse_execute(&mut cursor)?,
            b'D' => Self::parse_describe(&mut cursor)?,
            b'C' => Self::parse_close(&mut cursor)?,
            b'H' => FrontendMessage::Flush,
            b'S' => FrontendMessage::Sync,
            b'X' => FrontendMessage::Terminate,
            b'p' => Self::parse_password(&mut cursor)?,
            _ => {
                return Err(ProtocolError::PostgresError(format!(
                    "Unknown message type: {}",
                    msg_type as char
                )))
            }
        };

        Ok(Some(message))
    }

    fn parse_startup(buf: &mut BytesMut) -> ProtocolResult<Option<Self>> {
        let mut cursor = Cursor::new(&buf[..]);
        let len = cursor.get_i32() as usize;

        if buf.len() < len {
            return Ok(None);
        }

        let protocol_version = cursor.get_i32();

        // SSL request
        if protocol_version == 80877103 {
            buf.advance(len);
            // Return None to indicate SSL not supported
            return Ok(None);
        }

        let mut parameters = HashMap::new();
        while cursor.position() < len as u64 - 1 {
            let key = read_cstring(&mut cursor)?;
            if key.is_empty() {
                break;
            }
            let value = read_cstring(&mut cursor)?;
            parameters.insert(key, value);
        }

        buf.advance(len);

        Ok(Some(FrontendMessage::Startup {
            protocol_version,
            parameters,
        }))
    }

    fn parse_query(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let query = read_cstring(cursor)?;
        Ok(FrontendMessage::Query { query })
    }

    fn parse_parse(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let statement_name = read_cstring(cursor)?;
        let query = read_cstring(cursor)?;
        let param_count = cursor.get_i16() as usize;
        let mut param_types = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            param_types.push(cursor.get_i32());
        }
        Ok(FrontendMessage::Parse {
            statement_name,
            query,
            param_types,
        })
    }

    fn parse_bind(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let portal = read_cstring(cursor)?;
        let statement = read_cstring(cursor)?;

        let format_count = cursor.get_i16() as usize;
        let mut param_formats = Vec::with_capacity(format_count);
        for _ in 0..format_count {
            param_formats.push(cursor.get_i16());
        }

        let param_count = cursor.get_i16() as usize;
        let mut params = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            let len = cursor.get_i32();
            if len == -1 {
                params.push(None);
            } else {
                let mut data = vec![0u8; len as usize];
                cursor.copy_to_slice(&mut data);
                params.push(Some(Bytes::from(data)));
            }
        }

        let result_format_count = cursor.get_i16() as usize;
        let mut result_formats = Vec::with_capacity(result_format_count);
        for _ in 0..result_format_count {
            result_formats.push(cursor.get_i16());
        }

        Ok(FrontendMessage::Bind {
            portal,
            statement,
            param_formats,
            params,
            result_formats,
        })
    }

    fn parse_execute(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let portal = read_cstring(cursor)?;
        let max_rows = cursor.get_i32();
        Ok(FrontendMessage::Execute { portal, max_rows })
    }

    fn parse_describe(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let target_type = cursor.get_u8();
        let target = match target_type {
            b'S' => DescribeTarget::Statement,
            b'P' => DescribeTarget::Portal,
            _ => {
                return Err(ProtocolError::PostgresError(format!(
                    "Invalid describe target: {}",
                    target_type as char
                )))
            }
        };
        let name = read_cstring(cursor)?;
        Ok(FrontendMessage::Describe { target, name })
    }

    fn parse_close(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let target_type = cursor.get_u8();
        let target = match target_type {
            b'S' => CloseTarget::Statement,
            b'P' => CloseTarget::Portal,
            _ => {
                return Err(ProtocolError::PostgresError(format!(
                    "Invalid close target: {}",
                    target_type as char
                )))
            }
        };
        let name = read_cstring(cursor)?;
        Ok(FrontendMessage::Close { target, name })
    }

    fn parse_password(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let password = read_cstring(cursor)?;
        Ok(FrontendMessage::Password { password })
    }
}

impl BackendMessage {
    /// Encode a backend message to bytes
    pub fn encode(&self, buf: &mut BytesMut) {
        match self {
            BackendMessage::Authentication(auth) => {
                buf.put_u8(b'R');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder for length

                match auth {
                    AuthenticationResponse::Ok => buf.put_i32(0),
                    AuthenticationResponse::CleartextPassword => buf.put_i32(3),
                    AuthenticationResponse::MD5Password { salt } => {
                        buf.put_i32(5);
                        buf.put_slice(salt);
                    }
                    _ => buf.put_i32(0), // TODO: Implement other auth types
                }

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::ParameterStatus { name, value } => {
                buf.put_u8(b'S');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder

                write_cstring(buf, name);
                write_cstring(buf, value);

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::ReadyForQuery { status } => {
                buf.put_u8(b'Z');
                buf.put_i32(5); // Length
                buf.put_u8(*status as u8);
            }
            BackendMessage::RowDescription { fields } => {
                buf.put_u8(b'T');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder

                buf.put_i16(fields.len() as i16);
                for field in fields {
                    write_cstring(buf, &field.name);
                    buf.put_i32(field.table_oid);
                    buf.put_i16(field.column_id);
                    buf.put_i32(field.type_oid);
                    buf.put_i16(field.type_size);
                    buf.put_i32(field.type_modifier);
                    buf.put_i16(field.format);
                }

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::DataRow { values } => {
                buf.put_u8(b'D');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder

                buf.put_i16(values.len() as i16);
                for value in values {
                    match value {
                        None => buf.put_i32(-1),
                        Some(data) => {
                            buf.put_i32(data.len() as i32);
                            buf.put_slice(data);
                        }
                    }
                }

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::CommandComplete { tag } => {
                buf.put_u8(b'C');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder

                write_cstring(buf, tag);

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::EmptyQueryResponse => {
                buf.put_u8(b'I');
                buf.put_i32(4); // Length
            }
            BackendMessage::ErrorResponse { fields } | BackendMessage::NoticeResponse { fields } => {
                let msg_type = if matches!(self, BackendMessage::ErrorResponse { .. }) {
                    b'E'
                } else {
                    b'N'
                };
                buf.put_u8(msg_type);
                let pos = buf.len();
                buf.put_i32(0); // Placeholder

                for (field_type, value) in fields {
                    buf.put_u8(*field_type);
                    write_cstring(buf, value);
                }
                buf.put_u8(0); // Terminator

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::ParseComplete => {
                buf.put_u8(b'1');
                buf.put_i32(4);
            }
            BackendMessage::BindComplete => {
                buf.put_u8(b'2');
                buf.put_i32(4);
            }
            BackendMessage::CloseComplete => {
                buf.put_u8(b'3');
                buf.put_i32(4);
            }
            BackendMessage::NoData => {
                buf.put_u8(b'n');
                buf.put_i32(4);
            }
            BackendMessage::BackendKeyData {
                process_id,
                secret_key,
            } => {
                buf.put_u8(b'K');
                buf.put_i32(12);
                buf.put_i32(*process_id);
                buf.put_i32(*secret_key);
            }
            BackendMessage::ParameterDescription { param_types } => {
                buf.put_u8(b't');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder

                buf.put_i16(param_types.len() as i16);
                for oid in param_types {
                    buf.put_i32(*oid);
                }

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
        }
    }
}

/// Read a null-terminated string from cursor
fn read_cstring(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<String> {
    let start = cursor.position() as usize;
    let buf = cursor.get_ref();
    
    let end = buf[start..]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| ProtocolError::PostgresError("Unterminated string".to_string()))?;
    
    let s = String::from_utf8(buf[start..start + end].to_vec())
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid UTF-8: {}", e)))?;
    
    cursor.set_position((start + end + 1) as u64);
    Ok(s)
}

/// Write a null-terminated string to buffer
fn write_cstring(buf: &mut BytesMut, s: &str) {
    buf.put_slice(s.as_bytes());
    buf.put_u8(0);
}

/// PostgreSQL type OIDs (commonly used types)
#[allow(dead_code)]
pub mod type_oids {
    pub const BOOL: i32 = 16;
    pub const BYTEA: i32 = 17;
    pub const INT8: i32 = 20;
    pub const INT2: i32 = 21;
    pub const INT4: i32 = 23;
    pub const TEXT: i32 = 25;
    pub const JSON: i32 = 114;
    pub const JSONB: i32 = 3802;
    pub const VARCHAR: i32 = 1043;
    pub const TIMESTAMP: i32 = 1114;
    pub const TIMESTAMPTZ: i32 = 1184;
    pub const UUID: i32 = 2950;
}
