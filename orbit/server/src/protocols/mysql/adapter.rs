//! MySQL protocol adapter implementation

use super::auth::{AuthPlugin, AuthState, HandshakeResponse, MySqlAuth};
use super::packet::MySqlPacket;
use super::protocol::{build_handshake, MySqlCommand, MySqlPacket as MySqlPacketBuilder};
use super::types::MySqlType;
use super::MySqlConfig;
use crate::protocols::common::storage::memory::MemoryTableStorage;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::types::{SqlType, SqlValue};
use crate::protocols::postgres_wire::SqlEngine;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// Prepared statement information
#[derive(Debug, Clone)]
struct PreparedStatement {
    #[allow(dead_code)]
    statement_id: u32,
    query: String,
    num_params: u16,
    #[allow(dead_code)]
    num_columns: u16,
    param_types: Vec<super::types::MySqlType>, // Parameter types
    /// Long data sent via COM_STMT_SEND_LONG_DATA (param_id -> accumulated data)
    long_data: HashMap<u16, Vec<u8>>,
}

/// MySQL metrics
#[derive(Debug, Clone, Default)]
pub struct MySqlMetrics {
    /// Total queries executed
    pub total_queries: u64,
    /// Total errors encountered
    pub total_errors: u64,
    /// Active connections
    pub active_connections: usize,
    /// Prepared statements count
    pub prepared_statements_count: usize,
}

/// MySQL protocol adapter
pub struct MySqlAdapter {
    config: MySqlConfig,
    pub(crate) sql_engine: Arc<RwLock<SqlEngine>>,
    storage: Arc<dyn crate::protocols::common::storage::TableStorage>,
    prepared_statements: Arc<RwLock<HashMap<u32, PreparedStatement>>>,
    next_statement_id: Arc<RwLock<u32>>,
    metrics: Arc<RwLock<MySqlMetrics>>,
}

impl MySqlAdapter {
    /// Create a new MySQL adapter with shared storage
    pub async fn new_with_storage(
        config: MySqlConfig,
        storage: Arc<dyn crate::protocols::common::storage::TableStorage>,
    ) -> ProtocolResult<Self> {
        let sql_engine = SqlEngine::new();

        Ok(Self {
            config,
            sql_engine: Arc::new(RwLock::new(sql_engine)),
            storage,
            prepared_statements: Arc::new(RwLock::new(HashMap::new())),
            next_statement_id: Arc::new(RwLock::new(1)),
            metrics: Arc::new(RwLock::new(MySqlMetrics::default())),
        })
    }

    /// Create a new MySQL adapter (creates its own isolated storage)
    /// For backward compatibility. Use new_with_storage() to share storage with other protocols.
    pub async fn new(config: MySqlConfig) -> ProtocolResult<Self> {
        let storage = Arc::new(MemoryTableStorage::new());
        Self::new_with_storage(config, storage).await
    }

    /// Start the MySQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        println!("[MySQL] Server listening on {}", self.config.listen_addr);

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
        // Note: Metrics will be updated in handle_connection

        Self {
            config: self.config.clone(),
            sql_engine: Arc::clone(&self.sql_engine),
            storage: Arc::clone(&self.storage),
            prepared_statements: Arc::clone(&self.prepared_statements),
            next_statement_id: Arc::clone(&self.next_statement_id),
            metrics: Arc::clone(&self.metrics),
        }
    }

    /// Handle a client connection
    async fn handle_connection(&self, mut socket: TcpStream) -> ProtocolResult<()> {
        // Update metrics for new connection
        {
            let mut metrics = self.metrics.write().await;
            metrics.active_connections += 1;
        }

        // Generate connection ID using timestamp
        let connection_id = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32)
            % u32::MAX;

        // Create authentication handler with credentials if enabled
        let mut auth = if self.config.authentication_enabled {
            MySqlAuth::with_credentials(
                AuthPlugin::NativePassword,
                self.config.username.clone(),
                self.config.password.clone(),
            )
        } else {
            MySqlAuth::new(AuthPlugin::NativePassword)
        };

        // Send handshake
        let handshake = build_handshake(connection_id, &self.config.server_version);
        let packet = MySqlPacket::new(0, handshake);
        socket
            .write_all(&packet.encode())
            .await
            .map_err(|e| ProtocolError::IoError(e.to_string()))?;

        // Initialize sequence_id - will be set from first packet
        // The initial value is overwritten before use, but needed for variable initialization
        #[allow(unused_assignments)]
        let mut sequence_id: u8 = 0;

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

            // Update sequence_id for response packets (increment from received packet's sequence)
            // This value is used to create response packets below
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
                                // Note: sequence_id will be set from next packet's sequence_id on next loop iteration
                            }
                            Ok(false) => {
                                // Send error packet
                                let err = MySqlPacketBuilder::error(1045, "Access denied");
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
                        let err = MySqlPacketBuilder::error(1043, "Invalid handshake response");
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
            let response = match self.handle_command(&packet).await {
                Ok(resp) => resp,
                Err(e) => {
                    // Update error metrics
                    {
                        let mut metrics = self.metrics.write().await;
                        metrics.total_errors += 1;
                    }
                    return Err(e);
                }
            };

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
        socket.read_exact(&mut header).await.map_err(|e| {
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
            MySqlCommand::StmtSendLongData => self.handle_stmt_send_long_data(payload).await,
            MySqlCommand::StmtClose => self.handle_stmt_close(payload).await,
            MySqlCommand::StmtReset => self.handle_stmt_reset(payload).await,
            MySqlCommand::StmtFetch => self.handle_stmt_fetch(payload).await,
            MySqlCommand::FieldList => self.handle_field_list(payload).await,
            MySqlCommand::Statistics => self.handle_statistics().await,
            MySqlCommand::CreateDb => self.handle_create_db(payload).await,
            MySqlCommand::DropDb => self.handle_drop_db(payload).await,
            MySqlCommand::Refresh => self.handle_refresh(payload).await,
            MySqlCommand::SetOption => self.handle_set_option(payload).await,
            MySqlCommand::ResetConnection => self.handle_reset_connection().await,
            _ => {
                // Unsupported command
                Ok(vec![MySqlPacketBuilder::error(
                    super::protocol::error_codes::ER_UNKNOWN_COM_ERROR,
                    &format!("Unknown command: {:?}", command),
                )])
            }
        }
    }

    /// Handle MySQL-specific queries (SHOW commands, INFORMATION_SCHEMA queries, SET commands)
    async fn handle_mysql_specific_query(&self, query: &str) -> Option<ProtocolResult<Vec<Bytes>>> {
        let query_upper = query.trim().to_uppercase();

        // Handle SET NAMES with COLLATE (e.g., SET NAMES 'utf8mb4' COLLATE 'utf8mb4_general_ci')
        if query_upper.starts_with("SET NAMES") {
            println!("[MySQL] Handling SET NAMES command (ignoring)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET with COLLATE (e.g., SET collation_connection = ...)
        if query_upper.starts_with("SET ") && query_upper.contains("COLLAT") {
            println!("[MySQL] Handling SET COLLATION command (ignoring)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET @@SESSION or SET @@GLOBAL variables
        if query_upper.starts_with("SET @@") {
            println!("[MySQL] Handling SET @@variable command (ignoring)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET SESSION or SET GLOBAL
        if query_upper.starts_with("SET SESSION") || query_upper.starts_with("SET GLOBAL") {
            println!("[MySQL] Handling SET SESSION/GLOBAL command (ignoring)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET character_set_* commands
        if query_upper.starts_with("SET CHARACTER_SET") || query_upper.contains("CHARACTER_SET") {
            println!("[MySQL] Handling SET character_set command (ignoring)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SELECT @@version and similar system variables
        if query_upper.starts_with("SELECT @@") {
            return Some(self.handle_select_system_variable(&query_upper));
        }

        // Handle SHOW DATABASES
        if query_upper.starts_with("SHOW DATABASES") {
            println!("[MySQL] Handling SHOW DATABASES");
            return Some(self.build_show_databases_result());
        }

        // Handle SHOW TABLES (with optional FROM database)
        if query_upper.starts_with("SHOW TABLES") {
            println!("[MySQL] Handling SHOW TABLES");
            return Some(self.build_show_tables_result().await);
        }

        // Handle SHOW VARIABLES
        if query_upper.starts_with("SHOW VARIABLES") || query_upper.starts_with("SHOW SESSION VARIABLES") {
            println!("[MySQL] Handling SHOW VARIABLES");
            return Some(self.build_show_variables_result());
        }

        // Handle SHOW STATUS
        if query_upper.starts_with("SHOW STATUS") || query_upper.starts_with("SHOW SESSION STATUS") {
            println!("[MySQL] Handling SHOW STATUS");
            return Some(self.build_show_status_result());
        }

        // Handle SHOW COLLATION
        if query_upper.starts_with("SHOW COLLATION") {
            println!("[MySQL] Handling SHOW COLLATION");
            return Some(self.build_show_collation_result());
        }

        // Handle SHOW CHARACTER SET
        if query_upper.starts_with("SHOW CHARACTER SET") || query_upper.starts_with("SHOW CHARSET") {
            println!("[MySQL] Handling SHOW CHARACTER SET");
            return Some(self.build_show_charset_result());
        }

        // Handle INFORMATION_SCHEMA.TABLES queries
        if query_upper.contains("INFORMATION_SCHEMA.TABLES") {
            println!("[MySQL] Handling INFORMATION_SCHEMA.TABLES query");
            return Some(self.build_information_schema_tables_result().await);
        }

        // Handle INFORMATION_SCHEMA.SCHEMATA queries
        if query_upper.contains("INFORMATION_SCHEMA.SCHEMATA") {
            println!("[MySQL] Handling INFORMATION_SCHEMA.SCHEMATA query");
            return Some(self.build_information_schema_schemata_result());
        }

        // Handle INFORMATION_SCHEMA.COLLATIONS queries
        if query_upper.contains("INFORMATION_SCHEMA.COLLATIONS") {
            println!("[MySQL] Handling INFORMATION_SCHEMA.COLLATIONS query");
            return Some(self.build_show_collation_result());
        }

        None // Not a MySQL-specific query, let SQL engine handle it
    }

    /// Handle SELECT @@variable queries
    fn handle_select_system_variable(&self, query_upper: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Extract variable name
        let var_name = query_upper
            .trim_start_matches("SELECT ")
            .trim_start_matches("@@")
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches(',')
            .to_lowercase();

        let (column_name, value) = match var_name.as_str() {
            "version" => ("@@version", "8.0.27-Orbit-DB"),
            "version_comment" => ("@@version_comment", "Orbit-DB MySQL Protocol"),
            "max_allowed_packet" => ("@@max_allowed_packet", "67108864"),
            "character_set_client" => ("@@character_set_client", "utf8mb4"),
            "character_set_connection" => ("@@character_set_connection", "utf8mb4"),
            "character_set_results" => ("@@character_set_results", "utf8mb4"),
            "character_set_server" => ("@@character_set_server", "utf8mb4"),
            "collation_connection" => ("@@collation_connection", "utf8mb4_general_ci"),
            "collation_server" => ("@@collation_server", "utf8mb4_general_ci"),
            "sql_mode" => ("@@sql_mode", "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ZERO_IN_DATE,NO_ZERO_DATE,ERROR_FOR_DIVISION_BY_ZERO,NO_ENGINE_SUBSTITUTION"),
            "autocommit" => ("@@autocommit", "1"),
            "tx_isolation" | "transaction_isolation" => ("@@transaction_isolation", "REPEATABLE-READ"),
            "wait_timeout" => ("@@wait_timeout", "28800"),
            "interactive_timeout" => ("@@interactive_timeout", "28800"),
            "session.auto_increment_increment" => ("@@session.auto_increment_increment", "1"),
            "auto_increment_increment" => ("@@auto_increment_increment", "1"),
            _ => ("@@unknown", ""),
        };

        let result = UnifiedExecutionResult::Select {
            columns: vec![column_name.to_string()],
            rows: vec![vec![Some(value.to_string())]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW VARIABLES command
    fn build_show_variables_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let variables = vec![
            ("character_set_client", "utf8mb4"),
            ("character_set_connection", "utf8mb4"),
            ("character_set_results", "utf8mb4"),
            ("character_set_server", "utf8mb4"),
            ("collation_connection", "utf8mb4_general_ci"),
            ("collation_server", "utf8mb4_general_ci"),
            ("version", "8.0.27-Orbit-DB"),
            ("version_comment", "Orbit-DB MySQL Protocol"),
            ("max_allowed_packet", "67108864"),
            ("sql_mode", "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES"),
            ("autocommit", "ON"),
        ];

        let rows: Vec<Vec<Option<String>>> = variables
            .iter()
            .map(|(name, value)| vec![Some(name.to_string()), Some(value.to_string())])
            .collect();

        let result = UnifiedExecutionResult::Select {
            columns: vec!["Variable_name".to_string(), "Value".to_string()],
            rows: rows.clone(),
            row_count: rows.len(),
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW STATUS command
    fn build_show_status_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let status = vec![
            ("Uptime", "0"),
            ("Threads_connected", "1"),
            ("Connections", "1"),
            ("Questions", "0"),
        ];

        let rows: Vec<Vec<Option<String>>> = status
            .iter()
            .map(|(name, value)| vec![Some(name.to_string()), Some(value.to_string())])
            .collect();

        let result = UnifiedExecutionResult::Select {
            columns: vec!["Variable_name".to_string(), "Value".to_string()],
            rows: rows.clone(),
            row_count: rows.len(),
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW COLLATION command
    fn build_show_collation_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let collations = vec![
            ("utf8mb4_general_ci", "utf8mb4", "45", "Yes", "Yes", "1"),
            ("utf8mb4_bin", "utf8mb4", "46", "", "Yes", "1"),
            ("utf8mb4_unicode_ci", "utf8mb4", "224", "", "Yes", "8"),
            ("utf8_general_ci", "utf8", "33", "Yes", "Yes", "1"),
            ("latin1_swedish_ci", "latin1", "8", "Yes", "Yes", "1"),
        ];

        let rows: Vec<Vec<Option<String>>> = collations
            .iter()
            .map(|(col, charset, id, default, compiled, sortlen)| {
                vec![
                    Some(col.to_string()),
                    Some(charset.to_string()),
                    Some(id.to_string()),
                    Some(default.to_string()),
                    Some(compiled.to_string()),
                    Some(sortlen.to_string()),
                ]
            })
            .collect();

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Collation".to_string(),
                "Charset".to_string(),
                "Id".to_string(),
                "Default".to_string(),
                "Compiled".to_string(),
                "Sortlen".to_string(),
            ],
            rows: rows.clone(),
            row_count: rows.len(),
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW CHARACTER SET command
    fn build_show_charset_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let charsets = vec![
            ("utf8mb4", "UTF-8 Unicode", "utf8mb4_general_ci", "4"),
            ("utf8", "UTF-8 Unicode", "utf8_general_ci", "3"),
            ("latin1", "cp1252 West European", "latin1_swedish_ci", "1"),
            ("ascii", "US ASCII", "ascii_general_ci", "1"),
        ];

        let rows: Vec<Vec<Option<String>>> = charsets
            .iter()
            .map(|(charset, desc, default_col, maxlen)| {
                vec![
                    Some(charset.to_string()),
                    Some(desc.to_string()),
                    Some(default_col.to_string()),
                    Some(maxlen.to_string()),
                ]
            })
            .collect();

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Charset".to_string(),
                "Description".to_string(),
                "Default collation".to_string(),
                "Maxlen".to_string(),
            ],
            rows: rows.clone(),
            row_count: rows.len(),
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW DATABASES command
    fn build_show_databases_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec!["Database".to_string()],
            rows: vec![vec![Some("orbit".to_string())]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW TABLES command
    async fn build_show_tables_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Get table schemas directly from storage
        let table_schemas = self.storage.list_table_schemas().await.unwrap_or_default();

        // Convert to rows format
        let table_rows: Vec<Vec<Option<String>>> = table_schemas
            .iter()
            .map(|schema| vec![Some(schema.name.clone())])
            .collect();

        let result = UnifiedExecutionResult::Select {
            columns: vec!["Tables_in_orbit".to_string()],
            rows: table_rows.clone(),
            row_count: table_rows.len(),
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for INFORMATION_SCHEMA.TABLES queries
    async fn build_information_schema_tables_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Get table schemas directly from storage
        let table_schemas = self.storage.list_table_schemas().await.unwrap_or_default();

        // Build result with common INFORMATION_SCHEMA.TABLES columns
        let table_rows: Vec<Vec<Option<String>>> = table_schemas
            .iter()
            .map(|schema| {
                vec![
                    Some("def".to_string()),                // TABLE_CATALOG
                    Some("orbit".to_string()),              // TABLE_SCHEMA
                    Some(schema.name.clone()),              // TABLE_NAME
                    Some("BASE TABLE".to_string()),         // TABLE_TYPE
                    Some("Orbit".to_string()),              // ENGINE
                    Some("10".to_string()),                 // VERSION
                    Some("Dynamic".to_string()),            // ROW_FORMAT
                    Some("0".to_string()),                  // TABLE_ROWS
                    Some("0".to_string()),                  // AVG_ROW_LENGTH
                    Some("0".to_string()),                  // DATA_LENGTH
                    Some("0".to_string()),                  // MAX_DATA_LENGTH
                    Some("0".to_string()),                  // INDEX_LENGTH
                    Some("0".to_string()),                  // DATA_FREE
                    None,                                   // AUTO_INCREMENT
                    None,                                   // CREATE_TIME
                    None,                                   // UPDATE_TIME
                    None,                                   // CHECK_TIME
                    Some("utf8mb4_0900_ai_ci".to_string()), // TABLE_COLLATION
                    None,                                   // CHECKSUM
                    Some("".to_string()),                   // CREATE_OPTIONS
                    Some("".to_string()),                   // TABLE_COMMENT
                ]
            })
            .collect();

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "TABLE_CATALOG".to_string(),
                "TABLE_SCHEMA".to_string(),
                "TABLE_NAME".to_string(),
                "TABLE_TYPE".to_string(),
                "ENGINE".to_string(),
                "VERSION".to_string(),
                "ROW_FORMAT".to_string(),
                "TABLE_ROWS".to_string(),
                "AVG_ROW_LENGTH".to_string(),
                "DATA_LENGTH".to_string(),
                "MAX_DATA_LENGTH".to_string(),
                "INDEX_LENGTH".to_string(),
                "DATA_FREE".to_string(),
                "AUTO_INCREMENT".to_string(),
                "CREATE_TIME".to_string(),
                "UPDATE_TIME".to_string(),
                "CHECK_TIME".to_string(),
                "TABLE_COLLATION".to_string(),
                "CHECKSUM".to_string(),
                "CREATE_OPTIONS".to_string(),
                "TABLE_COMMENT".to_string(),
            ],
            rows: table_rows.clone(),
            row_count: table_rows.len(),
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for INFORMATION_SCHEMA.SCHEMATA queries
    fn build_information_schema_schemata_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "CATALOG_NAME".to_string(),
                "SCHEMA_NAME".to_string(),
                "DEFAULT_CHARACTER_SET_NAME".to_string(),
                "DEFAULT_COLLATION_NAME".to_string(),
                "SQL_PATH".to_string(),
            ],
            rows: vec![vec![
                Some("def".to_string()),
                Some("orbit".to_string()),
                Some("utf8mb4".to_string()),
                Some("utf8mb4_0900_ai_ci".to_string()),
                None,
            ]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Handle COM_QUERY
    async fn handle_query(&self, payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        // Validate payload is not empty
        if payload.is_empty() {
            return Err(ProtocolError::IncompleteFrame);
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_queries += 1;
        }

        let query = String::from_utf8(payload.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        // Validate query is not empty
        if query.trim().is_empty() {
            return Err(ProtocolError::ParseError("Empty query".to_string()));
        }

        println!("[MySQL] Query: {}", query);

        // Check for MySQL-specific queries first
        if let Some(result) = self.handle_mysql_specific_query(&query).await {
            return result;
        }

        // Parse and execute using SQL engine
        match self.sql_engine.write().await.execute(&query).await {
            Ok(result) => {
                // Debug: Log result structure
                if let crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                    columns,
                    rows,
                    ..
                } = &result
                {
                    println!(
                        "[MySQL] Result: {} columns, {} rows",
                        columns.len(),
                        rows.len()
                    );
                    println!("[MySQL] Columns: {:?}", columns);
                    if let Some(first_row) = rows.first() {
                        println!("[MySQL] First row: {:?}", first_row);
                    }
                }
                self.build_result_set(result)
            }
            Err(e) => {
                // Update error metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.total_errors += 1;
                }
                Ok(vec![MySqlPacketBuilder::error_from_protocol_error(&e)])
            }
        }
    }

    /// Count parameters in a query (count `?` placeholders)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn count_parameters(query: &str) -> u16 {
        query.matches('?').count() as u16
    }

    /// Handle COM_STMT_PREPARE
    async fn handle_prepare(&self, payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        let query = String::from_utf8(payload.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        println!("[MySQL] Prepare: {}", query);

        // Count parameters
        let num_params = Self::count_parameters(&query);

        // Generate statement ID
        let mut next_id = self.next_statement_id.write().await;
        let statement_id = *next_id;
        *next_id += 1;
        drop(next_id);

        // Store param_types before creating statement (to avoid borrow issues)
        let param_types = vec![super::types::MySqlType::VarString; num_params as usize]; // Default to string type

        // Store prepared statement
        let stmt = PreparedStatement {
            statement_id,
            query: query.clone(),
            num_params,
            num_columns: 0, // Will be determined when we know the result set
            param_types: param_types.clone(),
            long_data: HashMap::new(),
        };

        self.prepared_statements
            .write()
            .await
            .insert(statement_id, stmt);

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.prepared_statements_count = self.prepared_statements.read().await.len();
        }

        // Build COM_STMT_PREPARE_OK response
        let mut response = BytesMut::new();
        response.put_u8(0x00); // OK status
        response.put_u32_le(statement_id);
        response.put_u16_le(0); // num_columns (will be sent in separate packets if needed)
        response.put_u16_le(num_params);
        response.put_u8(0); // filler
        response.put_u16_le(0); // warning_count

        // If there are parameters, send parameter metadata packets
        let mut packets = vec![response.freeze()];

        if num_params > 0 {
            // Send parameter metadata packets
            // Each parameter gets a column definition packet
            for i in 0..num_params {
                let param_name = format!("?{}", i + 1);
                let param_type = param_types
                    .get(i as usize)
                    .copied()
                    .unwrap_or(super::types::MySqlType::VarString);

                let param_def = MySqlPacketBuilder::column_definition(
                    "def",       // catalog
                    "",          // schema (empty for parameters)
                    "",          // table (empty for parameters)
                    "",          // org_table (empty for parameters)
                    &param_name, // name
                    &param_name, // org_name
                    param_type,  // column_type
                );
                packets.push(param_def);
            }
        }

        Ok(packets)
    }

    /// Decode parameter value from COM_STMT_EXECUTE packet
    fn decode_parameter_value(
        &self,
        payload: &mut Bytes,
        param_type: super::types::MySqlType,
        is_null: bool,
    ) -> ProtocolResult<String> {
        if is_null {
            return Ok("NULL".to_string());
        }

        use super::packet::read_lenenc_string;
        use super::types::MySqlType;

        match param_type {
            MySqlType::Tiny => {
                if payload.is_empty() {
                    return Err(ProtocolError::IncompleteFrame);
                }
                Ok(payload.get_i8().to_string())
            }
            MySqlType::Short => {
                if payload.len() < 2 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                Ok(payload.get_i16_le().to_string())
            }
            MySqlType::Long | MySqlType::Int24 => {
                if payload.len() < 4 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                Ok(payload.get_i32_le().to_string())
            }
            MySqlType::LongLong => {
                if payload.len() < 8 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                Ok(payload.get_i64_le().to_string())
            }
            MySqlType::Float => {
                if payload.len() < 4 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                Ok(payload.get_f32_le().to_string())
            }
            MySqlType::Double => {
                if payload.len() < 8 {
                    return Err(ProtocolError::IncompleteFrame);
                }
                Ok(payload.get_f64_le().to_string())
            }
            MySqlType::VarString | MySqlType::VarChar | MySqlType::String => {
                read_lenenc_string(payload).map(|s| format!("'{}'", s.replace('\'', "''")))
            }
            MySqlType::Date | MySqlType::DateTime | MySqlType::Timestamp => {
                let len = payload.get_u8();
                let year = if len >= 4 { payload.get_u16_le() } else { 0 };
                let month = if len >= 4 { payload.get_u8() } else { 0 };
                let day = if len >= 4 { payload.get_u8() } else { 0 };
                let hour = if len >= 7 { payload.get_u8() } else { 0 };
                let minute = if len >= 7 { payload.get_u8() } else { 0 };
                let second = if len >= 7 { payload.get_u8() } else { 0 };
                let _microsecond = if len >= 11 { payload.get_u32_le() } else { 0 };

                if len == 0 {
                    Ok("'0000-00-00'".to_string())
                } else if len == 4 {
                    Ok(format!("'{:04}-{:02}-{:02}'", year, month, day))
                } else {
                    Ok(format!(
                        "'{:04}-{:02}-{:02} {:02}:{:02}:{:02}'",
                        year, month, day, hour, minute, second
                    ))
                }
            }
            MySqlType::Time => {
                let len = payload.get_u8();
                let _is_negative = if len >= 8 { payload.get_u8() } else { 0 };
                let _days = if len >= 8 { payload.get_u32_le() } else { 0 };
                let hour = if len >= 8 { payload.get_u8() } else { 0 };
                let minute = if len >= 8 { payload.get_u8() } else { 0 };
                let second = if len >= 8 { payload.get_u8() } else { 0 };
                let _microsecond = if len >= 12 { payload.get_u32_le() } else { 0 };

                // Note: We are ignoring days and microseconds for simplicity in this string representation
                // A proper implementation would handle intervals correctly
                if len == 0 {
                    Ok("'00:00:00'".to_string())
                } else {
                    Ok(format!("'{:02}:{:02}:{:02}'", hour, minute, second))
                }
            }
            _ => {
                // Default: try to read as string
                read_lenenc_string(payload).map(|s| format!("'{}'", s.replace('\'', "''")))
            }
        }
    }

    /// Handle COM_STMT_EXECUTE
    async fn handle_execute(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        if payload.len() < 5 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let statement_id = payload.get_u32_le();
        let flags = payload.get_u8(); // Flags
        let _iteration_count = payload.get_u32_le(); // Usually 1

        println!(
            "[MySQL] Execute statement: {} (flags: {})",
            statement_id, flags
        );

        // Look up prepared statement
        let statements = self.prepared_statements.read().await;
        let stmt = statements.get(&statement_id).ok_or_else(|| {
            // Note: Error metrics will be updated in error response handler
            ProtocolError::InvalidStatement(format!("Statement {} not found", statement_id))
        })?;

        let num_params = stmt.num_params;
        let query_template = stmt.query.clone();
        let param_types = stmt.param_types.clone();
        let long_data = stmt.long_data.clone();
        drop(statements);

        // If no parameters, execute directly
        if num_params == 0 {
            match self.sql_engine.write().await.execute(&query_template).await {
                Ok(result) => return self.build_result_set(result),
                Err(e) => {
                    // Update error metrics
                    {
                        let mut metrics = self.metrics.write().await;
                        metrics.total_errors += 1;
                    }
                    return Ok(vec![MySqlPacketBuilder::error_from_protocol_error(&e)]);
                }
            }
        }

        // Read NULL bitmap
        let null_bitmap_len = num_params.div_ceil(8) as usize;
        if payload.len() < null_bitmap_len {
            return Err(ProtocolError::IncompleteFrame);
        }
        let null_bitmap = payload.copy_to_bytes(null_bitmap_len);

        // Check if new parameters flag is set (bit 0 of flags)
        let new_params = (flags & 0x01) != 0;

        // If new_params is set, read parameter types
        // For now, we'll use stored types or default to string
        let mut param_types_to_use = param_types;
        if new_params && num_params > 0 {
            // Read parameter types (2 bytes per parameter: type, flags)
            let types_len = (num_params * 2) as usize;
            if payload.len() < types_len {
                return Err(ProtocolError::IncompleteFrame);
            }
            param_types_to_use = Vec::new();
            for _ in 0..num_params {
                let type_byte = payload.get_u8();
                let _flags = payload.get_u8(); // Parameter flags (unused for now)
                param_types_to_use.push(
                    super::types::MySqlType::from_u8(type_byte)
                        .unwrap_or(super::types::MySqlType::VarString),
                );
            }
        }

        // Decode parameter values
        let mut param_values = Vec::new();
        for i in 0..num_params {
            let param_idx = i as usize;
            let byte_idx = param_idx / 8;
            let bit_idx = param_idx % 8;
            let is_null = (null_bitmap[byte_idx] & (1 << bit_idx)) != 0;

            // Check if this parameter has long data sent via COM_STMT_SEND_LONG_DATA
            if let Some(data) = long_data.get(&i) {
                // Use long data instead of inline data
                // Convert to string with proper escaping for SQL
                let value = if let Ok(s) = String::from_utf8(data.clone()) {
                    format!("'{}'", s.replace('\'', "''"))
                } else {
                    // Binary data - encode as hex literal
                    format!("X'{}'", hex::encode(data))
                };
                param_values.push(value);
            } else if is_null {
                param_values.push("NULL".to_string());
            } else {
                let param_type = param_types_to_use
                    .get(param_idx)
                    .copied()
                    .unwrap_or(super::types::MySqlType::VarString);

                let value = self.decode_parameter_value(&mut payload, param_type, false)?;
                param_values.push(value);
            }
        }

        // Bind parameters to query (replace `?` with values)
        let mut bound_query = query_template.clone();
        for value in param_values {
            bound_query = bound_query.replacen("?", &value, 1);
        }

        println!("[MySQL] Bound query: {}", bound_query);

        // Execute bound query
        match self.sql_engine.write().await.execute(&bound_query).await {
            Ok(result) => self.build_result_set(result),
            Err(e) => {
                // Update error metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.total_errors += 1;
                }
                Ok(vec![MySqlPacketBuilder::error_from_protocol_error(&e)])
            }
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

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.prepared_statements_count = self.prepared_statements.read().await.len();
        }

        // No response for COM_STMT_CLOSE
        Ok(vec![])
    }

    /// Handle COM_STMT_RESET
    async fn handle_stmt_reset(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        if payload.len() < 4 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let statement_id = payload.get_u32_le();

        println!("[MySQL] Reset statement: {}", statement_id);

        // Reset clears long data and resets parameter state
        let mut statements = self.prepared_statements.write().await;
        if let Some(stmt) = statements.get_mut(&statement_id) {
            stmt.long_data.clear();
        }
        drop(statements);

        Ok(vec![MySqlPacketBuilder::ok(0, 0)])
    }

    /// Handle COM_STMT_SEND_LONG_DATA
    /// Used to send binary data for a parameter in chunks before execution
    async fn handle_stmt_send_long_data(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        // COM_STMT_SEND_LONG_DATA format:
        // - statement_id: 4 bytes
        // - param_id: 2 bytes
        // - data: remaining bytes

        if payload.len() < 6 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let statement_id = payload.get_u32_le();
        let param_id = payload.get_u16_le();
        let data = payload.to_vec();

        println!(
            "[MySQL] Send long data: stmt={}, param={}, len={}",
            statement_id,
            param_id,
            data.len()
        );

        // Accumulate long data for this parameter
        let mut statements = self.prepared_statements.write().await;
        if let Some(stmt) = statements.get_mut(&statement_id) {
            stmt.long_data
                .entry(param_id)
                .or_insert_with(Vec::new)
                .extend(data);
        } else {
            // Statement not found, but per MySQL protocol, no response is sent
            println!("[MySQL] Warning: Long data for unknown statement {}", statement_id);
        }
        drop(statements);

        // COM_STMT_SEND_LONG_DATA does not send any response
        Ok(vec![])
    }

    /// Handle COM_STMT_FETCH
    /// Used to fetch rows from a cursor in batches (requires SERVER_STATUS_CURSOR_EXISTS)
    async fn handle_stmt_fetch(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        // COM_STMT_FETCH format:
        // - statement_id: 4 bytes
        // - num_rows: 4 bytes (number of rows to fetch)

        if payload.len() < 8 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let statement_id = payload.get_u32_le();
        let num_rows = payload.get_u32_le();

        println!(
            "[MySQL] Fetch: stmt={}, rows={}",
            statement_id, num_rows
        );

        // Note: Full cursor support would require:
        // 1. Storing the result set from COM_STMT_EXECUTE when CURSOR_TYPE_READ_ONLY is set
        // 2. Tracking cursor position for each statement
        // 3. Returning rows in batches
        //
        // For now, we return an error indicating cursor is not available
        // since we don't set SERVER_STATUS_CURSOR_EXISTS in execute responses

        // Check if statement exists
        let statements = self.prepared_statements.read().await;
        if !statements.contains_key(&statement_id) {
            return Ok(vec![MySqlPacketBuilder::error(
                super::protocol::error_codes::ER_UNKNOWN_ERROR,
                &format!("Statement {} not found", statement_id),
            )]);
        }
        drop(statements);

        // Return EOF indicating no more rows (cursor exhausted)
        // This is the expected response when there are no more rows to fetch
        Ok(vec![MySqlPacketBuilder::eof()])
    }

    /// Handle COM_FIELD_LIST
    async fn handle_field_list(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        // COM_FIELD_LIST format: table name (null-terminated) + optional pattern
        let table_name = if payload.is_empty() {
            return Err(ProtocolError::IncompleteFrame);
        } else {
            let mut name_bytes = Vec::new();
            while !payload.is_empty() {
                let byte = payload.get_u8();
                if byte == 0 {
                    break;
                }
                name_bytes.push(byte);
            }
            String::from_utf8(name_bytes).map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?
        };

        println!("[MySQL] Field list for table: {}", table_name);

        // Query table schema to get columns
        let query = format!("SELECT * FROM {} LIMIT 0", table_name);
        match self.sql_engine.write().await.execute(&query).await {
            Ok(result) => {
                // Extract column information from result
                if let crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                    columns,
                    ..
                } = result
                {
                    let mut packets = Vec::new();

                    // Send column definition packets
                    for column in columns {
                        let col_def = MySqlPacketBuilder::column_definition(
                            "def",
                            "orbit",
                            &table_name,
                            &table_name,
                            &column,
                            &column,
                            MySqlType::VarString, // Default type, could be improved
                        );
                        packets.push(col_def);
                    }

                    // EOF packet
                    packets.push(MySqlPacketBuilder::eof());

                    Ok(packets)
                } else {
                    Ok(vec![MySqlPacketBuilder::error(
                        super::protocol::error_codes::ER_NO_SUCH_TABLE,
                        &format!("Table '{}' doesn't exist", table_name),
                    )])
                }
            }
            Err(e) => {
                // Update error metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.total_errors += 1;
                }
                Ok(vec![MySqlPacketBuilder::error_from_protocol_error(&e)])
            }
        }
    }

    /// Handle COM_STATISTICS
    async fn handle_statistics(&self) -> ProtocolResult<Vec<Bytes>> {
        let metrics = self.metrics.read().await;
        let stats = format!(
            "Uptime: {} seconds\nQueries: {}\nErrors: {}\nConnections: {}\nPrepared Statements: {}",
            0, // Uptime would need to be tracked separately
            metrics.total_queries,
            metrics.total_errors,
            metrics.active_connections,
            metrics.prepared_statements_count
        );
        drop(metrics);

        // Statistics is returned as a string
        let mut buf = BytesMut::new();
        buf.put(stats.as_bytes());
        Ok(vec![buf.freeze()])
    }

    /// Handle COM_CREATE_DB
    async fn handle_create_db(&self, payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        let db_name = String::from_utf8(payload.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        println!("[MySQL] Create database: {}", db_name);

        // In Orbit, we don't have separate databases/schemas in the same way MySQL does
        // For compatibility, we'll just return OK
        // In the future, this could map to CREATE SCHEMA
        Ok(vec![MySqlPacketBuilder::ok(0, 0)])
    }

    /// Handle COM_DROP_DB
    async fn handle_drop_db(&self, payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        let db_name = String::from_utf8(payload.to_vec())
            .map_err(|e| ProtocolError::InvalidUtf8(e.to_string()))?;

        println!("[MySQL] Drop database: {}", db_name);

        // In Orbit, we don't have separate databases/schemas in the same way MySQL does
        // For compatibility, we'll just return OK
        // In the future, this could map to DROP SCHEMA
        Ok(vec![MySqlPacketBuilder::ok(0, 0)])
    }

    /// Handle COM_REFRESH
    async fn handle_refresh(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        if payload.is_empty() {
            return Err(ProtocolError::IncompleteFrame);
        }

        let refresh_flags = payload.get_u8();
        println!("[MySQL] Refresh with flags: 0x{:02x}", refresh_flags);

        // Refresh commands:
        // 0x01 = REFRESH_GRANT (reload privileges)
        // 0x02 = REFRESH_LOG (flush logs)
        // 0x04 = REFRESH_TABLES (close all tables)
        // 0x08 = REFRESH_HOSTS (flush host cache)
        // 0x10 = REFRESH_STATUS (reset status variables)
        // 0x20 = REFRESH_THREADS (flush thread cache)
        // 0x40 = REFRESH_SLAVE (reset slave)
        // 0x80 = REFRESH_MASTER (reset master)

        // For now, we'll just return OK as a no-op
        // In the future, this could flush caches, reset metrics, etc.
        Ok(vec![MySqlPacketBuilder::ok(0, 0)])
    }

    /// Handle COM_SET_OPTION
    async fn handle_set_option(&self, mut payload: Bytes) -> ProtocolResult<Vec<Bytes>> {
        if payload.len() < 2 {
            return Err(ProtocolError::IncompleteFrame);
        }

        let option = payload.get_u16_le();
        println!("[MySQL] Set option: {}", option);

        // Options:
        // 0 = MYSQL_OPTION_MULTI_STATEMENTS_ON
        // 1 = MYSQL_OPTION_MULTI_STATEMENTS_OFF

        // We'll just acknowledge it for now
        Ok(vec![MySqlPacketBuilder::eof()])
    }

    /// Handle COM_RESET_CONNECTION
    async fn handle_reset_connection(&self) -> ProtocolResult<Vec<Bytes>> {
        println!("[MySQL] Reset connection");

        // Reset connection state (user variables, prepared statements, etc.)
        // For now, we just clear prepared statements for this connection
        // Note: In a real implementation, this would be per-connection state,
        // but here we are sharing state across the adapter.
        // Since we clone the adapter for each connection, this might be okay if the state was truly local.
        // However, prepared_statements is Arc<RwLock<...>>, so it's shared.
        // To do this correctly, we should have per-connection state.
        // For now, we'll just return OK.

        Ok(vec![MySqlPacketBuilder::ok(0, 0)])
    }

    /// Build result set from SQL execution result
    fn build_result_set(
        &self,
        result: crate::protocols::postgres_wire::sql::UnifiedExecutionResult,
    ) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        match result {
            UnifiedExecutionResult::Select { columns, rows, .. } => {
                let mut packets = Vec::new();

                // Column count packet
                let mut col_count = BytesMut::new();
                super::packet::write_lenenc_int(&mut col_count, columns.len() as u64);
                packets.push(col_count.freeze());

                // Column definition packets
                // Try to infer types from first row if available
                let mut column_types = vec![MySqlType::VarString; columns.len()];
                if let Some(first_row) = rows.first() {
                    for (i, value) in first_row.iter().enumerate() {
                        if let Some(val_str) = value {
                            // Try to infer type from value
                            column_types[i] = if val_str.parse::<i64>().is_ok() {
                                // Check if it fits in i32
                                if val_str.parse::<i32>().is_ok() {
                                    MySqlType::Long
                                } else {
                                    MySqlType::LongLong
                                }
                            } else if val_str.parse::<f64>().is_ok() {
                                MySqlType::Double
                            } else {
                                MySqlType::VarString
                            };
                        }
                    }
                }

                for (i, column) in columns.iter().enumerate() {
                    let mysql_type = column_types.get(i).copied().unwrap_or(MySqlType::VarString);
                    // Try to get table name from query context if available
                    // For now, use generic values
                    let col_def = MySqlPacketBuilder::column_definition(
                        "def",   // catalog
                        "orbit", // schema
                        "",      // table (empty if unknown)
                        "",      // org_table (empty if unknown)
                        column,  // name
                        column,  // org_name
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
    #[allow(dead_code)]
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
    #[allow(dead_code)] // Reserved for future text protocol implementation
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

    /// Get access to the SQL engine (for testing)
    #[cfg_attr(test, allow(dead_code))]
    pub fn sql_engine(&self) -> &Arc<RwLock<SqlEngine>> {
        &self.sql_engine
    }

    /// Get access to metrics (for monitoring)
    #[cfg_attr(test, allow(dead_code))]
    pub fn metrics(&self) -> &Arc<RwLock<MySqlMetrics>> {
        &self.metrics
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
        let _adapter = MySqlAdapter::new(config);
        // Type conversion tests would go here
    }

    #[tokio::test]
    async fn test_mysql_specific_show_databases() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SHOW DATABASES
        let result = adapter.handle_mysql_specific_query("SHOW DATABASES").await;
        assert!(result.is_some());

        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_specific_show_tables() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a test table first
        let _ = adapter
            .sql_engine
            .write()
            .await
            .execute("CREATE TABLE test_table (id INTEGER, name TEXT)")
            .await;

        // Test SHOW TABLES
        let result = adapter.handle_mysql_specific_query("SHOW TABLES").await;
        assert!(result.is_some());

        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_specific_information_schema_tables() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test INFORMATION_SCHEMA.TABLES query
        let result = adapter
            .handle_mysql_specific_query("SELECT * FROM information_schema.tables")
            .await;
        assert!(result.is_some());

        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_specific_information_schema_schemata() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test INFORMATION_SCHEMA.SCHEMATA query
        let result = adapter
            .handle_mysql_specific_query("SELECT * FROM information_schema.schemata")
            .await;
        assert!(result.is_some());

        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_specific_query_passthrough() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test that non-MySQL-specific queries return None
        let result = adapter
            .handle_mysql_specific_query("SELECT * FROM users")
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_stmt_send_long_data() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a prepared statement first
        let prepare_payload = Bytes::from("INSERT INTO test_table (id, data) VALUES (?, ?)");
        let prepare_result = adapter.handle_prepare(prepare_payload).await;
        assert!(prepare_result.is_ok());
        let packets = prepare_result.unwrap();
        assert!(!packets.is_empty());

        // Extract statement_id from response (first 4 bytes after status byte)
        let first_packet = &packets[0];
        assert_eq!(first_packet[0], 0x00); // OK status
        let statement_id = u32::from_le_bytes([first_packet[1], first_packet[2], first_packet[3], first_packet[4]]);

        // Send long data for param 1 (the data column)
        let mut long_data_payload = BytesMut::new();
        long_data_payload.put_u32_le(statement_id); // statement_id
        long_data_payload.put_u16_le(1); // param_id (0-indexed)
        long_data_payload.put(&b"This is some long text data for the test"[..]);

        let result = adapter.handle_stmt_send_long_data(long_data_payload.freeze()).await;
        assert!(result.is_ok());
        // COM_STMT_SEND_LONG_DATA returns no response
        assert!(result.unwrap().is_empty());

        // Verify long data was stored
        let statements = adapter.prepared_statements.read().await;
        let stmt = statements.get(&statement_id).unwrap();
        assert!(stmt.long_data.contains_key(&1));
        assert_eq!(stmt.long_data[&1], b"This is some long text data for the test");
    }

    #[tokio::test]
    async fn test_stmt_send_long_data_multiple_chunks() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a prepared statement
        let prepare_payload = Bytes::from("INSERT INTO test (data) VALUES (?)");
        let prepare_result = adapter.handle_prepare(prepare_payload).await.unwrap();
        let first_packet = &prepare_result[0];
        let statement_id = u32::from_le_bytes([first_packet[1], first_packet[2], first_packet[3], first_packet[4]]);

        // Send first chunk
        let mut chunk1 = BytesMut::new();
        chunk1.put_u32_le(statement_id);
        chunk1.put_u16_le(0); // param_id
        chunk1.put(&b"First chunk "[..]);
        adapter.handle_stmt_send_long_data(chunk1.freeze()).await.unwrap();

        // Send second chunk
        let mut chunk2 = BytesMut::new();
        chunk2.put_u32_le(statement_id);
        chunk2.put_u16_le(0); // same param_id
        chunk2.put(&b"Second chunk"[..]);
        adapter.handle_stmt_send_long_data(chunk2.freeze()).await.unwrap();

        // Verify chunks were concatenated
        let statements = adapter.prepared_statements.read().await;
        let stmt = statements.get(&statement_id).unwrap();
        assert_eq!(stmt.long_data[&0], b"First chunk Second chunk");
    }

    #[tokio::test]
    async fn test_stmt_reset_clears_long_data() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create and prepare statement
        let prepare_payload = Bytes::from("INSERT INTO test (data) VALUES (?)");
        let prepare_result = adapter.handle_prepare(prepare_payload).await.unwrap();
        let first_packet = &prepare_result[0];
        let statement_id = u32::from_le_bytes([first_packet[1], first_packet[2], first_packet[3], first_packet[4]]);

        // Send long data
        let mut long_data = BytesMut::new();
        long_data.put_u32_le(statement_id);
        long_data.put_u16_le(0);
        long_data.put(&b"Some data"[..]);
        adapter.handle_stmt_send_long_data(long_data.freeze()).await.unwrap();

        // Reset statement
        let mut reset_payload = BytesMut::new();
        reset_payload.put_u32_le(statement_id);
        let result = adapter.handle_stmt_reset(reset_payload.freeze()).await;
        assert!(result.is_ok());

        // Verify long data was cleared
        let statements = adapter.prepared_statements.read().await;
        let stmt = statements.get(&statement_id).unwrap();
        assert!(stmt.long_data.is_empty());
    }

    #[tokio::test]
    async fn test_stmt_fetch() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a prepared statement
        let prepare_payload = Bytes::from("SELECT * FROM test");
        let prepare_result = adapter.handle_prepare(prepare_payload).await.unwrap();
        let first_packet = &prepare_result[0];
        let statement_id = u32::from_le_bytes([first_packet[1], first_packet[2], first_packet[3], first_packet[4]]);

        // Try to fetch
        let mut fetch_payload = BytesMut::new();
        fetch_payload.put_u32_le(statement_id);
        fetch_payload.put_u32_le(10); // num_rows

        let result = adapter.handle_stmt_fetch(fetch_payload.freeze()).await;
        assert!(result.is_ok());

        // Should return EOF (no cursor support yet)
        let packets = result.unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0][0], 0xFE); // EOF packet
    }

    #[tokio::test]
    async fn test_stmt_fetch_unknown_statement() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Try to fetch from non-existent statement
        let mut fetch_payload = BytesMut::new();
        fetch_payload.put_u32_le(99999); // non-existent statement_id
        fetch_payload.put_u32_le(10);

        let result = adapter.handle_stmt_fetch(fetch_payload.freeze()).await;
        assert!(result.is_ok());

        // Should return error
        let packets = result.unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0][0], 0xFF); // ERR packet
    }

    #[test]
    fn test_mysql_command_send_long_data_parsing() {
        use super::super::protocol::MySqlCommand;

        // Test COM_STMT_SEND_LONG_DATA (0x18)
        let cmd = MySqlCommand::from_u8(0x18);
        assert!(cmd.is_ok());
        assert_eq!(cmd.unwrap(), MySqlCommand::StmtSendLongData);

        // Test COM_STMT_FETCH (0x1C)
        let cmd = MySqlCommand::from_u8(0x1C);
        assert!(cmd.is_ok());
        assert_eq!(cmd.unwrap(), MySqlCommand::StmtFetch);
    }
}
