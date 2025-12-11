//! PostgreSQL wire protocol message types
//!
//! This module implements all message types used in the PostgreSQL wire protocol v3.
//!
//! ## References
//! - PostgreSQL Protocol Spec: `specifications/protocols/postgresql18-reference-rust.md`
//! - ANTLR4 Grammar: <https://github.com/TuringWorks/grammars-v4/tree/master/postgresql>
//! - Official Docs: <https://www.postgresql.org/docs/current/protocol-message-formats.html>

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::io::Cursor;

use crate::protocols::error::{ProtocolError, ProtocolResult};

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
    /// PostgreSQL 18 (protocol 3.2): Protocol version negotiation
    NegotiateProtocolVersion = b'v' as isize,
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
    Execute { portal: String, max_rows: i32 },
    /// Describe prepared statement or portal
    Describe {
        target: DescribeTarget,
        name: String,
    },
    /// Close prepared statement or portal
    Close { target: CloseTarget, name: String },
    /// Flush output
    Flush,
    /// Sync (end of extended query)
    Sync,
    /// Terminate connection
    Terminate,
    /// Password message
    Password { password: String },
    /// SASL Initial Response
    SASLInitialResponse {
        mechanism: String,
        data: Option<Bytes>,
    },
    /// SASL Response
    SASLResponse { data: Bytes },
    CopyData { data: Bytes },
    CopyDone,
    CopyFail { message: String },
    /// SSL request
    SSLRequest,
    /// Function call (older protocol, but part of standard)
    FunctionCall {
        oid: i32,
        args: Vec<Option<Bytes>>,
    },
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
    /// PostgreSQL 18 (protocol 3.2): supports variable-length keys (4-256 bytes)
    /// For backward compatibility with protocol 3.0, use 4-byte keys by default
    BackendKeyData {
        process_id: i32,
        secret_key: Vec<u8>,
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
    /// PostgreSQL 18 (protocol 3.2): Protocol version negotiation
    /// Sent when client requests unsupported protocol version or options
    NegotiateProtocolVersion {
        /// Newest minor protocol version supported by server
        newest_minor_version: i32,
        /// List of protocol options not recognized
        unrecognized_options: Vec<String>,
    },
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
    /// Notification response
    NotificationResponse {
        process_id: i32,
        channel: String,
        payload: String,
    },
    /// Portal suspended
    PortalSuspended,
    /// Function call response
    FunctionCallResponse {
        val: Option<Bytes>,
    },
    /// Copy In/Out/Data/Done/Fail messages
    CopyInResponse {
        format: i8, // 0=text, 1=binary
        column_formats: Vec<i16>,
    },
    CopyOutResponse {
        format: i8,
        column_formats: Vec<i16>,
    },
    CopyData {
        data: Bytes,
    },
    CopyDone,
    CopyFail {
        message: String,
    },
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
    Certificate,
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
            b'd' => Self::parse_copy_data(&mut cursor)?,
            b'c' => Self::parse_copy_done(&mut cursor)?,
            b'f' => Self::parse_copy_fail(&mut cursor)?,
            b'F' => Self::parse_function_call(&mut cursor)?,
            b'p' => Self::parse_sasl_or_password(&mut cursor)?,
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
            // Return SSL request message
            return Ok(Some(FrontendMessage::SSLRequest));
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

    /// Parse function call
    fn parse_function_call(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let oid = cursor.get_i32();
        let num_args = cursor.get_i16();
        
        let mut args = Vec::with_capacity(num_args as usize);
        for _ in 0..num_args {
             let arg_len = cursor.get_i32();
             if arg_len == -1 {
                 args.push(None);
             } else {
                  let mut arg_data = vec![0u8; arg_len as usize];
                  if cursor.remaining() < arg_len as usize {
                      return Err(ProtocolError::PostgresError("Unexpected EOF in FunctionCall args".to_string()));
                  }
                  cursor.copy_to_slice(&mut arg_data);
                 args.push(Some(Bytes::from(arg_data)));
             }
        }
        
        Ok(FrontendMessage::FunctionCall { oid, args })
    }

    /// Parse password or SASL response
    fn parse_sasl_or_password(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let start_pos = cursor.position();
        let len = cursor.get_ref().len(); // Get the total length of the message data

        // Check if it starts with a null-terminated string
        if let Ok(s) = read_cstring(cursor) {
            let after_string_pos = cursor.position();

            // Case 1: PasswordMessage (String consumes entire message)
            if after_string_pos == len as u64 {
                return Ok(FrontendMessage::Password { password: s });
            }

            // Case 2: SASLInitialResponse (String + Int32 + Data)
            if (len as u64 - after_string_pos) >= 4 {
                let data_len = cursor.get_i32();
                if data_len == -1 {
                    return Ok(FrontendMessage::SASLInitialResponse {
                        mechanism: s,
                        data: None,
                    });
                }
                if data_len >= 0 {
                    let remaining = len as u64 - cursor.position();
                    if remaining == data_len as u64 {
                        let mut data = vec![0u8; data_len as usize];
                        cursor.copy_to_slice(&mut data);
                        return Ok(FrontendMessage::SASLInitialResponse {
                            mechanism: s,
                            data: Some(Bytes::from(data)),
                        });
                    }
                }
            }
        }

        // Case 3: SASLResponse (Raw bytes) - or fallback
        cursor.set_position(start_pos);
        let mut data = vec![0u8; len];
        cursor.copy_to_slice(&mut data);
        Ok(FrontendMessage::SASLResponse {
            data: Bytes::from(data),
        })
    }


    fn parse_copy_data(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let len = cursor.get_ref().len() as u64 - cursor.position();
        let mut data = vec![0u8; len as usize];
        if cursor.remaining() < len as usize {
            return Err(ProtocolError::PostgresError("Unexpected EOF in CopyData".to_string()));
        }
        cursor.copy_to_slice(&mut data);
        Ok(FrontendMessage::CopyData {
            data: Bytes::from(data),
        })
    }

    fn parse_copy_done(_cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        Ok(FrontendMessage::CopyDone)
    }

    fn parse_copy_fail(cursor: &mut Cursor<&[u8]>) -> ProtocolResult<Self> {
        let message = read_cstring(cursor)?;
        Ok(FrontendMessage::CopyFail { message })
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
                    AuthenticationResponse::SASL { mechanisms } => {
                        buf.put_i32(10);
                        for mech in mechanisms {
                            write_cstring(buf, mech);
                        }
                        buf.put_u8(0); // Terminator for list of mechanisms
                    }
                    AuthenticationResponse::SASLContinue { data } => {
                        buf.put_i32(11);
                        buf.put_slice(data);
                    }
                    AuthenticationResponse::SASLFinal { data } => {
                        buf.put_i32(12);
                        buf.put_slice(data);
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
            BackendMessage::CommandComplete { tag } => {
                buf.put_u8(b'C');
                let tag_bytes = tag.as_bytes();
                buf.put_i32(4 + tag_bytes.len() as i32 + 1);
                write_cstring(buf, tag);
            }
            BackendMessage::EmptyQueryResponse => {
                buf.put_u8(b'I');
                buf.put_i32(4); // Length
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
            BackendMessage::ErrorResponse { fields }
            | BackendMessage::NoticeResponse { fields } => {
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
                // PostgreSQL 18 (protocol 3.2): Variable-length cancel keys
                // Message length = 4 (length field) + 4 (process_id) + key_length
                let msg_len = 4 + 4 + secret_key.len() as i32;
                buf.put_i32(msg_len);
                buf.put_i32(*process_id);
                buf.put_slice(secret_key);
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
            BackendMessage::NegotiateProtocolVersion {
                newest_minor_version,
                unrecognized_options,
            } => {
                // PostgreSQL 18 (protocol 3.2): NegotiateProtocolVersion message
                buf.put_u8(b'v');
                let pos = buf.len();
                buf.put_i32(0); // Placeholder for length

                // Newest minor protocol version this server supports
                buf.put_i32(*newest_minor_version);

                // Number of protocol options not recognized
                buf.put_i32(unrecognized_options.len() as i32);

                // List of unrecognized option names (null-terminated strings)
                for option in unrecognized_options {
                    write_cstring(buf, option);
                }

                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::NotificationResponse {
                process_id,
                channel,
                payload,
            } => {
                buf.put_u8(b'A');
                let pos = buf.len();
                buf.put_i32(0);
                buf.put_i32(*process_id);
                write_cstring(buf, channel);
                write_cstring(buf, payload);
                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::PortalSuspended => {
                buf.put_u8(b's');
                buf.put_i32(4);
            }
            BackendMessage::FunctionCallResponse { val } => {
                buf.put_u8(b'V');
                let pos = buf.len();
                buf.put_i32(0);
                if let Some(data) = val {
                    buf.put_i32(data.len() as i32);
                    buf.put_slice(data);
                } else {
                    buf.put_i32(-1);
                }
                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::CopyInResponse {
                format,
                column_formats,
            } => {
                buf.put_u8(b'G');
                let pos = buf.len();
                buf.put_i32(0);
                buf.put_i8(*format);
                buf.put_i16(column_formats.len() as i16);
                for fmt in column_formats {
                    buf.put_i16(*fmt);
                }
                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::CopyOutResponse {
                format,
                column_formats,
            } => {
                buf.put_u8(b'H');
                let pos = buf.len();
                buf.put_i32(0);
                buf.put_i8(*format);
                buf.put_i16(column_formats.len() as i16);
                for fmt in column_formats {
                    buf.put_i16(*fmt);
                }
                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::CopyData { data } => {
                buf.put_u8(b'd');
                let pos = buf.len();
                buf.put_i32(0);
                buf.put_slice(data);
                let len = buf.len() - pos;
                buf[pos..pos + 4].copy_from_slice(&(len as i32).to_be_bytes());
            }
            BackendMessage::CopyDone => {
                buf.put_u8(b'c');
                buf.put_i32(4);
            }
            BackendMessage::CopyFail { message } => {
                buf.put_u8(b'f');
                let pos = buf.len();
                buf.put_i32(0);
                write_cstring(buf, message);
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
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid UTF-8: {e}")))?;

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
    pub const FLOAT4: i32 = 700;
    pub const FLOAT8: i32 = 701;
    pub const JSON: i32 = 114;
    pub const JSONB: i32 = 3802;
    pub const VARCHAR: i32 = 1043;
    pub const TIMESTAMP: i32 = 1114;
    pub const TIMESTAMPTZ: i32 = 1184;
    pub const UUID: i32 = 2950;
    pub const JSONPATH: i32 = 4072;
    pub const INT4MULTIRANGE: i32 = 4451;
    pub const NUMMULTIRANGE: i32 = 4532;
    pub const TSMULTIRANGE: i32 = 4533;
    pub const TSTZMULTIRANGE: i32 = 4534;
    pub const DATEMULTIRANGE: i32 = 4535;
    pub const INT8MULTIRANGE: i32 = 4536;

    // pgvector extension types
    // Note: In real pgvector, these OIDs are assigned dynamically
    // We use high numbers to avoid conflicts with standard types
    pub const VECTOR: i32 = 16385; // vector type
    pub const HALFVEC: i32 = 16386; // halfvec type (half precision)
    pub const SPARSEVEC: i32 = 16387; // sparsevec type
}
