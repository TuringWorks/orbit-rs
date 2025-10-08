//! PostgreSQL wire protocol handler

use bytes::{BufMut, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info};

use super::messages::{
    type_oids, AuthenticationResponse, BackendMessage, FieldDescription, FrontendMessage,
    TransactionStatus,
};
use super::query_engine::{QueryEngine, QueryResult};
use crate::error::{ProtocolError, ProtocolResult};

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Initial,
    Authenticating,
    Authenticated,
    Ready,
    InTransaction,
    Closed,
}

/// PostgreSQL wire protocol handler
pub struct PostgresWireProtocol {
    state: ConnectionState,
    username: Option<String>,
    database: Option<String>,
    parameters: HashMap<String, String>,
    query_engine: Arc<QueryEngine>,
    process_id: i32,
    secret_key: i32,
    prepared_statements: HashMap<String, String>,
    portals: HashMap<String, (String, Vec<Option<bytes::Bytes>>)>,
}

impl PostgresWireProtocol {
    /// Create a new PostgreSQL protocol handler
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Initial,
            username: None,
            database: None,
            parameters: HashMap::new(),
            query_engine: Arc::new(QueryEngine::new()),
            process_id: std::process::id() as i32,
            secret_key: Self::random(),
            prepared_statements: HashMap::new(),
            portals: HashMap::new(),
        }
    }

    /// Create a new PostgreSQL protocol handler with custom query engine
    pub fn new_with_query_engine(query_engine: Arc<QueryEngine>) -> Self {
        Self {
            state: ConnectionState::Initial,
            username: None,
            database: None,
            parameters: HashMap::new(),
            query_engine,
            process_id: std::process::id() as i32,
            secret_key: Self::random(),
            prepared_statements: HashMap::new(),
            portals: HashMap::new(),
        }
    }

    /// Handle a client connection
    pub async fn handle_connection(&mut self, mut stream: TcpStream) -> ProtocolResult<()> {
        info!("New PostgreSQL client connection");

        let mut read_buf = BytesMut::with_capacity(8192);
        let mut write_buf = BytesMut::with_capacity(8192);

        loop {
            // Read data from client
            let n = stream.read_buf(&mut read_buf).await?;

            if n == 0 {
                info!("Client disconnected");
                break;
            }

            // Process messages
            while let Some(msg) = FrontendMessage::parse(&mut read_buf)? {
                debug!("Received message: {:?}", msg);

                match self.handle_message(msg, &mut write_buf).await {
                    Ok(should_continue) => {
                        if !should_continue {
                            info!("Client terminated connection");
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        error!("Error handling message: {}", e);
                        self.send_error(&mut write_buf, &e.to_string());
                    }
                }

                // Flush write buffer
                if !write_buf.is_empty() {
                    stream.write_all(&write_buf).await?;
                    write_buf.clear();
                }
            }
        }

        Ok(())
    }

    /// Handle a single frontend message
    async fn handle_message(
        &mut self,
        msg: FrontendMessage,
        buf: &mut BytesMut,
    ) -> ProtocolResult<bool> {
        match msg {
            FrontendMessage::Startup {
                protocol_version,
                parameters,
            } => {
                self.handle_startup(protocol_version, parameters, buf)
                    .await?;
            }
            FrontendMessage::Password { password } => {
                self.handle_password(&password, buf).await?;
            }
            FrontendMessage::Query { query } => {
                self.handle_query(&query, buf).await?;
            }
            FrontendMessage::Parse {
                statement_name,
                query,
                param_types,
            } => {
                self.handle_parse(&statement_name, &query, param_types, buf)?;
            }
            FrontendMessage::Bind {
                portal,
                statement,
                param_formats,
                params,
                result_formats,
            } => {
                self.handle_bind(
                    &portal,
                    &statement,
                    param_formats,
                    params,
                    result_formats,
                    buf,
                )?;
            }
            FrontendMessage::Execute { portal, max_rows } => {
                self.handle_execute(&portal, max_rows, buf).await?;
            }
            FrontendMessage::Describe { target, name } => {
                self.handle_describe(target, &name, buf)?;
            }
            FrontendMessage::Close { target, name } => {
                self.handle_close(target, &name, buf)?;
            }
            FrontendMessage::Sync => {
                BackendMessage::ReadyForQuery {
                    status: TransactionStatus::Idle,
                }
                .encode(buf);
            }
            FrontendMessage::Flush => {
                // Nothing to do, data is flushed after each message
            }
            FrontendMessage::Terminate => {
                return Ok(false);
            }
            FrontendMessage::SSLRequest => {
                self.handle_ssl_request(buf).await?;
            }
        }

        Ok(true)
    }

    /// Handle startup message
    async fn handle_startup(
        &mut self,
        protocol_version: i32,
        parameters: HashMap<String, String>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        info!(
            "Startup: protocol_version={}, parameters={:?}",
            protocol_version, parameters
        );

        self.username = parameters.get("user").cloned();
        self.database = parameters.get("database").cloned();
        self.parameters = parameters;
        self.state = ConnectionState::Authenticating;

        // For simplicity, use trust authentication (no password required)
        // In production, should use MD5 or SCRAM-SHA-256
        BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);

        // Send parameter status
        BackendMessage::ParameterStatus {
            name: "server_version".to_string(),
            value: "14.0 (Orbit-RS Protocol Adapter)".to_string(),
        }
        .encode(buf);

        BackendMessage::ParameterStatus {
            name: "server_encoding".to_string(),
            value: "UTF8".to_string(),
        }
        .encode(buf);

        BackendMessage::ParameterStatus {
            name: "client_encoding".to_string(),
            value: "UTF8".to_string(),
        }
        .encode(buf);

        // Send backend key data
        BackendMessage::BackendKeyData {
            process_id: self.process_id,
            secret_key: self.secret_key,
        }
        .encode(buf);

        // Ready for query
        BackendMessage::ReadyForQuery {
            status: TransactionStatus::Idle,
        }
        .encode(buf);

        self.state = ConnectionState::Ready;

        Ok(())
    }

    /// Handle password message
    async fn handle_password(&mut self, _password: &str, buf: &mut BytesMut) -> ProtocolResult<()> {
        // For trust authentication, this shouldn't be called
        BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);
        self.state = ConnectionState::Authenticated;
        Ok(())
    }

    /// Handle simple query
    async fn handle_query(&mut self, query: &str, buf: &mut BytesMut) -> ProtocolResult<()> {
        info!("Query: {}", query);

        if query.trim().is_empty() {
            BackendMessage::EmptyQueryResponse.encode(buf);
            BackendMessage::ReadyForQuery {
                status: TransactionStatus::Idle,
            }
            .encode(buf);
            return Ok(());
        }

        match self.query_engine.execute_query(query).await {
            Ok(result) => {
                self.send_query_result(&result, buf);
                BackendMessage::ReadyForQuery {
                    status: TransactionStatus::Idle,
                }
                .encode(buf);
            }
            Err(e) => {
                self.send_error(buf, &e.to_string());
                BackendMessage::ReadyForQuery {
                    status: TransactionStatus::Idle,
                }
                .encode(buf);
            }
        }

        Ok(())
    }

    /// Handle parse message (prepared statement)
    fn handle_parse(
        &mut self,
        statement_name: &str,
        query: &str,
        _param_types: Vec<i32>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Parse: name={}, query={}", statement_name, query);

        self.prepared_statements
            .insert(statement_name.to_string(), query.to_string());

        BackendMessage::ParseComplete.encode(buf);
        Ok(())
    }

    /// Handle bind message
    fn handle_bind(
        &mut self,
        portal: &str,
        statement: &str,
        _param_formats: Vec<i16>,
        params: Vec<Option<bytes::Bytes>>,
        _result_formats: Vec<i16>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Bind: portal={}, statement={}", portal, statement);

        self.portals
            .insert(portal.to_string(), (statement.to_string(), params));

        BackendMessage::BindComplete.encode(buf);
        Ok(())
    }

    /// Handle execute message
    async fn handle_execute(
        &mut self,
        portal: &str,
        _max_rows: i32,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Execute: portal={}", portal);

        let (statement_name, params) = self
            .portals
            .get(portal)
            .ok_or_else(|| ProtocolError::PostgresError(format!("Portal not found: {}", portal)))?;

        let query = self
            .prepared_statements
            .get(statement_name)
            .ok_or_else(|| {
                ProtocolError::PostgresError(format!("Statement not found: {}", statement_name))
            })?;

        // For now, ignore parameters and execute the query
        // TODO: Implement parameter substitution
        let _ = params;

        match self.query_engine.execute_query(query).await {
            Ok(result) => {
                self.send_query_result(&result, buf);
            }
            Err(e) => {
                self.send_error(buf, &e.to_string());
            }
        }

        Ok(())
    }

    /// Handle describe message
    fn handle_describe(
        &mut self,
        target: super::messages::DescribeTarget,
        name: &str,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Describe: target={:?}, name={}", target, name);

        // For now, return NoData
        // TODO: Implement proper description based on query
        BackendMessage::NoData.encode(buf);
        Ok(())
    }

    /// Handle close message
    fn handle_close(
        &mut self,
        target: super::messages::CloseTarget,
        name: &str,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Close: target={:?}, name={}", target, name);

        match target {
            super::messages::CloseTarget::Statement => {
                self.prepared_statements.remove(name);
            }
            super::messages::CloseTarget::Portal => {
                self.portals.remove(name);
            }
        }

        BackendMessage::CloseComplete.encode(buf);
        Ok(())
    }

    /// Send query result
    fn send_query_result(&self, result: &QueryResult, buf: &mut BytesMut) {
        match result {
            QueryResult::Select { columns, rows } => {
                // Send row description
                let fields: Vec<FieldDescription> = columns
                    .iter()
                    .map(|col| FieldDescription {
                        name: col.clone(),
                        table_oid: 0,
                        column_id: 0,
                        type_oid: type_oids::TEXT, // Default to TEXT
                        type_size: -1,
                        type_modifier: -1,
                        format: 0,
                    })
                    .collect();

                BackendMessage::RowDescription { fields }.encode(buf);

                // Send data rows
                for row in rows {
                    let values: Vec<Option<bytes::Bytes>> = row
                        .iter()
                        .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                        .collect();
                    BackendMessage::DataRow { values }.encode(buf);
                }

                // Send command complete
                BackendMessage::CommandComplete {
                    tag: format!("SELECT {}", rows.len()),
                }
                .encode(buf);
            }
            QueryResult::Insert { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("INSERT 0 {}", count),
                }
                .encode(buf);
            }
            QueryResult::Update { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("UPDATE {}", count),
                }
                .encode(buf);
            }
            QueryResult::Delete { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("DELETE {}", count),
                }
                .encode(buf);
            }
        }
    }

    /// Handle SSL request
    async fn handle_ssl_request(&mut self, buf: &mut BytesMut) -> ProtocolResult<()> {
        // Reject SSL request - send 'N' to indicate SSL not supported
        buf.put_u8(b'N');
        Ok(())
    }

    /// Send error response
    fn send_error(&self, buf: &mut BytesMut, message: &str) {
        let mut fields = HashMap::new();
        fields.insert(b'S', "ERROR".to_string());
        fields.insert(b'C', "XX000".to_string()); // Internal error
        fields.insert(b'M', message.to_string());

        BackendMessage::ErrorResponse { fields }.encode(buf);
    }
}

impl Default for PostgresWireProtocol {
    fn default() -> Self {
        Self::new()
    }
}

// Add rand dependency for secret_key generation
use rand::Rng;
impl PostgresWireProtocol {
    fn random() -> i32 {
        rand::thread_rng().gen()
    }
}
