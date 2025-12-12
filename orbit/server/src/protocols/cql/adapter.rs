//! CQL adapter implementation
//!
//! This module provides the main CQL adapter that handles client connections
//! and translates CQL operations to Orbit engine calls.

use super::parser::{ComparisonOperator, CqlParser, CqlStatement};
use super::protocol::{
    build_empty_rows_result, build_error_from_protocol_error, build_error_response,
    build_ready_response, build_supported_response, build_system_local_response,
    build_system_peers_v2_response, build_system_schema_aggregates_response,
    build_system_schema_columns_response, build_system_schema_functions_response,
    build_system_schema_indexes_response, build_system_schema_keyspaces_response,
    build_system_schema_tables_response, build_system_schema_triggers_response,
    build_system_schema_types_response, build_system_schema_views_response,
    build_system_virtual_schema_response, build_void_result, read_string, read_string_map,
    CqlFrame, CqlOpcode, QueryParameters,
};
use super::types::{CqlEvent, CqlEventType, CqlValue};
use super::CqlConfig;
use crate::protocols::common::storage::memory::MemoryTableStorage;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use crate::protocols::postgres_wire::QueryEngine;
use crate::protocols::tls::OrbitTlsAcceptor;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[cfg(feature = "js-quickjs")]
use crate::js::{JsValue, QuickJsRuntime, SecurityConfig};

/// Convert SqlValue to JsValue for trigger execution
#[cfg(feature = "js-quickjs")]
fn sql_value_to_js_value(value: &SqlValue) -> JsValue {
    match value {
        SqlValue::Null => JsValue::Null,
        SqlValue::Boolean(b) => JsValue::Bool(*b),
        SqlValue::SmallInt(n) => JsValue::Integer(*n as i64),
        SqlValue::Integer(n) => JsValue::Integer(*n as i64),
        SqlValue::BigInt(n) => JsValue::Integer(*n),
        SqlValue::Real(f) => JsValue::Float(*f as f64),
        SqlValue::DoublePrecision(f) => JsValue::Float(*f),
        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => JsValue::String(s.clone()),
        SqlValue::Bytea(b) => JsValue::Binary(b.clone()),
        SqlValue::Timestamp(dt) => JsValue::Date(dt.to_string()),
        SqlValue::Date(d) => JsValue::Date(d.to_string()),
        SqlValue::Time(t) => JsValue::String(t.to_string()),
        SqlValue::Interval(iv) => JsValue::String(iv.to_string()),
        SqlValue::Uuid(u) => JsValue::String(u.to_string()),
        SqlValue::Json(j) | SqlValue::Jsonb(j) => {
            // Parse JSON string to JsValue
            JsValue::from_json(j).unwrap_or(JsValue::String(j.clone()))
        }
        SqlValue::Array(arr) => {
            let js_arr: Vec<JsValue> = arr.iter().map(sql_value_to_js_value).collect();
            JsValue::Array(js_arr)
        }
        SqlValue::Numeric(n) => {
            // Convert numeric to string or float
            JsValue::String(n.to_string())
        }
        SqlValue::Decimal(d) => JsValue::String(d.to_string()),
        _ => JsValue::String(format!("{:?}", value)),
    }
}

/// CQL adapter
pub struct CqlAdapter {
    /// Configuration
    config: CqlConfig,
    /// Storage backend
    storage: Arc<dyn crate::protocols::common::storage::TableStorage>,
    /// Parser
    parser: Arc<RwLock<CqlParser>>,
    /// Query engine for executing SQL
    query_engine: Arc<QueryEngine>,
    /// Prepared statements
    prepared_statements: Arc<RwLock<HashMap<Vec<u8>, PreparedStatement>>>,
    /// Connection metrics (for production monitoring)
    #[allow(dead_code)]
    metrics: Arc<RwLock<CqlMetrics>>,
    /// Trigger registry (keyspace.table -> list of triggers)
    triggers: Arc<RwLock<HashMap<String, Vec<TriggerDefinition>>>>,
    /// Event bus for broadcasting server events
    event_bus: broadcast::Sender<CqlEvent>,
    /// Events subscribed by the current connection
    subscribed_events: Arc<RwLock<Vec<CqlEventType>>>,
}

/// CQL adapter metrics
#[derive(Debug, Default)]
pub struct CqlMetrics {
    /// Total queries executed
    pub total_queries: u64,
    /// Total errors
    pub total_errors: u64,
    /// Active connections
    pub active_connections: usize,
    /// Prepared statements count
    pub prepared_statements_count: usize,
}

/// CQL batch types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchType {
    /// LOGGED batch - atomic, all-or-nothing (default)
    Logged,
    /// UNLOGGED batch - no atomicity guarantees, better performance
    Unlogged,
    /// COUNTER batch - for counter operations only
    Counter,
}

/// Prepared statement
#[allow(dead_code)] // Fields reserved for future prepared statement implementation
struct PreparedStatement {
    /// Statement ID
    id: Vec<u8>,
    /// Query string
    query: String,
    /// Parsed statement
    statement: CqlStatement,
}

/// Trigger definition
#[derive(Debug, Clone)]
pub struct TriggerDefinition {
    /// Trigger name
    pub name: String,
    /// Table name (qualified with keyspace)
    pub table: String,
    /// Trigger class (Java class name)
    pub trigger_class: String,
    /// Whether the trigger is enabled
    pub enabled: bool,
}

/// Trigger event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
}

impl TriggerDefinition {
    /// Execute the trigger with JavaScript runtime (if available)
    pub fn execute(
        &self,
        event: TriggerEvent,
        row_data: &HashMap<String, SqlValue>,
    ) -> ProtocolResult<()> {
        info!(
            "[CQL Trigger] Executing trigger '{}' on table '{}' for {:?} event",
            self.name, self.table, event
        );

        #[cfg(feature = "js-quickjs")]
        {
            self.execute_with_javascript(event, row_data)
        }

        #[cfg(not(feature = "js-quickjs"))]
        {
            // Fallback: just log the trigger execution
            debug!(
                "[CQL Trigger] '{}' triggered with data (JS disabled): {:?}",
                self.name, row_data
            );
            Ok(())
        }
    }

    #[cfg(feature = "js-quickjs")]
    fn execute_with_javascript(
        &self,
        event: TriggerEvent,
        row_data: &HashMap<String, SqlValue>,
    ) -> ProtocolResult<()> {
        // Convert row data to JavaScript values
        let mut js_row = HashMap::new();
        for (key, value) in row_data {
            js_row.insert(key.clone(), sql_value_to_js_value(value));
        }

        // Create JavaScript context with row data and event
        let event_str = match event {
            TriggerEvent::Insert => "INSERT",
            TriggerEvent::Update => "UPDATE",
            TriggerEvent::Delete => "DELETE",
        };

        // Build the JavaScript code to execute
        // We wrap the trigger class in a function call with the data
        let script = format!(
            r#"
            var row = {};
            var event = "{}";
            var table = "{}";

            // Execute trigger class method (simplified)
            // In a real implementation, this would instantiate the Java class
            // For now, we execute it as a JavaScript function
            (function() {{
                // Trigger implementation would go here
                // For now, just log
                return {{ success: true, row: row, event: event }};
            }})();
            "#,
            serde_json::to_string(&js_row).unwrap_or_else(|_| "{}".to_string()),
            event_str,
            self.table
        );

        // Execute with QuickJS runtime
        let runtime = QuickJsRuntime::with_config(SecurityConfig::default())
            .map_err(|e| ProtocolError::CqlError(format!("Failed to create JS runtime: {}", e)))?;

        match runtime.execute(&script) {
            Ok(result) => {
                debug!(
                    "[CQL Trigger] '{}' executed successfully: {:?}",
                    self.name, result
                );
                Ok(())
            }
            Err(e) => {
                error!("[CQL Trigger] '{}' execution failed: {}", self.name, e);
                Err(ProtocolError::CqlError(format!(
                    "Trigger execution failed: {}",
                    e
                )))
            }
        }
    }
}

impl CqlAdapter {
    /// Create a new CQL adapter with shared storage
    pub async fn new_with_storage(
        config: CqlConfig,
        storage: Arc<dyn crate::protocols::common::storage::TableStorage>,
    ) -> ProtocolResult<Self> {
        let query_engine = Arc::new(QueryEngine::new());
        let (event_bus, _) = broadcast::channel(100);

        Ok(Self {
            config,
            storage,
            parser: Arc::new(RwLock::new(CqlParser::new())),
            query_engine,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(CqlMetrics::default())),
            triggers: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            subscribed_events: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create a new CQL adapter (creates its own isolated storage)
    /// For backward compatibility. Use new_with_storage() to share storage with other protocols.
    pub async fn new(config: CqlConfig) -> ProtocolResult<Self> {
        let storage = Arc::new(MemoryTableStorage::new());
        Self::new_with_storage(config, storage).await
    }

    /// Create a new CQL adapter with a specific query engine
    ///
    /// This is primarily for testing to allow sharing a query engine between
    /// adapter and test setup code.
    pub async fn with_query_engine(
        config: CqlConfig,
        query_engine: Arc<QueryEngine>,
    ) -> ProtocolResult<Self> {
        let storage = Arc::new(MemoryTableStorage::new());
        let (event_bus, _) = broadcast::channel(100);

        Ok(Self {
            config,
            storage,
            parser: Arc::new(RwLock::new(CqlParser::new())),
            query_engine,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(CqlMetrics::default())),
            triggers: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            subscribed_events: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Get access to the query engine (for testing)
    #[cfg(test)]
    #[allow(dead_code)] // Used in integration tests
    pub(crate) fn query_engine(&self) -> &Arc<QueryEngine> {
        &self.query_engine
    }

    /// Execute SQL directly using the internal SQL engine (for testing/setup)
    /// This uses QueryEngine's comprehensive SQL engine which can handle CREATE TABLE
    pub async fn execute_sql(
        &self,
        sql: &str,
    ) -> ProtocolResult<crate::protocols::postgres_wire::sql::UnifiedExecutionResult> {
        // Use QueryEngine's execute_sql_direct which bypasses persistent storage checks
        // and uses ConfigurableSqlEngine directly
        let result = self.query_engine.execute_sql_direct(sql).await?;

        // Convert QueryResult to UnifiedExecutionResult
        match result {
            crate::protocols::postgres_wire::QueryResult::Select { columns, rows } => {
                let row_count = rows.len();
                Ok(
                    crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                        columns,
                        rows,
                        row_count,
                        transaction_id: None,
                    },
                )
            }
            crate::protocols::postgres_wire::QueryResult::Insert { count } => Ok(
                crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Insert {
                    count,
                    transaction_id: None,
                },
            ),
            crate::protocols::postgres_wire::QueryResult::Update { count } => Ok(
                crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Update {
                    count,
                    transaction_id: None,
                },
            ),
            crate::protocols::postgres_wire::QueryResult::Delete { count } => Ok(
                crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Delete {
                    count,
                    transaction_id: None,
                },
            ),
            crate::protocols::postgres_wire::QueryResult::Set { variable, value } => Ok(
                crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Set {
                    variable,
                    value,
                    transaction_id: None,
                },
            ),
            crate::protocols::postgres_wire::QueryResult::Merge {
                count,
                rows,
                columns,
            } => Ok(
                crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Merge {
                    count,
                    rows,
                    columns,
                    transaction_id: None,
                },
            ),
        }
    }

    /// Convert a CqlValue to a SQL string representation for INSERT statements
    fn cql_value_to_sql_string(v: &CqlValue) -> String {
        match v {
            CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
            CqlValue::Int(i) => i.to_string(),
            CqlValue::Bigint(i) => i.to_string(),
            CqlValue::Smallint(i) => i.to_string(),
            CqlValue::Tinyint(i) => i.to_string(),
            CqlValue::Boolean(b) => b.to_string(),
            CqlValue::Float(f) => f.to_string(),
            CqlValue::Double(f) => f.to_string(),
            CqlValue::Timestamp(ts) => (ts / 1000).to_string(),
            CqlValue::Uuid(s) => format!("'{}'", s),
            CqlValue::Null => "NULL".to_string(),
            CqlValue::Set(items) => {
                // Store as JSON string
                let json = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
                format!("'{}'", json.replace('\'', "''"))
            }
            CqlValue::List(items) => {
                // Store as JSON string
                let json = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
                format!("'{}'", json.replace('\'', "''"))
            }
            CqlValue::Map(entries) => {
                // Store as JSON string
                let json = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
                format!("'{}'", json.replace('\'', "''"))
            }
            CqlValue::Tuple(items) => {
                // Store as JSON string
                let json = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
                format!("'{}'", json.replace('\'', "''"))
            }
            CqlValue::Vector(values) => {
                // Store as JSON array of floats
                let json = serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string());
                format!("'{}'", json.replace('\'', "''"))
            }
        }
    }

    /// Start the CQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        self.start_with_tls(None).await
    }

    /// Start the CQL server with optional TLS
    pub async fn start_with_tls(
        &self,
        tls_acceptor: Option<OrbitTlsAcceptor>,
    ) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.config.listen_addr)
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        info!("[CQL] Server listening on {}", self.config.listen_addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("[CQL] New connection from {}", addr);
                    let adapter = self.clone_for_connection();
                    let tls_acceptor = tls_acceptor.clone();

                    tokio::spawn(async move {
                        if let Some(acceptor) = tls_acceptor {
                            match acceptor.accept(socket).await {
                                Ok(tls_stream) => {
                                    if let Err(e) = adapter.handle_connection(tls_stream).await {
                                        error!("[CQL] Connection error from {}: {:?}", addr, e);
                                    }
                                }
                                Err(e) => {
                                    error!("[CQL] TLS handshake failed: {}", e);
                                }
                            }
                        } else {
                            if let Err(e) = adapter.handle_connection(socket).await {
                                error!("[CQL] Connection error from {}: {:?}", addr, e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("[CQL] Accept error: {}", e);
                }
            }
        }
    }

    /// Clone adapter for a new connection
    fn clone_for_connection(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: self.storage.clone(),
            parser: Arc::new(RwLock::new(CqlParser::new())), // New parser for each connection
            query_engine: self.query_engine.clone(),
            prepared_statements: self.prepared_statements.clone(),
            metrics: self.metrics.clone(),
            triggers: self.triggers.clone(),
            event_bus: self.event_bus.clone(),
            subscribed_events: Arc::new(RwLock::new(Vec::new())), // New subscriptions for new connection
        }
    }

    /// Handle a client connection
    async fn handle_connection<S>(&self, socket: S) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (mut reader, mut writer) = tokio::io::split(socket);
        let mut buffer = BytesMut::with_capacity(4096);
        let mut event_rx = self.event_bus.subscribe();

        loop {
            tokio::select! {
                // Handle incoming data
                read_result = reader.read_buf(&mut buffer) => {
                    let n = read_result.map_err(|e| ProtocolError::IoError(e.to_string()))?;
                    if n == 0 {
                        // Connection closed
                        return Ok(());
                    }

                    // Process all complete frames in the buffer
                    loop {
                        // Check if we have enough data for a frame header
                        if buffer.len() < 9 {
                            break;
                        }

                        // Check if we have the full frame
                        let body_len = {
                            let mut buf = buffer.as_ref();
                            buf.advance(5); // Skip to length field
                            buf.get_u32() as usize
                        };

                        if buffer.len() < 9 + body_len {
                            break;
                        }

                        // Parse frame
                        let frame_bytes = buffer.split_to(9 + body_len).freeze();
                        let frame = CqlFrame::decode(frame_bytes)?;

                        // Extract protocol version (lower 7 bits, bit 7 is direction flag)
                        let protocol_version = frame.version & 0x7F;

                        // Handle frame
                        let mut response = self.handle_frame(&frame).await?;

                        // Set response version: use protocol v4 with response bit (0x84)
                        let response_version = if protocol_version > 4 {
                            0x84 // v4 response
                        } else {
                            (protocol_version & 0x7F) | 0x80
                        };
                        response.version = response_version;

                        // Send response
                        let response_bytes = response.encode();
                        writer
                            .write_all(&response_bytes)
                            .await
                            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

                        writer
                            .flush()
                            .await
                            .map_err(|e| ProtocolError::IoError(e.to_string()))?;
                    }
                }

                // Handle server events
                event_result = event_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                             let subscribed = self.subscribed_events.read().await;
                             let event_type = match &event {
                                 CqlEvent::TopologyChange(_, _) => CqlEventType::TopologyChange,
                                 CqlEvent::StatusChange(_, _) => CqlEventType::StatusChange,
                                 CqlEvent::SchemaChange(_, _, _, _) => CqlEventType::SchemaChange,
                             };

                             if subscribed.contains(&event_type) {
                                 if let Ok(response) = super::protocol::build_event_response(-1, event) {
                                     let response_bytes = response.encode();
                                     if let Err(e) = writer.write_all(&response_bytes).await {
                                         error!("Failed to send event to client: {}", e);
                                         break Ok(()); // Connection error
                                     }
                                     let _ = writer.flush().await;
                                 }
                             }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break Ok(()); // Bus closed
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("Client lagged, skipped {} events", skipped);
                        }
                    }
                }
            }
        }
    }

    /// Execute triggers for a table
    #[allow(dead_code)]
    async fn execute_triggers(
        &self,
        table: &str,
        event: TriggerEvent,
        row_data: &HashMap<String, SqlValue>,
    ) -> ProtocolResult<()> {
        let triggers = self.triggers.read().await;
        if let Some(table_triggers) = triggers.get(table) {
            for trigger in table_triggers {
                if trigger.enabled {
                    trigger.execute(event, row_data)?;
                }
            }
        }
        Ok(())
    }

    /// Handle a CQL frame
    async fn handle_frame(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let result = match frame.opcode {
            CqlOpcode::Startup => self.handle_startup(frame).await,
            CqlOpcode::Options => self.handle_options(frame).await,
            CqlOpcode::Query => self.handle_query(frame).await,
            CqlOpcode::Prepare => self.handle_prepare(frame).await,
            CqlOpcode::Execute => self.handle_execute(frame).await,
            CqlOpcode::Batch => self.handle_batch(frame).await,
            CqlOpcode::Register => self.handle_register(frame).await,
            CqlOpcode::AuthResponse => self.handle_auth_response(frame).await,
            _ => {
                warn!("Unsupported CQL opcode: {:?}", frame.opcode);
                Ok(build_error_response(
                    frame.stream,
                    super::protocol::error_codes::PROTOCOL_ERROR,
                    &format!("Unsupported opcode: {:?}", frame.opcode),
                ))
            }
        };

        // Convert ProtocolError to CQL error frame
        match result {
            Ok(frame) => Ok(frame),
            Err(e) => Ok(build_error_from_protocol_error(frame.stream, &e)),
        }
    }

    /// Handle STARTUP request
    async fn handle_startup(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let mut body = frame.body.clone();
        let _options = read_string_map(&mut body)?;

        // If authentication is enabled, send AUTHENTICATE
        if self.config.authentication_enabled {
            let mut response_body = BytesMut::new();
            response_body.put_u16(11); // Length of "PasswordAuthenticator"
            response_body.put(&b"PasswordAuthenticator"[..]);
            Ok(CqlFrame::response(
                frame.stream,
                CqlOpcode::Authenticate,
                response_body.freeze(),
            ))
        } else {
            Ok(build_ready_response(frame.stream))
        }
    }

    /// Handle REGISTER request
    async fn handle_register(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let body = frame.body.clone();
        let message = super::protocol::RegisterMessage::decode(body)?;

        info!("Client registered for events: {:?}", message.event_types);

        let mut subscribed = self.subscribed_events.write().await;
        subscribed.clear();
        for event_type_str in &message.event_types {
            if let Some(event_type) = CqlEventType::from_str(event_type_str) {
                subscribed.push(event_type);
            }
        }

        Ok(build_ready_response(frame.stream))
    }

    /// Handle OPTIONS request
    async fn handle_options(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        Ok(build_supported_response(frame.stream))
    }

    /// Handle AUTH_RESPONSE request (authentication)
    async fn handle_auth_response(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        if !self.config.authentication_enabled {
            // Authentication not enabled, but client sent auth response
            return Ok(build_error_response(
                frame.stream,
                super::protocol::error_codes::PROTOCOL_ERROR,
                "Authentication not enabled",
            ));
        }

        let mut body = frame.body.clone();

        // Read token (password)
        let token_len = body.get_u32();
        let token_bytes = body.copy_to_bytes(token_len as usize);
        let password = String::from_utf8(token_bytes.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        // Verify password
        let password_valid = if let Some(expected_password) = &self.config.password {
            // Simple password verification (in production, use hashing)
            password == *expected_password
        } else {
            // No password configured, accept any password (for testing)
            true
        };

        if !password_valid {
            return Ok(build_error_response(
                frame.stream,
                super::protocol::error_codes::BAD_CREDENTIALS,
                "Invalid credentials",
            ));
        }

        // Send AUTH_SUCCESS
        let mut response_body = BytesMut::new();
        response_body.put_u32(0); // Empty token (success)
        Ok(CqlFrame::response(
            frame.stream,
            CqlOpcode::AuthSuccess,
            response_body.freeze(),
        ))
    }

    /// Handle QUERY request
    async fn handle_query(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        println!("DEBUG: handle_query called. Body len: {}", frame.body.len());
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_queries += 1;
        }

        let mut body = frame.body.clone();

        // Read query string
        if body.remaining() < 4 {
            return Err(ProtocolError::IncompleteFrame);
        }
        let query_len = body.get_u32();
        if body.remaining() < query_len as usize {
            return Err(ProtocolError::IncompleteFrame);
        }
        let query_bytes = body.copy_to_bytes(query_len as usize);
        let query = String::from_utf8(query_bytes.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;
        println!("DEBUG: Received query: {}", query);

        // Read query parameters
        let params = QueryParameters::decode(body)?;

        // Parse and execute query
        let parser = self.parser.read().await;
        let statement = parser.parse(&query)?;
        drop(parser);

        // Execute statement
        self.execute_statement(
            &statement,
            frame.stream,
            params.page_size,
            params.paging_state,
        )
        .await
    }

    /// Handle PREPARE request
    async fn handle_prepare(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let mut body = frame.body.clone();

        // Read query string
        let query = read_string(&mut body)?;

        // Parse statement
        let parser = self.parser.read().await;
        let statement = parser.parse(&query)?;
        drop(parser);

        // Generate statement ID (hash of query)
        let id = md5::compute(query.as_bytes()).0.to_vec();

        // Store prepared statement
        let statement_clone = statement.clone();
        let prepared = PreparedStatement {
            id: id.clone(),
            query: query.clone(),
            statement: statement_clone.clone(),
        };
        self.prepared_statements
            .write()
            .await
            .insert(id.clone(), prepared);

        // Build PREPARED response
        let mut response_body = BytesMut::new();
        response_body.put_i32(0x0004); // RESULT::Prepared
        response_body.put_u16(id.len() as u16);
        response_body.put(&id[..]);

        // Add metadata based on statement type
        match &statement_clone {
            CqlStatement::Select { columns, .. } => {
                // Metadata flags (0x0001 = global tables spec, 0x0002 = has more pages)
                response_body.put_i32(0x0001);

                // Column count
                let col_count = if columns.contains(&"*".to_string()) {
                    // For SELECT *, we don't know column count yet - use 0
                    0
                } else {
                    columns.len() as i32
                };
                response_body.put_i32(col_count);

                // Column metadata (only if we know the columns)
                if !columns.contains(&"*".to_string()) {
                    for col_name in columns {
                        // Keyspace name (empty)
                        response_body.put_u16(0);
                        // Table name (empty - we don't track this yet)
                        response_body.put_u16(0);
                        // Column name
                        response_body.put_u16(col_name.len() as u16);
                        response_body.put(col_name.as_bytes());
                        // Column type (0x0003 = VARCHAR/TEXT - simplified)
                        response_body.put_i32(0x0003);
                    }
                }

                // Partition key indices (empty for now)
                response_body.put_i16(0);
            }
            CqlStatement::Insert { columns, .. } => {
                // For INSERT, metadata describes bound variables
                response_body.put_i32(0x0001); // Flags
                response_body.put_i32(columns.len() as i32); // Variable count

                // Variable metadata (bound variables)
                for col_name in columns {
                    response_body.put_u16(0); // Keyspace
                    response_body.put_u16(0); // Table
                    response_body.put_u16(col_name.len() as u16);
                    response_body.put(col_name.as_bytes());
                    response_body.put_i32(0x0003); // Type (VARCHAR)
                }
            }
            _ => {
                // For other statements, minimal metadata
                response_body.put_i32(0x0001); // Flags
                response_body.put_i32(0); // Column count
            }
        }

        Ok(CqlFrame::response(
            frame.stream,
            CqlOpcode::Result,
            response_body.freeze(),
        ))
    }

    /// Handle EXECUTE request
    async fn handle_execute(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let mut body = frame.body.clone();

        // Read statement ID
        let id_len = body.get_u16();
        let id = body.copy_to_bytes(id_len as usize).to_vec();

        // Read query parameters
        let params = QueryParameters::decode(body)?;

        // Get prepared statement
        let prepared_statements = self.prepared_statements.read().await;
        let prepared = prepared_statements.get(&id).ok_or_else(|| {
            ProtocolError::InvalidStatement("Prepared statement not found".to_string())
        })?;

        // Execute statement
        self.execute_statement(
            &prepared.statement,
            frame.stream,
            params.page_size,
            params.paging_state,
        )
        .await
    }

    /// Handle BATCH request
    ///
    /// CQL BATCH format (protocol v4):
    /// - type: 1 byte (0=LOGGED, 1=UNLOGGED, 2=COUNTER)
    /// - n: 2 bytes (number of statements)
    /// - For each statement:
    ///   - kind: 1 byte (0=query string, 1=prepared statement ID)
    ///   - query/id: [int][bytes] for string, [short][bytes] for prepared
    ///   - n_values: 2 bytes
    ///   - For each value: [int][bytes] (-1 for NULL, -2 for NOT SET)
    /// - consistency: 2 bytes
    /// - flags: 1 byte (v4) or 4 bytes (v5)
    /// - Optional based on flags: serial_consistency, timestamp, keyspace, now_in_seconds
    async fn handle_batch(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let mut body = frame.body.clone();

        if body.remaining() < 1 {
            return Err(ProtocolError::IncompleteFrame);
        }

        // Read batch type (1 byte: 0=LOGGED, 1=UNLOGGED, 2=COUNTER)
        let batch_type_byte = body.get_u8();
        let batch_type = match batch_type_byte {
            0 => BatchType::Logged,
            1 => BatchType::Unlogged,
            2 => BatchType::Counter,
            _ => BatchType::Logged, // Default
        };

        if body.remaining() < 2 {
            return Err(ProtocolError::IncompleteFrame);
        }

        // Read number of statements
        let statement_count = body.get_u16();

        // Parse all statements first
        let mut batch_statements = Vec::with_capacity(statement_count as usize);

        for _ in 0..statement_count {
            if body.remaining() < 1 {
                return Err(ProtocolError::IncompleteFrame);
            }

            // Read statement kind (1 byte: 0=query string, 1=prepared statement ID)
            let kind = body.get_u8();

            let statement = if kind == 0 {
                // Query string (long string: 4 bytes length)
                if body.remaining() < 4 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                let query_len = body.get_i32();
                if query_len < 0 || body.remaining() < query_len as usize {
                    return Err(ProtocolError::IncompleteFrame);
                }
                let query_bytes = body.copy_to_bytes(query_len as usize);
                let query = String::from_utf8(query_bytes.to_vec())
                    .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

                // Parse the query
                let parser = self.parser.read().await;
                let parsed = parser.parse(&query)?;
                drop(parser);
                parsed
            } else {
                // Prepared statement ID (short bytes: 2 bytes length)
                if body.remaining() < 2 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                let id_len = body.get_u16();
                if body.remaining() < id_len as usize {
                    return Err(ProtocolError::IncompleteFrame);
                }
                let id = body.copy_to_bytes(id_len as usize).to_vec();

                // Get prepared statement
                let prepared_statements = self.prepared_statements.read().await;
                let prepared = prepared_statements.get(&id).ok_or_else(|| {
                    ProtocolError::InvalidStatement("Prepared statement not found".to_string())
                })?;
                prepared.statement.clone()
            };

            // Read values for this statement
            if body.remaining() < 2 {
                return Err(ProtocolError::IncompleteFrame);
            }
            let n_values = body.get_u16();

            // Read and skip values (we don't use them yet, but must consume them)
            for _ in 0..n_values {
                if body.remaining() < 4 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                let val_len = body.get_i32();
                if val_len > 0 {
                    if body.remaining() < val_len as usize {
                        return Err(ProtocolError::IncompleteFrame);
                    }
                    body.advance(val_len as usize);
                }
                // val_len == -1 is NULL, val_len == -2 is NOT_SET (v4+)
            }

            batch_statements.push(statement);
        }

        // Read consistency level
        if body.remaining() < 2 {
            return Err(ProtocolError::IncompleteFrame);
        }
        let _consistency = body.get_u16();

        // Read batch flags (1 byte in v4)
        let _flags = if body.remaining() >= 1 {
            body.get_u8()
        } else {
            0
        };

        // Skip optional fields based on flags
        // 0x10 = serial consistency, 0x20 = default timestamp, etc.

        // Execute all statements
        // For LOGGED batches, we should use transactions, but for now execute sequentially
        let mut errors = Vec::new();
        let mut success_count = 0;

        for statement in &batch_statements {
            match self
                .execute_statement(statement, frame.stream, None, None)
                .await
            {
                Ok(_) => {
                    success_count += 1;
                }
                Err(e) => {
                    // For LOGGED batches, we should roll back on error
                    // For now, just collect errors
                    errors.push(format!("Statement error: {}", e));

                    // For strict atomicity, break on first error in LOGGED batch
                    if matches!(batch_type, BatchType::Logged) {
                        break;
                    }
                }
            }
        }

        // If there were errors, return error response
        if !errors.is_empty() {
            return Ok(build_error_response(
                frame.stream,
                super::protocol::error_codes::INVALID,
                &format!(
                    "Batch execution failed: {} of {} succeeded. Errors: {}",
                    success_count,
                    batch_statements.len(),
                    errors.join("; ")
                ),
            ));
        }

        // Return VOID result for successful batch
        Ok(build_void_result(frame.stream))
    }

    /// Execute a CQL statement
    #[cfg_attr(test, allow(dead_code))]
    /// Publish a schema change event
    fn publish_schema_change_event(
        &self,
        change_type: super::types::SchemaChangeType,
        target_type: &str,
        keyspace: &str,
        name: &str,
    ) {
        let event = CqlEvent::SchemaChange(
            change_type,
            keyspace.to_string(),
            name.to_string(),
            target_type.to_string(),
        );
        // We ignore errors if there are no subscribers
        let _ = self.event_bus.send(event);
    }

    pub async fn execute_statement(
        &self,
        statement: &CqlStatement,
        stream: i16,
        page_size: Option<i32>,
        paging_state: Option<Bytes>,
    ) -> ProtocolResult<CqlFrame> {
        match statement {
            CqlStatement::Select {
                columns,
                table,
                where_clause,
                limit,
                ..
            } => {
                println!("DEBUG: execute_statement SELECT table={}", table);
                // Handle system tables (required for driver initialization)
                let table_lower = table.to_lowercase();
                println!("DEBUG: table_lower={}", table_lower);
                if table_lower == "system.local" || table_lower == "local" {
                    return Ok(build_system_local_response(stream));
                }
                if table_lower == "system.peers"
                    || table_lower == "peers"
                    || table_lower == "system.peers_v2"
                    || table_lower == "peers_v2"
                {
                    return Ok(build_system_peers_v2_response(stream));
                }
                // Handle system_schema tables (required for driver initialization)
                if table_lower == "system_schema.keyspaces" || table_lower == "keyspaces" {
                    return Ok(build_system_schema_keyspaces_response(stream));
                }
                if table_lower == "system_schema.tables" || table_lower == "tables" {
                    return Ok(build_system_schema_tables_response(stream));
                }
                if table_lower == "system_schema.columns" || table_lower == "columns" {
                    return Ok(build_system_schema_columns_response(stream));
                }
                if table_lower == "system_schema.types" || table_lower == "types" {
                    return Ok(build_system_schema_types_response(stream));
                }
                if table_lower == "system_schema.functions" || table_lower == "functions" {
                    return Ok(build_system_schema_functions_response(stream));
                }
                if table_lower == "system_schema.aggregates" || table_lower == "aggregates" {
                    return Ok(build_system_schema_aggregates_response(stream));
                }
                if table_lower == "system_schema.views" || table_lower == "views" {
                    return Ok(build_system_schema_views_response(stream));
                }
                if table_lower == "system_schema.indexes" || table_lower == "indexes" {
                    return Ok(build_system_schema_indexes_response(stream));
                }
                // Handle system_schema.triggers (required for driver initialization)
                if table_lower == "system_schema.triggers" || table_lower == "triggers" {
                    return Ok(build_system_schema_triggers_response(stream));
                }
                // Handle system_virtual_schema tables (required for driver initialization)
                if table_lower.starts_with("system_virtual_schema.") {
                    return Ok(build_system_virtual_schema_response(stream, &table_lower));
                }
                // Skip other system_schema tables with empty results
                if table_lower.starts_with("system_schema.") || table_lower.starts_with("system.") {
                    return Ok(build_empty_rows_result(stream));
                }

                // Convert CQL SELECT to SQL and execute
                let sql_columns = if columns.contains(&"*".to_string()) {
                    "*".to_string()
                } else {
                    columns.join(", ")
                };

                // Qualify table name with keyspace if not already qualified
                let qualified_table = if table.contains('.') {
                    table.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, table)
                    } else {
                        table.clone()
                    }
                };

                let mut sql = format!("SELECT {} FROM {}", sql_columns, qualified_table);

                // Add WHERE clause
                if let Some(conditions) = where_clause {
                    if !conditions.is_empty() {
                        sql.push_str(" WHERE ");
                        let where_parts: Vec<String> = conditions
                            .iter()
                            .map(|cond| {
                                let op_str = match cond.operator {
                                    ComparisonOperator::Equal => "=",
                                    ComparisonOperator::GreaterThan => ">",
                                    ComparisonOperator::GreaterThanOrEqual => ">=",
                                    ComparisonOperator::LessThan => "<",
                                    ComparisonOperator::LessThanOrEqual => "<=",
                                    ComparisonOperator::NotEqual => "!=",
                                    ComparisonOperator::In => "IN",
                                    ComparisonOperator::Contains => "CONTAINS",
                                    ComparisonOperator::ContainsKey => "CONTAINS KEY",
                                    ComparisonOperator::Like => "LIKE",
                                    ComparisonOperator::Token => "TOKEN",
                                };
                                let val_str = match &cond.value {
                                    CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                                    CqlValue::Int(i) => i.to_string(),
                                    CqlValue::Bigint(i) => i.to_string(),
                                    CqlValue::Boolean(b) => b.to_string(),
                                    CqlValue::Float(f) => f.to_string(),
                                    CqlValue::Double(f) => f.to_string(),
                                    CqlValue::Timestamp(ts) => (ts / 1000).to_string(),
                                    CqlValue::Null => "NULL".to_string(),
                                    CqlValue::List(values)
                                        if cond.operator == ComparisonOperator::In =>
                                    {
                                        // Format IN operator with proper parentheses
                                        let formatted_values: Vec<String> = values
                                            .iter()
                                            .map(|v| match v {
                                                CqlValue::Text(s) => {
                                                    format!("'{}'", s.replace('\'', "''"))
                                                }
                                                CqlValue::Int(i) => i.to_string(),
                                                CqlValue::Bigint(i) => i.to_string(),
                                                CqlValue::Boolean(b) => b.to_string(),
                                                CqlValue::Float(f) => f.to_string(),
                                                CqlValue::Double(f) => f.to_string(),
                                                _ => format!("'{:?}'", v),
                                            })
                                            .collect();
                                        format!("({})", formatted_values.join(", "))
                                    }
                                    _ => format!("'{:?}'", cond.value),
                                };
                                format!("{} {} {}", cond.column, op_str, val_str)
                            })
                            .collect();
                        sql.push_str(&where_parts.join(" AND "));
                    }
                }

                if let Some(lim) = limit {
                    sql.push_str(&format!(" LIMIT {}", lim));
                }

                // Execute using query engine's direct SQL execution (bypasses persistent storage check)
                // This ensures we use the same storage backend as setup operations
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(result) => {
                        // Convert QueryResult to CQL format
                        match result {
                            crate::protocols::postgres_wire::QueryResult::Select {
                                columns,
                                rows,
                            } => {
                                // Convert Vec<Vec<Option<String>>> to Vec<HashMap<String, SqlValue>>
                                let cql_rows: Vec<HashMap<String, SqlValue>> = rows
                                    .into_iter()
                                    .map(|row| {
                                        let mut map = HashMap::new();
                                        for (i, col) in columns.iter().enumerate() {
                                            if let Some(val_str) =
                                                &row.get(i).and_then(|v| v.as_ref())
                                            {
                                                // Try to parse as appropriate type
                                                let sql_val = if let Ok(int_val) =
                                                    val_str.parse::<i32>()
                                                {
                                                    SqlValue::Integer(int_val)
                                                } else if let Ok(bigint_val) =
                                                    val_str.parse::<i64>()
                                                {
                                                    SqlValue::BigInt(bigint_val)
                                                } else if let Ok(bool_val) = val_str.parse::<bool>()
                                                {
                                                    SqlValue::Boolean(bool_val)
                                                } else if let Ok(float_val) = val_str.parse::<f64>()
                                                {
                                                    SqlValue::DoublePrecision(float_val)
                                                } else {
                                                    SqlValue::Text(val_str.to_string())
                                                };
                                                map.insert(col.clone(), sql_val);
                                            } else {
                                                map.insert(col.clone(), SqlValue::Null);
                                            }
                                        }
                                        map
                                    })
                                    .collect();
                                Ok(self.build_rows_result(
                                    stream,
                                    cql_rows,
                                    columns,
                                    page_size,
                                    paging_state,
                                ))
                            }
                            _ => Ok(self.build_rows_result(stream, vec![], vec![], None, None)),
                        }
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::Insert {
                table,
                columns,
                values,
                if_not_exists,
                ..
            } => {
                // Qualify table name with keyspace if not already qualified
                let qualified_table = if table.contains('.') {
                    table.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, table)
                    } else {
                        table.clone()
                    }
                };

                // Handle IF NOT EXISTS (lightweight transaction)
                if *if_not_exists && !columns.is_empty() && !values.is_empty() {
                    // Build WHERE clause from primary key columns (assume first column is PK for simplicity)
                    // In a full implementation, we'd need to know the table schema to identify PK columns
                    let pk_col = &columns[0];
                    let pk_val = Self::cql_value_to_sql_string(&values[0]);
                    let where_clause = format!("{} = {}", pk_col, pk_val);

                    // Check if row already exists
                    if let Ok(Some(existing_row)) =
                        self.check_row_exists(&qualified_table, &where_clause).await
                    {
                        // Row exists, return [applied] = false with current values
                        return Ok(self.build_lwt_result(
                            stream,
                            false,
                            Some(existing_row),
                            columns,
                        ));
                    }
                }

                // Convert CQL INSERT to SQL and execute
                let col_str = if columns.is_empty() {
                    "".to_string()
                } else {
                    format!("({})", columns.join(", "))
                };

                let val_str = if values.is_empty() {
                    "".to_string()
                } else {
                    let val_parts: Vec<String> =
                        values.iter().map(Self::cql_value_to_sql_string).collect();
                    format!(" VALUES ({})", val_parts.join(", "))
                };

                let sql = format!("INSERT INTO {}{}{}", qualified_table, col_str, val_str);

                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        // If this was an IF NOT EXISTS, return [applied] = true
                        if *if_not_exists {
                            Ok(self.build_lwt_result(stream, true, None, columns))
                        } else {
                            Ok(build_void_result(stream))
                        }
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::Update {
                table,
                assignments,
                counter_assignments,
                where_clause,
                if_clause,
                ..
            } => {
                // Qualify table name with keyspace if not already qualified
                let qualified_table = if table.contains('.') {
                    table.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, table)
                    } else {
                        table.clone()
                    }
                };

                // Build WHERE clause string for row lookup
                let where_str = if !where_clause.is_empty() {
                    let where_parts: Vec<String> = where_clause
                        .iter()
                        .map(|cond| {
                            let op_str = match cond.operator {
                                ComparisonOperator::Equal => "=",
                                ComparisonOperator::GreaterThan => ">",
                                ComparisonOperator::GreaterThanOrEqual => ">=",
                                ComparisonOperator::LessThan => "<",
                                ComparisonOperator::LessThanOrEqual => "<=",
                                ComparisonOperator::NotEqual => "!=",
                                _ => "=",
                            };
                            let val_str = match &cond.value {
                                CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                                CqlValue::Int(i) => i.to_string(),
                                CqlValue::Bigint(i) => i.to_string(),
                                CqlValue::Boolean(b) => b.to_string(),
                                CqlValue::Float(f) => f.to_string(),
                                CqlValue::Double(f) => f.to_string(),
                                CqlValue::Timestamp(ts) => (ts / 1000).to_string(),
                                CqlValue::Null => "NULL".to_string(),
                                _ => format!("'{:?}'", cond.value),
                            };
                            format!("{} {} {}", cond.column, op_str, val_str)
                        })
                        .collect();
                    where_parts.join(" AND ")
                } else {
                    String::new()
                };

                // Handle IF clause (lightweight transaction)
                let column_names: Vec<String> = assignments.keys().cloned().collect();
                if let Some(if_conditions) = if_clause {
                    if !where_str.is_empty() {
                        // Fetch current row
                        if let Ok(Some(existing_row)) =
                            self.check_row_exists(&qualified_table, &where_str).await
                        {
                            // Evaluate IF conditions
                            if !self.evaluate_if_conditions(&existing_row, if_conditions) {
                                // Conditions not met, return [applied] = false with current values
                                return Ok(self.build_lwt_result(
                                    stream,
                                    false,
                                    Some(existing_row),
                                    &column_names,
                                ));
                            }
                        } else {
                            // Row doesn't exist, return [applied] = false
                            return Ok(self.build_lwt_result(stream, false, None, &column_names));
                        }
                    }
                }

                // Convert CQL UPDATE to SQL and execute
                let mut set_parts: Vec<String> = Vec::new();

                // Add simple value assignments
                for (col, val) in assignments.iter() {
                    let val_str = match val {
                        CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                        CqlValue::Int(i) => i.to_string(),
                        CqlValue::Bigint(i) => i.to_string(),
                        CqlValue::Boolean(b) => b.to_string(),
                        CqlValue::Float(f) => f.to_string(),
                        CqlValue::Double(f) => f.to_string(),
                        CqlValue::Timestamp(ts) => (ts / 1000).to_string(),
                        CqlValue::Null => "NULL".to_string(),
                        _ => format!("'{:?}'", val),
                    };
                    set_parts.push(format!("{} = {}", col, val_str));
                }

                // Add counter and collection assignments
                for (col, assignment) in counter_assignments.iter() {
                    use crate::protocols::cql::parser::CqlAssignment;
                    let assignment_str = match assignment {
                        CqlAssignment::CounterIncrement(inc) => {
                            // For counter increment: column = column + value
                            format!("{} = {} + {}", col, col, inc)
                        }
                        CqlAssignment::CounterDecrement(dec) => {
                            // For counter decrement: column = column - value
                            format!("{} = {} - {}", col, col, dec)
                        }
                        CqlAssignment::ListAppend(values) => {
                            // For list append, serialize the values as JSON and use concatenation
                            let json_vals = serde_json::to_string(values).unwrap_or_default();
                            format!("{} = {} || '{}'", col, col, json_vals)
                        }
                        CqlAssignment::ListPrepend(values) => {
                            let json_vals = serde_json::to_string(values).unwrap_or_default();
                            format!("{} = '{}' || {}", col, json_vals, col)
                        }
                        CqlAssignment::SetAdd(values) => {
                            // Set union operation
                            let json_vals = serde_json::to_string(values).unwrap_or_default();
                            format!("{} = {} || '{}'", col, col, json_vals)
                        }
                        CqlAssignment::SetRemove(values) => {
                            // Set difference (not directly supported in SQL, use JSON functions)
                            let json_vals = serde_json::to_string(values).unwrap_or_default();
                            format!("{} = {} - '{}'", col, col, json_vals)
                        }
                        CqlAssignment::MapPut(entries) => {
                            let json_map = serde_json::to_string(entries).unwrap_or_default();
                            format!("{} = {} || '{}'", col, col, json_map)
                        }
                        CqlAssignment::MapRemove(key) => {
                            let key_str = serde_json::to_string(key).unwrap_or_default();
                            format!("{} = {} - '{}'", col, col, key_str)
                        }
                        CqlAssignment::Value(val) => {
                            let val_str = match val {
                                CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                                CqlValue::Int(i) => i.to_string(),
                                CqlValue::Bigint(i) => i.to_string(),
                                _ => format!("'{:?}'", val),
                            };
                            format!("{} = {}", col, val_str)
                        }
                    };
                    set_parts.push(assignment_str);
                }

                let mut sql = format!("UPDATE {} SET {}", qualified_table, set_parts.join(", "));

                if !where_str.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_str);
                }

                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        // If this was a conditional update, return [applied] = true
                        if if_clause.is_some() {
                            Ok(self.build_lwt_result(stream, true, None, &column_names))
                        } else {
                            Ok(build_void_result(stream))
                        }
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::Delete {
                table,
                columns,
                where_clause,
                if_clause,
            } => {
                // Qualify table name with keyspace if not already qualified
                let qualified_table = if table.contains('.') {
                    table.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, table)
                    } else {
                        table.clone()
                    }
                };

                // Build WHERE clause string for row lookup
                let where_str = if !where_clause.is_empty() {
                    let where_parts: Vec<String> = where_clause
                        .iter()
                        .map(|cond| {
                            let op_str = match cond.operator {
                                ComparisonOperator::Equal => "=",
                                ComparisonOperator::GreaterThan => ">",
                                ComparisonOperator::GreaterThanOrEqual => ">=",
                                ComparisonOperator::LessThan => "<",
                                ComparisonOperator::LessThanOrEqual => "<=",
                                ComparisonOperator::NotEqual => "!=",
                                _ => "=",
                            };
                            let val_str = match &cond.value {
                                CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                                CqlValue::Int(i) => i.to_string(),
                                CqlValue::Bigint(i) => i.to_string(),
                                CqlValue::Boolean(b) => b.to_string(),
                                CqlValue::Float(f) => f.to_string(),
                                CqlValue::Double(f) => f.to_string(),
                                CqlValue::Timestamp(ts) => (ts / 1000).to_string(),
                                CqlValue::Null => "NULL".to_string(),
                                _ => format!("'{:?}'", cond.value),
                            };
                            format!("{} {} {}", cond.column, op_str, val_str)
                        })
                        .collect();
                    where_parts.join(" AND ")
                } else {
                    String::new()
                };

                // Handle IF clause (lightweight transaction)
                // Use specified columns or all columns from IF clause
                let column_names: Vec<String> = if !columns.is_empty() {
                    columns.clone()
                } else if let Some(ref if_conds) = if_clause {
                    if_conds.iter().map(|c| c.column.clone()).collect()
                } else {
                    vec![]
                };

                if let Some(if_conditions) = if_clause {
                    if !where_str.is_empty() {
                        // Fetch current row
                        if let Ok(Some(existing_row)) =
                            self.check_row_exists(&qualified_table, &where_str).await
                        {
                            // Evaluate IF conditions
                            if !self.evaluate_if_conditions(&existing_row, if_conditions) {
                                // Conditions not met, return [applied] = false with current values
                                return Ok(self.build_lwt_result(
                                    stream,
                                    false,
                                    Some(existing_row),
                                    &column_names,
                                ));
                            }
                        } else {
                            // Row doesn't exist, return [applied] = false
                            return Ok(self.build_lwt_result(stream, false, None, &column_names));
                        }
                    }
                }

                // Convert CQL DELETE to SQL and execute
                let mut sql = format!("DELETE FROM {}", qualified_table);

                if !where_str.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_str);
                }

                #[cfg(test)]
                println!("[CQL] Executing DELETE SQL: {}", sql);

                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(result) => {
                        #[cfg(test)]
                        println!("[CQL] DELETE result: {:?}", result);
                        // Check if DELETE actually deleted rows
                        if let crate::protocols::postgres_wire::QueryResult::Delete { count } =
                            result
                        {
                            #[cfg(test)]
                            println!("[CQL] DELETE affected {} rows", count);
                            #[cfg(not(test))]
                            let _ = count; // Suppress unused variable warning in non-test builds
                        }
                        // If this was a conditional delete, return [applied] = true
                        if if_clause.is_some() {
                            Ok(self.build_lwt_result(stream, true, None, &column_names))
                        } else {
                            Ok(build_void_result(stream))
                        }
                    }
                    Err(e) => {
                        #[cfg(test)]
                        println!("[CQL] DELETE error: {:?}", e);
                        Ok(build_error_from_protocol_error(stream, &e))
                    }
                }
            }
            CqlStatement::CreateKeyspace {
                name,
                if_not_exists,
                ..
            } => {
                println!("[CQL] CREATE KEYSPACE {}", name);
                let sql = if *if_not_exists {
                    format!("CREATE SCHEMA IF NOT EXISTS {}", name)
                } else {
                    format!("CREATE SCHEMA {}", name)
                };

                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        self.publish_schema_change_event(
                            super::types::SchemaChangeType::Created,
                            "KEYSPACE",
                            name,
                            "",
                        );
                        Ok(self.build_schema_change_result(stream))
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::CreateTable {
                name,
                columns,
                if_not_exists,
                primary_key,
                ..
            } => {
                println!("[CQL] CREATE TABLE {}", name);

                // Qualify table name with keyspace if not already qualified
                let qualified_table = if name.contains('.') {
                    name.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, name)
                    } else {
                        name.clone()
                    }
                };

                // Build column definitions
                let mut col_defs = Vec::new();
                for col in columns {
                    let type_str = match &col.data_type {
                        super::types::CqlType::Text
                        | super::types::CqlType::Varchar
                        | super::types::CqlType::Ascii => "TEXT",
                        super::types::CqlType::Int => "INT",
                        super::types::CqlType::Bigint
                        | super::types::CqlType::Counter
                        | super::types::CqlType::Varint => "BIGINT",
                        super::types::CqlType::Boolean => "BOOLEAN",
                        super::types::CqlType::Float => "REAL",
                        super::types::CqlType::Double => "DOUBLE PRECISION",
                        super::types::CqlType::Uuid | super::types::CqlType::Timeuuid => "TEXT", // Store UUID as TEXT
                        super::types::CqlType::Timestamp => "TIMESTAMP",
                        super::types::CqlType::Date => "DATE",
                        super::types::CqlType::Time => "TIME",
                        super::types::CqlType::Smallint => "SMALLINT",
                        super::types::CqlType::Tinyint => "SMALLINT",
                        _ => "TEXT", // Default to TEXT for collections etc (stored as JSON)
                    };
                    col_defs.push(format!("{} {}", col.name, type_str));
                }

                // Add primary key constraint if present
                if !primary_key.is_empty() {
                    col_defs.push(format!("PRIMARY KEY ({})", primary_key.join(", ")));
                }

                let sql = if *if_not_exists {
                    format!(
                        "CREATE TABLE IF NOT EXISTS {} ({})",
                        qualified_table,
                        col_defs.join(", ")
                    )
                } else {
                    format!("CREATE TABLE {} ({})", qualified_table, col_defs.join(", "))
                };

                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        let keyspace = if qualified_table.contains('.') {
                            qualified_table.split('.').next().unwrap_or("").to_string()
                        } else {
                            self.parser
                                .read()
                                .await
                                .current_keyspace()
                                .unwrap_or("")
                                .to_string()
                        };
                        let table_name = if qualified_table.contains('.') {
                            qualified_table.split('.').nth(1).unwrap_or("").to_string()
                        } else {
                            qualified_table.clone()
                        };

                        self.publish_schema_change_event(
                            super::types::SchemaChangeType::Created,
                            "TABLE",
                            &keyspace,
                            &table_name,
                        );
                        Ok(self.build_schema_change_result(stream))
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::DropKeyspace { name, if_exists } => {
                println!("[CQL] DROP KEYSPACE {}", name);
                let sql = if *if_exists {
                    format!("DROP SCHEMA IF EXISTS {} CASCADE", name)
                } else {
                    format!("DROP SCHEMA {} CASCADE", name)
                };
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        self.publish_schema_change_event(
                            super::types::SchemaChangeType::Dropped,
                            "KEYSPACE",
                            name,
                            "",
                        );
                        Ok(self.build_schema_change_result(stream))
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::DropTable { name, if_exists } => {
                println!("[CQL] DROP TABLE {}", name);
                // Qualify table name with keyspace if not already qualified
                let qualified_table = if name.contains('.') {
                    name.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, name)
                    } else {
                        name.clone()
                    }
                };

                let sql = if *if_exists {
                    format!("DROP TABLE IF EXISTS {}", qualified_table)
                } else {
                    format!("DROP TABLE {}", qualified_table)
                };
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        let keyspace = if qualified_table.contains('.') {
                            qualified_table.split('.').next().unwrap_or("").to_string()
                        } else {
                            self.parser
                                .read()
                                .await
                                .current_keyspace()
                                .unwrap_or("")
                                .to_string()
                        };
                        let table_name = if qualified_table.contains('.') {
                            qualified_table.split('.').nth(1).unwrap_or("").to_string()
                        } else {
                            qualified_table.clone()
                        };

                        self.publish_schema_change_event(
                            super::types::SchemaChangeType::Dropped,
                            "TABLE",
                            &keyspace,
                            &table_name,
                        );
                        Ok(self.build_schema_change_result(stream))
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::CreateIndex {
                name,
                table,
                column,
                if_not_exists,
                ..
            } => {
                println!("[CQL] CREATE INDEX {} ON {} ({})", name, table, column);
                // Qualify table name with keyspace if not already qualified
                let qualified_table = if table.contains('.') {
                    table.clone()
                } else {
                    let parser = self.parser.read().await;
                    if let Some(ks) = parser.current_keyspace() {
                        format!("{}.{}", ks, table)
                    } else {
                        table.clone()
                    }
                };

                let sql = if *if_not_exists {
                    format!(
                        "CREATE INDEX IF NOT EXISTS {} ON {} ({})",
                        name, qualified_table, column
                    )
                } else {
                    format!("CREATE INDEX {} ON {} ({})", name, qualified_table, column)
                };
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => Ok(self.build_schema_change_result(stream)),
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::CreateType { name, .. } => {
                println!("[CQL] CREATE TYPE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::CreateMaterializedView {
                name, source_table, ..
            } => {
                println!(
                    "[CQL] CREATE MATERIALIZED VIEW {} FROM {}",
                    name, source_table
                );
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::Use { keyspace } => {
                let mut parser = self.parser.write().await;
                parser.set_keyspace(keyspace.clone());
                println!("[CQL] USE {}", keyspace);
                Ok(self.build_set_keyspace_result(stream, keyspace))
            }
            CqlStatement::Truncate { table } => {
                println!("[CQL] TRUNCATE {}", table);
                // Execute DELETE FROM table to actually truncate
                let sql = format!("DELETE FROM {}", table);
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => {
                        println!("[CQL] Truncated table: {}", table);
                    }
                    Err(e) => {
                        println!("[CQL] Error truncating table {}: {}", table, e);
                    }
                }
                Ok(build_void_result(stream))
            }
            CqlStatement::Batch { .. } => Ok(build_void_result(stream)),
            CqlStatement::AlterTable { name, alteration } => {
                println!("[CQL] ALTER TABLE {} {:?}", name, alteration);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::AlterKeyspace { name, .. } => {
                println!("[CQL] ALTER KEYSPACE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::AlterType { name, alteration } => {
                println!("[CQL] ALTER TYPE {} {:?}", name, alteration);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropIndex { name, .. } => {
                println!("[CQL] DROP INDEX {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropType { name, .. } => {
                println!("[CQL] DROP TYPE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropMaterializedView { name, .. } => {
                println!("[CQL] DROP MATERIALIZED VIEW {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::CreateFunction { name, .. } => {
                println!("[CQL] CREATE FUNCTION {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropFunction { name, .. } => {
                println!("[CQL] DROP FUNCTION {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::CreateAggregate { name, .. } => {
                println!("[CQL] CREATE AGGREGATE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropAggregate { name, .. } => {
                println!("[CQL] DROP AGGREGATE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::CreateRole { name, .. } => {
                println!("[CQL] CREATE ROLE {}", name);
                Ok(build_void_result(stream))
            }
            CqlStatement::AlterRole { name, .. } => {
                println!("[CQL] ALTER ROLE {}", name);
                Ok(build_void_result(stream))
            }
            CqlStatement::DropRole { name, .. } => {
                println!("[CQL] DROP ROLE {}", name);
                Ok(build_void_result(stream))
            }
            CqlStatement::Grant { role, .. } => {
                println!("[CQL] GRANT TO {}", role);
                Ok(build_void_result(stream))
            }
            CqlStatement::Revoke { role, .. } => {
                println!("[CQL] REVOKE FROM {}", role);
                Ok(build_void_result(stream))
            }
            CqlStatement::ListRoles { .. } => {
                // Return empty result set for now
                println!("[CQL] LIST ROLES");
                Ok(self.build_rows_result(
                    stream,
                    vec![],
                    vec!["role".to_string(), "super".to_string(), "login".to_string()],
                    None,
                    None,
                ))
            }
            CqlStatement::ListPermissions { .. } => {
                // Return empty result set for now
                println!("[CQL] LIST PERMISSIONS");
                Ok(self.build_rows_result(
                    stream,
                    vec![],
                    vec![
                        "role".to_string(),
                        "resource".to_string(),
                        "permission".to_string(),
                    ],
                    None,
                    None,
                ))
            }
            CqlStatement::Describe { target } => {
                println!("[CQL] DESCRIBE {:?}", target);
                // Return schema information based on target
                Ok(build_void_result(stream))
            }
            CqlStatement::CreateTrigger {
                name,
                table,
                trigger_class,
                ..
            } => {
                info!(
                    "[CQL] CREATE TRIGGER {} ON {} USING {}",
                    name, table, trigger_class
                );

                // Create trigger definition
                let trigger = TriggerDefinition {
                    name: name.clone(),
                    table: table.clone(),
                    trigger_class: trigger_class.clone(),
                    enabled: true,
                };

                // Register trigger
                let mut triggers = self.triggers.write().await;
                triggers
                    .entry(table.clone())
                    .or_insert_with(Vec::new)
                    .push(trigger);

                info!("[CQL] Trigger '{}' registered for table '{}'", name, table);
                Ok(build_void_result(stream))
            }
            CqlStatement::DropTrigger { name, table, .. } => {
                info!("[CQL] DROP TRIGGER {} ON {}", name, table);

                // Unregister trigger
                let mut triggers = self.triggers.write().await;
                if let Some(table_triggers) = triggers.get_mut(table.as_str()) {
                    table_triggers.retain(|t| t.name != *name);
                    if table_triggers.is_empty() {
                        triggers.remove(table.as_str());
                    }
                }

                info!("[CQL] Trigger '{}' dropped from table '{}'", name, table);
                Ok(build_void_result(stream))
            }
            CqlStatement::ListUsers => {
                println!("[CQL] LIST USERS");
                Ok(self.build_rows_result(
                    stream,
                    vec![],
                    vec!["name".to_string(), "super".to_string()],
                    None,
                    None,
                ))
            }
            CqlStatement::CreateUser { name, .. } => {
                println!("[CQL] CREATE USER {}", name);
                Ok(build_void_result(stream))
            }
            CqlStatement::AlterUser { name, .. } => {
                println!("[CQL] ALTER USER {}", name);
                Ok(build_void_result(stream))
            }
            CqlStatement::DropUser { name, .. } => {
                println!("[CQL] DROP USER {}", name);
                Ok(build_void_result(stream))
            }
            CqlStatement::GrantRole { role, to_role } => {
                println!("[CQL] GRANT {} TO {}", role, to_role);
                Ok(build_void_result(stream))
            }
            CqlStatement::RevokeRole { role, from_role } => {
                println!("[CQL] REVOKE {} FROM {}", role, from_role);
                Ok(build_void_result(stream))
            }
        }
    }

    /// Build a ROWS result
    fn build_rows_result(
        &self,
        stream: i16,
        all_rows: Vec<HashMap<String, SqlValue>>,
        columns: Vec<String>,
        page_size: Option<i32>,
        paging_state: Option<Bytes>,
    ) -> CqlFrame {
        let mut body = BytesMut::new();
        body.put_i32(0x0002); // RESULT::Rows

        // Determine offset from paging_state
        let offset = if let Some(state) = paging_state {
            if state.len() >= 4 {
                let mut s = state.clone();
                s.get_i32() as usize
            } else {
                0
            }
        } else {
            0
        };

        // Slice rows based on offset and page_size
        let total_rows = all_rows.len();
        let (rows, has_more_pages, next_offset) = if offset >= total_rows {
            (Vec::new(), false, 0)
        } else {
            let remaining_rows = &all_rows[offset..];
            if let Some(size) = page_size {
                let size = size as usize;
                if remaining_rows.len() > size {
                    (remaining_rows[..size].to_vec(), true, offset + size)
                } else {
                    (remaining_rows.to_vec(), false, 0)
                }
            } else {
                (remaining_rows.to_vec(), false, 0)
            }
        };

        if rows.is_empty() && !has_more_pages && columns.is_empty() {
            // No rows and no columns - return empty result
            body.put_i32(0x0001); // Global tables spec
            body.put_i32(0); // Column count
            body.put_i32(0); // Row count
            return CqlFrame::response(stream, CqlOpcode::Result, body.freeze());
        }

        // Metadata flags (0x0001 = global tables spec)
        // We don't use global table spec because we don't know the table name for sure
        // So we unset 0x0001 and provide keyspace/table for each column (even if empty)
        let mut flags = 0x0000;
        if has_more_pages {
            flags |= 0x0002;
        }
        body.put_i32(flags);

        // Column count
        body.put_i32(columns.len() as i32);

        // Paging state (if has_more_pages)
        if has_more_pages {
            let mut state = BytesMut::with_capacity(4);
            state.put_i32(next_offset as i32);
            let state_bytes = state.freeze();
            body.put_i32(state_bytes.len() as i32);
            body.put(state_bytes);
        }

        // Column metadata
        for col_name in &columns {
            // Keyspace name (empty string)
            body.put_u16(0);
            // Table name (empty string)
            body.put_u16(0);
            // Column name
            body.put_u16(col_name.len() as u16);
            body.put(col_name.as_bytes());

            // Infer column type from rows
            let mut cql_type_code = 0x000D; // Default Varchar
            for row in &rows {
                if let Some(val) = row.get(col_name) {
                    if !val.is_null() {
                        cql_type_code = self.sql_type_to_cql_code(&val.sql_type());
                        break;
                    }
                }
            }
            body.put_u16(cql_type_code);
        }

        // Row count
        body.put_i32(rows.len() as i32);

        // Encode rows
        for row in &rows {
            for col_name in &columns {
                if let Some(value) = row.get(col_name) {
                    // Convert SqlValue to bytes
                    let value_bytes = self.sql_value_to_bytes(value);
                    body.put_i32(value_bytes.len() as i32);
                    body.put(&value_bytes[..]);
                } else {
                    // NULL value (-1 length)
                    body.put_i32(-1);
                }
            }
        }

        CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
    }

    /// Convert SqlValue to CQL bytes
    fn sql_value_to_bytes(&self, value: &SqlValue) -> Vec<u8> {
        match value {
            SqlValue::Text(s) => {
                // Check if it's a JSON-encoded collection
                if s.starts_with('[') || s.starts_with('{') {
                    // It's a collection stored as JSON, return as-is (will be decoded by client)
                    s.as_bytes().to_vec()
                } else {
                    s.as_bytes().to_vec()
                }
            }
            SqlValue::Integer(i) => i.to_string().as_bytes().to_vec(),
            SqlValue::BigInt(i) => i.to_string().as_bytes().to_vec(),
            SqlValue::Boolean(b) => b.to_string().as_bytes().to_vec(),
            SqlValue::DoublePrecision(f) => f.to_string().as_bytes().to_vec(),
            SqlValue::Real(f) => f.to_string().as_bytes().to_vec(),
            SqlValue::Uuid(u) => u.as_bytes().to_vec(),
            SqlValue::Null => vec![],
            _ => format!("{}", value).as_bytes().to_vec(),
        }
    }

    /// Convert SqlType to CQL type code
    fn sql_type_to_cql_code(&self, t: &SqlType) -> u16 {
        match t {
            SqlType::Text | SqlType::Varchar(_) | SqlType::Char(_) => 0x000D, // Varchar
            SqlType::Integer => 0x0009,                                       // Int
            SqlType::BigInt => 0x0002,                                        // Bigint
            SqlType::Boolean => 0x0004,                                       // Boolean
            SqlType::Real => 0x0008,                                          // Float
            SqlType::DoublePrecision => 0x0007,                               // Double
            SqlType::Timestamp { .. } => 0x000B,                              // Timestamp
            SqlType::Uuid => 0x000C,                                          // Uuid
            SqlType::SmallInt => 0x0013,                                      // Smallint
            _ => 0x000D,                                                      // Default to Varchar
        }
    }

    /// Build a SET_KEYSPACE result
    fn build_set_keyspace_result(&self, stream: i16, keyspace: &str) -> CqlFrame {
        let mut body = BytesMut::new();
        body.put_i32(0x0003); // RESULT::SetKeyspace
        body.put_u16(keyspace.len() as u16);
        body.put(keyspace.as_bytes());

        CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
    }

    /// Build a lightweight transaction (LWT) result
    ///
    /// CQL lightweight transactions return a result with an `[applied]` boolean column.
    /// If the operation was not applied, the current row values are also returned.
    fn build_lwt_result(
        &self,
        stream: i16,
        applied: bool,
        current_row: Option<HashMap<String, SqlValue>>,
        columns: &[String],
    ) -> CqlFrame {
        let mut body = BytesMut::new();
        body.put_i32(0x0002); // RESULT::Rows

        // Build column list: [applied] + original columns if not applied
        let mut result_columns = vec!["[applied]".to_string()];
        if !applied {
            result_columns.extend(columns.iter().cloned());
        }

        // Metadata flags (no global tables spec)
        body.put_i32(0x0000);

        // Column count
        body.put_i32(result_columns.len() as i32);

        // Column metadata
        for col_name in &result_columns {
            // Keyspace name (empty string)
            body.put_u16(0);
            // Table name (empty string)
            body.put_u16(0);
            // Column name
            body.put_u16(col_name.len() as u16);
            body.put(col_name.as_bytes());

            // Column type
            if col_name == "[applied]" {
                body.put_u16(0x0004); // Boolean
            } else {
                body.put_u16(0x000D); // Varchar (default)
            }
        }

        // Row count (always 1 for LWT)
        body.put_i32(1);

        // [applied] value
        let applied_bytes = if applied {
            b"true".to_vec()
        } else {
            b"false".to_vec()
        };
        body.put_i32(applied_bytes.len() as i32);
        body.put(&applied_bytes[..]);

        // If not applied, include current row values
        if !applied {
            for col_name in columns {
                if let Some(ref row) = current_row {
                    if let Some(value) = row.get(col_name) {
                        let value_bytes = self.sql_value_to_bytes(value);
                        body.put_i32(value_bytes.len() as i32);
                        body.put(&value_bytes[..]);
                    } else {
                        body.put_i32(-1); // NULL
                    }
                } else {
                    body.put_i32(-1); // NULL
                }
            }
        }

        CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
    }

    /// Check if a row exists for the given WHERE clause
    /// Returns the row as a HashMap<column_name, SqlValue>
    async fn check_row_exists(
        &self,
        table: &str,
        where_clause: &str,
    ) -> ProtocolResult<Option<HashMap<String, SqlValue>>> {
        let select_sql = format!("SELECT * FROM {} WHERE {} LIMIT 1", table, where_clause);

        match self.query_engine.execute_sql_direct(&select_sql).await {
            Ok(crate::protocols::postgres_wire::QueryResult::Select { columns, rows }) => {
                if rows.is_empty() {
                    Ok(None)
                } else {
                    // Convert Vec<Option<String>> to HashMap<String, SqlValue>
                    let mut row_map = HashMap::new();
                    for (i, col_name) in columns.iter().enumerate() {
                        let value = if let Some(ref val) = rows[0].get(i).and_then(|v| v.as_ref()) {
                            // Try to parse as various types
                            if val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("false")
                            {
                                SqlValue::Boolean(val.eq_ignore_ascii_case("true"))
                            } else if let Ok(i) = val.parse::<i32>() {
                                SqlValue::Integer(i)
                            } else if let Ok(i) = val.parse::<i64>() {
                                SqlValue::BigInt(i)
                            } else if let Ok(f) = val.parse::<f64>() {
                                SqlValue::DoublePrecision(f)
                            } else {
                                SqlValue::Text(val.to_string())
                            }
                        } else {
                            SqlValue::Null
                        };
                        row_map.insert(col_name.clone(), value);
                    }
                    Ok(Some(row_map))
                }
            }
            _ => Ok(None),
        }
    }

    /// Evaluate IF conditions against a row
    fn evaluate_if_conditions(
        &self,
        row: &HashMap<String, SqlValue>,
        conditions: &[super::parser::WhereCondition],
    ) -> bool {
        for cond in conditions {
            let row_value = row.get(&cond.column);
            let condition_value = self.cql_value_to_sql_value(&cond.value);

            let matches = match cond.operator {
                ComparisonOperator::Equal => {
                    if let Some(rv) = row_value {
                        self.sql_values_equal(rv, &condition_value)
                    } else {
                        condition_value.is_null()
                    }
                }
                ComparisonOperator::NotEqual => {
                    if let Some(rv) = row_value {
                        !self.sql_values_equal(rv, &condition_value)
                    } else {
                        !condition_value.is_null()
                    }
                }
                ComparisonOperator::GreaterThan => {
                    if let Some(rv) = row_value {
                        self.sql_value_cmp(rv, &condition_value) > 0
                    } else {
                        false
                    }
                }
                ComparisonOperator::GreaterThanOrEqual => {
                    if let Some(rv) = row_value {
                        self.sql_value_cmp(rv, &condition_value) >= 0
                    } else {
                        false
                    }
                }
                ComparisonOperator::LessThan => {
                    if let Some(rv) = row_value {
                        self.sql_value_cmp(rv, &condition_value) < 0
                    } else {
                        false
                    }
                }
                ComparisonOperator::LessThanOrEqual => {
                    if let Some(rv) = row_value {
                        self.sql_value_cmp(rv, &condition_value) <= 0
                    } else {
                        false
                    }
                }
                _ => true, // IN, CONTAINS, etc. - default to true for now
            };

            if !matches {
                return false;
            }
        }
        true
    }

    /// Convert CqlValue to SqlValue
    fn cql_value_to_sql_value(&self, value: &CqlValue) -> SqlValue {
        match value {
            CqlValue::Text(s) => SqlValue::Text(s.clone()),
            CqlValue::Int(i) => SqlValue::Integer(*i),
            CqlValue::Bigint(i) => SqlValue::BigInt(*i),
            CqlValue::Boolean(b) => SqlValue::Boolean(*b),
            CqlValue::Float(f) => SqlValue::Real(*f),
            CqlValue::Double(f) => SqlValue::DoublePrecision(*f),
            CqlValue::Null => SqlValue::Null,
            _ => SqlValue::Text(format!("{:?}", value)),
        }
    }

    /// Compare two SqlValues for equality
    fn sql_values_equal(&self, a: &SqlValue, b: &SqlValue) -> bool {
        match (a, b) {
            (SqlValue::Null, SqlValue::Null) => true,
            (SqlValue::Text(s1), SqlValue::Text(s2)) => s1 == s2,
            (SqlValue::Integer(i1), SqlValue::Integer(i2)) => i1 == i2,
            (SqlValue::BigInt(i1), SqlValue::BigInt(i2)) => i1 == i2,
            (SqlValue::Boolean(b1), SqlValue::Boolean(b2)) => b1 == b2,
            (SqlValue::Real(f1), SqlValue::Real(f2)) => (f1 - f2).abs() < f32::EPSILON,
            (SqlValue::DoublePrecision(f1), SqlValue::DoublePrecision(f2)) => {
                (f1 - f2).abs() < f64::EPSILON
            }
            // Cross-type comparisons
            (SqlValue::Integer(i), SqlValue::BigInt(b)) => *i as i64 == *b,
            (SqlValue::BigInt(b), SqlValue::Integer(i)) => *b == *i as i64,
            _ => false,
        }
    }

    /// Compare two SqlValues, returns -1, 0, or 1
    fn sql_value_cmp(&self, a: &SqlValue, b: &SqlValue) -> i32 {
        match (a, b) {
            (SqlValue::Integer(i1), SqlValue::Integer(i2)) => i1.cmp(i2) as i32,
            (SqlValue::BigInt(i1), SqlValue::BigInt(i2)) => i1.cmp(i2) as i32,
            (SqlValue::Integer(i), SqlValue::BigInt(b)) => (*i as i64).cmp(b) as i32,
            (SqlValue::BigInt(b), SqlValue::Integer(i)) => b.cmp(&(*i as i64)) as i32,
            (SqlValue::Real(f1), SqlValue::Real(f2)) => {
                f1.partial_cmp(f2).map(|o| o as i32).unwrap_or(0)
            }
            (SqlValue::DoublePrecision(f1), SqlValue::DoublePrecision(f2)) => {
                f1.partial_cmp(f2).map(|o| o as i32).unwrap_or(0)
            }
            (SqlValue::Text(s1), SqlValue::Text(s2)) => s1.cmp(s2) as i32,
            _ => 0,
        }
    }

    /// Build a schema change result frame
    /// This is returned to the client that executed the DDL statement
    fn build_schema_change_result(&self, stream: i16) -> CqlFrame {
        let mut body = BytesMut::new();
        // Result kind: SchemaChange (0x0005)
        body.put_i32(0x0005);

        // Change type: CREATED ("CREATED")
        super::protocol::write_string(&mut body, "CREATED");
        // Target: TABLE ("TABLE")
        super::protocol::write_string(&mut body, "TABLE");
        // Options: keyspace, table
        super::protocol::write_string(&mut body, "test_keyspace");
        super::protocol::write_string(&mut body, "test_table");

        CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
    }

    /// Build a schema change event frame
    /// This would be broadcast to all registered clients
    #[allow(dead_code)]
    fn build_schema_change_event(
        &self,
        change_type: &str,
        keyspace: &str,
        table: &str,
    ) -> ProtocolResult<CqlFrame> {
        use super::types::{CqlEvent, SchemaChangeType};

        let change = match change_type {
            "CREATED" => SchemaChangeType::Created,
            "UPDATED" => SchemaChangeType::Updated,
            "DROPPED" => SchemaChangeType::Dropped,
            _ => SchemaChangeType::Updated,
        };

        let event = CqlEvent::SchemaChange(
            change,
            keyspace.to_string(),
            table.to_string(),
            "TABLE".to_string(),
        );

        super::protocol::build_event_response(-1, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();
        assert!(!adapter.config.authentication_enabled);
    }

    #[tokio::test]
    async fn test_execute_use_statement() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        let statement = CqlStatement::Use {
            keyspace: "test_ks".to_string(),
        };

        let result = adapter
            .execute_statement(&statement, 0, None, None)
            .await
            .unwrap();
        assert_eq!(result.opcode, CqlOpcode::Result);

        // Verify keyspace was set
        let parser = adapter.parser.read().await;
        assert_eq!(parser.current_keyspace(), Some("test_ks"));
    }

    #[test]
    fn test_batch_type_enum() {
        assert_eq!(BatchType::Logged, BatchType::Logged);
        assert_ne!(BatchType::Logged, BatchType::Unlogged);
        assert_ne!(BatchType::Logged, BatchType::Counter);
        assert_ne!(BatchType::Unlogged, BatchType::Counter);
    }

    /// Helper to build a BATCH frame for testing
    fn build_batch_frame(batch_type: u8, queries: &[&str], stream: i16) -> CqlFrame {
        let mut body = BytesMut::new();

        // Batch type
        body.put_u8(batch_type);

        // Number of statements
        body.put_u16(queries.len() as u16);

        // Each statement
        for query in queries {
            // Kind: 0 = query string
            body.put_u8(0);

            // Query string (long string: 4 bytes length + bytes)
            body.put_i32(query.len() as i32);
            body.put(query.as_bytes());

            // Number of values: 0
            body.put_u16(0);
        }

        // Consistency level (ONE = 0x0001)
        body.put_u16(0x0001);

        // Flags (none)
        body.put_u8(0);

        CqlFrame {
            version: 0x04, // Protocol v4 request
            flags: 0,
            stream,
            opcode: CqlOpcode::Batch,
            body: body.freeze(),
        }
    }

    #[tokio::test]
    async fn test_batch_empty() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Create table first
        adapter
            .execute_sql("CREATE TABLE batch_test (id INT PRIMARY KEY, name TEXT)")
            .await
            .unwrap();

        // Empty batch
        let frame = build_batch_frame(0, &[], 1);
        let result = adapter.handle_batch(&frame).await.unwrap();

        // Should return VOID result
        assert_eq!(result.opcode, CqlOpcode::Result);
    }

    #[tokio::test]
    async fn test_batch_logged_multiple_inserts() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Create table first
        adapter
            .execute_sql("CREATE TABLE batch_users (id INT PRIMARY KEY, name TEXT)")
            .await
            .unwrap();

        // Batch with multiple INSERTs
        let queries = [
            "INSERT INTO batch_users (id, name) VALUES (1, 'Alice')",
            "INSERT INTO batch_users (id, name) VALUES (2, 'Bob')",
            "INSERT INTO batch_users (id, name) VALUES (3, 'Charlie')",
        ];

        let frame = build_batch_frame(0, &queries, 1); // 0 = LOGGED
        let result = adapter.handle_batch(&frame).await.unwrap();

        // Should return VOID result
        assert_eq!(result.opcode, CqlOpcode::Result);
    }

    #[tokio::test]
    async fn test_batch_unlogged() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Create table first
        adapter
            .execute_sql("CREATE TABLE batch_unlogged (id INT PRIMARY KEY, val INT)")
            .await
            .unwrap();

        // Unlogged batch
        let queries = [
            "INSERT INTO batch_unlogged (id, val) VALUES (1, 100)",
            "INSERT INTO batch_unlogged (id, val) VALUES (2, 200)",
        ];

        let frame = build_batch_frame(1, &queries, 2); // 1 = UNLOGGED
        let result = adapter.handle_batch(&frame).await.unwrap();

        assert_eq!(result.opcode, CqlOpcode::Result);
    }

    #[tokio::test]
    async fn test_batch_counter() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Counter batches are for counter updates
        // For now, we just verify the batch type is parsed correctly
        let frame = build_batch_frame(2, &[], 3); // 2 = COUNTER
        let result = adapter.handle_batch(&frame).await.unwrap();

        assert_eq!(result.opcode, CqlOpcode::Result);
    }

    #[tokio::test]
    async fn test_batch_incomplete_frame() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Incomplete frame - just batch type, no statement count
        let mut body = BytesMut::new();
        body.put_u8(0); // Batch type only

        let frame = CqlFrame {
            version: 0x04,
            flags: 0,
            stream: 1,
            opcode: CqlOpcode::Batch,
            body: body.freeze(),
        };

        let result = adapter.handle_batch(&frame).await;
        assert!(result.is_err());
    }

    // ==================== Lightweight Transaction Tests ====================

    use crate::protocols::cql::parser::WhereCondition;

    /// Helper to execute a CQL statement through the adapter
    #[allow(dead_code)]
    async fn execute_cql_statement(adapter: &CqlAdapter, cql: &str) -> ProtocolResult<CqlFrame> {
        let parser = adapter.parser.read().await;
        let statement = parser.parse(cql)?;
        drop(parser); // Release the lock before calling execute_statement
        adapter.execute_statement(&statement, 0, None, None).await
    }

    #[tokio::test]
    async fn test_lwt_insert_if_not_exists_parsing() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Test that IF NOT EXISTS parses correctly
        let parser = adapter.parser.read().await;
        let statement =
            parser.parse("INSERT INTO lwt_test (id, name) VALUES (1, 'test') IF NOT EXISTS");
        assert!(statement.is_ok());

        if let Ok(CqlStatement::Insert { if_not_exists, .. }) = statement {
            assert!(if_not_exists, "IF NOT EXISTS should be parsed as true");
        } else {
            panic!("Expected INSERT statement");
        }
    }

    #[tokio::test]
    async fn test_lwt_update_if_condition_parsing() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Test that UPDATE IF parses correctly
        let parser = adapter.parser.read().await;
        let statement =
            parser.parse("UPDATE lwt_test SET status = 'done' WHERE id = 1 IF status = 'pending'");

        // Debug: print the error if parsing fails
        if statement.is_err() {
            println!("Parse error: {:?}", statement.as_ref().err());
        }
        assert!(statement.is_ok(), "Parsing should succeed");

        if let Ok(CqlStatement::Update { if_clause, .. }) = statement {
            assert!(if_clause.is_some(), "IF clause should be parsed");
            let conditions = if_clause.unwrap();
            assert_eq!(conditions.len(), 1);
            assert_eq!(conditions[0].column, "status");
        } else {
            panic!("Expected UPDATE statement");
        }
    }

    #[tokio::test]
    async fn test_lwt_delete_if_condition_parsing() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Test that DELETE IF parses correctly
        let parser = adapter.parser.read().await;
        let statement = parser.parse("DELETE FROM lwt_test WHERE id = 1 IF status = 'inactive'");

        // Debug: print the error if parsing fails
        if statement.is_err() {
            println!("Parse error: {:?}", statement.as_ref().err());
        }
        assert!(statement.is_ok(), "Parsing should succeed");

        if let Ok(CqlStatement::Delete { if_clause, .. }) = statement {
            assert!(if_clause.is_some(), "IF clause should be parsed");
            let conditions = if_clause.unwrap();
            assert_eq!(conditions.len(), 1);
            assert_eq!(conditions[0].column, "status");
        } else {
            panic!("Expected DELETE statement");
        }
    }

    #[tokio::test]
    async fn test_lwt_result_builder_applied_true() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Test build_lwt_result for applied = true
        let result = adapter.build_lwt_result(1, true, None, &["col1".to_string()]);

        // Should have Result opcode
        assert_eq!(result.opcode, CqlOpcode::Result);
        // Body should be non-empty (contains [applied] column)
        assert!(!result.body.is_empty());
    }

    #[tokio::test]
    async fn test_lwt_result_builder_applied_false() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Test build_lwt_result for applied = false with existing row
        let mut existing_row = HashMap::new();
        existing_row.insert("col1".to_string(), SqlValue::Text("value1".to_string()));

        let result = adapter.build_lwt_result(1, false, Some(existing_row), &["col1".to_string()]);

        // Should have Result opcode
        assert_eq!(result.opcode, CqlOpcode::Result);
        // Body should be non-empty (contains [applied] column + original columns)
        assert!(!result.body.is_empty());
    }

    #[tokio::test]
    async fn test_lwt_evaluate_if_conditions_equal() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        let mut row = HashMap::new();
        row.insert("status".to_string(), SqlValue::Text("pending".to_string()));
        row.insert("count".to_string(), SqlValue::Integer(10));

        // Test equality condition that matches
        let conditions = vec![WhereCondition {
            column: "status".to_string(),
            operator: ComparisonOperator::Equal,
            value: CqlValue::Text("pending".to_string()),
        }];
        assert!(adapter.evaluate_if_conditions(&row, &conditions));

        // Test equality condition that doesn't match
        let conditions_fail = vec![WhereCondition {
            column: "status".to_string(),
            operator: ComparisonOperator::Equal,
            value: CqlValue::Text("done".to_string()),
        }];
        assert!(!adapter.evaluate_if_conditions(&row, &conditions_fail));
    }

    #[tokio::test]
    async fn test_lwt_evaluate_if_conditions_numeric() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        let mut row = HashMap::new();
        row.insert("count".to_string(), SqlValue::Integer(10));

        // Test greater than
        let conditions_gt = vec![WhereCondition {
            column: "count".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: CqlValue::Int(5),
        }];
        assert!(adapter.evaluate_if_conditions(&row, &conditions_gt));

        // Test less than or equal
        let conditions_lte = vec![WhereCondition {
            column: "count".to_string(),
            operator: ComparisonOperator::LessThanOrEqual,
            value: CqlValue::Int(10),
        }];
        assert!(adapter.evaluate_if_conditions(&row, &conditions_lte));

        // Test not equal
        let conditions_ne = vec![WhereCondition {
            column: "count".to_string(),
            operator: ComparisonOperator::NotEqual,
            value: CqlValue::Int(5),
        }];
        assert!(adapter.evaluate_if_conditions(&row, &conditions_ne));
    }

    #[tokio::test]
    async fn test_register_events() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Build a REGISTER frame manually
        let mut body = BytesMut::new();
        // Count: 2
        body.put_u16(2);
        // Event 1: "TOPOLOGY_CHANGE"
        super::super::protocol::write_string(&mut body, "TOPOLOGY_CHANGE");
        // Event 2: "SCHEMA_CHANGE"
        super::super::protocol::write_string(&mut body, "SCHEMA_CHANGE");

        let frame = CqlFrame {
            version: 0x04,
            flags: 0,
            stream: 1,
            opcode: CqlOpcode::Register,
            body: body.freeze(),
        };

        // Handle register
        let result = adapter.handle_register(&frame).await.unwrap();

        // Should return READY
        assert_eq!(result.opcode, CqlOpcode::Ready);
    }

    #[tokio::test]
    async fn test_schema_change_event_publishing() {
        let config = CqlConfig::default();
        let adapter = CqlAdapter::new(config).await.unwrap();

        // Subscribe to event bus directly
        let mut rx = adapter.event_bus.subscribe();

        // Perform schema change (CREATE KEYSPACE)
        let parser = CqlParser::new();
        let statement = parser.parse("CREATE KEYSPACE ks1 WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}").unwrap();

        let result = adapter
            .execute_statement(&statement, 1, None, None)
            .await
            .unwrap();
        assert_ne!(
            result.opcode,
            CqlOpcode::Error,
            "Statement failed: {:?}",
            result
        );

        // Check if event is received
        let event = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
            .await
            .expect("Timeout waiting for event")
            .unwrap();

        match event {
            super::CqlEvent::SchemaChange(change_type, keyspace, name, target_type) => {
                assert_eq!(change_type, super::super::types::SchemaChangeType::Created);
                assert_eq!(keyspace, "ks1");
                assert_eq!(target_type, "KEYSPACE");
                assert_eq!(name, ""); // Name is empty for KEYSPACE changes
            }
            _ => panic!("Expected SchemaChange event"),
        }

        // Test Create Table
        let statement_use = parser.parse("USE ks1").unwrap();
        adapter
            .execute_statement(&statement_use, 1, None, None)
            .await
            .unwrap();

        let statement_table = parser
            .parse("CREATE TABLE test_table (id int PRIMARY KEY, val text)")
            .unwrap();
        adapter
            .execute_statement(&statement_table, 1, None, None)
            .await
            .unwrap();

        let event_table = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .expect("Timeout waiting for table event")
            .unwrap();

        match event_table {
            super::CqlEvent::SchemaChange(change_type, keyspace, name, target_type) => {
                assert_eq!(change_type, super::super::types::SchemaChangeType::Created);
                assert_eq!(keyspace, "ks1");
                assert_eq!(target_type, "TABLE");
                assert_eq!(name, "test_table");
            }
            _ => panic!("Expected SchemaChange event for table"),
        }
    }
}
