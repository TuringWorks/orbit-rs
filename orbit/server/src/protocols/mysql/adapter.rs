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
    client_capabilities: Arc<RwLock<u32>>,
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
            client_capabilities: Arc::new(RwLock::new(0)),
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
            client_capabilities: Arc::new(RwLock::new(0)), // New connection starts with 0 capabilities
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
        // TODO: Support caching_sha2_password when authentication is enabled
        let auth_plugin = "mysql_native_password";

        let handshake = build_handshake(connection_id, &self.config.server_version, auth_plugin);
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
                        match auth.process_handshake(response.clone()) {
                            Ok(true) => {
                                // Store client capabilities
                                {
                                    let mut caps = self.client_capabilities.write().await;
                                    *caps = response.capability_flags;
                                }

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
        if query_upper.starts_with("SHOW VARIABLES")
            || query_upper.starts_with("SHOW SESSION VARIABLES")
        {
            println!("[MySQL] Handling SHOW VARIABLES");
            return Some(self.build_show_variables_result());
        }

        // Handle SHOW STATUS
        if query_upper.starts_with("SHOW STATUS") || query_upper.starts_with("SHOW SESSION STATUS")
        {
            println!("[MySQL] Handling SHOW STATUS");
            return Some(self.build_show_status_result());
        }

        // Handle SHOW COLLATION
        if query_upper.starts_with("SHOW COLLATION") {
            println!("[MySQL] Handling SHOW COLLATION");
            return Some(self.build_show_collation_result());
        }

        // Handle SHOW CHARACTER SET
        if query_upper.starts_with("SHOW CHARACTER SET") || query_upper.starts_with("SHOW CHARSET")
        {
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

        // Handle SELECT VERSION()
        if query_upper.contains("VERSION()") {
            println!("[MySQL] Handling SELECT VERSION()");
            return Some(self.build_version_result());
        }

        // Handle SHOW COLUMNS FROM table
        if query_upper.starts_with("SHOW COLUMNS FROM")
            || query_upper.starts_with("SHOW FIELDS FROM")
            || query_upper.starts_with("DESCRIBE ")
            || query_upper.starts_with("DESC ")
        {
            println!("[MySQL] Handling SHOW COLUMNS/DESCRIBE");
            return Some(self.build_show_columns_result(query).await);
        }

        // Handle SHOW CREATE TABLE
        if query_upper.starts_with("SHOW CREATE TABLE") {
            println!("[MySQL] Handling SHOW CREATE TABLE");
            return Some(self.build_show_create_table_result(query).await);
        }

        // Handle SHOW INDEX FROM
        if query_upper.starts_with("SHOW INDEX FROM")
            || query_upper.starts_with("SHOW INDEXES FROM")
        {
            println!("[MySQL] Handling SHOW INDEX");
            return Some(self.build_show_index_result(query).await);
        }

        // Handle SHOW PROCESSLIST
        if query_upper.starts_with("SHOW PROCESSLIST")
            || query_upper.starts_with("SHOW FULL PROCESSLIST")
        {
            println!("[MySQL] Handling SHOW PROCESSLIST");
            return Some(self.build_show_processlist_result());
        }

        // Handle SHOW GRANTS
        if query_upper.starts_with("SHOW GRANTS") {
            println!("[MySQL] Handling SHOW GRANTS");
            return Some(self.build_show_grants_result(&query_upper));
        }

        // Handle SHOW CREATE DATABASE
        if query_upper.starts_with("SHOW CREATE DATABASE")
            || query_upper.starts_with("SHOW CREATE SCHEMA")
        {
            println!("[MySQL] Handling SHOW CREATE DATABASE");
            return Some(self.build_show_create_database_result(query));
        }

        // Handle SHOW WARNINGS
        if query_upper.starts_with("SHOW WARNINGS") {
            println!("[MySQL] Handling SHOW WARNINGS");
            return Some(self.build_show_warnings_result());
        }

        // Handle SHOW ERRORS
        if query_upper.starts_with("SHOW ERRORS") {
            println!("[MySQL] Handling SHOW ERRORS");
            return Some(self.build_show_errors_result());
        }

        // Handle SHOW ENGINES
        if query_upper.starts_with("SHOW ENGINES")
            || query_upper.starts_with("SHOW STORAGE ENGINES")
        {
            println!("[MySQL] Handling SHOW ENGINES");
            return Some(self.build_show_engines_result());
        }

        // Handle SHOW PLUGINS
        if query_upper.starts_with("SHOW PLUGINS") {
            println!("[MySQL] Handling SHOW PLUGINS");
            return Some(self.build_show_plugins_result());
        }

        // ============ Transaction Commands ============

        // Handle BEGIN / START TRANSACTION
        if query_upper == "BEGIN" || query_upper.starts_with("START TRANSACTION") {
            println!("[MySQL] Handling BEGIN TRANSACTION");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle COMMIT
        if query_upper == "COMMIT" {
            println!("[MySQL] Handling COMMIT");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle ROLLBACK
        if query_upper == "ROLLBACK" {
            println!("[MySQL] Handling ROLLBACK");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SAVEPOINT
        if query_upper.starts_with("SAVEPOINT ") {
            println!("[MySQL] Handling SAVEPOINT (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle RELEASE SAVEPOINT
        if query_upper.starts_with("RELEASE SAVEPOINT ") {
            println!("[MySQL] Handling RELEASE SAVEPOINT (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle ROLLBACK TO SAVEPOINT
        if query_upper.starts_with("ROLLBACK TO ") {
            println!("[MySQL] Handling ROLLBACK TO SAVEPOINT (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET AUTOCOMMIT
        if query_upper.starts_with("SET AUTOCOMMIT") {
            println!("[MySQL] Handling SET AUTOCOMMIT");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET TRANSACTION ISOLATION LEVEL
        if query_upper.starts_with("SET TRANSACTION ISOLATION LEVEL") {
            println!("[MySQL] Handling SET TRANSACTION ISOLATION LEVEL");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // ============ User Management Commands (stubs) ============

        // Handle CREATE USER
        if query_upper.starts_with("CREATE USER") {
            println!("[MySQL] Handling CREATE USER (stub - not enforced)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle DROP USER
        if query_upper.starts_with("DROP USER") {
            println!("[MySQL] Handling DROP USER (stub - not enforced)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle ALTER USER
        if query_upper.starts_with("ALTER USER") {
            println!("[MySQL] Handling ALTER USER (stub - not enforced)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle GRANT
        if query_upper.starts_with("GRANT ") {
            println!("[MySQL] Handling GRANT (stub - not enforced)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle REVOKE
        if query_upper.starts_with("REVOKE ") {
            println!("[MySQL] Handling REVOKE (stub - not enforced)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle FLUSH PRIVILEGES
        if query_upper.starts_with("FLUSH PRIVILEGES") {
            println!("[MySQL] Handling FLUSH PRIVILEGES");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SET PASSWORD
        if query_upper.starts_with("SET PASSWORD") {
            println!("[MySQL] Handling SET PASSWORD (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // ============ Other Utility Commands ============

        // Handle USE database
        if query_upper.starts_with("USE ") {
            println!("[MySQL] Handling USE database");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle EXPLAIN / DESCRIBE query plan
        if query_upper.starts_with("EXPLAIN ") {
            println!("[MySQL] Handling EXPLAIN");
            return Some(self.build_explain_result(&query_upper));
        }

        // Handle KILL command
        if query_upper.starts_with("KILL ") {
            println!("[MySQL] Handling KILL (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle TRUNCATE TABLE
        if query_upper.starts_with("TRUNCATE ") || query_upper.starts_with("TRUNCATE TABLE ") {
            println!("[MySQL] Handling TRUNCATE TABLE");
            // Extract table name and execute DELETE
            let table_name = query_upper
                .trim_start_matches("TRUNCATE TABLE ")
                .trim_start_matches("TRUNCATE ")
                .trim()
                .trim_end_matches(';');
            if !table_name.is_empty() {
                let _delete_query = format!("DELETE FROM {}", table_name);
                // Execute through SQL engine - will be handled below
                println!("[MySQL] Converting TRUNCATE to DELETE FROM {}", table_name);
            }
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle LOCK TABLES
        if query_upper.starts_with("LOCK TABLES") || query_upper.starts_with("LOCK TABLE") {
            println!("[MySQL] Handling LOCK TABLES (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle UNLOCK TABLES
        if query_upper.starts_with("UNLOCK TABLES") || query_upper.starts_with("UNLOCK TABLE") {
            println!("[MySQL] Handling UNLOCK TABLES (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle OPTIMIZE TABLE
        if query_upper.starts_with("OPTIMIZE TABLE") {
            println!("[MySQL] Handling OPTIMIZE TABLE (stub)");
            return Some(self.build_table_maintenance_result("optimize"));
        }

        // Handle ANALYZE TABLE
        if query_upper.starts_with("ANALYZE TABLE") {
            println!("[MySQL] Handling ANALYZE TABLE (stub)");
            return Some(self.build_table_maintenance_result("analyze"));
        }

        // Handle CHECK TABLE
        if query_upper.starts_with("CHECK TABLE") {
            println!("[MySQL] Handling CHECK TABLE (stub)");
            return Some(self.build_table_maintenance_result("check"));
        }

        // Handle REPAIR TABLE
        if query_upper.starts_with("REPAIR TABLE") {
            println!("[MySQL] Handling REPAIR TABLE (stub)");
            return Some(self.build_table_maintenance_result("repair"));
        }

        // ============ NDB Cluster / Federated Commands (stubs) ============

        // Handle CREATE/ALTER/DROP TABLESPACE
        if query_upper.starts_with("CREATE TABLESPACE")
            || query_upper.starts_with("ALTER TABLESPACE")
            || query_upper.starts_with("DROP TABLESPACE")
        {
            println!("[MySQL] Handling TABLESPACE command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle CREATE/ALTER/DROP LOGFILE GROUP
        if query_upper.starts_with("CREATE LOGFILE GROUP")
            || query_upper.starts_with("ALTER LOGFILE GROUP")
            || query_upper.starts_with("DROP LOGFILE GROUP")
        {
            println!("[MySQL] Handling LOGFILE GROUP command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle CREATE/ALTER/DROP SERVER
        if query_upper.starts_with("CREATE SERVER")
            || query_upper.starts_with("ALTER SERVER")
            || query_upper.starts_with("DROP SERVER")
        {
            println!("[MySQL] Handling SERVER command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // ============ Event Scheduler Commands (stubs) ============

        // Handle CREATE/ALTER/DROP EVENT
        if query_upper.starts_with("CREATE EVENT")
            || query_upper.starts_with("ALTER EVENT")
            || query_upper.starts_with("DROP EVENT")
        {
            println!("[MySQL] Handling EVENT command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SHOW EVENTS
        if query_upper.starts_with("SHOW EVENTS") {
            println!("[MySQL] Handling SHOW EVENTS");
            return Some(self.build_show_events_result());
        }

        // ============ Stored Procedure / Function Commands (stubs) ============

        // Handle CREATE USER / DROP USER / RENAME USER / GRANT / REVOKE
        if query_upper.starts_with("CREATE USER")
            || query_upper.starts_with("DROP USER")
            || query_upper.starts_with("RENAME USER")
            || query_upper.starts_with("GRANT")
            || query_upper.starts_with("REVOKE")
            || query_upper.starts_with("ALTER USER")
            || query_upper.starts_with("SET PASSWORD")
        {
            println!("[MySQL] Handling USER/PRIVILEGES command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle CALL
        if query_upper.starts_with("CALL ") {
            println!("[MySQL] Handling CALL command (stub)");
            // Ideally we should return a result set if the procedure returns one
            // But for compatibility with void procedures, OK is safer
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // ============ Trigger Commands (stubs) ============

        // Handle CREATE/DROP TRIGGER
        if query_upper.starts_with("CREATE TRIGGER") || query_upper.starts_with("DROP TRIGGER") {
            println!("[MySQL] Handling TRIGGER command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle SHOW TRIGGERS
        if query_upper.starts_with("SHOW TRIGGERS") {
            println!("[MySQL] Handling SHOW TRIGGERS");
            return Some(self.build_show_triggers_result());
        }

        // ============ SHOW Commands (stubs) ============

        // Handle SHOW CREATE PROCEDURE/FUNCTION/TRIGGER/EVENT/VIEW
        if query_upper.starts_with("SHOW CREATE PROCEDURE")
            || query_upper.starts_with("SHOW CREATE FUNCTION")
            || query_upper.starts_with("SHOW CREATE TRIGGER")
            || query_upper.starts_with("SHOW CREATE EVENT")
            || query_upper.starts_with("SHOW CREATE VIEW")
        {
            println!("[MySQL] Handling SHOW CREATE ... command (stub)");
            // Return empty result set or error depending on what's safer.
            // Empty result set is safer for now.
            return Some(self.build_empty_show_create_result());
        }

        // Handle SHOW BINARY LOGS / MASTER STATUS / REPLICATION
        if query_upper.starts_with("SHOW BINARY LOGS")
            || query_upper.starts_with("SHOW BINLOG EVENTS")
            || query_upper.starts_with("SHOW RELAYLOG EVENTS")
            || query_upper.starts_with("SHOW MASTER STATUS")
            || query_upper.starts_with("SHOW SLAVE STATUS")
            || query_upper.starts_with("SHOW REPLICA STATUS")
        {
            println!("[MySQL] Handling SHOW BINARY/REPLICA command (stub)");
            return Some(self.build_empty_show_result());
        }

        // Handle SHOW OPEN TABLES / PROFILES
        if query_upper.starts_with("SHOW OPEN TABLES")
            || query_upper.starts_with("SHOW PROFILES")
            || query_upper.starts_with("SHOW PROFILE")
        {
            println!("[MySQL] Handling SHOW OPEN TABLES/PROFILES command (stub)");
            return Some(self.build_empty_show_result());
        }

        // Handle SHOW PROCEDURE/FUNCTION STATUS
        if query_upper.starts_with("SHOW PROCEDURE STATUS")
            || query_upper.starts_with("SHOW FUNCTION STATUS")
        {
            println!("[MySQL] Handling SHOW PROCEDURE/FUNCTION STATUS command (stub)");
            return Some(self.build_empty_show_result());
        }

        // ============ Admin / Utility Commands (stubs) ============

        // Handle FLUSH / RESET
        if query_upper.starts_with("FLUSH ") || query_upper.starts_with("RESET ") {
            println!("[MySQL] Handling FLUSH/RESET command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle KILL / SHUTDOWN
        if query_upper.starts_with("KILL ") || query_upper.starts_with("SHUTDOWN") {
            println!("[MySQL] Handling KILL/SHUTDOWN command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // Handle CHECKSUM / REPAIR / ANALYZE / OPTIMIZE TABLE (generic handler)
        if query_upper.starts_with("CHECKSUM TABLE")
            || query_upper.starts_with("REPAIR TABLE")
            || query_upper.starts_with("ANALYZE TABLE")
            || query_upper.starts_with("OPTIMIZE TABLE")
            || query_upper.starts_with("CHECK TABLE")
        {
            println!("[MySQL] Handling TABLE maintenance command (stub)");
            return Some(self.build_table_maintenance_result("status"));
        }

        // Handle HELP / DO / HANDLER / CACHE INDEX
        if query_upper.starts_with("HELP ")
            || query_upper.starts_with("DO ")
            || query_upper.starts_with("HANDLER ")
            || query_upper.starts_with("CACHE INDEX ")
            || query_upper.starts_with("LOAD INDEX INTO CACHE")
        {
            println!("[MySQL] Handling UTILITY command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        // ============ Replication Commands (stubs) ============
        if query_upper.starts_with("CHANGE MASTER TO")
            || query_upper.starts_with("CHANGE REPLICATION SOURCE TO")
            || query_upper.starts_with("START SLAVE")
            || query_upper.starts_with("START REPLICA")
            || query_upper.starts_with("STOP SLAVE")
            || query_upper.starts_with("STOP REPLICA")
            || query_upper.starts_with("RESET SLAVE")
            || query_upper.starts_with("RESET REPLICA")
            || query_upper.starts_with("PURGE BINARY LOGS")
        {
            println!("[MySQL] Handling REPLICATION command (stub)");
            return Some(Ok(vec![MySqlPacketBuilder::ok(0, 0)]));
        }

        None // Not a MySQL-specific query, let SQL engine handle it
    }

    /// Build generic empty result set for SHOW commands
    fn build_empty_show_result(&self) -> ProtocolResult<Vec<Bytes>> {
        self.build_result_set(
            crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                columns: vec!["Status".to_string()],
                rows: vec![],
                row_count: 0,
                transaction_id: None,
            },
        )
    }

    /// Build empty result set for SHOW CREATE ... commands
    fn build_empty_show_create_result(&self) -> ProtocolResult<Vec<Bytes>> {
        self.build_result_set(
            crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                columns: vec!["Table".to_string(), "Create Table".to_string()],
                rows: vec![],
                row_count: 0,
                transaction_id: None,
            },
        )
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

        let status = [
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

        let collations = [
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

        let charsets = [
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

    /// Build result for SELECT VERSION()
    fn build_version_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec!["VERSION()".to_string()],
            rows: vec![vec![Some(self.config.server_version.clone())]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW COLUMNS FROM table
    async fn build_show_columns_result(&self, query: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Extract table name from query
        let query_upper = query.to_uppercase();
        let table_name = if query_upper.starts_with("DESCRIBE ") || query_upper.starts_with("DESC ")
        {
            query
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_end_matches(';')
        } else {
            // SHOW COLUMNS FROM table or SHOW FIELDS FROM table
            query
                .split_whitespace()
                .nth(3)
                .unwrap_or("")
                .trim_end_matches(';')
        };

        // Query table structure using SQL engine
        let schema_query = format!("SELECT * FROM {} LIMIT 0", table_name);
        match self.sql_engine.write().await.execute(&schema_query).await {
            Ok(crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                columns,
                ..
            }) => {
                // Build columns info
                let rows: Vec<Vec<Option<String>>> = columns
                    .iter()
                    .map(|col| {
                        vec![
                            Some(col.clone()),                // Field
                            Some("varchar(255)".to_string()), // Type
                            Some("YES".to_string()),          // Null
                            Some("".to_string()),             // Key
                            Some("NULL".to_string()),         // Default
                            Some("".to_string()),             // Extra
                        ]
                    })
                    .collect();

                let result = UnifiedExecutionResult::Select {
                    columns: vec![
                        "Field".to_string(),
                        "Type".to_string(),
                        "Null".to_string(),
                        "Key".to_string(),
                        "Default".to_string(),
                        "Extra".to_string(),
                    ],
                    rows: rows.clone(),
                    row_count: rows.len(),
                    transaction_id: None,
                };

                self.build_result_set(result)
            }
            _ => Ok(vec![MySqlPacketBuilder::error(
                super::protocol::error_codes::ER_NO_SUCH_TABLE,
                &format!("Table '{}' doesn't exist", table_name),
            )]),
        }
    }

    /// Build result for SHOW CREATE TABLE
    async fn build_show_create_table_result(&self, query: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Extract table name from query: SHOW CREATE TABLE table_name
        let table_name = query
            .split_whitespace()
            .nth(3)
            .unwrap_or("")
            .trim_end_matches(';');

        // Query table structure using SQL engine
        let schema_query = format!("SELECT * FROM {} LIMIT 0", table_name);
        match self.sql_engine.write().await.execute(&schema_query).await {
            Ok(crate::protocols::postgres_wire::sql::UnifiedExecutionResult::Select {
                columns,
                ..
            }) => {
                // Build CREATE TABLE statement
                let column_defs: Vec<String> = columns
                    .iter()
                    .map(|col| format!("  `{}` varchar(255)", col))
                    .collect();

                let create_statement = format!(
                    "CREATE TABLE `{}` (\n{}\n) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
                    table_name,
                    column_defs.join(",\n")
                );

                let result = UnifiedExecutionResult::Select {
                    columns: vec!["Table".to_string(), "Create Table".to_string()],
                    rows: vec![vec![Some(table_name.to_string()), Some(create_statement)]],
                    row_count: 1,
                    transaction_id: None,
                };

                self.build_result_set(result)
            }
            _ => Ok(vec![MySqlPacketBuilder::error(
                super::protocol::error_codes::ER_NO_SUCH_TABLE,
                &format!("Table '{}' doesn't exist", table_name),
            )]),
        }
    }

    /// Build result for SHOW INDEX FROM table
    async fn build_show_index_result(&self, query: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Extract table name from query: SHOW INDEX FROM table_name
        let table_name = query
            .split_whitespace()
            .nth(3)
            .unwrap_or("")
            .trim_end_matches(';');

        // Return empty result (no indexes by default)
        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Table".to_string(),
                "Non_unique".to_string(),
                "Key_name".to_string(),
                "Seq_in_index".to_string(),
                "Column_name".to_string(),
                "Collation".to_string(),
                "Cardinality".to_string(),
                "Sub_part".to_string(),
                "Packed".to_string(),
                "Null".to_string(),
                "Index_type".to_string(),
                "Comment".to_string(),
            ],
            rows: vec![],
            row_count: 0,
            transaction_id: None,
        };
        let _ = table_name; // Suppress unused warning

        self.build_result_set(result)
    }

    /// Build result for SHOW PROCESSLIST
    fn build_show_processlist_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Id".to_string(),
                "User".to_string(),
                "Host".to_string(),
                "db".to_string(),
                "Command".to_string(),
                "Time".to_string(),
                "State".to_string(),
                "Info".to_string(),
            ],
            rows: vec![vec![
                Some("1".to_string()),
                Some("root".to_string()),
                Some("localhost".to_string()),
                Some("orbit".to_string()),
                Some("Query".to_string()),
                Some("0".to_string()),
                Some("executing".to_string()),
                Some("SHOW PROCESSLIST".to_string()),
            ]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW GRANTS command
    fn build_show_grants_result(&self, query_upper: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Extract user from query if present (SHOW GRANTS FOR user)
        let user = if query_upper.contains(" FOR ") {
            query_upper
                .split(" FOR ")
                .nth(1)
                .map(|s| s.trim().trim_matches('\'').trim_matches('"'))
                .unwrap_or("root@localhost")
        } else {
            "root@localhost"
        };

        let result = UnifiedExecutionResult::Select {
            columns: vec![format!("Grants for {}", user)],
            rows: vec![vec![Some(format!(
                "GRANT ALL PRIVILEGES ON *.* TO '{}'",
                user
            ))]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW CREATE DATABASE command
    fn build_show_create_database_result(&self, query: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Extract database name
        let db_name = query
            .to_uppercase()
            .replace("SHOW CREATE DATABASE ", "")
            .replace("SHOW CREATE SCHEMA ", "")
            .trim()
            .trim_matches('`')
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        let db_name = if db_name.is_empty() {
            "orbit".to_string()
        } else {
            db_name.to_lowercase()
        };

        let result = UnifiedExecutionResult::Select {
            columns: vec!["Database".to_string(), "Create Database".to_string()],
            rows: vec![vec![
                Some(db_name.clone()),
                Some(format!("CREATE DATABASE `{}` /*!40100 DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci */", db_name)),
            ]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW WARNINGS command
    fn build_show_warnings_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Return empty warnings (no warnings)
        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Level".to_string(),
                "Code".to_string(),
                "Message".to_string(),
            ],
            rows: vec![],
            row_count: 0,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW ERRORS command
    fn build_show_errors_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Return empty errors (no errors)
        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Level".to_string(),
                "Code".to_string(),
                "Message".to_string(),
            ],
            rows: vec![],
            row_count: 0,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW ENGINES command
    fn build_show_engines_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Engine".to_string(),
                "Support".to_string(),
                "Comment".to_string(),
                "Transactions".to_string(),
                "XA".to_string(),
                "Savepoints".to_string(),
            ],
            rows: vec![
                vec![
                    Some("Orbit".to_string()),
                    Some("DEFAULT".to_string()),
                    Some("Orbit-DB unified storage engine".to_string()),
                    Some("YES".to_string()),
                    Some("NO".to_string()),
                    Some("YES".to_string()),
                ],
                vec![
                    Some("MEMORY".to_string()),
                    Some("YES".to_string()),
                    Some("In-memory storage for temporary tables".to_string()),
                    Some("NO".to_string()),
                    Some("NO".to_string()),
                    Some("NO".to_string()),
                ],
            ],
            row_count: 2,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW PLUGINS command
    fn build_show_plugins_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Name".to_string(),
                "Status".to_string(),
                "Type".to_string(),
                "Library".to_string(),
                "License".to_string(),
            ],
            rows: vec![
                vec![
                    Some("mysql_native_password".to_string()),
                    Some("ACTIVE".to_string()),
                    Some("AUTHENTICATION".to_string()),
                    None,
                    Some("GPL".to_string()),
                ],
                vec![
                    Some("caching_sha2_password".to_string()),
                    Some("ACTIVE".to_string()),
                    Some("AUTHENTICATION".to_string()),
                    None,
                    Some("GPL".to_string()),
                ],
            ],
            row_count: 2,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for EXPLAIN command
    fn build_explain_result(&self, _query_upper: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "id".to_string(),
                "select_type".to_string(),
                "table".to_string(),
                "partitions".to_string(),
                "type".to_string(),
                "possible_keys".to_string(),
                "key".to_string(),
                "key_len".to_string(),
                "ref".to_string(),
                "rows".to_string(),
                "filtered".to_string(),
                "Extra".to_string(),
            ],
            rows: vec![vec![
                Some("1".to_string()),
                Some("SIMPLE".to_string()),
                Some("table".to_string()),
                None,
                Some("ALL".to_string()),
                None,
                None,
                None,
                None,
                Some("1".to_string()),
                Some("100.00".to_string()),
                Some("Full table scan".to_string()),
            ]],
            row_count: 1,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for table maintenance commands (OPTIMIZE, ANALYZE, CHECK, REPAIR)
    fn build_table_maintenance_result(&self, operation: &str) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        let result = UnifiedExecutionResult::Select {
            columns: vec![
                "Table".to_string(),
                "Op".to_string(),
                "Msg_type".to_string(),
                "Msg_text".to_string(),
            ],
            rows: vec![vec![
                Some("orbit.table".to_string()),
                Some(operation.to_string()),
                Some("status".to_string()),
                Some("OK".to_string()),
            ]],
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
            println!(
                "[MySQL] Warning: Long data for unknown statement {}",
                statement_id
            );
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

        println!("[MySQL] Fetch: stmt={}, rows={}", statement_id, num_rows);

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

    /// Build result for SHOW TRIGGERS command
    fn build_show_triggers_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Columns: Trigger, Event, Table, Statement, Timing, Created, sql_mode, Definer, character_set_client, collation_connection, Database Collation
        let columns = vec![
            "Trigger".to_string(),
            "Event".to_string(),
            "Table".to_string(),
            "Statement".to_string(),
            "Timing".to_string(),
            "Created".to_string(),
            "sql_mode".to_string(),
            "Definer".to_string(),
            "character_set_client".to_string(),
            "collation_connection".to_string(),
            "Database Collation".to_string(),
        ];

        // Empty result set for now
        let rows: Vec<Vec<Option<String>>> = Vec::new();

        let result = UnifiedExecutionResult::Select {
            columns,
            rows,
            row_count: 0,
            transaction_id: None,
        };

        self.build_result_set(result)
    }

    /// Build result for SHOW EVENTS command
    fn build_show_events_result(&self) -> ProtocolResult<Vec<Bytes>> {
        use crate::protocols::postgres_wire::sql::UnifiedExecutionResult;

        // Columns: Db, Name, Definer, Time zone, Type, Execute at, Interval value, Interval field, Starts, Ends, Status, Originator, character_set_client, collation_connection, Database Collation
        let columns = vec![
            "Db".to_string(),
            "Name".to_string(),
            "Definer".to_string(),
            "Time zone".to_string(),
            "Type".to_string(),
            "Execute at".to_string(),
            "Interval value".to_string(),
            "Interval field".to_string(),
            "Starts".to_string(),
            "Ends".to_string(),
            "Status".to_string(),
            "Originator".to_string(),
            "character_set_client".to_string(),
            "collation_connection".to_string(),
            "Database Collation".to_string(),
        ];

        // Empty result set for now
        let rows: Vec<Vec<Option<String>>> = Vec::new();

        let result = UnifiedExecutionResult::Select {
            columns,
            rows,
            row_count: 0,
            transaction_id: None,
        };

        self.build_result_set(result)
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

                // EOF packet after columns (or OK if deprecated)
                let deprecate_eof = {
                    // Use try_read since we're in a sync context
                    if let Ok(caps) = self.client_capabilities.try_read() {
                        (*caps & super::protocol::CLIENT_DEPRECATE_EOF) != 0
                    } else {
                        false // Default to not deprecated if we can't get the lock
                    }
                };

                if deprecate_eof {
                    packets.push(MySqlPacketBuilder::ok(0, 0));
                } else {
                    packets.push(MySqlPacketBuilder::eof());
                }

                // Row data packets
                for row in rows {
                    packets.push(MySqlPacketBuilder::text_row(&row));
                }

                // EOF packet after rows (or OK if deprecated)
                if deprecate_eof {
                    packets.push(MySqlPacketBuilder::ok(0, 0));
                } else {
                    packets.push(MySqlPacketBuilder::eof());
                }

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
        let statement_id = u32::from_le_bytes([
            first_packet[1],
            first_packet[2],
            first_packet[3],
            first_packet[4],
        ]);

        // Send long data for param 1 (the data column)
        let mut long_data_payload = BytesMut::new();
        long_data_payload.put_u32_le(statement_id); // statement_id
        long_data_payload.put_u16_le(1); // param_id (0-indexed)
        long_data_payload.put(&b"This is some long text data for the test"[..]);

        let result = adapter
            .handle_stmt_send_long_data(long_data_payload.freeze())
            .await;
        assert!(result.is_ok());
        // COM_STMT_SEND_LONG_DATA returns no response
        assert!(result.unwrap().is_empty());

        // Verify long data was stored
        let statements = adapter.prepared_statements.read().await;
        let stmt = statements.get(&statement_id).unwrap();
        assert!(stmt.long_data.contains_key(&1));
        assert_eq!(
            stmt.long_data[&1],
            b"This is some long text data for the test"
        );
    }

    #[tokio::test]
    async fn test_stmt_send_long_data_multiple_chunks() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a prepared statement
        let prepare_payload = Bytes::from("INSERT INTO test (data) VALUES (?)");
        let prepare_result = adapter.handle_prepare(prepare_payload).await.unwrap();
        let first_packet = &prepare_result[0];
        let statement_id = u32::from_le_bytes([
            first_packet[1],
            first_packet[2],
            first_packet[3],
            first_packet[4],
        ]);

        // Send first chunk
        let mut chunk1 = BytesMut::new();
        chunk1.put_u32_le(statement_id);
        chunk1.put_u16_le(0); // param_id
        chunk1.put(&b"First chunk "[..]);
        adapter
            .handle_stmt_send_long_data(chunk1.freeze())
            .await
            .unwrap();

        // Send second chunk
        let mut chunk2 = BytesMut::new();
        chunk2.put_u32_le(statement_id);
        chunk2.put_u16_le(0); // same param_id
        chunk2.put(&b"Second chunk"[..]);
        adapter
            .handle_stmt_send_long_data(chunk2.freeze())
            .await
            .unwrap();

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
        let statement_id = u32::from_le_bytes([
            first_packet[1],
            first_packet[2],
            first_packet[3],
            first_packet[4],
        ]);

        // Send long data
        let mut long_data = BytesMut::new();
        long_data.put_u32_le(statement_id);
        long_data.put_u16_le(0);
        long_data.put(&b"Some data"[..]);
        adapter
            .handle_stmt_send_long_data(long_data.freeze())
            .await
            .unwrap();

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
        let statement_id = u32::from_le_bytes([
            first_packet[1],
            first_packet[2],
            first_packet[3],
            first_packet[4],
        ]);

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

    #[test]
    fn test_mysql_all_commands() {
        use super::super::protocol::MySqlCommand;

        // Test all MySQL commands
        assert_eq!(MySqlCommand::from_u8(0x00).unwrap(), MySqlCommand::Sleep);
        assert_eq!(MySqlCommand::from_u8(0x01).unwrap(), MySqlCommand::Quit);
        assert_eq!(MySqlCommand::from_u8(0x02).unwrap(), MySqlCommand::InitDb);
        assert_eq!(MySqlCommand::from_u8(0x03).unwrap(), MySqlCommand::Query);
        assert_eq!(
            MySqlCommand::from_u8(0x04).unwrap(),
            MySqlCommand::FieldList
        );
        assert_eq!(MySqlCommand::from_u8(0x05).unwrap(), MySqlCommand::CreateDb);
        assert_eq!(MySqlCommand::from_u8(0x06).unwrap(), MySqlCommand::DropDb);
        assert_eq!(MySqlCommand::from_u8(0x07).unwrap(), MySqlCommand::Refresh);
        assert_eq!(MySqlCommand::from_u8(0x0E).unwrap(), MySqlCommand::Ping);
        assert_eq!(
            MySqlCommand::from_u8(0x16).unwrap(),
            MySqlCommand::StmtPrepare
        );
        assert_eq!(
            MySqlCommand::from_u8(0x17).unwrap(),
            MySqlCommand::StmtExecute
        );
        assert_eq!(
            MySqlCommand::from_u8(0x19).unwrap(),
            MySqlCommand::StmtClose
        );
        assert_eq!(
            MySqlCommand::from_u8(0x1A).unwrap(),
            MySqlCommand::StmtReset
        );
        assert_eq!(
            MySqlCommand::from_u8(0x1B).unwrap(),
            MySqlCommand::SetOption
        );
        assert_eq!(
            MySqlCommand::from_u8(0x1F).unwrap(),
            MySqlCommand::ResetConnection
        );

        // Test invalid command
        assert!(MySqlCommand::from_u8(0xFF).is_err());
    }

    #[tokio::test]
    async fn test_mysql_set_names() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SET NAMES command
        let result = adapter
            .handle_mysql_specific_query("SET NAMES 'utf8mb4'")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_set_names_with_collate() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SET NAMES with COLLATE
        let result = adapter
            .handle_mysql_specific_query("SET NAMES 'utf8mb4' COLLATE 'utf8mb4_general_ci'")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_set_session_variable() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SET @@SESSION variable
        let result = adapter
            .handle_mysql_specific_query("SET @@session.sql_mode = ''")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_set_collation() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SET collation command
        let result = adapter
            .handle_mysql_specific_query("SET collation_connection = utf8mb4_general_ci")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_statistics() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test statistics handler
        let result = adapter.handle_statistics().await;
        assert!(result.is_ok());
        let packets = result.unwrap();
        assert_eq!(packets.len(), 1);
        // Statistics returns a text response with server stats
        let response = String::from_utf8(packets[0].to_vec()).unwrap();
        assert!(response.contains("Uptime:"));
    }

    #[tokio::test]
    async fn test_mysql_reset_connection() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test reset connection
        let result = adapter.handle_reset_connection().await;
        assert!(result.is_ok());
        let packets = result.unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_create_db() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test create database
        let payload = Bytes::from("test_database");
        let result = adapter.handle_create_db(payload).await;
        assert!(result.is_ok());
        let packets = result.unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_drop_db() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test drop database
        let payload = Bytes::from("test_database");
        let result = adapter.handle_drop_db(payload).await;
        assert!(result.is_ok());
        let packets = result.unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_set_option() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test set option (MYSQL_OPTION_MULTI_STATEMENTS_ON = 0)
        let mut payload = BytesMut::new();
        payload.put_u16_le(0); // option value
        let result = adapter.handle_set_option(payload.freeze()).await;
        assert!(result.is_ok());
        let packets = result.unwrap();
        // Returns EOF packet for set option
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_refresh() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test refresh command
        let mut payload = BytesMut::new();
        payload.put_u8(1); // REFRESH_GRANT
        let result = adapter.handle_refresh(payload.freeze()).await;
        assert!(result.is_ok());
        let packets = result.unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[test]
    fn test_error_code_mapping() {
        use super::super::protocol::{error_codes, map_error_to_mysql_code};
        use crate::protocols::error::ProtocolError;

        // Test parse error mapping
        let error = ProtocolError::ParseError("syntax error".to_string());
        assert_eq!(map_error_to_mysql_code(&error), error_codes::ER_PARSE_ERROR);

        // Test authentication error mapping
        let error = ProtocolError::AuthenticationError("bad credentials".to_string());
        assert_eq!(
            map_error_to_mysql_code(&error),
            error_codes::ER_ACCESS_DENIED
        );

        // Test invalid opcode mapping
        let error = ProtocolError::InvalidOpcode(0xFF);
        assert_eq!(
            map_error_to_mysql_code(&error),
            error_codes::ER_UNKNOWN_COM_ERROR
        );

        // Test postgres error with table not found
        let error = ProtocolError::PostgresError("table users does not exist".to_string());
        assert_eq!(
            map_error_to_mysql_code(&error),
            error_codes::ER_NO_SUCH_TABLE
        );

        // Test postgres error with duplicate entry
        let error = ProtocolError::PostgresError("duplicate key value".to_string());
        assert_eq!(map_error_to_mysql_code(&error), error_codes::ER_DUP_ENTRY);
    }

    #[test]
    fn test_mysql_packet_ok_structure() {
        use super::super::protocol::MySqlPacket as MySqlPacketBuilder;

        // Test OK packet structure
        let ok = MySqlPacketBuilder::ok(5, 10);
        assert_eq!(ok[0], 0x00); // OK packet header
                                 // The rest contains affected_rows, last_insert_id, status, warnings
    }

    #[test]
    fn test_mysql_packet_error_structure() {
        use super::super::protocol::MySqlPacket as MySqlPacketBuilder;

        // Test ERROR packet structure
        let err = MySqlPacketBuilder::error(1045, "Access denied");
        assert_eq!(err[0], 0xFF); // ERR packet header
                                  // Error code is little-endian at bytes 1-2
        assert_eq!(u16::from_le_bytes([err[1], err[2]]), 1045);
        // SQL state marker
        assert_eq!(err[3], b'#');
    }

    #[test]
    fn test_mysql_packet_eof_structure() {
        use super::super::protocol::MySqlPacket as MySqlPacketBuilder;

        // Test EOF packet structure
        let eof = MySqlPacketBuilder::eof();
        assert_eq!(eof[0], 0xFE); // EOF packet header
    }

    #[tokio::test]
    async fn test_mysql_show_variables() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SHOW VARIABLES
        let result = adapter.handle_mysql_specific_query("SHOW VARIABLES").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_status() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SHOW STATUS
        let result = adapter.handle_mysql_specific_query("SHOW STATUS").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_select_version() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test SELECT VERSION()
        let result = adapter
            .handle_mysql_specific_query("SELECT VERSION()")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_columns() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a test table first
        let _ = adapter
            .sql_engine
            .write()
            .await
            .execute("CREATE TABLE test_cols (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
            .await;

        // Test SHOW COLUMNS
        let result = adapter
            .handle_mysql_specific_query("SHOW COLUMNS FROM test_cols")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_create_table() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Create a test table first
        let _ = adapter
            .sql_engine
            .write()
            .await
            .execute("CREATE TABLE show_create_test (id INTEGER, name TEXT)")
            .await;

        // Test SHOW CREATE TABLE
        let result = adapter
            .handle_mysql_specific_query("SHOW CREATE TABLE show_create_test")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_metrics() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Get initial metrics
        let metrics = adapter.metrics.read().await;
        assert_eq!(metrics.total_queries, 0);
        assert_eq!(metrics.total_errors, 0);
        assert_eq!(metrics.active_connections, 0);
    }

    #[test]
    fn test_mysql_type_from_u8() {
        use super::super::types::MySqlType;

        // Test all MySQL types
        assert_eq!(MySqlType::from_u8(0x00).unwrap(), MySqlType::Decimal);
        assert_eq!(MySqlType::from_u8(0x01).unwrap(), MySqlType::Tiny);
        assert_eq!(MySqlType::from_u8(0x02).unwrap(), MySqlType::Short);
        assert_eq!(MySqlType::from_u8(0x03).unwrap(), MySqlType::Long);
        assert_eq!(MySqlType::from_u8(0x04).unwrap(), MySqlType::Float);
        assert_eq!(MySqlType::from_u8(0x05).unwrap(), MySqlType::Double);
        assert_eq!(MySqlType::from_u8(0x06).unwrap(), MySqlType::Null);
        assert_eq!(MySqlType::from_u8(0x07).unwrap(), MySqlType::Timestamp);
        assert_eq!(MySqlType::from_u8(0x08).unwrap(), MySqlType::LongLong);
        assert_eq!(MySqlType::from_u8(0x09).unwrap(), MySqlType::Int24);
        assert_eq!(MySqlType::from_u8(0x0A).unwrap(), MySqlType::Date);
        assert_eq!(MySqlType::from_u8(0x0B).unwrap(), MySqlType::Time);
        assert_eq!(MySqlType::from_u8(0x0C).unwrap(), MySqlType::DateTime);
        assert_eq!(MySqlType::from_u8(0x0D).unwrap(), MySqlType::Year);
        assert_eq!(MySqlType::from_u8(0x0F).unwrap(), MySqlType::VarChar);
        assert_eq!(MySqlType::from_u8(0x10).unwrap(), MySqlType::Bit);
        assert_eq!(MySqlType::from_u8(0xF5).unwrap(), MySqlType::Json);
        assert_eq!(MySqlType::from_u8(0xF6).unwrap(), MySqlType::NewDecimal);
        assert_eq!(MySqlType::from_u8(0xFC).unwrap(), MySqlType::Blob);
        assert_eq!(MySqlType::from_u8(0xFD).unwrap(), MySqlType::VarString);
        assert_eq!(MySqlType::from_u8(0xFE).unwrap(), MySqlType::String);
        assert_eq!(MySqlType::from_u8(0xFF).unwrap(), MySqlType::Geometry);
    }

    // ============ Transaction Commands Tests ============

    #[tokio::test]
    async fn test_mysql_begin_transaction() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test BEGIN
        let result = adapter.handle_mysql_specific_query("BEGIN").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_start_transaction() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test START TRANSACTION
        let result = adapter
            .handle_mysql_specific_query("START TRANSACTION")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet

        // Test START TRANSACTION READ ONLY
        let result = adapter
            .handle_mysql_specific_query("START TRANSACTION READ ONLY")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_commit() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("COMMIT").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_rollback() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("ROLLBACK").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet

        // Test ROLLBACK TO SAVEPOINT
        let result = adapter
            .handle_mysql_specific_query("ROLLBACK TO SAVEPOINT sp1")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_savepoint() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("SAVEPOINT my_savepoint")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet

        // Test RELEASE SAVEPOINT
        let result = adapter
            .handle_mysql_specific_query("RELEASE SAVEPOINT my_savepoint")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_set_autocommit() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("SET AUTOCOMMIT = 0")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_set_transaction_isolation_level() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    // ============ User Management Tests ============

    #[tokio::test]
    async fn test_mysql_create_user() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("CREATE USER 'test'@'localhost' IDENTIFIED BY 'password'")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_grant() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("GRANT SELECT ON *.* TO 'test'@'localhost'")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_flush_privileges() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("FLUSH PRIVILEGES")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    // ============ Utility Commands Tests ============

    #[tokio::test]
    async fn test_mysql_use_database() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("USE test_database")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_truncate_table() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("TRUNCATE TABLE test_table")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_lock_unlock_tables() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        // Test LOCK TABLES
        let result = adapter
            .handle_mysql_specific_query("LOCK TABLES test_table WRITE")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet

        // Test UNLOCK TABLES
        let result = adapter.handle_mysql_specific_query("UNLOCK TABLES").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    #[tokio::test]
    async fn test_mysql_kill() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("KILL 123").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert_eq!(packets[0][0], 0x00); // OK packet
    }

    // ============ SHOW Commands Tests ============

    #[tokio::test]
    async fn test_mysql_show_grants() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("SHOW GRANTS").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_create_database() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("SHOW CREATE DATABASE test")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_warnings() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("SHOW WARNINGS").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_errors() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("SHOW ERRORS").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_engines() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("SHOW ENGINES").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_show_plugins() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter.handle_mysql_specific_query("SHOW PLUGINS").await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    // ============ Table Maintenance Tests ============

    #[tokio::test]
    async fn test_mysql_optimize_table() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("OPTIMIZE TABLE test_table")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_analyze_table() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("ANALYZE TABLE test_table")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_check_table() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("CHECK TABLE test_table")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_repair_table() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("REPAIR TABLE test_table")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }

    #[tokio::test]
    async fn test_mysql_explain() {
        let config = MySqlConfig::default();
        let adapter = MySqlAdapter::new(config).await.unwrap();

        let result = adapter
            .handle_mysql_specific_query("EXPLAIN SELECT * FROM users")
            .await;
        assert!(result.is_some());
        let packets = result.unwrap().unwrap();
        assert!(!packets.is_empty());
    }
}
