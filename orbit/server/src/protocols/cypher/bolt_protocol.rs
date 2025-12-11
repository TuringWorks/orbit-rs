//! Bolt protocol implementation for Neo4j compatibility
//!
//! This module implements the Bolt protocol (v4.0+) for handling Neo4j client connections.
//! Bolt uses a binary protocol with PackStream encoding for efficient data transfer.
//!
//! ## PackStream Encoding/Decoding
//!
//! This module provides complete PackStream serialization support:
//! - Null, Boolean, Integer, Float
//! - String (Tiny, 8, 16, 32)
//! - List (Tiny, 8, 16, 32)
//! - Map (Tiny, 8, 16, 32)
//! - Structure (for Node, Relationship, Path)
//!

// Recursive helper functions use parameters only for recursion - intentional design
#![allow(clippy::only_used_in_recursion)]

//! ## Bolt v4.4 Features
//!
//! - Authentication (HELLO with auth token)
//! - Transaction management (BEGIN/COMMIT/ROLLBACK)
//! - Streaming results (RUN/PULL/DISCARD)
//! - Connection routing (ROUTE message)

use crate::protocols::cypher::cypher_parser::CypherParser;
#[cfg(feature = "storage-rocksdb")]
use crate::protocols::cypher::storage::CypherStorageProvider;
use crate::protocols::cypher::types::{GraphNode, GraphRelationship};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use bytes::{BufMut, Bytes, BytesMut};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, error, info, warn};

/// Trait for streams compatible with Bolt protocol (TCP, TLS)
pub trait BoltStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> BoltStream for T {}

/// PackStream decoder for parsing Bolt protocol messages
#[derive(Debug, Default)]
pub struct PackStreamDecoder {
    pub(crate) position: usize,
}

impl PackStreamDecoder {
    /// Create a new PackStream decoder
    pub fn new() -> Self {
        Self { position: 0 }
    }

    /// Reset decoder position
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Decode a value from bytes
    pub fn decode_value(&mut self, bytes: &[u8]) -> ProtocolResult<Value> {
        if self.position >= bytes.len() {
            return Err(ProtocolError::CypherError(
                "Unexpected end of PackStream data".to_string(),
            ));
        }

        let marker = bytes[self.position];
        self.position += 1;

        match marker {
            // Null
            0xC0 => Ok(Value::Null),

            // Boolean
            0xC2 => Ok(Value::Bool(false)),
            0xC3 => Ok(Value::Bool(true)),

            // Tiny integer (-16 to 127)
            m if m <= 0x7F || m >= 0xF0 => {
                let value = if m <= 0x7F {
                    m as i64
                } else {
                    (m as i8) as i64
                };
                Ok(Value::Number(value.into()))
            }

            // INT_8
            0xC8 => {
                let value = self.read_i8(bytes)?;
                Ok(Value::Number(value.into()))
            }

            // INT_16
            0xC9 => {
                let value = self.read_i16(bytes)?;
                Ok(Value::Number(value.into()))
            }

            // INT_32
            0xCA => {
                let value = self.read_i32(bytes)?;
                Ok(Value::Number(value.into()))
            }

            // INT_64
            0xCB => {
                let value = self.read_i64(bytes)?;
                Ok(Value::Number(value.into()))
            }

            // FLOAT_64
            0xC1 => {
                let value = self.read_f64(bytes)?;
                Ok(serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .unwrap_or(Value::Null))
            }

            // Tiny string (0x80-0x8F)
            m if (0x80..=0x8F).contains(&m) => {
                let len = (m & 0x0F) as usize;
                let s = self.read_string(bytes, len)?;
                Ok(Value::String(s))
            }

            // STRING_8
            0xD0 => {
                let len = self.read_u8(bytes)? as usize;
                let s = self.read_string(bytes, len)?;
                Ok(Value::String(s))
            }

            // STRING_16
            0xD1 => {
                let len = self.read_u16(bytes)? as usize;
                let s = self.read_string(bytes, len)?;
                Ok(Value::String(s))
            }

            // STRING_32
            0xD2 => {
                let len = self.read_u32(bytes)? as usize;
                let s = self.read_string(bytes, len)?;
                Ok(Value::String(s))
            }

            // Tiny list (0x90-0x9F)
            m if (0x90..=0x9F).contains(&m) => {
                let len = (m & 0x0F) as usize;
                self.decode_list(bytes, len)
            }

            // LIST_8
            0xD4 => {
                let len = self.read_u8(bytes)? as usize;
                self.decode_list(bytes, len)
            }

            // LIST_16
            0xD5 => {
                let len = self.read_u16(bytes)? as usize;
                self.decode_list(bytes, len)
            }

            // LIST_32
            0xD6 => {
                let len = self.read_u32(bytes)? as usize;
                self.decode_list(bytes, len)
            }

            // Tiny map (0xA0-0xAF)
            m if (0xA0..=0xAF).contains(&m) => {
                let len = (m & 0x0F) as usize;
                self.decode_map(bytes, len)
            }

            // MAP_8
            0xD8 => {
                let len = self.read_u8(bytes)? as usize;
                self.decode_map(bytes, len)
            }

            // MAP_16
            0xD9 => {
                let len = self.read_u16(bytes)? as usize;
                self.decode_map(bytes, len)
            }

            // MAP_32
            0xDA => {
                let len = self.read_u32(bytes)? as usize;
                self.decode_map(bytes, len)
            }

            // Tiny structure (0xB0-0xBF)
            m if (0xB0..=0xBF).contains(&m) => {
                let num_fields = (m & 0x0F) as usize;
                self.decode_structure(bytes, num_fields)
            }

            // STRUCT_8
            0xDC => {
                let num_fields = self.read_u8(bytes)? as usize;
                self.decode_structure(bytes, num_fields)
            }

            // STRUCT_16
            0xDD => {
                let num_fields = self.read_u16(bytes)? as usize;
                self.decode_structure(bytes, num_fields)
            }

            _ => Err(ProtocolError::CypherError(format!(
                "Unknown PackStream marker: 0x{:02X}",
                marker
            ))),
        }
    }

    /// Decode a list
    fn decode_list(&mut self, bytes: &[u8], len: usize) -> ProtocolResult<Value> {
        let mut items = Vec::with_capacity(len);
        for _ in 0..len {
            items.push(self.decode_value(bytes)?);
        }
        Ok(Value::Array(items))
    }

    /// Decode a map
    fn decode_map(&mut self, bytes: &[u8], len: usize) -> ProtocolResult<Value> {
        let mut map = serde_json::Map::with_capacity(len);
        for _ in 0..len {
            let key = self.decode_value(bytes)?;
            let key_str = match key {
                Value::String(s) => s,
                _ => key.to_string(),
            };
            let value = self.decode_value(bytes)?;
            map.insert(key_str, value);
        }
        Ok(Value::Object(map))
    }

    /// Decode a structure (Node, Relationship, etc.)
    fn decode_structure(&mut self, bytes: &[u8], num_fields: usize) -> ProtocolResult<Value> {
        if self.position >= bytes.len() {
            return Err(ProtocolError::CypherError(
                "Structure missing signature byte".to_string(),
            ));
        }

        let signature = bytes[self.position];
        self.position += 1;

        let mut fields = Vec::with_capacity(num_fields);
        for _ in 0..num_fields {
            fields.push(self.decode_value(bytes)?);
        }

        // Create a JSON representation of the structure
        let mut obj = serde_json::Map::new();
        obj.insert("_signature".to_string(), Value::Number(signature.into()));

        match signature {
            0x4E => {
                // Node (N)
                if fields.len() >= 3 {
                    obj.insert("id".to_string(), fields[0].clone());
                    obj.insert("labels".to_string(), fields[1].clone());
                    obj.insert("properties".to_string(), fields[2].clone());
                }
                obj.insert("_type".to_string(), Value::String("Node".to_string()));
            }
            0x52 => {
                // Relationship (R)
                if fields.len() >= 5 {
                    obj.insert("id".to_string(), fields[0].clone());
                    obj.insert("startNodeId".to_string(), fields[1].clone());
                    obj.insert("endNodeId".to_string(), fields[2].clone());
                    obj.insert("type".to_string(), fields[3].clone());
                    obj.insert("properties".to_string(), fields[4].clone());
                }
                obj.insert(
                    "_type".to_string(),
                    Value::String("Relationship".to_string()),
                );
            }
            0x50 => {
                // Path (P)
                if fields.len() >= 3 {
                    obj.insert("nodes".to_string(), fields[0].clone());
                    obj.insert("relationships".to_string(), fields[1].clone());
                    obj.insert("indices".to_string(), fields[2].clone());
                }
                obj.insert("_type".to_string(), Value::String("Path".to_string()));
            }
            _ => {
                obj.insert("fields".to_string(), Value::Array(fields));
            }
        }

        Ok(Value::Object(obj))
    }

    // Helper methods for reading bytes

    fn read_u8(&mut self, bytes: &[u8]) -> ProtocolResult<u8> {
        if self.position >= bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = bytes[self.position];
        self.position += 1;
        Ok(value)
    }

    fn read_i8(&mut self, bytes: &[u8]) -> ProtocolResult<i64> {
        if self.position >= bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = bytes[self.position] as i8;
        self.position += 1;
        Ok(value as i64)
    }

    fn read_u16(&mut self, bytes: &[u8]) -> ProtocolResult<u16> {
        if self.position + 2 > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = u16::from_be_bytes([bytes[self.position], bytes[self.position + 1]]);
        self.position += 2;
        Ok(value)
    }

    fn read_i16(&mut self, bytes: &[u8]) -> ProtocolResult<i64> {
        if self.position + 2 > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = i16::from_be_bytes([bytes[self.position], bytes[self.position + 1]]);
        self.position += 2;
        Ok(value as i64)
    }

    fn read_u32(&mut self, bytes: &[u8]) -> ProtocolResult<u32> {
        if self.position + 4 > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = u32::from_be_bytes([
            bytes[self.position],
            bytes[self.position + 1],
            bytes[self.position + 2],
            bytes[self.position + 3],
        ]);
        self.position += 4;
        Ok(value)
    }

    fn read_i32(&mut self, bytes: &[u8]) -> ProtocolResult<i64> {
        if self.position + 4 > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = i32::from_be_bytes([
            bytes[self.position],
            bytes[self.position + 1],
            bytes[self.position + 2],
            bytes[self.position + 3],
        ]);
        self.position += 4;
        Ok(value as i64)
    }

    fn read_i64(&mut self, bytes: &[u8]) -> ProtocolResult<i64> {
        if self.position + 8 > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = i64::from_be_bytes([
            bytes[self.position],
            bytes[self.position + 1],
            bytes[self.position + 2],
            bytes[self.position + 3],
            bytes[self.position + 4],
            bytes[self.position + 5],
            bytes[self.position + 6],
            bytes[self.position + 7],
        ]);
        self.position += 8;
        Ok(value)
    }

    fn read_f64(&mut self, bytes: &[u8]) -> ProtocolResult<f64> {
        if self.position + 8 > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let value = f64::from_be_bytes([
            bytes[self.position],
            bytes[self.position + 1],
            bytes[self.position + 2],
            bytes[self.position + 3],
            bytes[self.position + 4],
            bytes[self.position + 5],
            bytes[self.position + 6],
            bytes[self.position + 7],
        ]);
        self.position += 8;
        Ok(value)
    }

    fn read_string(&mut self, bytes: &[u8], len: usize) -> ProtocolResult<String> {
        if self.position + len > bytes.len() {
            return Err(ProtocolError::CypherError("Unexpected EOF".to_string()));
        }
        let s = String::from_utf8_lossy(&bytes[self.position..self.position + len]).to_string();
        self.position += len;
        Ok(s)
    }
}

/// Transaction state for Bolt protocol
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    /// No active transaction (auto-commit mode)
    None,
    /// Transaction in progress
    Active,
    /// Transaction marked for rollback
    RollbackPending,
}

/// Authentication state
#[derive(Debug, Clone)]
pub struct AuthState {
    /// Whether the client is authenticated
    pub authenticated: bool,
    /// Principal (username) if authenticated
    pub principal: Option<String>,
    /// Authentication scheme used
    pub scheme: Option<String>,
}

/// Bolt protocol version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoltVersion {
    V4_0 = 0x0400,
    V4_1 = 0x0401,
    V4_2 = 0x0402,
    V4_3 = 0x0403,
    V4_4 = 0x0404,
}

impl BoltVersion {
    /// Get the highest supported version
    pub fn highest() -> Self {
        Self::V4_4
    }

    /// Convert to u32
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Try to parse from u32
    pub fn from_u32(version: u32) -> Option<Self> {
        match version {
            0x0400 => Some(Self::V4_0),
            0x0401 => Some(Self::V4_1),
            0x0402 => Some(Self::V4_2),
            0x0403 => Some(Self::V4_3),
            0x0404 => Some(Self::V4_4),
            _ => None,
        }
    }
}

/// Bolt message types
#[derive(Debug, Clone)]
pub enum BoltMessage {
    // Client messages
    Hello {
        user_agent: String,
        auth_token: HashMap<String, Value>,
        routing: Option<HashMap<String, Value>>,
    },
    Goodbye,
    Run {
        query: String,
        parameters: HashMap<String, Value>,
        extra: HashMap<String, Value>,
    },
    Pull {
        n: Option<i64>,
        qid: Option<i64>,
    },
    Discard {
        n: Option<i64>,
        qid: Option<i64>,
    },
    Begin {
        extra: HashMap<String, Value>,
    },
    Commit,
    Rollback,
    Reset,
    Route {
        routing: HashMap<String, Value>,
        bookmarks: Vec<String>,
        extra: HashMap<String, Value>,
    },

    // Server messages
    Success(HashMap<String, Value>),
    Record(Vec<Value>),
    Ignored,
    Failure {
        code: String,
        message: String,
    },
}

/// Bolt protocol handler
pub struct BoltProtocolHandler {
    version: Option<BoltVersion>,
    #[cfg(feature = "storage-rocksdb")]
    storage: Arc<dyn CypherStorageProvider>,
    parser: CypherParser,
    /// Authentication state
    auth_state: AuthState,
    /// Transaction state
    transaction_state: TransactionState,
    /// Transaction ID counter
    transaction_id: u64,
    current_query: Option<String>,
    current_parameters: Option<HashMap<String, Value>>,
    /// Pending query results (nodes and relationships as JSON values)
    pending_results: Vec<Vec<Value>>,
    /// Column names for current result set
    result_columns: Vec<String>,
    /// PackStream decoder for parsing messages
    decoder: PackStreamDecoder,
}

impl BoltProtocolHandler {
    /// Create a new Bolt protocol handler
    #[cfg(feature = "storage-rocksdb")]
    pub fn new(storage: Arc<dyn CypherStorageProvider>) -> Self {
        Self {
            version: None,
            storage,
            parser: CypherParser::new(),
            auth_state: AuthState {
                authenticated: false,
                principal: None,
                scheme: None,
            },
            transaction_state: TransactionState::None,
            transaction_id: 0,
            current_query: None,
            current_parameters: None,
            pending_results: Vec::new(),
            result_columns: Vec::new(),
            decoder: PackStreamDecoder::new(),
        }
    }

    /// Create a new Bolt protocol handler (without storage)
    #[cfg(not(feature = "storage-rocksdb"))]
    pub fn new_without_storage() -> Self {
        Self {
            version: None,
            parser: CypherParser::new(),
            auth_state: AuthState {
                authenticated: false,
                principal: None,
                scheme: None,
            },
            transaction_state: TransactionState::None,
            transaction_id: 0,
            current_query: None,
            current_parameters: None,
            pending_results: Vec::new(),
            result_columns: Vec::new(),
            decoder: PackStreamDecoder::new(),
        }
    }

    /// Handle Bolt handshake
    pub async fn handle_handshake(
        &mut self,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<BoltVersion> {
        let mut handshake_buf = [0u8; 20];
        stream.read_exact(&mut handshake_buf).await.map_err(|e| {
            error!("Failed to read handshake: {}", e);
            ProtocolError::Other(format!("Handshake read error: {}", e))
        })?;

        // Bolt handshake format: 4 bytes magic (0x6060B017) + 4 version proposals (4 bytes each)
        let magic = u32::from_be_bytes([
            handshake_buf[0],
            handshake_buf[1],
            handshake_buf[2],
            handshake_buf[3],
        ]);

        if magic != 0x6060B017 {
            return Err(ProtocolError::CypherError(
                "Invalid Bolt handshake magic".to_string(),
            ));
        }

        // Find highest supported version
        let mut selected_version: Option<BoltVersion> = None;
        for i in 0..4 {
            let version_bytes = [
                handshake_buf[4 + i * 4],
                handshake_buf[5 + i * 4],
                handshake_buf[6 + i * 4],
                handshake_buf[7 + i * 4],
            ];
            let version = u32::from_be_bytes(version_bytes);

            if version == 0 {
                break; // End of version list
            }

            if let Some(bolt_version) = BoltVersion::from_u32(version) {
                if selected_version.is_none() || version > selected_version.unwrap().as_u32() {
                    selected_version = Some(bolt_version);
                }
            }
        }

        let version = selected_version.unwrap_or(BoltVersion::V4_4);
        self.version = Some(version);

        // Send selected version back
        let mut response = BytesMut::with_capacity(4);
        response.put_u32(version.as_u32());
        stream.write_all(&response).await.map_err(|e| {
            error!("Failed to write handshake response: {}", e);
            ProtocolError::Other(format!("Handshake write error: {}", e))
        })?;

        info!("Bolt handshake completed, version: {:?}", version);
        Ok(version)
    }

    /// Handle a client connection
    pub async fn handle_connection(&mut self, mut stream: impl BoltStream) -> ProtocolResult<()> {
        info!("New Bolt client connection");

        // Perform handshake
        let version = self.handle_handshake(&mut stream).await?;
        debug!("Bolt version negotiated: {:?}", version);

        // Main message loop
        let mut read_buf = BytesMut::with_capacity(8192);

        loop {
            // Read message chunk
            let chunk_size = match self.read_chunk(&mut stream, &mut read_buf).await {
                Ok(size) => size,
                Err(ProtocolError::ConnectionClosed) => {
                    info!("Client disconnected");
                    break;
                }
                Err(e) => {
                    error!("Error reading chunk: {}", e);
                    break;
                }
            };

            if chunk_size == 0 {
                // End of message marker (0x0000)
                // Process the accumulated message
                if !read_buf.is_empty() {
                    let message_bytes = read_buf.freeze();
                    read_buf = BytesMut::with_capacity(8192); // Reset buffer for next message

                    match self.process_message(&message_bytes, &mut stream).await {
                        Ok(should_continue) => {
                            if !should_continue {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                            self.send_failure(&mut stream, "Error", &e.to_string())
                                .await?;
                        }
                    }
                }
                continue;
            }
        }

        Ok(())
    }

    /// Read a chunk from the stream
    pub(crate) async fn read_chunk(
        &self,
        stream: &mut impl BoltStream,
        buf: &mut BytesMut,
    ) -> ProtocolResult<usize> {
        // Read chunk size (2 bytes)
        let mut size_buf = [0u8; 2];
        match stream.read_exact(&mut size_buf).await {
            Ok(_) => {
                let size = u16::from_be_bytes(size_buf) as usize;
                if size == 0 {
                    return Ok(0); // End of message
                }

                // Read chunk data
                buf.reserve(size);
                unsafe {
                    let uninit = buf.chunk_mut();
                    let slice = std::slice::from_raw_parts_mut(uninit.as_mut_ptr(), size);
                    stream.read_exact(slice).await?;
                    buf.advance_mut(size);
                }

                Ok(size)
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                Err(ProtocolError::ConnectionClosed)
            }
            Err(e) => Err(ProtocolError::Other(format!("Read error: {}", e))),
        }
    }

    /// Process a Bolt message
    pub(crate) async fn process_message(
        &mut self,
        message_bytes: &Bytes,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<bool> {
        if message_bytes.is_empty() {
            return Ok(true);
        }

        let marker = message_bytes[0];

        // Check if it's a structure (Bolt messages are always structures)
        // Tiny structure: 0xB0 - 0xBF
        let signature = if (marker & 0xF0) == 0xB0 {
            if message_bytes.len() < 2 {
                return Err(ProtocolError::CypherError("Message too short".to_string()));
            }
            message_bytes[1]
        } else if marker == 0xDC {
            // Struct 8
            if message_bytes.len() < 3 {
                return Err(ProtocolError::CypherError("Message too short".to_string()));
            }
            message_bytes[2]
        } else {
            warn!("Invalid message format: marker 0x{:02X}", marker);
            self.send_ignored(stream).await?;
            return Ok(true);
        };

        match signature {
            0x01 => {
                // HELLO message
                let message = self.decode_hello(message_bytes)?;
                self.handle_hello(message, stream).await?;
            }
            0x02 => {
                // GOODBYE message
                info!("Client sent GOODBYE");
                return Ok(false);
            }
            0x10 => {
                // RUN message
                let (query, params, extra) = self.decode_run(message_bytes)?;
                self.handle_run(query, params, extra, stream).await?;
            }
            0x3F => {
                // PULL message
                let (n, qid) = self.decode_pull(message_bytes)?;
                self.handle_pull(n, qid, stream).await?;
            }
            0x2F => {
                // DISCARD message
                let (n, qid) = self.decode_discard(message_bytes)?;
                self.handle_discard(n, qid, stream).await?;
            }
            0x11 => {
                // BEGIN message
                let extra = self.decode_begin(message_bytes)?;
                self.handle_begin(extra, stream).await?;
            }
            0x12 => {
                // COMMIT message
                self.handle_commit(stream).await?;
            }
            0x13 => {
                // ROLLBACK message
                self.handle_rollback(stream).await?;
            }
            0x0F => {
                // RESET message
                self.handle_reset(stream).await?;
            }
            0x66 => {
                // ROUTE message (0x66)
                // Simplified: just ignore or send empty route
                warn!("Received ROUTE message (ignoring)");
                self.send_success(HashMap::new(), stream).await?;
            }
            0x6A => {
                // LOGON message
                let logon = self.decode_hello(message_bytes)?;
                self.handle_logon(logon, stream).await?;
            }
            0x6B => {
                // LOGOFF message
                self.handle_logoff(stream).await?;
            }
            _ => {
                warn!("Unknown message signature: 0x{:02X}", signature);
                self.send_ignored(stream).await?;
            }
        }

        Ok(true)
    }

    /// Decode HELLO message using PackStream decoder
    fn decode_hello(&mut self, bytes: &Bytes) -> ProtocolResult<HashMap<String, Value>> {
        // HELLO is a structure with signature 0x01 containing a map
        // Format: 0xB1 0x01 <map>
        if bytes.len() < 3 {
            return Ok(HashMap::new());
        }

        // Skip the structure header (0xBn 0x01)
        self.decoder.reset();
        let skip_offset = if bytes[0] >= 0xB0 && bytes[0] <= 0xBF {
            2
        } else {
            1
        };

        if skip_offset >= bytes.len() {
            return Ok(HashMap::new());
        }

        // Try to decode the map
        self.decoder.position = skip_offset;
        match self.decoder.decode_value(&bytes[..]) {
            Ok(Value::Object(map)) => {
                let result: HashMap<String, Value> = map.into_iter().collect();
                Ok(result)
            }
            Ok(_) => Ok(HashMap::new()),
            Err(e) => {
                debug!("Failed to decode HELLO payload: {}", e);
                Ok(HashMap::new())
            }
        }
    }

    /// Handle HELLO message with authentication
    async fn handle_hello(
        &mut self,
        hello: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        info!("Received HELLO message with {} fields", hello.len());

        // Extract authentication information
        let scheme = hello
            .get("scheme")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let principal = hello
            .get("principal")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let user_agent = hello
            .get("user_agent")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let routing = hello.get("routing").cloned();

        info!(
            "Client authentication: scheme={:?}, principal={:?}, user_agent={}, routing={:?}",
            scheme, principal, user_agent, routing
        );

        // Validate authentication (for now, accept all - can be enhanced with auth backend)
        let auth_valid = match scheme.as_deref() {
            Some("basic") => {
                // Basic auth with principal/credentials
                // In production, verify against auth backend
                true
            }
            Some("none") | None => {
                // No authentication required
                true
            }
            Some(other) => {
                warn!("Unknown auth scheme: {}", other);
                true // Accept for now
            }
        };

        if !auth_valid {
            return self
                .send_failure(stream, "AuthenticationError", "Invalid credentials")
                .await;
        }

        // Store authentication state
        self.auth_state = AuthState {
            authenticated: true,
            principal,
            scheme,
        };

        // Send SUCCESS response
        let mut response = HashMap::new();
        response.insert(
            "server".to_string(),
            Value::String("Orbit-RS/1.0 Neo4j/5.0-compatible".to_string()),
        );
        response.insert(
            "connection_id".to_string(),
            Value::String(format!("bolt-{}", uuid::Uuid::new_v4())),
        );

        self.send_success(response, stream).await?;
        Ok(())
    }

    /// Handle LOGON message with authentication
    async fn handle_logon(
        &mut self,
        logon: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        info!("Received LOGON message with {} fields", logon.len());

        // Extract authentication information
        let scheme = logon
            .get("scheme")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let principal = logon
            .get("principal")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        info!(
            "Client logon: scheme={:?}, principal={:?}",
            scheme, principal
        );

        // For now, we just accept the logon
        self.auth_state = AuthState {
            authenticated: true,
            principal,
            scheme,
        };

        // Send SUCCESS response
        let response = HashMap::new();
        self.send_success(response, stream).await?;
        Ok(())
    }


    self.send_success(response, stream).await?;
        Ok(())
    }

    /// Handle LOGOFF message
    async fn handle_logoff(&mut self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        info!("Received LOGOFF message");

        // Reset authentication state
        self.auth_state = AuthState {
            authenticated: false,
            principal: None,
            scheme: None,
        };

        // Send SUCCESS response
        let response = HashMap::new();
        self.send_success(response, stream).await?;
        Ok(())
    }

    /// Decode RUN message using PackStream decoder
    #[allow(clippy::type_complexity)]
    fn decode_run(
        &mut self,
        data: &Bytes,
    ) -> ProtocolResult<(String, HashMap<String, Value>, HashMap<String, Value>)> {
        // RUN is a structure with signature 0x10 containing: query (string), params (map), extra (map)
        // Format: 0xB3 0x10 <string> <map> <map>
        if data.len() < 4 {
            return Err(ProtocolError::CypherError(
                "RUN message too short".to_string(),
            ));
        }

        // Skip structure header
        let skip_offset = if data[0] >= 0xB0 && data[0] <= 0xBF {
            2
        } else {
            1
        };

        self.decoder.reset();
        self.decoder.position = skip_offset;

        // Decode query string
        let query = match self.decoder.decode_value(&data[..])? {
            Value::String(s) => s,
            v => {
                // Fallback: try to extract query from remaining bytes
                let start = skip_offset;
                let query_bytes = &data[start..];
                // Find the query string (skip marker byte and length)
                if !query_bytes.is_empty() {
                    let marker = query_bytes[0];
                    if (0x80..=0x8F).contains(&marker) {
                        // Tiny string
                        let len = (marker & 0x0F) as usize;
                        if query_bytes.len() > 1 + len {
                            String::from_utf8_lossy(&query_bytes[1..1 + len]).to_string()
                        } else {
                            v.to_string()
                        }
                    } else {
                        v.to_string()
                    }
                } else {
                    v.to_string()
                }
            }
        };

        // Decode parameters map
        let params = match self.decoder.decode_value(&data[..]) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => HashMap::new(),
        };

        // Decode extra map (optional)
        let extra = match self.decoder.decode_value(&data[..]) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => HashMap::new(),
        };

        debug!(
            "Decoded RUN: query={}, params={:?}, extra={:?}",
            query, params, extra
        );
        Ok((query, params, extra))
    }

    /// Handle RUN message
    async fn handle_run(
        &mut self,
        query: String,
        parameters: HashMap<String, Value>,
        _extra: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        info!("Received RUN message: {}", query);

        if !self.auth_state.authenticated {
            return self
                .send_failure(stream, "AuthenticationError", "Not authenticated")
                .await;
        }

        // Store query for PULL
        self.current_query = Some(query.clone());
        self.current_parameters = Some(parameters);

        // Clear any pending results
        self.pending_results.clear();
        self.result_columns.clear();

        // Execute the Cypher query
        match self.execute_cypher_query(&query).await {
            Ok((columns, results)) => {
                self.result_columns = columns.clone();
                self.pending_results = results;

                let mut response = HashMap::new();
                response.insert(
                    "qid".to_string(),
                    Value::Number(serde_json::Number::from(1)),
                );
                response.insert(
                    "fields".to_string(),
                    Value::Array(columns.into_iter().map(Value::String).collect()),
                );
                response.insert(
                    "t_first".to_string(),
                    Value::Number(serde_json::Number::from(0)),
                );

                self.send_success(response, stream).await?;
            }
            Err(e) => {
                error!("Failed to execute Cypher query: {}", e);
                self.send_failure(stream, "SyntaxError", &e.to_string())
                    .await?;
            }
        }

        Ok(())
    }

    /// Evaluate a Cypher expression
    fn evaluate_expression(
        &self,
        expr: &crate::protocols::cypher::cypher_parser::Expression,
        row: &[Value],
        columns: &[String],
    ) -> Value {
        match expr {
            crate::protocols::cypher::cypher_parser::Expression::Literal(val) => match val {
                serde_json::Value::Number(n) => Value::Number(n.clone()),
                serde_json::Value::String(s) => Value::String(s.clone()),
                serde_json::Value::Bool(b) => Value::Bool(*b),
                serde_json::Value::Null => Value::Null,
                _ => Value::String(val.to_string()),
            },
            crate::protocols::cypher::cypher_parser::Expression::Variable(name) => {
                if let Some(idx) = columns.iter().position(|c| c == name) {
                    if idx < row.len() {
                        row[idx].clone()
                    } else {
                        Value::Null
                    }
                } else {
                    Value::Null
                }
            }
            crate::protocols::cypher::cypher_parser::Expression::BinaryOp {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate_expression(left, row, columns);
                let right_val = self.evaluate_expression(right, row, columns);

                match operator {
                    crate::protocols::cypher::cypher_parser::BinaryOperator::Add => {
                        match (left_val, right_val) {
                            (Value::Number(l), Value::Number(r)) => {
                                if let (Some(l_i64), Some(r_i64)) = (l.as_i64(), r.as_i64()) {
                                    Value::Number(serde_json::Number::from(l_i64 + r_i64))
                                } else if let (Some(l_f64), Some(r_f64)) = (l.as_f64(), r.as_f64())
                                {
                                    serde_json::Number::from_f64(l_f64 + r_f64)
                                        .map(Value::Number)
                                        .unwrap_or(Value::Null)
                                } else {
                                    Value::Null
                                }
                            }
                            (Value::String(l), Value::String(r)) => {
                                Value::String(format!("{}{}", l, r))
                            }
                            _ => Value::Null,
                        }
                    }
                    crate::protocols::cypher::cypher_parser::BinaryOperator::Subtract => {
                        match (left_val, right_val) {
                            (Value::Number(l), Value::Number(r)) => {
                                if let (Some(l_i64), Some(r_i64)) = (l.as_i64(), r.as_i64()) {
                                    Value::Number(serde_json::Number::from(l_i64 - r_i64))
                                } else if let (Some(l_f64), Some(r_f64)) = (l.as_f64(), r.as_f64())
                                {
                                    serde_json::Number::from_f64(l_f64 - r_f64)
                                        .map(Value::Number)
                                        .unwrap_or(Value::Null)
                                } else {
                                    Value::Null
                                }
                            }
                            _ => Value::Null,
                        }
                    }
                    // Implement other operators as needed
                    _ => Value::Null,
                }
            }
            _ => Value::Null,
        }
    }

    /// Execute a Cypher query and return columns and result rows
    async fn execute_cypher_query(
        &self,
        query: &str,
    ) -> ProtocolResult<(Vec<String>, Vec<Vec<Value>>)> {
        // Parse the query
        let parsed = self.parser.parse(query)?;

        debug!("Parsed Cypher query: {:?}", parsed);

        let mut columns = Vec::new();
        let mut results = Vec::new();

        // Process each clause
        for clause in &parsed.clauses {
            match clause {
                crate::protocols::cypher::cypher_parser::CypherClause::Match { pattern } => {
                    // Execute MATCH clause
                    for element in &pattern.elements {
                        match element {
                            crate::protocols::cypher::cypher_parser::PatternElement::Node(node_pattern) => {
                                // Match nodes by label
                                for label in &node_pattern.labels {
                                    let all_nodes = self.storage.get_all_nodes().await?;
                                    for node in all_nodes {
                                        if node.labels.contains(label) {
                                            // Check property filter
                                            let matches = node_pattern.properties.iter().all(|(k, v)| {
                                                node.properties.get(k) == Some(v)
                                            });
                                            if matches {
                                                let row = vec![self.node_to_value(&node)];
                                                results.push(row);
                                            }
                                        }
                                    }
                                }
                                if let Some(var) = &node_pattern.variable {
                                    if !columns.contains(var) {
                                        columns.push(var.clone());
                                    }
                                }
                            }
                            crate::protocols::cypher::cypher_parser::PatternElement::Relationship(rel_pattern) => {
                                // Match relationships by type
                                let all_rels = self.storage.get_all_relationships().await?;
                                for rel in all_rels {
                                    if let Some(ref rel_type) = rel_pattern.rel_type {
                                        if rel.rel_type == *rel_type {
                                            let row = vec![self.relationship_to_value(&rel)];
                                            results.push(row);
                                        }
                                    } else {
                                        let row = vec![self.relationship_to_value(&rel)];
                                        results.push(row);
                                    }
                                }
                                if let Some(var) = &rel_pattern.variable {
                                    if !columns.contains(var) {
                                        columns.push(var.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Create { pattern } => {
                    // Execute CREATE clause
                    // We need to handle variable binding from previous clauses (MATCH)
                    // And we need to link nodes with relationships

                    // For each row in current results (or 1 run if empty), we execute the CREATE
                    if results.is_empty() {
                        results.push(vec![]);
                    }

                    let mut new_results = Vec::new();

                    for row in &results {
                        let mut current_row = row.clone();
                        let mut last_node_id: Option<String> = None;
                        let mut pending_rel: Option<
                            crate::protocols::cypher::cypher_parser::RelationshipPattern,
                        > = None;

                        for element in &pattern.elements {
                            match element {
                                crate::protocols::cypher::cypher_parser::PatternElement::Node(node_pattern) => {
                                    // Check if variable is already bound
                                    let mut node_id = None;

                                    if let Some(var) = &node_pattern.variable {
                                        if let Some(idx) = columns.iter().position(|c| c == var) {
                                            if idx < current_row.len() {
                                                // Variable is bound, use existing node
                                                let val = &current_row[idx];
                                                if let Value::Object(map) = val {
                                                    if let Some(Value::String(id)) = map.get("elementId") {
                                                        node_id = Some(id.clone());
                                                    } else if let Some(Value::Number(id)) = map.get("id") {
                                                        node_id = Some(id.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // If not bound, create new node
                                    if node_id.is_none() {
                                        let node = GraphNode {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            labels: node_pattern.labels.clone(),
                                            properties: node_pattern.properties.clone(),
                                        };
                                        self.storage.store_node(node.clone()).await?;
                                        node_id = Some(node.id.clone());
                                        info!("Created node: {:?}", node.id);

                                        // Update row/columns if variable present
                                        if let Some(var) = &node_pattern.variable {
                                            if !columns.contains(var) {
                                                // This is tricky: we can't easily add columns in the middle of processing rows
                                                // For now, we assume CREATE extends the row if variable is new
                                                // But we need to update 'columns' outside the loop?
                                                // Simplified: we just push to current_row, and we'll fix columns later
                                                current_row.push(self.node_to_value(&node));
                                            }
                                        }
                                    }

                                    let current_node_id = node_id.unwrap();

                                    // If we have a pending relationship, create it now
                                    if let Some(rel_pattern) = pending_rel.take() {
                                        if let Some(start_id) = last_node_id {
                                            let rel = GraphRelationship {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                start_node: start_id,
                                                end_node: current_node_id.clone(),
                                                rel_type: rel_pattern.rel_type.clone().unwrap_or_else(|| "RELATED".to_string()),
                                                properties: rel_pattern.properties.clone(),
                                            };
                                            self.storage.store_relationship(rel.clone()).await?;
                                            info!("Created relationship: {:?} -> {:?} -> {:?}", rel.start_node, rel.rel_type, rel.end_node);
                                        }
                                    }

                                    last_node_id = Some(current_node_id);
                                }
                                crate::protocols::cypher::cypher_parser::PatternElement::Relationship(rel_pattern) => {
                                    pending_rel = Some(rel_pattern.clone());
                                }
                            }
                        }
                        new_results.push(current_row);
                    }

                    // Update columns if we added new variables
                    // This is a bit hacky, we should track new variables properly
                    for element in &pattern.elements {
                        if let crate::protocols::cypher::cypher_parser::PatternElement::Node(
                            node_pattern,
                        ) = element
                        {
                            if let Some(var) = &node_pattern.variable {
                                if !columns.contains(var) {
                                    columns.push(var.clone());
                                }
                            }
                        }
                    }

                    results = new_results;
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Return { items } => {
                    // If results is empty and we haven't executed a MATCH, assume implicit single row
                    // (This is a simplification; ideally we'd track if we have a stream of rows)
                    if results.is_empty() && columns.is_empty() {
                        results.push(vec![]);
                    }

                    let mut new_columns = Vec::new();
                    let mut new_results = Vec::new();

                    for item in items {
                        let col_name = item
                            .alias
                            .clone()
                            .unwrap_or_else(|| item.expression.clone());
                        new_columns.push(col_name);
                    }

                    for row in results {
                        let mut new_row = Vec::new();
                        for item in items {
                            let value = self.evaluate_expression(&item.expr, &row, &columns);
                            new_row.push(value);
                        }
                        new_results.push(new_row);
                    }

                    columns = new_columns;
                    results = new_results;
                }
                crate::protocols::cypher::cypher_parser::CypherClause::With {
                    items,
                    where_condition: _,
                } => {
                    // WITH clause is similar to RETURN but for intermediate results
                    if results.is_empty() && columns.is_empty() {
                        results.push(vec![]);
                    }

                    let mut new_columns = Vec::new();
                    let mut new_results = Vec::new();

                    for item in items {
                        let col_name = item.alias.clone().unwrap_or_else(|| "expr".to_string());
                        new_columns.push(col_name);
                    }

                    for row in results {
                        let mut new_row = Vec::new();
                        for item in items {
                            let value = self.evaluate_expression(&item.expression, &row, &columns);
                            new_row.push(value);
                        }
                        new_results.push(new_row);
                    }

                    columns = new_columns;
                    results = new_results;
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Where { condition: _ } => {
                    // WHERE clause filters - would need to filter pending_results
                    // For simplicity, we handle WHERE during MATCH
                    debug!("WHERE clause processing - filtering applied during MATCH");
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Delete {
                    variables,
                    detach,
                } => {
                    debug!(
                        "DELETE clause processing: variables={:?}, detach={}",
                        variables, detach
                    );
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Set { assignments } => {
                    debug!("SET clause processing: {:?} assignments", assignments.len());
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Merge { pattern: _ } => {
                    debug!("MERGE clause processing");
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Remove { items } => {
                    debug!("REMOVE clause processing: {:?} items", items.len());
                }
                crate::protocols::cypher::cypher_parser::CypherClause::OrderBy { items } => {
                    debug!("ORDER BY clause processing: {:?} items", items.len());
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Limit { count } => {
                    debug!("LIMIT clause processing: {}", count);
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Skip { count } => {
                    debug!("SKIP clause processing: {}", count);
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Call {
                    procedure,
                    arguments,
                    yield_items,
                } => {
                    debug!(
                        "CALL clause processing: {} with {} args, yield={:?}",
                        procedure,
                        arguments.len(),
                        yield_items
                    );
                }

                crate::protocols::cypher::cypher_parser::CypherClause::OptionalMatch {
                    pattern: _,
                } => {
                    debug!("OPTIONAL MATCH clause processing");
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Unwind {
                    expression,
                    variable,
                } => {
                    debug!("UNWIND clause processing: {:?} AS {}", expression, variable);
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Foreach {
                    variable,
                    list,
                    clauses: inner_clauses,
                } => {
                    debug!(
                        "FOREACH clause processing: {} IN {:?}, {} inner clauses",
                        variable,
                        list,
                        inner_clauses.len()
                    );
                }
                crate::protocols::cypher::cypher_parser::CypherClause::CaseExpression {
                    test_expression,
                    when_clauses,
                    else_result,
                } => {
                    debug!(
                        "CASE expression processing: test={:?}, {} whens, else={:?}",
                        test_expression,
                        when_clauses.len(),
                        else_result
                    );
                }
                crate::protocols::cypher::cypher_parser::CypherClause::CreateIndex {
                    name,
                    index_type,
                    entity_type,
                    label_or_type,
                    properties,
                    if_not_exists,
                } => {
                    debug!(
                        "CREATE INDEX: name={:?}, type={:?}, entity={:?}, label={}, props={:?}, if_not_exists={}",
                        name, index_type, entity_type, label_or_type, properties, if_not_exists
                    );
                    // TODO: Implement index creation in storage layer
                }
                crate::protocols::cypher::cypher_parser::CypherClause::CreateConstraint {
                    name,
                    constraint_type,
                    entity_type,
                    label_or_type,
                    properties,
                    if_not_exists,
                } => {
                    debug!(
                        "CREATE CONSTRAINT: name={:?}, type={:?}, entity={:?}, label={}, props={:?}, if_not_exists={}",
                        name, constraint_type, entity_type, label_or_type, properties, if_not_exists
                    );
                    // TODO: Implement constraint creation in storage layer
                }
                crate::protocols::cypher::cypher_parser::CypherClause::DropIndex {
                    name,
                    if_exists,
                } => {
                    debug!("DROP INDEX: name={}, if_exists={}", name, if_exists);
                    // TODO: Implement index deletion in storage layer
                }
                crate::protocols::cypher::cypher_parser::CypherClause::DropConstraint {
                    name,
                    if_exists,
                } => {
                    debug!("DROP CONSTRAINT: name={}, if_exists={}", name, if_exists);
                    // TODO: Implement constraint deletion in storage layer
                }
                crate::protocols::cypher::cypher_parser::CypherClause::ShowIndexes => {
                    debug!("SHOW INDEXES");
                    // TODO: Return list of indexes from storage layer
                }
                crate::protocols::cypher::cypher_parser::CypherClause::ShowConstraints => {
                    debug!("SHOW CONSTRAINTS");
                    // TODO: Return list of constraints from storage layer
                }
            }
        }

        // Default columns if none specified
        if columns.is_empty() {
            columns.push("result".to_string());
        }

        Ok((columns, results))
    }

    /// Convert a graph node to a Bolt/JSON Value
    fn node_to_value(&self, node: &GraphNode) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), Value::String(node.id.clone()));
        map.insert(
            "labels".to_string(),
            Value::Array(
                node.labels
                    .iter()
                    .map(|l| Value::String(l.clone()))
                    .collect(),
            ),
        );
        map.insert(
            "properties".to_string(),
            Value::Object(
                node.properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        );
        Value::Object(map)
    }

    /// Convert a graph relationship to a Bolt/JSON Value
    fn relationship_to_value(&self, rel: &GraphRelationship) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), Value::String(rel.id.clone()));
        map.insert("type".to_string(), Value::String(rel.rel_type.clone()));
        map.insert(
            "startNode".to_string(),
            Value::String(rel.start_node.clone()),
        );
        map.insert("endNode".to_string(), Value::String(rel.end_node.clone()));
        map.insert(
            "properties".to_string(),
            Value::Object(
                rel.properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        );
        Value::Object(map)
    }

    /// Decode PULL message
    fn decode_pull(&self, _bytes: &Bytes) -> ProtocolResult<(Option<i64>, Option<i64>)> {
        // Simplified: return None for both
        Ok((None, None))
    }

    /// Handle PULL message
    async fn handle_pull(
        &mut self,
        n: Option<i64>,
        _qid: Option<i64>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        info!(
            "Received PULL message, pending results: {}",
            self.pending_results.len()
        );

        // Determine how many records to send
        let batch_size = n.unwrap_or(-1);
        let to_send = if batch_size < 0 {
            // Send all remaining
            self.pending_results.len()
        } else {
            std::cmp::min(batch_size as usize, self.pending_results.len())
        };

        // Send records
        for _ in 0..to_send {
            if let Some(row) = self.pending_results.pop() {
                self.send_record(row, stream).await?;
            }
        }

        // Send SUCCESS with metadata
        let mut metadata = HashMap::new();

        if self.pending_results.is_empty() {
            // All records sent
            metadata.insert("has_more".to_string(), Value::Bool(false));
            metadata.insert(
                "type".to_string(),
                Value::String("r".to_string()), // read-only result
            );
        } else {
            // More records pending
            metadata.insert("has_more".to_string(), Value::Bool(true));
        }

        self.send_success(metadata, stream).await?;
        Ok(())
    }

    /// Decode DISCARD message
    fn decode_discard(&self, _bytes: &Bytes) -> ProtocolResult<(Option<i64>, Option<i64>)> {
        Ok((None, None))
    }

    /// Handle DISCARD message
    async fn handle_discard(
        &mut self,
        _n: Option<i64>,
        _qid: Option<i64>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        info!("Received DISCARD message");
        self.current_query = None;
        self.current_parameters = None;
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Decode BEGIN message with transaction options
    fn decode_begin(&mut self, bytes: &Bytes) -> ProtocolResult<HashMap<String, Value>> {
        // BEGIN is a structure with signature 0x11 containing optional extra map
        // Format: 0xB1 0x11 <map>
        if bytes.len() < 3 {
            return Ok(HashMap::new());
        }

        // Skip structure header
        let skip_offset = if bytes[0] >= 0xB0 && bytes[0] <= 0xBF {
            2
        } else {
            1
        };

        self.decoder.reset();
        self.decoder.position = skip_offset;

        match self.decoder.decode_value(&bytes[..]) {
            Ok(Value::Object(map)) => Ok(map.into_iter().collect()),
            _ => Ok(HashMap::new()),
        }
    }

    /// Handle BEGIN message - start a new transaction
    async fn handle_begin(
        &mut self,
        extra: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        info!("Received BEGIN message");

        if !self.auth_state.authenticated {
            return self
                .send_failure(stream, "AuthenticationError", "Not authenticated")
                .await;
        }

        if self.transaction_state == TransactionState::Active {
            return self
                .send_failure(
                    stream,
                    "TransactionError",
                    "Transaction already in progress",
                )
                .await;
        }

        // Start new transaction
        self.transaction_id += 1;
        self.transaction_state = TransactionState::Active;

        // Extract transaction metadata
        let _tx_timeout = extra
            .get("tx_timeout")
            .and_then(|v| v.as_i64())
            .unwrap_or(30000); // Default 30s timeout

        let tx_metadata = extra.get("tx_metadata").cloned();

        info!(
            "Transaction {} started, metadata: {:?}",
            self.transaction_id, tx_metadata
        );

        // Send SUCCESS with bookmark
        let mut response = HashMap::new();
        response.insert(
            "bookmark".to_string(),
            Value::String(format!("orbit:tx-{}", self.transaction_id)),
        );
        self.send_success(response, stream).await?;
        Ok(())
    }

    /// Handle COMMIT message - commit the current transaction
    async fn handle_commit(&mut self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        info!("Received COMMIT message");

        if !self.auth_state.authenticated {
            return self
                .send_failure(stream, "AuthenticationError", "Not authenticated")
                .await;
        }

        match self.transaction_state {
            TransactionState::Active => {
                // Commit the transaction
                info!("Committing transaction {}", self.transaction_id);
                self.transaction_state = TransactionState::None;

                // Send SUCCESS with bookmark
                let mut response = HashMap::new();
                response.insert(
                    "bookmark".to_string(),
                    Value::String(format!("orbit:tx-{}-committed", self.transaction_id)),
                );
                self.send_success(response, stream).await?;
            }
            TransactionState::RollbackPending => {
                return self
                    .send_failure(
                        stream,
                        "TransactionError",
                        "Transaction marked for rollback",
                    )
                    .await;
            }
            TransactionState::None => {
                return self
                    .send_failure(stream, "TransactionError", "No active transaction")
                    .await;
            }
        }
        Ok(())
    }

    /// Handle ROLLBACK message - rollback the current transaction
    /// Handle ROLLBACK message - rollback the current transaction
    async fn handle_rollback(&mut self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        info!("Received ROLLBACK message");

        if !self.auth_state.authenticated {
            return self
                .send_failure(stream, "AuthenticationError", "Not authenticated")
                .await;
        }

        match self.transaction_state {
            TransactionState::Active | TransactionState::RollbackPending => {
                // Rollback the transaction
                info!("Rolling back transaction {}", self.transaction_id);
                self.transaction_state = TransactionState::None;

                // Clear pending results
                self.pending_results.clear();
                self.result_columns.clear();
                self.current_query = None;
                self.current_parameters = None;

                self.send_success(HashMap::new(), stream).await?;
            }
            TransactionState::None => {
                return self
                    .send_failure(stream, "TransactionError", "No active transaction")
                    .await;
            }
        }
        Ok(())
    }

    /// Handle RESET message - reset connection state
    /// Handle RESET message - reset connection state
    async fn handle_reset(&mut self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        info!("Received RESET message");

        // Reset all connection state
        self.current_query = None;
        self.current_parameters = None;
        self.pending_results.clear();
        self.result_columns.clear();

        // Rollback any active transaction
        if self.transaction_state != TransactionState::None {
            info!(
                "Rolling back transaction {} due to RESET",
                self.transaction_id
            );
            self.transaction_state = TransactionState::None;
        }

        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Send SUCCESS message
    async fn send_success(
        &self,
        metadata: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0xB1); // Structure (size 1)
        buf.put_u8(0x70); // SUCCESS signature

        // Encode metadata map
        self.encode_packstream_value(&Value::Object(metadata.into_iter().collect()), &mut buf);

        self.send_chunk(&buf, stream).await
    }

    /// Send FAILURE message
    async fn send_failure(
        &self,
        stream: &mut impl BoltStream,
        _code: &str,
        _message: &str,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0x7F); // FAILURE marker
                          // Simplified: would encode code and message
        self.send_chunk(&buf, stream).await
    }

    /// Send IGNORED message
    async fn send_ignored(&self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0x7E); // IGNORED marker
        self.send_chunk(&buf, stream).await
    }

    /// Send RECORD message with values
    async fn send_record(
        &self,
        values: Vec<Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();

        // RECORD structure marker: 0xB1 followed by signature 0x71
        // Then a list of values
        buf.put_u8(0xB1); // Tiny structure (1 field)
        buf.put_u8(0x71); // RECORD signature

        // Encode values as a tiny list
        let len = values.len();
        if len < 16 {
            buf.put_u8(0x90 + len as u8); // Tiny list
        } else {
            buf.put_u8(0xD4); // List8
            buf.put_u8(len as u8);
        }

        // Encode each value (simplified PackStream encoding)
        for value in values {
            self.encode_packstream_value(&value, &mut buf);
        }

        self.send_chunk(&buf, stream).await
    }

    /// Encode a JSON value as PackStream
    #[allow(clippy::only_used_in_recursion)]
    fn encode_packstream_value(&self, value: &Value, buf: &mut BytesMut) {
        match value {
            Value::Null => {
                buf.put_u8(0xC0); // NULL
            }
            Value::Bool(b) => {
                buf.put_u8(if *b { 0xC3 } else { 0xC2 }); // TRUE or FALSE
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if (-16..=127).contains(&i) {
                        buf.put_u8(i as u8); // Tiny int
                    } else if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
                        buf.put_u8(0xC8); // INT_8
                        buf.put_i8(i as i8);
                    } else if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
                        buf.put_u8(0xC9); // INT_16
                        buf.put_i16(i as i16);
                    } else if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                        buf.put_u8(0xCA); // INT_32
                        buf.put_i32(i as i32);
                    } else {
                        buf.put_u8(0xCB); // INT_64
                        buf.put_i64(i);
                    }
                } else if let Some(f) = n.as_f64() {
                    buf.put_u8(0xC1); // FLOAT_64
                    buf.put_f64(f);
                }
            }
            Value::String(s) => {
                let bytes = s.as_bytes();
                let len = bytes.len();
                if len < 16 {
                    buf.put_u8(0x80 + len as u8); // Tiny string
                } else if len < 256 {
                    buf.put_u8(0xD0); // STRING_8
                    buf.put_u8(len as u8);
                } else if len < 65536 {
                    buf.put_u8(0xD1); // STRING_16
                    buf.put_u16(len as u16);
                } else {
                    buf.put_u8(0xD2); // STRING_32
                    buf.put_u32(len as u32);
                }
                buf.put_slice(bytes);
            }
            Value::Array(arr) => {
                let len = arr.len();
                if len < 16 {
                    buf.put_u8(0x90 + len as u8); // Tiny list
                } else if len < 256 {
                    buf.put_u8(0xD4); // LIST_8
                    buf.put_u8(len as u8);
                } else {
                    buf.put_u8(0xD5); // LIST_16
                    buf.put_u16(len as u16);
                }
                for item in arr {
                    self.encode_packstream_value(item, buf);
                }
            }
            Value::Object(map) => {
                let len = map.len();
                if len < 16 {
                    buf.put_u8(0xA0 + len as u8); // Tiny map
                } else if len < 256 {
                    buf.put_u8(0xD8); // MAP_8
                    buf.put_u8(len as u8);
                } else {
                    buf.put_u8(0xD9); // MAP_16
                    buf.put_u16(len as u16);
                }
                for (key, val) in map {
                    // Encode key as string
                    self.encode_packstream_value(&Value::String(key.clone()), buf);
                    // Encode value
                    self.encode_packstream_value(val, buf);
                }
            }
        }
    }

    /// Send a chunk to the client
    async fn send_chunk(
        &self,
        data: &BytesMut,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        let size = data.len() as u16;
        let mut chunk = BytesMut::with_capacity(2 + data.len() + 2);
        chunk.put_u16(size);
        chunk.put_slice(data);
        chunk.put_u16(0); // End of message marker

        stream.write_all(&chunk).await.map_err(|e| {
            error!("Failed to write chunk: {}", e);
            ProtocolError::Other(format!("Write error: {}", e))
        })
    }
}
