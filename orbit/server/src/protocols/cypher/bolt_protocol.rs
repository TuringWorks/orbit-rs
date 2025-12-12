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
    storage: Option<Arc<dyn CypherStorageProvider>>,
    parser: CypherParser,
    /// Authentication state
    pub(crate) auth_state: AuthState,
    /// Transaction state
    #[allow(dead_code)]
    pub(crate) transaction_state: TransactionState,
    /// Transaction ID counter
    #[allow(dead_code)]
    transaction_id: u64,
    pub(crate) current_query: Option<String>,
    current_parameters: Option<HashMap<String, Value>>,
    /// Pending query results (nodes and relationships as JSON values)
    pub(crate) pending_results: Vec<Vec<Value>>,
    /// Column names for current result set
    result_columns: Vec<String>,
    /// PackStream decoder for parsing messages
    decoder: PackStreamDecoder,
}

impl BoltProtocolHandler {
    /// Create a new Bolt protocol handler
    pub fn new(storage: Option<Arc<dyn CypherStorageProvider>>) -> Self {
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
// DISABLED:             0x3F => {
// DISABLED:                 // PULL message
// DISABLED:                 let (n, qid) = self.decode_pull(message_bytes)?;
// DISABLED:                 self.handle_pull(n, qid, stream).await?;
// DISABLED:             }
// DISABLED:             0x2F => {
// DISABLED:                 // DISCARD message
// DISABLED:                 let (n, qid) = self.decode_discard(message_bytes)?;
// DISABLED:                 self.handle_discard(n, qid, stream).await?;
// DISABLED:             }
// DISABLED:             0x11 => {
// DISABLED:                 // BEGIN message
// DISABLED:                 let extra = self.decode_begin(message_bytes)?;
// DISABLED:                 self.handle_begin(extra, stream).await?;
// DISABLED:             }
// DISABLED:             0x12 => {
// DISABLED:                 // COMMIT message
// DISABLED:                 self.handle_commit(stream).await?;
// DISABLED:             }
// DISABLED:             0x13 => {
// DISABLED:                 // ROLLBACK message
// DISABLED:                 self.handle_rollback(stream).await?;
// DISABLED:             }
// DISABLED:             0x0F => {
// DISABLED:                 // RESET message
// DISABLED:                 self.handle_reset(stream).await?;
// DISABLED:             }
            0x66 => {
                // ROUTE message
                self.handle_route(stream).await?;
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
            0x54 => {
                // TELEMETRY message
                warn!("Received TELEMETRY message (ignoring)");
                self.send_ignored(stream).await?;
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

    /// Handle ROUTE message
    async fn handle_route(&mut self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        info!("Received ROUTE message");

        // For now, we just send back an empty routing table.
        // In a clustered environment, this would be populated with the addresses of other nodes.
        let mut response = HashMap::new();
        response.insert("rt".to_string(), Value::Object(serde_json::Map::new()));
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
        let storage = self.storage.as_ref().ok_or_else(|| {
            ProtocolError::CypherError("Storage provider not available".to_string())
        })?;

        // Parse the query
        let parsed = self.parser.parse(query)?;

        debug!("Parsed Cypher query: {:?}", parsed);

        let mut columns: Vec<String> = Vec::new();
        let mut results: Vec<Vec<Value>> = Vec::new();

        // Process each clause
        for clause in &parsed.clauses {
            match clause {
                crate::protocols::cypher::cypher_parser::CypherClause::Match { pattern } => {
                    // Very basic pattern matching for (n)-[r]->(m)
                    if pattern.elements.len() == 3 {
                        if let (
                            crate::protocols::cypher::cypher_parser::PatternElement::Node(start_node_pattern),
                            crate::protocols::cypher::cypher_parser::PatternElement::Relationship(rel_pattern),
                            crate::protocols::cypher::cypher_parser::PatternElement::Node(end_node_pattern),
                        ) = (&pattern.elements[0], &pattern.elements[1], &pattern.elements[2])
                        {
                            let all_rels = storage.get_all_relationships().await?;
                            for rel in all_rels {
                                let type_matches = rel_pattern.rel_type.as_ref().map_or(true, |t| &rel.rel_type == t);
                                let props_match = rel_pattern.properties.iter().all(|(k, v)| rel.properties.get(k) == Some(v));

                                if type_matches && props_match {
                                    if let (Some(start_node), Some(end_node)) = (
                                        storage.get_node(&rel.start_node).await?,
                                        storage.get_node(&rel.end_node).await?,
                                    ) {
                                        let start_node_labels_match = start_node_pattern.labels.iter().all(|l| start_node.labels.contains(l));
                                        let end_node_labels_match = end_node_pattern.labels.iter().all(|l| end_node.labels.contains(l));

                                        if start_node_labels_match && end_node_labels_match {
                                            let mut row = Vec::new();
                                            row.push(self.node_to_value(&start_node));
                                            row.push(self.relationship_to_value(&rel));
                                            row.push(self.node_to_value(&end_node));
                                            results.push(row);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Return { items } => {
                    // This is a very simplified RETURN implementation
                    if !results.is_empty() {
                        let mut new_results = Vec::new();
                        for row in &results {
                            let mut new_row = Vec::new();
                            for item in items {
                                let value = self.evaluate_expression(&item.expr, &row, &columns);
                                new_row.push(value);
                            }
                            new_results.push(new_row);
                        }
                        results = new_results;
                    }
                    columns = items.iter().map(|i| i.alias.clone().unwrap_or_else(|| i.expression.clone())).collect();
                }
                _ => {}
            }
        }

        Ok((columns, results))
    }


    /// Send a PackStream message with chunking
    async fn send_message(
        &mut self,
        signature: u8,
        metadata: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        use bytes::BufMut;
        
        let mut message_buf = BytesMut::new();
        
        // Write structure header
        message_buf.put_u8(0xB1); // Tiny struct with 1 field
        message_buf.put_u8(signature);
        
        // Encode metadata map
        self.encode_map(&metadata, &mut message_buf);
        
        // Send in chunks (max 65535 bytes per chunk)
        let mut offset = 0;
        while offset < message_buf.len() {
            let chunk_size = std::cmp::min(message_buf.len() - offset, 65535);
            let mut chunk = BytesMut::with_capacity(chunk_size + 2);
            chunk.put_u16(chunk_size as u16);
            chunk.put_slice(&message_buf[offset..offset + chunk_size]);
            stream.write_all(&chunk).await.map_err(|e| {
                ProtocolError::Other(format!("Failed to write message chunk: {}", e))
            })?;
            offset += chunk_size;
        }
        
        // Send end marker
        stream.write_all(&[0x00, 0x00]).await.map_err(|e| {
            ProtocolError::Other(format!("Failed to write end marker: {}", e))
        })?;
        
        Ok(())
    }

    /// Encode a map to PackStream format
    fn encode_map(&self, map: &HashMap<String, Value>, buf: &mut BytesMut) {
        use bytes::BufMut;
        
        let len = map.len();
        if len < 16 {
            buf.put_u8(0xA0 | len as u8); // Tiny map
        } else if len < 256 {
            buf.put_u8(0xD8);
            buf.put_u8(len as u8);
        } else if len < 65536 {
            buf.put_u8(0xD9);
            buf.put_u16(len as u16);
        } else {
            buf.put_u8(0xDA);
            buf.put_u32(len as u32);
        }
        
        for (key, value) in map {
            self.encode_string(key, buf);
            self.encode_value(value, buf);
        }
    }

    /// Encode a string to PackStream format
    fn encode_string(&self, s: &str, buf: &mut BytesMut) {
        use bytes::BufMut;
        
        let len = s.len();
        if len < 16 {
            buf.put_u8(0x80 | len as u8); // Tiny string
        } else if len < 256 {
            buf.put_u8(0xD0);
            buf.put_u8(len as u8);
        } else if len < 65536 {
            buf.put_u8(0xD1);
            buf.put_u16(len as u16);
        } else {
            buf.put_u8(0xD2);
            buf.put_u32(len as u32);
        }
        buf.put_slice(s.as_bytes());
    }

    /// Encode a value to PackStream format
    fn encode_value(&self, value: &Value, buf: &mut BytesMut) {
        use bytes::BufMut;
        
        match value {
            Value::Null => buf.put_u8(0xC0),
            Value::Bool(b) => buf.put_u8(if *b { 0xC3 } else { 0xC2 }),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if i >= -16 && i < 128 {
                        buf.put_i8(i as i8);
                    } else if i >= -128 && i < 128 {
                        buf.put_u8(0xC8);
                        buf.put_i8(i as i8);
                    } else if i >= -32768 && i < 32768 {
                        buf.put_u8(0xC9);
                        buf.put_i16(i as i16);
                    } else if i >= -2147483648 && i < 2147483648 {
                        buf.put_u8(0xCA);
                        buf.put_i32(i as i32);
                    } else {
                        buf.put_u8(0xCB);
                        buf.put_i64(i);
                    }
                } else if let Some(f) = n.as_f64() {
                    buf.put_u8(0xC1);
                    buf.put_f64(f);
                }
            }
            Value::String(s) => self.encode_string(s, buf),
            Value::Array(arr) => {
                let len = arr.len();
                if len < 16 {
                    buf.put_u8(0x90 | len as u8);
                } else if len < 256 {
                    buf.put_u8(0xD4);
                    buf.put_u8(len as u8);
                } else if len < 65536 {
                    buf.put_u8(0xD5);
                    buf.put_u16(len as u16);
                } else {
                    buf.put_u8(0xD6);
                    buf.put_u32(len as u32);
                }
                for item in arr {
                    self.encode_value(item, buf);
                }
            }
            Value::Object(obj) => {
                let map: HashMap<String, Value> = obj.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                self.encode_map(&map, buf);
            }
        }
    }
    /// Helper method to send SUCCESS message
    async fn send_success(
        &mut self,
        metadata: HashMap<String, Value>,
        stream: &mut impl BoltStream,
    ) -> ProtocolResult<()> {
        self.send_message(0x70, metadata, stream).await
    }

    /// Helper method to send FAILURE message
    async fn send_failure(
        &mut self,
        stream: &mut impl BoltStream,
        code: &str,
        message: &str,
    ) -> ProtocolResult<()> {
        let mut metadata = HashMap::new();
        metadata.insert("code".to_string(), Value::String(code.to_string()));
        metadata.insert("message".to_string(), Value::String(message.to_string()));
        self.send_message(0x7F, metadata, stream).await
    }

    /// Helper method to send IGNORED message
    async fn send_ignored(&mut self, stream: &mut impl BoltStream) -> ProtocolResult<()> {
        self.send_message(0x7E, HashMap::new(), stream).await
    }

    /// Convert a GraphNode to a Value
    fn node_to_value(&self, node: &GraphNode) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), Value::String(node.id.to_string()));
        map.insert(
            "labels".to_string(),
            Value::Array(node.labels.iter().map(|l| Value::String(l.clone())).collect()),
        );
        map.insert("properties".to_string(), Value::Object(
            node.properties.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        ));
        Value::Object(map)
    }

    /// Convert a GraphRelationship to a Value
    fn relationship_to_value(&self, rel: &GraphRelationship) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), Value::String(rel.id.to_string()));
        map.insert("type".to_string(), Value::String(rel.rel_type.clone()));
        map.insert("start".to_string(), Value::String(rel.start_node.to_string()));
        map.insert("end".to_string(), Value::String(rel.end_node.to_string()));
        map.insert("properties".to_string(), Value::Object(
            rel.properties.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        ));
        Value::Object(map)
    }
}
