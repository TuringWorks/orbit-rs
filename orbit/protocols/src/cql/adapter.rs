//! CQL adapter implementation
//!
//! This module provides the main CQL adapter that handles client connections
//! and translates CQL operations to Orbit engine calls.

use super::parser::{CqlParser, CqlStatement, ComparisonOperator};
use super::protocol::{
    build_error_from_protocol_error, build_error_response, build_ready_response,
    build_supported_response, build_void_result, read_string, read_string_map, CqlFrame,
    CqlOpcode, QueryParameters,
};
use super::types::CqlValue;
use super::CqlConfig;
use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::QueryEngine;
use crate::postgres_wire::sql::types::SqlValue;
use crate::common::storage::memory::MemoryTableStorage;
use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// CQL adapter
pub struct CqlAdapter {
    /// Configuration
    config: CqlConfig,
    /// Storage backend
    storage: Arc<dyn crate::common::storage::TableStorage>,
    /// Parser
    parser: Arc<RwLock<CqlParser>>,
    /// Query engine for executing SQL
    query_engine: Arc<QueryEngine>,
    /// Prepared statements
    prepared_statements: Arc<RwLock<HashMap<Vec<u8>, PreparedStatement>>>,
    /// Connection metrics (for production monitoring)
    #[allow(dead_code)]
    metrics: Arc<RwLock<CqlMetrics>>,
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

impl CqlAdapter {
    /// Create a new CQL adapter with shared storage
    pub async fn new_with_storage(
        config: CqlConfig,
        storage: Arc<dyn crate::common::storage::TableStorage>,
    ) -> ProtocolResult<Self> {
        let query_engine = Arc::new(QueryEngine::new());

        Ok(Self {
            config,
            storage,
            parser: Arc::new(RwLock::new(CqlParser::new())),
            query_engine,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(CqlMetrics::default())),
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
    pub async fn with_query_engine(config: CqlConfig, query_engine: Arc<QueryEngine>) -> ProtocolResult<Self> {
        let storage = Arc::new(MemoryTableStorage::new());

        Ok(Self {
            config,
            storage,
            parser: Arc::new(RwLock::new(CqlParser::new())),
            query_engine,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(CqlMetrics::default())),
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
    pub async fn execute_sql(&self, sql: &str) -> ProtocolResult<crate::postgres_wire::sql::UnifiedExecutionResult> {
        // Use QueryEngine's execute_sql_direct which bypasses persistent storage checks
        // and uses ConfigurableSqlEngine directly
        let result = self.query_engine.execute_sql_direct(sql).await?;
        
        // Convert QueryResult to UnifiedExecutionResult
        match result {
            crate::postgres_wire::QueryResult::Select { columns, rows } => {
                let row_count = rows.len();
                Ok(crate::postgres_wire::sql::UnifiedExecutionResult::Select {
                    columns,
                    rows,
                    row_count,
                    transaction_id: None,
                })
            }
            crate::postgres_wire::QueryResult::Insert { count } => {
                Ok(crate::postgres_wire::sql::UnifiedExecutionResult::Insert {
                    count,
                    transaction_id: None,
                })
            }
            crate::postgres_wire::QueryResult::Update { count } => {
                Ok(crate::postgres_wire::sql::UnifiedExecutionResult::Update {
                    count,
                    transaction_id: None,
                })
            }
            crate::postgres_wire::QueryResult::Delete { count } => {
                Ok(crate::postgres_wire::sql::UnifiedExecutionResult::Delete {
                    count,
                    transaction_id: None,
                })
            }
        }
    }

    /// Start the CQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        println!(
            "[CQL] Server listening on {}",
            self.config.listen_addr
        );

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    println!("[CQL] New connection from {}", addr);
                    let adapter = self.clone_for_connection();
                    tokio::spawn(async move {
                        if let Err(e) = adapter.handle_connection(socket).await {
                            eprintln!("[CQL] Connection error from {}: {:?}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[CQL] Accept error: {}", e);
                }
            }
        }
    }

    /// Clone adapter for a new connection
    fn clone_for_connection(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: self.storage.clone(),
            parser: self.parser.clone(),
            query_engine: self.query_engine.clone(),
            prepared_statements: self.prepared_statements.clone(),
            metrics: self.metrics.clone(),
        }
    }

    /// Handle a client connection
    async fn handle_connection(&self, mut socket: TcpStream) -> ProtocolResult<()> {
        let mut buffer = BytesMut::with_capacity(4096);

        loop {
            // Read frame
            let n = socket
                .read_buf(&mut buffer)
                .await
                .map_err(|e| ProtocolError::IoError(e.to_string()))?;

            if n == 0 {
                // Connection closed
                return Ok(());
            }

            // Try to parse frame
            if buffer.len() < 9 {
                // Need more data for header
                continue;
            }

            // Check if we have the full frame
            let body_len = {
                let mut buf = buffer.as_ref();
                buf.advance(5); // Skip to length field
                buf.get_u32() as usize
            };

            if buffer.len() < 9 + body_len {
                // Need more data
                continue;
            }

            // Parse frame
            let frame_bytes = buffer.split_to(9 + body_len).freeze();
            let frame = CqlFrame::decode(frame_bytes)?;

            // Handle frame
            let response = self.handle_frame(&frame).await?;

            // Send response
            let response_bytes = response.encode();
            socket
                .write_all(&response_bytes)
                .await
                .map_err(|e| ProtocolError::IoError(e.to_string()))?;
        }
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
            CqlOpcode::AuthResponse => self.handle_auth_response(frame).await,
            _ => Ok(build_error_response(
                frame.stream,
                super::protocol::error_codes::PROTOCOL_ERROR,
                "Unsupported opcode",
            )),
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
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_queries += 1;
        }
        
        let mut body = frame.body.clone();

        // Read query string
        let query_len = body.get_u32();
        let query_bytes = body.copy_to_bytes(query_len as usize);
        let query = String::from_utf8(query_bytes.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        // Read query parameters
        let _params = QueryParameters::decode(body)?;

        // Parse and execute query
        let parser = self.parser.read().await;
        let statement = parser.parse(&query)?;
        drop(parser);

        // Execute statement
        self.execute_statement(&statement, frame.stream).await
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
        self.prepared_statements.write().await.insert(id.clone(), prepared);

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
        let _params = QueryParameters::decode(body)?;

        // Get prepared statement
        let prepared_statements = self.prepared_statements.read().await;
        let prepared = prepared_statements.get(&id).ok_or_else(|| {
            ProtocolError::InvalidStatement("Prepared statement not found".to_string())
        })?;

        // Execute statement
        self.execute_statement(&prepared.statement, frame.stream)
            .await
    }

    /// Handle BATCH request
    async fn handle_batch(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
        let mut body = frame.body.clone();
        
        // Read batch type (1 byte: 0=LOGGED, 1=UNLOGGED, 2=COUNTER)
        let batch_type_byte = body.get_u8();
        let _batch_type = match batch_type_byte {
            0 => "LOGGED",
            1 => "UNLOGGED",
            2 => "COUNTER",
            _ => "LOGGED", // Default
        };
        
        // Read number of statements
        let statement_count = body.get_u16();
        
        // Execute each statement in the batch
        let mut errors = Vec::new();
        for _ in 0..statement_count {
            // Read statement kind (1 byte: 0=query string, 1=prepared statement ID)
            let kind = body.get_u8();
            
            if kind == 0 {
                // Query string
                let query_len = body.get_u32();
                let query_bytes = body.copy_to_bytes(query_len as usize);
                let query = String::from_utf8(query_bytes.to_vec())
                    .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;
                
                // Parse and execute
                let parser = self.parser.read().await;
                match parser.parse(&query) {
                    Ok(statement) => {
                        drop(parser);
                        match self.execute_statement(&statement, frame.stream).await {
                            Ok(_) => {} // Statement executed successfully
                            Err(e) => {
                                errors.push(format!("Batch statement error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Parse error: {}", e));
                    }
                }
            } else if kind == 1 {
                // Prepared statement ID
                let id_len = body.get_u16();
                let id = body.copy_to_bytes(id_len as usize).to_vec();
                
                // Get prepared statement
                let statement_to_execute = {
                    let prepared_statements = self.prepared_statements.read().await;
                    prepared_statements.get(&id).map(|p| p.statement.clone())
                };
                
                if let Some(statement) = statement_to_execute {
                    match self.execute_statement(&statement, frame.stream).await {
                        Ok(_) => {} // Statement executed successfully
                        Err(e) => {
                            errors.push(format!("Batch prepared statement error: {}", e));
                        }
                    }
                } else {
                    errors.push("Prepared statement not found".to_string());
                }
            }
            
            // Skip query parameters for this statement (simplified - we don't use them in batch)
            // In a full implementation, we'd decode and use QueryParameters here
        }
        
        // Read consistency level (not used for execution, but must be read)
        let _consistency = body.get_u16();
        
        // If there were errors, return error response
        if !errors.is_empty() {
            return Ok(build_error_response(
                frame.stream,
                super::protocol::error_codes::INVALID,
                &format!("Batch execution errors: {}", errors.join("; ")),
            ));
        }
        
        // Return VOID result for successful batch
        Ok(build_void_result(frame.stream))
    }

    /// Execute a CQL statement
    #[cfg_attr(test, allow(dead_code))]
    pub async fn execute_statement(
        &self,
        statement: &CqlStatement,
        stream: i16,
    ) -> ProtocolResult<CqlFrame> {
        match statement {
            CqlStatement::Select {
                columns,
                table,
                where_clause,
                limit,
                ..
            } => {
                // Convert CQL SELECT to SQL and execute
                let sql_columns = if columns.contains(&"*".to_string()) {
                    "*".to_string()
                } else {
                    columns.join(", ")
                };

                let mut sql = format!("SELECT {} FROM {}", sql_columns, table);
                
                // Add WHERE clause
                if let Some(conditions) = where_clause {
                    if !conditions.is_empty() {
                        sql.push_str(" WHERE ");
                        let where_parts: Vec<String> = conditions.iter()
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
                            crate::postgres_wire::QueryResult::Select { columns, rows } => {
                                // Convert Vec<Vec<Option<String>>> to Vec<HashMap<String, SqlValue>>
                                let cql_rows: Vec<HashMap<String, SqlValue>> = rows
                                    .into_iter()
                                    .map(|row| {
                                        let mut map = HashMap::new();
                                        for (i, col) in columns.iter().enumerate() {
                                            if let Some(val_str) = &row.get(i).and_then(|v| v.as_ref()) {
                                                // Try to parse as appropriate type
                                                let sql_val = if let Ok(int_val) = val_str.parse::<i32>() {
                                                    SqlValue::Integer(int_val)
                                                } else if let Ok(bigint_val) = val_str.parse::<i64>() {
                                                    SqlValue::BigInt(bigint_val)
                                                } else if let Ok(bool_val) = val_str.parse::<bool>() {
                                                    SqlValue::Boolean(bool_val)
                                                } else if let Ok(float_val) = val_str.parse::<f64>() {
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
                                Ok(self.build_rows_result(stream, cql_rows))
                            }
                            _ => Ok(self.build_rows_result(stream, vec![])),
                        }
                    }
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::Insert {
                table,
                columns,
                values,
                ..
            } => {
                // Convert CQL INSERT to SQL and execute
                let col_str = if columns.is_empty() {
                    "".to_string()
                } else {
                    format!("({})", columns.join(", "))
                };
                
                let val_str = if values.is_empty() {
                    "".to_string()
                } else {
                    let val_parts: Vec<String> = values.iter()
                        .map(|v| match v {
                            CqlValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
                            CqlValue::Int(i) => i.to_string(),
                            CqlValue::Bigint(i) => i.to_string(),
                            CqlValue::Boolean(b) => b.to_string(),
                            CqlValue::Float(f) => f.to_string(),
                            CqlValue::Double(f) => f.to_string(),
                            CqlValue::Timestamp(ts) => (ts / 1000).to_string(),
                            CqlValue::Null => "NULL".to_string(),
                            _ => format!("'{:?}'", v),
                        })
                        .collect();
                    format!(" VALUES ({})", val_parts.join(", "))
                };
                
                let sql = format!("INSERT INTO {}{}{}", table, col_str, val_str);
                
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => Ok(build_void_result(stream)),
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::Update {
                table,
                assignments,
                where_clause,
                ..
            } => {
                // Convert CQL UPDATE to SQL and execute
                let set_parts: Vec<String> = assignments.iter()
                    .map(|(col, val)| {
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
                        format!("{} = {}", col, val_str)
                    })
                    .collect();
                
                let mut sql = format!("UPDATE {} SET {}", table, set_parts.join(", "));
                
                if !where_clause.is_empty() {
                    sql.push_str(" WHERE ");
                    let where_parts: Vec<String> = where_clause.iter()
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
                    sql.push_str(&where_parts.join(" AND "));
                }
                
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(_) => Ok(build_void_result(stream)),
                    Err(e) => Ok(build_error_from_protocol_error(stream, &e)),
                }
            }
            CqlStatement::Delete {
                table,
                where_clause,
                ..
            } => {
                // Convert CQL DELETE to SQL and execute
                let mut sql = format!("DELETE FROM {}", table);
                
                if !where_clause.is_empty() {
                    sql.push_str(" WHERE ");
                    let where_parts: Vec<String> = where_clause.iter()
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
                    sql.push_str(&where_parts.join(" AND "));
                }
                
                #[cfg(test)]
                println!("[CQL] Executing DELETE SQL: {}", sql);
                
                match self.query_engine.execute_sql_direct(&sql).await {
                    Ok(result) => {
                        #[cfg(test)]
                        println!("[CQL] DELETE result: {:?}", result);
                        // Check if DELETE actually deleted rows
                        match result {
                            crate::postgres_wire::QueryResult::Delete { count } => {
                                #[cfg(test)]
                                println!("[CQL] DELETE affected {} rows", count);
                                #[cfg(not(test))]
                                let _ = count; // Suppress unused variable warning in non-test builds
                            }
                            _ => {
                                #[cfg(test)]
                                println!("[CQL] DELETE returned unexpected result type");
                            }
                        }
                        Ok(build_void_result(stream))
                    }
                    Err(e) => {
                        #[cfg(test)]
                        println!("[CQL] DELETE error: {:?}", e);
                        Ok(build_error_from_protocol_error(stream, &e))
                    }
                }
            }
            CqlStatement::CreateKeyspace { name, .. } => {
                let mut parser = self.parser.write().await;
                parser.set_keyspace(name.clone());
                println!("[CQL] CREATE KEYSPACE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::CreateTable { name, .. } => {
                println!("[CQL] CREATE TABLE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropKeyspace { name, .. } => {
                println!("[CQL] DROP KEYSPACE {}", name);
                Ok(self.build_schema_change_result(stream))
            }
            CqlStatement::DropTable { name, .. } => {
                println!("[CQL] DROP TABLE {}", name);
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
                Ok(build_void_result(stream))
            }
            CqlStatement::Batch { .. } => Ok(build_void_result(stream)),
        }
    }

    /// Build a ROWS result
    fn build_rows_result(&self, stream: i16, rows: Vec<HashMap<String, SqlValue>>) -> CqlFrame {
        let mut body = BytesMut::new();
        body.put_i32(0x0002); // RESULT::Rows

        if rows.is_empty() {
            // No rows - return empty result
            body.put_i32(0x0001); // Global tables spec
            body.put_i32(0); // Column count
            body.put_i32(0); // Row count
            return CqlFrame::response(stream, CqlOpcode::Result, body.freeze());
        }

        // Extract column names from first row
        let column_names: Vec<String> = rows[0].keys().cloned().collect();
        let column_count = column_names.len() as i32;

        // Metadata flags (0x0001 = global tables spec)
        body.put_i32(0x0001);

        // Column count
        body.put_i32(column_count);

        // Column metadata (simplified - all as TEXT for now)
        for col_name in &column_names {
            // Keyspace name (empty string)
            body.put_u16(0);
            // Table name (empty string)
            body.put_u16(0);
            // Column name
            body.put_u16(col_name.len() as u16);
            body.put(col_name.as_bytes());
            // Column type (0x0003 = VARCHAR/TEXT)
            body.put_i32(0x0003);
        }

        // Row count
        body.put_i32(rows.len() as i32);

        // Encode rows
        for row in &rows {
            for col_name in &column_names {
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
            SqlValue::Null => vec![],
            _ => format!("{}", value).as_bytes().to_vec(),
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

    /// Build a SCHEMA_CHANGE result
    fn build_schema_change_result(&self, stream: i16) -> CqlFrame {
        let mut body = BytesMut::new();
        body.put_i32(0x0005); // RESULT::SchemaChange

        // Change type (CREATED)
        body.put_u16(7);
        body.put(&b"CREATED"[..]);

        // Target (KEYSPACE)
        body.put_u16(8);
        body.put(&b"KEYSPACE"[..]);

        // Keyspace name
        body.put_u16(7);
        body.put(&b"default"[..]);

        CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
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

        let result = adapter.execute_statement(&statement, 0).await.unwrap();
        assert_eq!(result.opcode, CqlOpcode::Result);

        // Verify keyspace was set
        let parser = adapter.parser.read().await;
        assert_eq!(parser.current_keyspace(), Some("test_ks"));
    }
}
