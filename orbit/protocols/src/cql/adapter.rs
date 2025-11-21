//! CQL adapter implementation
//!
//! This module provides the main CQL adapter that handles client connections
//! and translates CQL operations to Orbit engine calls.

use super::parser::{CqlParser, CqlStatement};
use super::protocol::{
    build_error_response, build_ready_response, build_supported_response, build_void_result,
    read_string, read_string_map, CqlFrame, CqlOpcode, QueryParameters,
};
use super::CqlConfig;
use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::SqlEngine;
use crate::postgres_wire::sql::types::SqlValue;
use crate::postgres_wire::storage::memory::MemoryTableStorage;
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
    storage: Arc<dyn crate::postgres_wire::storage::TableStorage>,
    /// Parser
    parser: Arc<RwLock<CqlParser>>,
    /// SQL engine
    sql_engine: Arc<SqlEngine>,
    /// Prepared statements
    prepared_statements: Arc<RwLock<HashMap<Vec<u8>, PreparedStatement>>>,
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
    /// Create a new CQL adapter
    pub async fn new(config: CqlConfig) -> ProtocolResult<Self> {
        let storage = Arc::new(MemoryTableStorage::new());
        let sql_engine = Arc::new(SqlEngine::new());

        Ok(Self {
            config,
            storage,
            parser: Arc::new(RwLock::new(CqlParser::new())),
            sql_engine,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
        })
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
            sql_engine: self.sql_engine.clone(),
            prepared_statements: self.prepared_statements.clone(),
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
        match frame.opcode {
            CqlOpcode::Startup => self.handle_startup(frame).await,
            CqlOpcode::Options => self.handle_options(frame).await,
            CqlOpcode::Query => self.handle_query(frame).await,
            CqlOpcode::Prepare => self.handle_prepare(frame).await,
            CqlOpcode::Execute => self.handle_execute(frame).await,
            CqlOpcode::Batch => self.handle_batch(frame).await,
            _ => Ok(build_error_response(
                frame.stream,
                0x000A,
                "Unsupported opcode",
            )),
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

    /// Handle QUERY request
    async fn handle_query(&self, frame: &CqlFrame) -> ProtocolResult<CqlFrame> {
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
        let prepared = PreparedStatement {
            id: id.clone(),
            query: query.clone(),
            statement,
        };
        self.prepared_statements.write().await.insert(id.clone(), prepared);

        // Build PREPARED response
        let mut response_body = BytesMut::new();
        response_body.put_i32(0x0004); // RESULT::Prepared
        response_body.put_u16(id.len() as u16);
        response_body.put(&id[..]);

        // TODO: Add metadata

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
        // Simplified: just return VOID for now
        Ok(build_void_result(frame.stream))
    }

    /// Execute a CQL statement
    async fn execute_statement(
        &self,
        statement: &CqlStatement,
        stream: i16,
    ) -> ProtocolResult<CqlFrame> {
        match statement {
            CqlStatement::Select {
                columns,
                table,
                where_clause: _,
                limit,
                ..
            } => {
                // Build SQL SELECT
                let sql_columns = if columns.contains(&"*".to_string()) {
                    "*".to_string()
                } else {
                    columns.join(", ")
                };

                let mut sql = format!("SELECT {} FROM {}", sql_columns, table);
                if let Some(lim) = limit {
                    sql.push_str(&format!(" LIMIT {}", lim));
                }

                // Execute query (simplified - not using actual executor yet)
                Ok(self.build_rows_result(stream, vec![]))
            }
            CqlStatement::Insert { table, .. } => {
                // Simplified: just return VOID
                println!("[CQL] INSERT INTO {}", table);
                Ok(build_void_result(stream))
            }
            CqlStatement::Update { table, .. } => {
                println!("[CQL] UPDATE {}", table);
                Ok(build_void_result(stream))
            }
            CqlStatement::Delete { table, .. } => {
                println!("[CQL] DELETE FROM {}", table);
                Ok(build_void_result(stream))
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
    fn build_rows_result(&self, stream: i16, _rows: Vec<HashMap<String, SqlValue>>) -> CqlFrame {
        let mut body = BytesMut::new();
        body.put_i32(0x0002); // RESULT::Rows

        // Metadata flags
        body.put_i32(0x0001); // Global tables spec

        // Column count
        body.put_i32(0);

        // Row count
        body.put_i32(0);

        CqlFrame::response(stream, CqlOpcode::Result, body.freeze())
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
