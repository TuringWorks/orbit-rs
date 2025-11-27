//! Bolt protocol implementation for Neo4j compatibility
//!
//! This module implements the Bolt protocol (v4.0+) for handling Neo4j client connections.
//! Bolt uses a binary protocol with PackStream encoding for efficient data transfer.

use crate::protocols::cypher::cypher_parser::CypherParser;
#[cfg(feature = "storage-rocksdb")]
use crate::protocols::cypher::storage::CypherGraphStorage;
use crate::protocols::cypher::types::{GraphNode, GraphRelationship};
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
    #[cfg(feature = "storage-rocksdb")]
    storage: Arc<CypherGraphStorage>,
    parser: CypherParser,
    authenticated: bool,
    current_query: Option<String>,
    current_parameters: Option<HashMap<String, Value>>,
    /// Pending query results (nodes and relationships as JSON values)
    pending_results: Vec<Vec<Value>>,
    /// Column names for current result set
    result_columns: Vec<String>,
}

impl BoltProtocolHandler {
    /// Create a new Bolt protocol handler
    #[cfg(feature = "storage-rocksdb")]
    pub fn new(storage: Arc<CypherGraphStorage>) -> Self {
        Self {
            version: None,
            storage,
            parser: CypherParser::new(),
            authenticated: false,
            current_query: None,
            current_parameters: None,
            pending_results: Vec::new(),
            result_columns: Vec::new(),
        }
    }

    /// Create a new Bolt protocol handler (without storage)
    #[cfg(not(feature = "storage-rocksdb"))]
    pub fn new_without_storage() -> Self {
        Self {
            version: None,
            parser: CypherParser::new(),
            authenticated: false,
            current_query: None,
            current_parameters: None,
            pending_results: Vec::new(),
            result_columns: Vec::new(),
        }
    }

    /// Handle Bolt handshake
    pub async fn handle_handshake(
        &mut self,
        stream: &mut TcpStream,
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
                        self.send_failure(&mut stream, "Error", &e.to_string())
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Read a chunk from the stream
    async fn read_chunk(
        &self,
        stream: &mut TcpStream,
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
    fn decode_hello(&self, _bytes: &Bytes) -> ProtocolResult<HashMap<String, Value>> {
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
        response.insert(
            "server".to_string(),
            Value::String("orbit-rs/1.0".to_string()),
        );
        response.insert(
            "connection_id".to_string(),
            Value::String("bolt-1".to_string()),
        );

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
                    for element in &pattern.elements {
                        match element {
                            crate::protocols::cypher::cypher_parser::PatternElement::Node(node_pattern) => {
                                let node = GraphNode {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    labels: node_pattern.labels.clone(),
                                    properties: node_pattern.properties.clone(),
                                };
                                self.storage.store_node(node.clone()).await?;
                                let row = vec![self.node_to_value(&node)];
                                results.push(row);

                                if let Some(var) = &node_pattern.variable {
                                    if !columns.contains(var) {
                                        columns.push(var.clone());
                                    }
                                }
                                info!("Created node: {:?}", node.id);
                            }
                            crate::protocols::cypher::cypher_parser::PatternElement::Relationship(rel_pattern) => {
                                // Relationships need start/end nodes - for now skip
                                debug!("Relationship creation requires start/end nodes: {:?}", rel_pattern);
                            }
                        }
                    }
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Return { items } => {
                    // RETURN clause updates columns
                    for item in items {
                        if !columns.contains(&item.expression) {
                            columns.push(item.expression.clone());
                        }
                    }
                }
                crate::protocols::cypher::cypher_parser::CypherClause::Where { condition: _ } => {
                    // WHERE clause filters - would need to filter pending_results
                    // For simplicity, we handle WHERE during MATCH
                    debug!("WHERE clause processing - filtering applied during MATCH");
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
        stream: &mut TcpStream,
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
        _metadata: HashMap<String, Value>,
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
        _code: &str,
        _message: &str,
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

    /// Send RECORD message with values
    async fn send_record(&self, values: Vec<Value>, stream: &mut TcpStream) -> ProtocolResult<()> {
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
                    if i >= -16 && i <= 127 {
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
    async fn send_chunk(&self, data: &BytesMut, stream: &mut TcpStream) -> ProtocolResult<()> {
        let size = data.len() as u16;
        let mut chunk = BytesMut::with_capacity(2 + data.len());
        chunk.put_u16(size);
        chunk.put_slice(data);
        stream
            .write_all(&chunk)
            .await
            .map_err(|e| ProtocolError::Other(format!("Write error: {}", e)))?;
        Ok(())
    }
}
