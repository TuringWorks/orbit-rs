//! OrbitWire protocol messages
//!
//! High-level message types that map to OrbitWire frames

use super::frame::{Frame, FrameFlags, MessageType};
use super::values::WireValue;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Hello message - initial handshake
#[derive(Debug, Clone)]
pub struct HelloMessage {
    pub protocol_version: u8,
    pub client_name: String,
    pub client_version: String,
    pub capabilities: Vec<String>,
}

impl HelloMessage {
    pub fn new(client_name: impl Into<String>, client_version: impl Into<String>) -> Self {
        Self {
            protocol_version: super::PROTOCOL_VERSION,
            client_name: client_name.into(),
            client_version: client_version.into(),
            capabilities: Vec::new(),
        }
    }

    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_u8(self.protocol_version);
        encode_string(&mut buf, &self.client_name);
        encode_string(&mut buf, &self.client_version);
        buf.put_u32(self.capabilities.len() as u32);
        for cap in &self.capabilities {
            encode_string(&mut buf, cap);
        }
        buf.freeze()
    }

    pub fn decode(data: &mut Bytes) -> Result<Self, MessageError> {
        ensure_remaining(data, 1)?;
        let protocol_version = data.get_u8();
        let client_name = decode_string(data)?;
        let client_version = decode_string(data)?;
        ensure_remaining(data, 4)?;
        let num_caps = data.get_u32() as usize;
        let mut capabilities = Vec::with_capacity(num_caps);
        for _ in 0..num_caps {
            capabilities.push(decode_string(data)?);
        }
        Ok(Self {
            protocol_version,
            client_name,
            client_version,
            capabilities,
        })
    }

    pub fn to_frame(&self) -> Frame {
        Frame::connection_frame(MessageType::Hello, self.encode())
    }
}

/// Hello acknowledgment
#[derive(Debug, Clone)]
pub struct HelloAckMessage {
    pub protocol_version: u8,
    pub server_name: String,
    pub server_version: String,
    pub session_id: String,
    pub capabilities: Vec<String>,
}

impl HelloAckMessage {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            protocol_version: super::PROTOCOL_VERSION,
            server_name: "Orbit-RS".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            session_id: session_id.into(),
            capabilities: Vec::new(),
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_u8(self.protocol_version);
        encode_string(&mut buf, &self.server_name);
        encode_string(&mut buf, &self.server_version);
        encode_string(&mut buf, &self.session_id);
        buf.put_u32(self.capabilities.len() as u32);
        for cap in &self.capabilities {
            encode_string(&mut buf, cap);
        }
        buf.freeze()
    }

    pub fn to_frame(&self) -> Frame {
        Frame::connection_frame(MessageType::HelloAck, self.encode())
    }
}

/// Authentication message
#[derive(Debug, Clone)]
pub struct AuthenticateMessage {
    pub method: AuthMethod,
    pub credentials: Bytes,
}

/// Authentication methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    None,
    Password,
    Token,
    Certificate,
}

impl AuthenticateMessage {
    pub fn password(username: &str, password: &str) -> Self {
        let mut creds = BytesMut::new();
        encode_string(&mut creds, username);
        encode_string(&mut creds, password);
        Self {
            method: AuthMethod::Password,
            credentials: creds.freeze(),
        }
    }

    pub fn token(token: &str) -> Self {
        let mut creds = BytesMut::new();
        encode_string(&mut creds, token);
        Self {
            method: AuthMethod::Token,
            credentials: creds.freeze(),
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(self.credentials.len() + 5);
        buf.put_u8(match self.method {
            AuthMethod::None => 0,
            AuthMethod::Password => 1,
            AuthMethod::Token => 2,
            AuthMethod::Certificate => 3,
        });
        buf.put_u32(self.credentials.len() as u32);
        buf.put_slice(&self.credentials);
        buf.freeze()
    }

    pub fn to_frame(&self) -> Frame {
        Frame::connection_frame(MessageType::Authenticate, self.encode())
    }
}

/// Query message
#[derive(Debug, Clone)]
pub struct QueryMessage {
    pub query: String,
    pub parameters: Option<Vec<WireValue>>,
    pub options: QueryOptions,
}

/// Query execution options
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    pub timeout_ms: Option<u32>,
    pub max_rows: Option<u64>,
    pub fetch_size: Option<u32>,
    pub transaction_id: Option<[u8; 16]>,
}

impl QueryMessage {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            parameters: None,
            options: QueryOptions::default(),
        }
    }

    pub fn with_parameters(mut self, params: Vec<WireValue>) -> Self {
        self.parameters = Some(params);
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.options.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        encode_string(&mut buf, &self.query);

        // Parameters
        match &self.parameters {
            Some(params) => {
                buf.put_u8(1);
                buf.put_u32(params.len() as u32);
                for param in params {
                    param.encode_to(&mut buf);
                }
            }
            None => {
                buf.put_u8(0);
            }
        }

        // Options
        buf.put_u8(encode_option_flags(&self.options));
        if let Some(timeout) = self.options.timeout_ms {
            buf.put_u32(timeout);
        }
        if let Some(max_rows) = self.options.max_rows {
            buf.put_u64(max_rows);
        }
        if let Some(fetch_size) = self.options.fetch_size {
            buf.put_u32(fetch_size);
        }
        if let Some(tx_id) = &self.options.transaction_id {
            buf.put_slice(tx_id);
        }

        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::Query, self.encode())
    }
}

/// Row description message (schema)
#[derive(Debug, Clone)]
pub struct RowDescriptionMessage {
    pub columns: Vec<ColumnDescription>,
}

/// Column description
#[derive(Debug, Clone)]
pub struct ColumnDescription {
    pub name: String,
    pub type_tag: u8,
    pub nullable: bool,
    pub precision: Option<u16>,
    pub scale: Option<u16>,
}

impl RowDescriptionMessage {
    pub fn new(columns: Vec<ColumnDescription>) -> Self {
        Self { columns }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_u16(self.columns.len() as u16);
        for col in &self.columns {
            encode_string(&mut buf, &col.name);
            buf.put_u8(col.type_tag);
            buf.put_u8(if col.nullable { 1 } else { 0 });
            match (col.precision, col.scale) {
                (Some(p), Some(s)) => {
                    buf.put_u8(1);
                    buf.put_u16(p);
                    buf.put_u16(s);
                }
                _ => {
                    buf.put_u8(0);
                }
            }
        }
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::RowDescription, self.encode())
    }
}

/// Row data message
#[derive(Debug, Clone)]
pub struct RowDataMessage {
    pub values: Vec<WireValue>,
}

impl RowDataMessage {
    pub fn new(values: Vec<WireValue>) -> Self {
        Self { values }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_u16(self.values.len() as u16);
        for value in &self.values {
            value.encode_to(&mut buf);
        }
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::RowData, self.encode())
    }
}

/// Command complete message
#[derive(Debug, Clone)]
pub struct CommandCompleteMessage {
    pub command: String,
    pub affected_rows: i64,
}

impl CommandCompleteMessage {
    pub fn new(command: impl Into<String>, affected_rows: i64) -> Self {
        Self {
            command: command.into(),
            affected_rows,
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(64);
        encode_string(&mut buf, &self.command);
        buf.put_i64(self.affected_rows);
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        let flags = FrameFlags::new().with_end_stream();
        Frame::with_flags(
            flags,
            stream_id,
            MessageType::CommandComplete,
            self.encode(),
        )
    }
}

/// LIVE subscribe message
#[derive(Debug, Clone)]
pub struct LiveSubscribeMessage {
    pub query: String,
    pub diff_mode: bool,
}

impl LiveSubscribeMessage {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            diff_mode: false,
        }
    }

    pub fn with_diff_mode(mut self) -> Self {
        self.diff_mode = true;
        self
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        encode_string(&mut buf, &self.query);
        buf.put_u8(if self.diff_mode { 1 } else { 0 });
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::LiveSubscribe, self.encode())
    }
}

/// LIVE subscribe OK message
#[derive(Debug, Clone)]
pub struct LiveSubscribeOkMessage {
    pub subscription_id: [u8; 16],
    pub schema: RowDescriptionMessage,
}

impl LiveSubscribeOkMessage {
    pub fn new(subscription_id: [u8; 16], schema: RowDescriptionMessage) -> Self {
        Self {
            subscription_id,
            schema,
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_slice(&self.subscription_id);
        buf.put_slice(&self.schema.encode());
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::LiveSubscribeOk, self.encode())
    }
}

/// LIVE event message
#[derive(Debug, Clone)]
pub struct LiveEventMessage {
    pub subscription_id: [u8; 16],
    pub event_id: u64,
    pub event_type: LiveEventType,
    pub data: Vec<WireValue>,
}

/// LIVE event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveEventType {
    Create,
    Update,
    Delete,
}

impl LiveEventMessage {
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        buf.put_slice(&self.subscription_id);
        buf.put_u64(self.event_id);
        buf.put_u8(match self.event_type {
            LiveEventType::Create => 1,
            LiveEventType::Update => 2,
            LiveEventType::Delete => 3,
        });
        buf.put_u32(self.data.len() as u32);
        for value in &self.data {
            value.encode_to(&mut buf);
        }
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::LiveEvent, self.encode())
    }
}

/// Transaction begin message
#[derive(Debug, Clone)]
pub struct BeginMessage {
    pub isolation_level: IsolationLevel,
    pub read_only: bool,
}

/// Isolation levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    ReadUncommitted,
    #[default]
    ReadCommitted,
    RepeatableRead,
    Serializable,
    Snapshot,
}

impl BeginMessage {
    pub fn new() -> Self {
        Self {
            isolation_level: IsolationLevel::default(),
            read_only: false,
        }
    }

    pub fn with_isolation(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(2);
        buf.put_u8(match self.isolation_level {
            IsolationLevel::ReadUncommitted => 1,
            IsolationLevel::ReadCommitted => 2,
            IsolationLevel::RepeatableRead => 3,
            IsolationLevel::Serializable => 4,
            IsolationLevel::Snapshot => 5,
        });
        buf.put_u8(if self.read_only { 1 } else { 0 });
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::Begin, self.encode())
    }
}

impl Default for BeginMessage {
    fn default() -> Self {
        Self::new()
    }
}

/// Transaction begin OK message
#[derive(Debug, Clone)]
pub struct BeginOkMessage {
    pub transaction_id: [u8; 16],
}

impl BeginOkMessage {
    pub fn new(transaction_id: [u8; 16]) -> Self {
        Self { transaction_id }
    }

    pub fn encode(&self) -> Bytes {
        Bytes::copy_from_slice(&self.transaction_id)
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        Frame::new(stream_id, MessageType::BeginOk, self.encode())
    }
}

/// Error message
#[derive(Debug, Clone)]
pub struct ErrorMessage {
    pub code: String,
    pub message: String,
    pub severity: ErrorSeverity,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub position: Option<u32>,
}

/// Error severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Error,
    Fatal,
    Panic,
}

impl ErrorMessage {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: ErrorSeverity::Error,
            detail: None,
            hint: None,
            position: None,
        }
    }

    pub fn fatal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: ErrorSeverity::Fatal,
            detail: None,
            hint: None,
            position: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_position(mut self, position: u32) -> Self {
        self.position = Some(position);
        self
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        encode_string(&mut buf, &self.code);
        encode_string(&mut buf, &self.message);
        buf.put_u8(match self.severity {
            ErrorSeverity::Error => 1,
            ErrorSeverity::Fatal => 2,
            ErrorSeverity::Panic => 3,
        });
        encode_optional_string(&mut buf, self.detail.as_deref());
        encode_optional_string(&mut buf, self.hint.as_deref());
        match self.position {
            Some(pos) => {
                buf.put_u8(1);
                buf.put_u32(pos);
            }
            None => {
                buf.put_u8(0);
            }
        }
        buf.freeze()
    }

    pub fn to_frame(&self, stream_id: u32) -> Frame {
        let flags = FrameFlags::new().with_end_stream();
        Frame::with_flags(flags, stream_id, MessageType::Error, self.encode())
    }
}

/// Message encoding/decoding errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageError {
    InsufficientData,
    InvalidUtf8,
    InvalidMessage,
}

impl std::fmt::Display for MessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageError::InsufficientData => write!(f, "Insufficient data"),
            MessageError::InvalidUtf8 => write!(f, "Invalid UTF-8"),
            MessageError::InvalidMessage => write!(f, "Invalid message"),
        }
    }
}

impl std::error::Error for MessageError {}

// Helper functions

fn encode_string(buf: &mut BytesMut, s: &str) {
    let bytes = s.as_bytes();
    buf.put_u32(bytes.len() as u32);
    buf.put_slice(bytes);
}

fn encode_optional_string(buf: &mut BytesMut, s: Option<&str>) {
    match s {
        Some(s) => {
            buf.put_u8(1);
            encode_string(buf, s);
        }
        None => {
            buf.put_u8(0);
        }
    }
}

fn decode_string(data: &mut Bytes) -> Result<String, MessageError> {
    ensure_remaining(data, 4)?;
    let len = data.get_u32() as usize;
    ensure_remaining(data, len)?;
    String::from_utf8(data.copy_to_bytes(len).to_vec()).map_err(|_| MessageError::InvalidUtf8)
}

fn ensure_remaining(data: &Bytes, n: usize) -> Result<(), MessageError> {
    if data.remaining() < n {
        Err(MessageError::InsufficientData)
    } else {
        Ok(())
    }
}

fn encode_option_flags(opts: &QueryOptions) -> u8 {
    let mut flags = 0u8;
    if opts.timeout_ms.is_some() {
        flags |= 0x01;
    }
    if opts.max_rows.is_some() {
        flags |= 0x02;
    }
    if opts.fetch_size.is_some() {
        flags |= 0x04;
    }
    if opts.transaction_id.is_some() {
        flags |= 0x08;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_message() {
        let msg = HelloMessage::new("orbit-cli", "1.0.0")
            .with_capability("COMPRESSION")
            .with_capability("LIVE_QUERIES");

        let encoded = msg.encode();
        let mut data = encoded;
        let decoded = HelloMessage::decode(&mut data).unwrap();

        assert_eq!(decoded.client_name, "orbit-cli");
        assert_eq!(decoded.capabilities.len(), 2);
    }

    #[test]
    fn test_query_message() {
        let msg = QueryMessage::new("SELECT * FROM users WHERE id = ?")
            .with_parameters(vec![WireValue::Int64(42)])
            .with_timeout(5000);

        let frame = msg.to_frame(1);
        assert_eq!(frame.stream_id, 1);
        assert_eq!(frame.message_type, MessageType::Query);
    }

    #[test]
    fn test_error_message() {
        let msg = ErrorMessage::new("42P01", "relation 'foo' does not exist")
            .with_detail("Table 'foo' was not found in schema 'public'")
            .with_hint("Check spelling or create the table first");

        let frame = msg.to_frame(1);
        assert!(frame.is_end_stream());
        assert_eq!(frame.message_type, MessageType::Error);
    }

    #[test]
    fn test_row_data_message() {
        let msg = RowDataMessage::new(vec![
            WireValue::Int64(1),
            WireValue::String("Alice".to_string()),
            WireValue::Float64(95.5),
        ]);

        let frame = msg.to_frame(1);
        assert_eq!(frame.message_type, MessageType::RowData);
    }
}
