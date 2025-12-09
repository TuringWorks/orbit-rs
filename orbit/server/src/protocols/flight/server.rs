//! Arrow Flight SQL Server Implementation
//!
//! Implements the Flight SQL protocol for high-performance OrbitQL query execution

use super::messages::*;
use super::session::*;
use super::types::*;
use super::FlightConfig;

use bytes::Bytes;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// Flight SQL Server for OrbitQL
pub struct FlightSqlServer {
    config: FlightConfig,
    session_manager: Arc<SessionManager>,
    server_info: ServerInfo,
    // Query executor would be injected here
    // executor: Arc<dyn QueryExecutor>,
}

impl FlightSqlServer {
    /// Create a new Flight SQL server
    pub fn new(config: FlightConfig) -> Self {
        Self {
            config,
            session_manager: Arc::new(SessionManager::default()),
            server_info: ServerInfo::default(),
        }
    }

    /// Get the session manager
    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    /// Start the Flight SQL server
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr =
            format!("{}:{}", self.config.bind_address, self.config.port).parse()?;

        info!("Starting Flight SQL server on {}", addr);

        // In a real implementation, we would start the tonic gRPC server here
        // using the arrow-flight crate's FlightService trait
        //
        // Example:
        // let svc = FlightServiceServer::new(self.clone())
        //     .max_decoding_message_size(self.config.max_message_size)
        //     .max_encoding_message_size(self.config.max_message_size);
        //
        // tonic::transport::Server::builder()
        //     .add_service(svc)
        //     .serve(addr)
        //     .await?;

        Ok(())
    }

    /// Handle GetFlightInfo request
    #[instrument(skip(self, command))]
    pub async fn get_flight_info(
        &self,
        command: FlightSqlCommand,
        session: Arc<RwLock<FlightSession>>,
    ) -> Result<FlightInfo, FlightSqlError> {
        match command {
            FlightSqlCommand::StatementQuery(stmt) => {
                self.handle_statement_query_info(&stmt, session).await
            }
            FlightSqlCommand::GetCatalogs(_) => self.handle_get_catalogs_info().await,
            FlightSqlCommand::GetDbSchemas(cmd) => self.handle_get_schemas_info(&cmd).await,
            FlightSqlCommand::GetTables(cmd) => self.handle_get_tables_info(&cmd).await,
            FlightSqlCommand::GetSqlInfo(cmd) => self.handle_get_sql_info(&cmd).await,
            FlightSqlCommand::PreparedStatementQuery(cmd) => {
                self.handle_prepared_query_info(&cmd, session).await
            }
            _ => Err(FlightSqlError::invalid_argument(
                "Command not supported for GetFlightInfo",
            )),
        }
    }

    /// Handle DoGet request - stream result data
    #[instrument(skip(self, ticket))]
    pub async fn do_get(
        &self,
        ticket: &FlightTicket,
        session: Arc<RwLock<FlightSession>>,
    ) -> Result<FlightDataStream, FlightSqlError> {
        debug!("DoGet for ticket: {:?}", ticket.handle_id);

        // Parse the ticket to determine what to stream
        // In real implementation, this would execute the query and stream results

        let schema = SchemaInfo::new(vec![
            FieldInfo::new("id", ArrowDataType::Int64, false),
            FieldInfo::new("name", ArrowDataType::Utf8, true),
        ]);

        Ok(FlightDataStream::new(schema))
    }

    /// Handle DoPut request - upload data or execute prepared statement
    #[instrument(skip(self, _stream))]
    pub async fn do_put(
        &self,
        _stream: FlightDataStream,
        _session: Arc<RwLock<FlightSession>>,
    ) -> Result<PutResult, FlightSqlError> {
        debug!("DoPut request");

        // In real implementation, this would:
        // 1. Parse the flight descriptor to determine the operation
        // 2. Bind parameters to prepared statements
        // 3. Upload bulk data for INSERT

        Ok(PutResult {
            affected_rows: 0,
            app_metadata: Bytes::new(),
        })
    }

    /// Handle DoExchange - bidirectional streaming (for LIVE queries)
    #[instrument(skip(self, _stream))]
    pub async fn do_exchange(
        &self,
        _stream: FlightDataStream,
        _session: Arc<RwLock<FlightSession>>,
    ) -> Result<FlightDataStream, FlightSqlError> {
        debug!("DoExchange request for LIVE query");

        // In real implementation, this would set up bidirectional streaming
        // for LIVE query subscriptions

        let schema = SchemaInfo::new(vec![
            FieldInfo::new("event_type", ArrowDataType::Utf8, false),
            FieldInfo::new("data", ArrowDataType::Utf8, true),
        ]);

        Ok(FlightDataStream::new(schema))
    }

    /// Handle DoAction - execute actions (create prepared statement, transactions, etc.)
    #[instrument(skip(self, action))]
    pub async fn do_action(
        &self,
        action: FlightAction,
        session: Arc<RwLock<FlightSession>>,
    ) -> Result<Vec<FlightActionResult>, FlightSqlError> {
        debug!("DoAction: {}", action.action_type);

        match action.action_type.as_str() {
            "CreatePreparedStatement" => {
                let query = String::from_utf8(action.body.to_vec())
                    .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in query"))?;

                let mut sess = session.write().await;
                let ps = sess.create_prepared_statement(&query)?;

                Ok(vec![FlightActionResult {
                    body: Bytes::copy_from_slice(&ps.handle),
                }])
            }
            "ClosePreparedStatement" => {
                if action.body.len() < 16 {
                    return Err(FlightSqlError::invalid_argument("Invalid handle"));
                }
                let mut handle = [0u8; 16];
                handle.copy_from_slice(&action.body[..16]);

                let mut sess = session.write().await;
                sess.close_prepared_statement(&handle);

                Ok(vec![FlightActionResult { body: Bytes::new() }])
            }
            "BeginTransaction" => {
                let isolation = if action.body.is_empty() {
                    IsolationLevel::default()
                } else {
                    match action.body[0] {
                        1 => IsolationLevel::ReadUncommitted,
                        2 => IsolationLevel::ReadCommitted,
                        3 => IsolationLevel::RepeatableRead,
                        4 => IsolationLevel::Serializable,
                        5 => IsolationLevel::Snapshot,
                        _ => IsolationLevel::default(),
                    }
                };

                let mut sess = session.write().await;
                let tx_id = sess.begin_transaction(isolation)?;

                Ok(vec![FlightActionResult {
                    body: Bytes::copy_from_slice(&tx_id),
                }])
            }
            "EndTransaction" => {
                if action.body.len() < 17 {
                    return Err(FlightSqlError::invalid_argument("Invalid transaction data"));
                }

                let action_type = action.body[16];
                let mut sess = session.write().await;

                let tx_id = if action_type == 1 {
                    sess.commit_transaction()?
                } else {
                    sess.rollback_transaction()?
                };

                Ok(vec![FlightActionResult {
                    body: Bytes::copy_from_slice(&tx_id),
                }])
            }
            "BeginSavepoint" => {
                // First 16 bytes: transaction ID, rest: savepoint name
                if action.body.len() < 17 {
                    return Err(FlightSqlError::invalid_argument("Invalid savepoint data"));
                }

                let name = String::from_utf8(action.body[16..].to_vec())
                    .map_err(|_| FlightSqlError::invalid_argument("Invalid savepoint name"))?;

                let mut sess = session.write().await;
                let sp_id = sess.create_savepoint(&name)?;

                Ok(vec![FlightActionResult {
                    body: Bytes::copy_from_slice(&sp_id),
                }])
            }
            "EndSavepoint" => {
                // First 16 bytes: transaction ID, next 16: savepoint ID, last byte: action
                if action.body.len() < 33 {
                    return Err(FlightSqlError::invalid_argument("Invalid savepoint data"));
                }

                let mut savepoint_id = [0u8; 16];
                savepoint_id.copy_from_slice(&action.body[16..32]);
                let action_type = action.body[32];

                let mut sess = session.write().await;

                // Find savepoint by ID (simplified - in real impl would track by ID)
                let name = sess
                    .transaction
                    .as_ref()
                    .and_then(|tx| tx.savepoints.last())
                    .map(|s| s.clone());

                if let Some(name) = name {
                    if action_type == 1 {
                        sess.release_savepoint(&name)?;
                    } else {
                        sess.rollback_to_savepoint(&name)?;
                    }
                }

                Ok(vec![FlightActionResult {
                    body: Bytes::copy_from_slice(&savepoint_id),
                }])
            }
            "SubscribeLiveQuery" => {
                let query = String::from_utf8(action.body.to_vec())
                    .map_err(|_| FlightSqlError::invalid_argument("Invalid UTF-8 in query"))?;

                let mut sess = session.write().await;
                let lq = sess.subscribe_live_query(&query);

                Ok(vec![FlightActionResult {
                    body: Bytes::copy_from_slice(&lq.subscription_id),
                }])
            }
            "KillLiveQuery" => {
                if action.body.len() < 16 {
                    return Err(FlightSqlError::invalid_argument("Invalid subscription ID"));
                }
                let mut subscription_id = [0u8; 16];
                subscription_id.copy_from_slice(&action.body[..16]);

                let mut sess = session.write().await;
                sess.unsubscribe_live_query(&subscription_id);

                Ok(vec![FlightActionResult { body: Bytes::new() }])
            }
            _ => Err(FlightSqlError::invalid_argument(format!(
                "Unknown action: {}",
                action.action_type
            ))),
        }
    }

    /// Handle statement query info request
    async fn handle_statement_query_info(
        &self,
        stmt: &StatementQuery,
        _session: Arc<RwLock<FlightSession>>,
    ) -> Result<FlightInfo, FlightSqlError> {
        debug!("Getting flight info for query: {}", stmt.query);

        // In real implementation:
        // 1. Parse the query
        // 2. Analyze to determine schema
        // 3. Create query handle

        let handle = QueryHandle::new(&stmt.query);

        // Simulated schema - would come from query analysis
        let schema = SchemaInfo::new(vec![FieldInfo::new("result", ArrowDataType::Utf8, true)]);

        Ok(FlightInfo {
            schema: Some(schema),
            endpoints: vec![FlightEndpoint {
                ticket: FlightTicket {
                    handle_id: handle.handle_id,
                },
                locations: vec![],
            }],
            total_records: None,
            total_bytes: None,
        })
    }

    /// Handle GetCatalogs info
    async fn handle_get_catalogs_info(&self) -> Result<FlightInfo, FlightSqlError> {
        let schema = SchemaInfo::new(vec![FieldInfo::new(
            "catalog_name",
            ArrowDataType::Utf8,
            false,
        )]);

        let mut handle_id = [0u8; 16];
        for byte in &mut handle_id {
            *byte = rand::random();
        }

        Ok(FlightInfo {
            schema: Some(schema),
            endpoints: vec![FlightEndpoint {
                ticket: FlightTicket { handle_id },
                locations: vec![],
            }],
            total_records: None,
            total_bytes: None,
        })
    }

    /// Handle GetDbSchemas info
    async fn handle_get_schemas_info(
        &self,
        _cmd: &GetDbSchemas,
    ) -> Result<FlightInfo, FlightSqlError> {
        let schema = SchemaInfo::new(vec![
            FieldInfo::new("catalog_name", ArrowDataType::Utf8, true),
            FieldInfo::new("db_schema_name", ArrowDataType::Utf8, false),
        ]);

        let mut handle_id = [0u8; 16];
        for byte in &mut handle_id {
            *byte = rand::random();
        }

        Ok(FlightInfo {
            schema: Some(schema),
            endpoints: vec![FlightEndpoint {
                ticket: FlightTicket { handle_id },
                locations: vec![],
            }],
            total_records: None,
            total_bytes: None,
        })
    }

    /// Handle GetTables info
    async fn handle_get_tables_info(&self, cmd: &GetTables) -> Result<FlightInfo, FlightSqlError> {
        let mut fields = vec![
            FieldInfo::new("catalog_name", ArrowDataType::Utf8, true),
            FieldInfo::new("db_schema_name", ArrowDataType::Utf8, true),
            FieldInfo::new("table_name", ArrowDataType::Utf8, false),
            FieldInfo::new("table_type", ArrowDataType::Utf8, false),
        ];

        if cmd.include_schema {
            fields.push(FieldInfo::new("table_schema", ArrowDataType::Binary, true));
        }

        let schema = SchemaInfo::new(fields);

        let mut handle_id = [0u8; 16];
        for byte in &mut handle_id {
            *byte = rand::random();
        }

        Ok(FlightInfo {
            schema: Some(schema),
            endpoints: vec![FlightEndpoint {
                ticket: FlightTicket { handle_id },
                locations: vec![],
            }],
            total_records: None,
            total_bytes: None,
        })
    }

    /// Handle GetSqlInfo
    async fn handle_get_sql_info(&self, cmd: &GetSqlInfo) -> Result<FlightInfo, FlightSqlError> {
        let schema = SchemaInfo::new(vec![
            FieldInfo::new("info_name", ArrowDataType::UInt32, false),
            FieldInfo::new("value", ArrowDataType::Union, false),
        ]);

        let mut handle_id = [0u8; 16];
        for byte in &mut handle_id {
            *byte = rand::random();
        }

        Ok(FlightInfo {
            schema: Some(schema),
            endpoints: vec![FlightEndpoint {
                ticket: FlightTicket { handle_id },
                locations: vec![],
            }],
            total_records: Some(cmd.info.len() as u64),
            total_bytes: None,
        })
    }

    /// Handle prepared statement query info
    async fn handle_prepared_query_info(
        &self,
        cmd: &PreparedStatementQuery,
        session: Arc<RwLock<FlightSession>>,
    ) -> Result<FlightInfo, FlightSqlError> {
        let sess = session.read().await;
        let ps = sess
            .get_prepared_statement(&cmd.prepared_statement_handle)
            .ok_or_else(|| FlightSqlError::not_found("Prepared statement not found"))?;

        let schema = ps.result_schema.clone().unwrap_or_else(|| {
            SchemaInfo::new(vec![FieldInfo::new("result", ArrowDataType::Utf8, true)])
        });

        Ok(FlightInfo {
            schema: Some(schema),
            endpoints: vec![FlightEndpoint {
                ticket: FlightTicket {
                    handle_id: cmd.prepared_statement_handle,
                },
                locations: vec![],
            }],
            total_records: None,
            total_bytes: None,
        })
    }

    /// Get SQL info values for a list of info codes
    pub fn get_sql_info_values(&self, info_codes: &[u32]) -> HashMap<u32, SqlInfoValue> {
        let mut result = HashMap::new();

        for &code in info_codes {
            let value = match code {
                sql_info::FLIGHT_SQL_SERVER_NAME => {
                    Some(SqlInfoValue::String(self.server_info.name.clone()))
                }
                sql_info::FLIGHT_SQL_SERVER_VERSION => {
                    Some(SqlInfoValue::String(self.server_info.version.clone()))
                }
                sql_info::FLIGHT_SQL_SERVER_ARROW_VERSION => {
                    Some(SqlInfoValue::String(self.server_info.arrow_version.clone()))
                }
                sql_info::FLIGHT_SQL_SERVER_READ_ONLY => Some(SqlInfoValue::Bool(false)),
                sql_info::FLIGHT_SQL_SERVER_SQL => Some(SqlInfoValue::Bool(true)),
                sql_info::FLIGHT_SQL_SERVER_TRANSACTION => {
                    Some(SqlInfoValue::Int32(3)) // TRANSACTION_SAVEPOINT
                }
                sql_info::SQL_IDENTIFIER_QUOTE_CHAR => Some(SqlInfoValue::String("\"".to_string())),
                sql_info::SQL_IDENTIFIER_CASE => {
                    Some(SqlInfoValue::Int32(2)) // SQL_IC_LOWER
                }
                sql_info::SQL_SAVEPOINTS_SUPPORTED => Some(SqlInfoValue::Bool(true)),
                sql_info::SQL_TRANSACTIONS_SUPPORTED => Some(SqlInfoValue::Bool(true)),
                sql_info::SQL_DDL_CATALOG => Some(SqlInfoValue::Bool(true)),
                sql_info::SQL_DDL_SCHEMA => Some(SqlInfoValue::Bool(true)),
                sql_info::SQL_DDL_TABLE => Some(SqlInfoValue::Bool(true)),
                sql_info::SQL_KEYWORDS => Some(SqlInfoValue::StringList(vec![
                    "DEFINE".to_string(),
                    "REMOVE".to_string(),
                    "LIVE".to_string(),
                    "KILL".to_string(),
                    "TRAVERSE".to_string(),
                    "RELATE".to_string(),
                    "MATCH".to_string(),
                    "SCHEMAFULL".to_string(),
                    "SCHEMALESS".to_string(),
                    "PERMISSIONS".to_string(),
                    "HNSW".to_string(),
                    "MTREE".to_string(),
                ])),
                _ => None,
            };

            if let Some(v) = value {
                result.insert(code, v);
            }
        }

        result
    }
}

/// Flight info response
#[derive(Debug, Clone)]
pub struct FlightInfo {
    pub schema: Option<SchemaInfo>,
    pub endpoints: Vec<FlightEndpoint>,
    pub total_records: Option<u64>,
    pub total_bytes: Option<u64>,
}

/// Flight endpoint
#[derive(Debug, Clone)]
pub struct FlightEndpoint {
    pub ticket: FlightTicket,
    pub locations: Vec<String>,
}

/// Flight ticket for DoGet
#[derive(Debug, Clone)]
pub struct FlightTicket {
    pub handle_id: [u8; 16],
}

/// Flight action request
#[derive(Debug, Clone)]
pub struct FlightAction {
    pub action_type: String,
    pub body: Bytes,
}

/// Flight action result
#[derive(Debug, Clone)]
pub struct FlightActionResult {
    pub body: Bytes,
}

/// Put result
#[derive(Debug, Clone)]
pub struct PutResult {
    pub affected_rows: i64,
    pub app_metadata: Bytes,
}

/// Flight data stream (placeholder for actual Arrow Flight stream)
pub struct FlightDataStream {
    schema: SchemaInfo,
    // In real implementation, this would contain the actual stream
}

impl FlightDataStream {
    pub fn new(schema: SchemaInfo) -> Self {
        Self { schema }
    }

    pub fn schema(&self) -> &SchemaInfo {
        &self.schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_flight_server_creation() {
        let config = FlightConfig::default();
        let server = FlightSqlServer::new(config);

        assert_eq!(server.config.port, 50052);
        assert_eq!(server.server_info.name, "Orbit-RS");
    }

    #[tokio::test]
    async fn test_create_prepared_statement_action() {
        let config = FlightConfig::default();
        let server = FlightSqlServer::new(config);

        let session = server.session_manager().create_session().await;

        let action = FlightAction {
            action_type: "CreatePreparedStatement".to_string(),
            body: Bytes::from("SELECT * FROM users WHERE id = ?"),
        };

        let results = server.do_action(action, session.clone()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].body.len(), 16);

        // Verify prepared statement was created
        let sess = session.read().await;
        assert_eq!(sess.prepared_statements.len(), 1);
    }

    #[tokio::test]
    async fn test_transaction_actions() {
        let config = FlightConfig::default();
        let server = FlightSqlServer::new(config);

        let session = server.session_manager().create_session().await;

        // Begin transaction
        let begin_action = FlightAction {
            action_type: "BeginTransaction".to_string(),
            body: Bytes::from(vec![2u8]), // ReadCommitted
        };

        let results = server
            .do_action(begin_action, session.clone())
            .await
            .unwrap();
        assert_eq!(results[0].body.len(), 16);

        let tx_id = results[0].body.clone();

        // End transaction (commit)
        let mut end_body = tx_id.to_vec();
        end_body.push(1); // Commit

        let end_action = FlightAction {
            action_type: "EndTransaction".to_string(),
            body: Bytes::from(end_body),
        };

        let results = server.do_action(end_action, session.clone()).await.unwrap();
        assert_eq!(results[0].body.len(), 16);

        // Verify transaction is ended
        let sess = session.read().await;
        assert!(!sess.has_transaction());
    }

    #[test]
    fn test_sql_info_values() {
        let config = FlightConfig::default();
        let server = FlightSqlServer::new(config);

        let info_codes = vec![
            sql_info::FLIGHT_SQL_SERVER_NAME,
            sql_info::FLIGHT_SQL_SERVER_VERSION,
            sql_info::SQL_SAVEPOINTS_SUPPORTED,
        ];

        let values = server.get_sql_info_values(&info_codes);

        assert!(values.contains_key(&sql_info::FLIGHT_SQL_SERVER_NAME));
        assert!(values.contains_key(&sql_info::SQL_SAVEPOINTS_SUPPORTED));

        match &values[&sql_info::FLIGHT_SQL_SERVER_NAME] {
            SqlInfoValue::String(s) => assert_eq!(s, "Orbit-RS"),
            _ => panic!("Expected string value"),
        }

        match &values[&sql_info::SQL_SAVEPOINTS_SUPPORTED] {
            SqlInfoValue::Bool(b) => assert!(*b),
            _ => panic!("Expected bool value"),
        }
    }
}
