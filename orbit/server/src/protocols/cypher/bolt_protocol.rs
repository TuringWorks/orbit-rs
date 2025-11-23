//! Bolt protocol implementation for Neo4j compatibility
//!
//! This module implements the Bolt protocol (v4.0+) for handling Neo4j client connections.
//! Bolt uses a binary protocol with PackStream encoding for efficient data transfer.

use crate::protocols::cypher::storage::CypherGraphStorage;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};

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
    storage: Arc<CypherGraphStorage>,
    authenticated: bool,
    current_query: Option<String>,
    current_parameters: Option<HashMap<String, Value>>,
}

impl BoltProtocolHandler {
    /// Create a new Bolt protocol handler
    pub fn new(storage: Arc<CypherGraphStorage>) -> Self {
        Self {
            version: None,
            storage,
            authenticated: false,
            current_query: None,
            current_parameters: None,
        }
    }

    /// Handle Bolt handshake
    pub async fn handle_handshake(&mut self, stream: &mut TcpStream) -> ProtocolResult<BoltVersion> {
        let mut handshake_buf = [0u8; 20];
        stream.read_exact(&mut handshake_buf).await.map_err(|e| {
            error!("Failed to read handshake: {}", e);
            ProtocolError::Other(format!("Handshake read error: {}", e))
        })?;

        // Bolt handshake format: 4 bytes magic (0x6060B017) + 4 version proposals (4 bytes each)
        let magic = u32::from_be_bytes([handshake_buf[0], handshake_buf[1], handshake_buf[2], handshake_buf[3]]);
        
        if magic != 0x6060B017 {
            return Err(ProtocolError::CypherError("Invalid Bolt handshake magic".to_string()));
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
    pub async fn handle_connection(&mut self, mut stream: TcpStream) -> ProtocolResult<()> {
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
                Err(e) => {
                    error!("Error reading chunk: {}", e);
                    break;
                }
            };

            if chunk_size == 0 {
                info!("Client disconnected");
                break;
            }

            // Process messages in buffer
            while read_buf.len() >= 2 {
                let message_size = u16::from_be_bytes([read_buf[0], read_buf[1]]) as usize;
                
                if read_buf.len() < 2 + message_size {
                    break; // Need more data
                }

                read_buf.advance(2); // Skip size
                let message_bytes = read_buf.split_to(message_size).freeze();

                match self.process_message(&message_bytes, &mut stream).await {
                    Ok(should_continue) => {
                        if !should_continue {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error processing message: {}", e);
                        self.send_failure(&mut stream, "Error", &e.to_string()).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Read a chunk from the stream
    async fn read_chunk(&self, stream: &mut TcpStream, buf: &mut BytesMut) -> ProtocolResult<usize> {
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
                    let slice = std::slice::from_raw_parts_mut(
                        uninit.as_mut_ptr(),
                        size,
                    );
                    stream.read_exact(slice).await?;
                    buf.advance_mut(size);
                }

                Ok(size)
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
            Err(e) => Err(ProtocolError::Other(format!("Read error: {}", e))),
        }
    }

    /// Process a Bolt message
    async fn process_message(
        &mut self,
        message_bytes: &Bytes,
        stream: &mut TcpStream,
    ) -> ProtocolResult<bool> {
        if message_bytes.is_empty() {
            return Ok(true);
        }

        let marker = message_bytes[0];
        let message_type = marker & 0xF0;

        match message_type {
            0x10 => {
                // HELLO message
                let message = self.decode_hello(message_bytes)?;
                self.handle_hello(message, stream).await?;
            }
            0x11 => {
                // GOODBYE message
                info!("Client sent GOODBYE");
                return Ok(false);
            }
            0x12 => {
                // RUN message
                let (query, params, extra) = self.decode_run(message_bytes)?;
                self.handle_run(query, params, extra, stream).await?;
            }
            0x13 => {
                // PULL message
                let (n, qid) = self.decode_pull(message_bytes)?;
                self.handle_pull(n, qid, stream).await?;
            }
            0x14 => {
                // DISCARD message
                let (n, qid) = self.decode_discard(message_bytes)?;
                self.handle_discard(n, qid, stream).await?;
            }
            0x15 => {
                // BEGIN message
                let extra = self.decode_begin(message_bytes)?;
                self.handle_begin(extra, stream).await?;
            }
            0x16 => {
                // COMMIT message
                self.handle_commit(stream).await?;
            }
            0x17 => {
                // ROLLBACK message
                self.handle_rollback(stream).await?;
            }
            0x0F => {
                // RESET message
                self.handle_reset(stream).await?;
            }
            _ => {
                warn!("Unknown message type: 0x{:02X}", message_type);
                self.send_ignored(stream).await?;
            }
        }

        Ok(true)
    }

    /// Decode HELLO message
    fn decode_hello(&self, bytes: &Bytes) -> ProtocolResult<HashMap<String, Value>> {
        // Simplified: assume PackStream map format
        // In production, would need full PackStream decoder
        let map = HashMap::new();
        
        // For now, accept any HELLO message
        // Full implementation would decode PackStream format
        Ok(map)
    }

    /// Handle HELLO message
    async fn handle_hello(
        &mut self,
        _hello: HashMap<String, Value>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        info!("Received HELLO message");
        
        // For now, accept all connections (no authentication)
        self.authenticated = true;

        // Send SUCCESS response
        let mut response = HashMap::new();
        response.insert("server".to_string(), Value::String("orbit-rs/1.0".to_string()));
        response.insert("connection_id".to_string(), Value::String("bolt-1".to_string()));
        
        self.send_success(response, stream).await?;
        Ok(())
    }

    /// Decode RUN message
    fn decode_run(
        &self,
        bytes: &Bytes,
    ) -> ProtocolResult<(String, HashMap<String, Value>, HashMap<String, Value>)> {
        // Simplified: extract query string
        // Full implementation would decode PackStream format
        let query = String::from_utf8_lossy(&bytes[1..]).to_string();
        Ok((query, HashMap::new(), HashMap::new()))
    }

    /// Handle RUN message
    async fn handle_run(
        &mut self,
        query: String,
        parameters: HashMap<String, Value>,
        _extra: HashMap<String, Value>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        info!("Received RUN message: {}", query);
        
        if !self.authenticated {
            return self.send_failure(stream, "AuthenticationError", "Not authenticated").await;
        }

        // Store query for PULL
        self.current_query = Some(query.clone());
        self.current_parameters = Some(parameters);

        // Execute query using Cypher parser and storage directly
        // For now, return success - full implementation would execute the query
        let mut response = HashMap::new();
        response.insert("qid".to_string(), Value::Number(serde_json::Number::from(1)));
        response.insert("fields".to_string(), Value::Array(vec![]));
        
        self.send_success(response, stream).await?;
        Ok(())
    }

    /// Decode PULL message
    fn decode_pull(&self, bytes: &Bytes) -> ProtocolResult<(Option<i64>, Option<i64>)> {
        // Simplified: return None for both
        Ok((None, None))
    }

    /// Handle PULL message
    async fn handle_pull(
        &mut self,
        _n: Option<i64>,
        _qid: Option<i64>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        info!("Received PULL message");
        
        // For now, send empty result
        // Full implementation would stream records
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Decode DISCARD message
    fn decode_discard(&self, bytes: &Bytes) -> ProtocolResult<(Option<i64>, Option<i64>)> {
        Ok((None, None))
    }

    /// Handle DISCARD message
    async fn handle_discard(
        &mut self,
        _n: Option<i64>,
        _qid: Option<i64>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        info!("Received DISCARD message");
        self.current_query = None;
        self.current_parameters = None;
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Decode BEGIN message
    fn decode_begin(&self, _bytes: &Bytes) -> ProtocolResult<HashMap<String, Value>> {
        Ok(HashMap::new())
    }

    /// Handle BEGIN message
    async fn handle_begin(
        &mut self,
        _extra: HashMap<String, Value>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        info!("Received BEGIN message");
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Handle COMMIT message
    async fn handle_commit(&mut self, stream: &mut TcpStream) -> ProtocolResult<()> {
        info!("Received COMMIT message");
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Handle ROLLBACK message
    async fn handle_rollback(&mut self, stream: &mut TcpStream) -> ProtocolResult<()> {
        info!("Received ROLLBACK message");
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Handle RESET message
    async fn handle_reset(&mut self, stream: &mut TcpStream) -> ProtocolResult<()> {
        info!("Received RESET message");
        self.current_query = None;
        self.current_parameters = None;
        self.send_success(HashMap::new(), stream).await?;
        Ok(())
    }

    /// Send SUCCESS message
    async fn send_success(
        &self,
        metadata: HashMap<String, Value>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0xB1); // SUCCESS marker
        // Simplified: would encode metadata as PackStream
        // For now, just send marker
        self.send_chunk(&buf, stream).await
    }

    /// Send FAILURE message
    async fn send_failure(
        &self,
        stream: &mut TcpStream,
        code: &str,
        message: &str,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0x7F); // FAILURE marker
        // Simplified: would encode code and message
        self.send_chunk(&buf, stream).await
    }

    /// Send IGNORED message
    async fn send_ignored(&self, stream: &mut TcpStream) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0x7E); // IGNORED marker
        self.send_chunk(&buf, stream).await
    }

    /// Send RECORD message
    async fn send_record(
        &self,
        values: Vec<Value>,
        stream: &mut TcpStream,
    ) -> ProtocolResult<()> {
        let mut buf = BytesMut::new();
        buf.put_u8(0x71); // RECORD marker
        // Simplified: would encode values as PackStream
        self.send_chunk(&buf, stream).await
    }

    /// Send a chunk to the client
    async fn send_chunk(&self, data: &BytesMut, stream: &mut TcpStream) -> ProtocolResult<()> {
        let size = data.len() as u16;
        let mut chunk = BytesMut::with_capacity(2 + data.len());
        chunk.put_u16(size);
        chunk.put_slice(data);
        stream.write_all(&chunk).await.map_err(|e| {
            ProtocolError::Other(format!("Write error: {}", e))
        })?;
        Ok(())
    }
}

