//! MySQL protocol adapter implementation

use super::auth::{AuthPlugin, AuthState, HandshakeResponse, MySqlAuth};
use super::packet::MySqlPacket;
use super::protocol::{build_handshake, MySqlCommand, MySqlPacket as MySqlPacketBuilder};
use super::types::MySqlType;
use super::MySqlConfig;
use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::types::{SqlType, SqlValue};
use crate::postgres_wire::storage::memory::MemoryTableStorage;
use crate::postgres_wire::SqlEngine;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// Prepared statement information
#[derive(Debug, Clone)]
struct PreparedStatement {
    statement_id: u32,
    query: String,
    num_params: u16,
    num_columns: u16,
}

/// MySQL protocol adapter
pub struct MySqlAdapter {
    config: MySqlConfig,
    sql_engine: Arc<RwLock<SqlEngine>>,
    storage: Arc<dyn crate::postgres_wire::storage::TableStorage>,
    prepared_statements: Arc<RwLock<HashMap<u32, PreparedStatement>>>,
    next_statement_id: Arc<RwLock<u32>>,
}

impl MySqlAdapter {
    /// Create a new MySQL adapter
    pub async fn new(config: MySqlConfig) -> ProtocolResult<Self> {
        let storage = Arc::new(MemoryTableStorage::new());
        let sql_engine = SqlEngine::new();

        Ok(Self {
            config,
            sql_engine: Arc::new(RwLock::new(sql_engine)),
            storage,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            next_statement_id: Arc::new(RwLock::new(1)),
        })
    }

    /// Start the MySQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        println!(
            "[MySQL] Server listening on {}",
            self.config.listen_addr
        );

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    println!("[MySQL] New connection from: {}", addr);
                    let adapter = self.clone_for_connection();
                    tokio::spawn(async move {
                        if let Err(e) = adapter.handle_connection(socket).await {
                            eprintln!("[MySQL] Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[MySQL] Accept error: {}", e);
                }
            }
        }
    }

    /// Clone adapter for a new connection
    fn clone_for_connection(&self) -> Self {
        Self {
            config: self.config.clone(),
            sql_engine: Arc::clone(&self.sql_engine),
            storage: Arc::clone(&self.storage),
            prepared_statements: Arc::clone(&self.prepared_statements),
            next_statement_id: Arc::clone(&self.next_statement_id),
        }
    }

    /// Handle a client connection
    async fn handle_connection(&self, mut socket: TcpStream) -> ProtocolResult<()> {
        // Generate connection ID using timestamp
        let connection_id = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32) % u32::MAX;

        // Create authentication handler
        let mut auth = MySqlAuth::new(AuthPlugin::NativePassword);

        // Send handshake
        let handshake = build_handshake(connection_id, &self.config.server_version);
        let packet = MySqlPacket::new(0, handshake);
        socket
            .write_all(&packet.encode())
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        let mut sequence_id = 1u8;

        loop {
            // Read packet
            let packet = match self.read_packet(&mut socket).await {
                Ok(p) => p,
                Err(ProtocolError::ConnectionClosed) => {
                    println!("[MySQL] Connection closed by client");
                    return Ok(());
                }
                Err(e) => return Err(e),
            };

            sequence_id = packet.sequence_id.wrapping_add(1);

            // Handle authentication first
            if auth.state() != &AuthState::Authenticated {
                match HandshakeResponse::parse(packet.payload.clone()) {
                    Ok(response) => {
                        match auth.process_handshake(response) {
                            Ok(true) => {
                                // Send OK packet
                                let ok = MySqlPacketBuilder::ok(0, 0);
                                let response_packet = MySqlPacket::new(sequence_id, ok);
                                socket
                                    .write_all(&response_packet.encode())
                                    .await
                                    .map_err(|e| ProtocolError::IoError(e.to_string()))?;
                                sequence_id = sequence_id.wrapping_add(1);
                            }
                            Ok(false) => {
                                // Send error packet
                                let err =
                                    MySqlPacketBuilder::error(1045, "Access denied");
                                let response_packet = MySqlPacket::new(sequence_id, err);
                                socket
                                    .write_all(&response_packet.encode())
                                    .await
                                    .map_err(|e| ProtocolError::IoError(e.to_string()))?;
                                return Ok(());
                            }
                            Err(e) => {
                                let err = MySqlPacketBuilder::error(
                                    1045,
                                    &format!("Authentication error: {}", e),
                                );
                                let response_packet = MySqlPacket::new(sequence_id, err);
                                socket
                                    .write_all(&response_packet.encode())
                                    .await
                                    .map_err(|e| ProtocolError::IoError(e.to_string()))?;
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[MySQL] Failed to parse handshake response: {}", e);
                        let err = MySqlPacketBuilder::error(
                            1043,
                            "Invalid handshake response",
                        );
                        let response_packet = MySqlPacket::new(sequence_id, err);
                        socket
                            .write_all(&response_packet.encode())
                            .await
                            .map_err(|e| ProtocolError::IoError(e.to_string()))?;
                        return Ok(());
                    }
                }
                continue;
            }

            // Handle commands
            let response = self.handle_command(&packet).await?;

            for response_payload in response {
                let response_packet = MySqlPacket::new(sequence_id, response_payload);
                socket
                    .write_all(&response_packet.encode())
                    .await
                    .map_err(|e| ProtocolError::IoError(e.to_string()))?;
                sequence_id = sequence_id.wrapping_add(1);
            }
        }
    }

    /// Read a MySQL packet from the socket
    async fn read_packet(&self, socket: &mut TcpStream) -> ProtocolResult<MySqlPacket> {
        // Read header (4 bytes)
        let mut header = [0u8; 4];
        socket
            .read_exact(&mut header)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    ProtocolError::ConnectionClosed
                } else {
                    ProtocolError::IoError(e.to_string())
                }
            })?;

        // Parse header
        let payload_length = u32::from_le_bytes([header[0], header[1], header[2], 0]);
        let sequence_id = header[3];

        // Read payload
        let mut payload = vec![0u8; payload_length as usize];
        socket
            .read_exact(&mut payload)
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        Ok(MySqlPacket::new(sequence_id, Bytes::from(payload)))
    }

    /// Handle a MySQL command
    async fn handle_command(&self, packet: &MySqlPacket) -> ProtocolResult<Vec<Bytes>> {
        if packet.payload.is_empty() {
            return Err(ProtocolError::IncompleteFrame);
        }

        let mut payload = packet.payload.clone();
        let command_byte = payload.get_u8();

        let command = MySqlCommand::from_u8(command_byte)?;

        match command {
            MySqlCommand::Query => self.handle_query(payload).await,
            MySqlCommand::Quit => {
                // Client is disconnecting
                Err(ProtocolError::ConnectionClosed)
            }
            MySqlCommand::Ping => {
                // Respond with OK
                Ok(vec![MySqlPacketBuilder::ok(0, 0)])
            }
            MySqlCommand::InitDb => {
                // Change database
                let db_name = String::from_utf8(payload.to_vec())
                    .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;
                println!("[MySQL] Switching to database: {}", db_name);
                Ok(vec![MySqlPacketBuilder::ok(0, 0)])
            }
            MySqlCommand::StmtPrepare => self.handle_prepare(payload).await,
            MySqlCommand::StmtExecute => self.handle_execute(payload).await,
            MySqlCommand::StmtClose => self.handle_stmt_close(payload).await,
            _ => {
                // Unsupported command
                Ok(vec![MySqlPacketBuilder::error(
                    1047,
                    &format!("Unknown command: {:?}", command),
                )])
            }
        }
    }

    /// Handle COM_QUERY
    async fn handle_query(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        let query = String::from_utf8(payload.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        println!("[MySQL] Query: {}", query);

        // Parse and execute using SQL engine
        match self.sql_engine.write().await.execute(&query).await {
            Ok(result) => self.build_result_set(result),
            Err(e) => Ok(vec![MySqlPacketBuilder::error(
                1064,
                &format!("Query error: {}", e),
            )]),
        }
    }

    /// Handle COM_STMT_PREPARE
    async fn handle_prepare(&self, payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        let query = String::from_utf8(payload.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        println!("[MySQL] Prepare: {}", query);

        // Generate statement ID
        let mut next_id = self.next_statement_id.write().await;
        let statement_id = *next_id;
        *next_id += 1;
        drop(next_id);

        // Store prepared statement
        let stmt = PreparedStatement {
            statement_id,
            query: query.clone(),
            num_params: 0, // Would need to parse query to determine
            num_columns: 0,
        };

        self.prepared_statements
            .write()
            .await
            .insert(statement_id, stmt);

        // Build COM_STMT_PREPARE_OK response
        let mut response = BytesMut::new();
        response.put_u8(0x00); // OK status
        response.put_u32_le(statement_id);
        response.put_u16_le(0); // num_columns
        response.put_u16_le(0); // num_params
        response.put_u8(0); // filler
        response.put_u16_le(0); // warning_count

        Ok(vec![response.freeze()])
    }

    /// Handle COM_STMT_EXECUTE
    async fn handle_execute(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        if payload.len() < 4 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let statement_id = payload.get_u32_le();

        println!("[MySQL] Execute statement: {}", statement_id);

        // Look up prepared statement
        let statements = self.prepared_statements.read().await;
        let stmt = statements
            .get(&statement_id)
            .ok_or_else(|| ProtocolError::InvalidStatement("Statement not found".to_string()))?;

        let query = stmt.query.clone();
        drop(statements);

        // Execute query
        match self.sql_engine.write().await.execute(&query).await {
            Ok(result) => self.build_result_set(result),
            Err(e) => Ok(vec![MySqlPacketBuilder::error(
                1064,
                &format!("Execute error: {}", e),
            )]),
        }
    }

    /// Handle COM_STMT_CLOSE
    async fn handle_stmt_close(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        if payload.len() < 4 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let statement_id = payload.get_u32_le();

        println!("[MySQL] Close statement: {}", statement_id);

        self.prepared_statements.write().await.remove(&statement_id);

        // No response for COM_STMT_CLOSE
        Ok(vec![])
    }

    /// Build result set from SQL execution result
    fn build_result_set(
        &self,
        result: crate::postgres_wire::sql::UnifiedExecutionResult,
    ) -> ProtocolResult<Vec<Bytes>> {
        use crate::postgres_wire::sql::UnifiedExecutionResult;

        match result {
            UnifiedExecutionResult::Select { columns, rows, .. } => {
                let mut packets = Vec::new();

                // Column count packet
                let mut col_count = BytesMut::new();
                super::packet::write_lenenc_int(&mut col_count, columns.len() as u64);
                packets.push(col_count.freeze());

                // Column definition packets
                for column in &columns {
                    let mysql_type = MySqlType::VarString; // Default type
                    let col_def = MySqlPacketBuilder::column_definition(
                        "def",
                        "orbit",
                        "table",
                        "table",
                        column,
                        column,
                        mysql_type,
                    );
                    packets.push(col_def);
                }

                // EOF packet after columns
                packets.push(MySqlPacketBuilder::eof());

                // Row data packets
                for row in rows {
                    packets.push(MySqlPacketBuilder::text_row(&row));
                }

                // EOF packet after rows
                packets.push(MySqlPacketBuilder::eof());

                Ok(packets)
            }
            UnifiedExecutionResult::Insert { count, .. } => {
                Ok(vec![MySqlPacketBuilder::ok(count as u64, 0)])
            }
            UnifiedExecutionResult::Update { count, .. } => {
                Ok(vec![MySqlPacketBuilder::ok(count as u64, 0)])
            }
            UnifiedExecutionResult::Delete { count, .. } => {
                Ok(vec![MySqlPacketBuilder::ok(count as u64, 0)])
            }
            UnifiedExecutionResult::CreateTable { .. } => Ok(vec![MySqlPacketBuilder::ok(0, 0)]),
            _ => Ok(vec![MySqlPacketBuilder::ok(0, 0)]),
        }
    }

    /// Convert SQL type to MySQL type
    fn sql_type_to_mysql_type(&self, sql_type: &SqlType) -> MySqlType {
        match sql_type {
            SqlType::SmallInt => MySqlType::Short,
            SqlType::Integer => MySqlType::Long,
            SqlType::BigInt => MySqlType::LongLong,
            SqlType::Real => MySqlType::Float,
            SqlType::DoublePrecision => MySqlType::Double,
            SqlType::Decimal { .. } => MySqlType::NewDecimal,
            SqlType::Varchar(_) | SqlType::Char(_) | SqlType::Text => MySqlType::VarString,
            SqlType::Bytea => MySqlType::Blob,
            SqlType::Boolean => MySqlType::Tiny,
            SqlType::Date => MySqlType::Date,
            SqlType::Time { .. } => MySqlType::Time,
            SqlType::Timestamp { .. } => MySqlType::Timestamp,
            SqlType::Json => MySqlType::Json,
            _ => MySqlType::VarString,
        }
    }

    /// Convert SQL value to string for text protocol
    fn sql_value_to_string(&self, value: &SqlValue) -> Option<String> {
        match value {
            SqlValue::Null => None,
            SqlValue::Boolean(b) => Some(if *b { "1".to_string() } else { "0".to_string() }),
            SqlValue::SmallInt(i) => Some(i.to_string()),
            SqlValue::Integer(i) => Some(i.to_string()),
            SqlValue::BigInt(i) => Some(i.to_string()),
            SqlValue::Real(f) => Some(f.to_string()),
            SqlValue::DoublePrecision(f) => Some(f.to_string()),
            SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => Some(s.clone()),
            SqlValue::Bytea(b) => Some(hex::encode(b)),
            SqlValue::Date(d) => Some(d.to_string()),
            SqlValue::Time(t) => Some(t.to_string()),
            SqlValue::Timestamp(ts) => Some(ts.to_string()),
            _ => Some(format!("{:?}", value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await;
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_type_conversion() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config);
        // Type conversion tests would go here
    }
}
