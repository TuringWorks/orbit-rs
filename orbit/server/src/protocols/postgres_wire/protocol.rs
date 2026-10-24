//! PostgreSQL wire protocol handler

use bytes::{BufMut, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tracing::{debug, error, info};

use super::auth::{AuthManager, AuthMethod, ScramAuth, UserStore};
use super::messages::{
    type_oids, AuthenticationResponse, BackendMessage, FieldDescription, FrontendMessage,
    TransactionStatus,
};
use super::query_engine::{QueryEngine, QueryResult};
use crate::protocols::error::{ProtocolError, ProtocolResult};

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
    /// PostgreSQL 18 (protocol 3.2): Variable-length cancel key (4-256 bytes)
    /// Default: 4 bytes for backward compatibility with protocol 3.0
    secret_key: Vec<u8>,
    prepared_statements: HashMap<String, String>,
    portals: HashMap<String, (String, Vec<Option<bytes::Bytes>>)>,
    auth_manager: AuthManager,
    scram_auth: Option<ScramAuth>,
    /// The durable engine's per-connection state (the open transaction).
    sql_session: super::sql::durable::Session,
    /// Create a user on first login with its name as the password. Only for development and tests.
    auto_register: bool,
    /// Close the connection once the messages queued for the client have been sent.
    close_after: bool,
}

/// Result of processing data in the connection loop
#[derive(Debug)]
enum ConnectionLoopResult {
    Continue,
    ClientDisconnected,
    ClientTerminated,
}

/// Result of processing a single message
#[derive(Debug)]
enum MessageResult {
    Continue,
    Terminate,
    Error(ProtocolError),
}

impl PostgresWireProtocol {
    /// Create a new PostgreSQL protocol handler
    pub fn new() -> Self {
        // Initialize user store with a default user
        let user_store = UserStore::new();
        // TODO: In a real app, we wouldn't add this user here or we'd load from config
        // Default: Enable SCRAM-SHA-256
        let auth_method = AuthMethod::ScramSha256;
        let auth_manager = AuthManager::new(auth_method, user_store);

        Self {
            state: ConnectionState::Initial,
            username: None,
            database: None,
            parameters: HashMap::new(),
            query_engine: Arc::new(QueryEngine::new()),
            process_id: std::process::id() as i32,
            secret_key: Self::random_secret_key(),
            prepared_statements: HashMap::new(),
            portals: HashMap::new(),
            auth_manager,
            scram_auth: None,
            sql_session: super::sql::durable::Session::default(),
            auto_register: true,
            close_after: false,
        }
    }

    /// Create a new PostgreSQL protocol handler with custom query engine
    pub fn new_with_query_engine(query_engine: Arc<QueryEngine>) -> Self {
        println!("DEBUG: PostgresWireProtocol initialized with custom QueryEngine");
        let user_store = UserStore::new();
        let auth_method = AuthMethod::ScramSha256;
        let auth_manager = AuthManager::new(auth_method, user_store);

        Self {
            state: ConnectionState::Initial,
            username: None,
            database: None,
            parameters: HashMap::new(),
            query_engine,
            process_id: std::process::id() as i32,
            secret_key: Self::random_secret_key(),
            prepared_statements: HashMap::new(),
            portals: HashMap::new(),
            auth_manager,
            scram_auth: None,
            sql_session: super::sql::durable::Session::default(),
            auto_register: true,
            close_after: false,
        }
    }

    /// Authenticate against `users`, which are provisioned by the operator. With `auto_register` a login
    /// by an unknown name creates the user with its own name as the password: for development and tests only.
    #[must_use]
    pub fn with_auth(mut self, users: UserStore, auto_register: bool) -> Self {
        self.auth_manager = AuthManager::new(AuthMethod::ScramSha256, users);
        self.auto_register = auto_register;
        self
    }

    /// Handle a generic connection stream (TCP or TLS)
    pub async fn handle_connection<S>(&mut self, mut stream: S) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        info!("New PostgreSQL client connection");

        let mut read_buf = BytesMut::with_capacity(8192);
        let mut write_buf = BytesMut::with_capacity(8192);

        loop {
            match self
                .read_and_process_data(&mut stream, &mut read_buf, &mut write_buf)
                .await?
            {
                ConnectionLoopResult::Continue => continue,
                ConnectionLoopResult::ClientDisconnected => {
                    info!("Client disconnected");
                    break;
                }
                ConnectionLoopResult::ClientTerminated => {
                    info!("Client terminated connection");
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Read and process data from the client
    async fn read_and_process_data<S>(
        &mut self,
        stream: &mut S,
        read_buf: &mut BytesMut,
        write_buf: &mut BytesMut,
    ) -> ProtocolResult<ConnectionLoopResult>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let n = stream.read_buf(read_buf).await?;

        if n == 0 {
            return Ok(ConnectionLoopResult::ClientDisconnected);
        }

        self.process_pending_messages(stream, read_buf, write_buf)
            .await
    }

    /// Process all pending messages in the read buffer
    async fn process_pending_messages<S>(
        &mut self,
        stream: &mut S,
        read_buf: &mut BytesMut,
        write_buf: &mut BytesMut,
    ) -> ProtocolResult<ConnectionLoopResult>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        while let Some(msg) = FrontendMessage::parse(read_buf)? {
            debug!("Received message: {:?}", msg);

            match self.process_single_message(msg, write_buf).await {
                MessageResult::Continue => {}
                MessageResult::Terminate => {
                    // a fatal error may be waiting to be sent
                    self.flush_write_buffer(stream, write_buf).await?;
                    return Ok(ConnectionLoopResult::ClientTerminated);
                }
                MessageResult::Error(e) => {
                    error!("Error handling message: {}", e);
                    self.send_error(write_buf, &e.to_string());
                }
            }

            self.flush_write_buffer(stream, write_buf).await?;
        }

        Ok(ConnectionLoopResult::Continue)
    }

    /// Process a single message
    async fn process_single_message(
        &mut self,
        msg: FrontendMessage,
        write_buf: &mut BytesMut,
    ) -> MessageResult {
        match self.handle_message(msg, write_buf).await {
            Ok(should_continue) => {
                if should_continue {
                    MessageResult::Continue
                } else {
                    MessageResult::Terminate
                }
            }
            Err(e) => MessageResult::Error(e),
        }
    }

    /// Flush the write buffer to the stream
    async fn flush_write_buffer<S>(
        &mut self,
        stream: &mut S,
        write_buf: &mut BytesMut,
    ) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncWrite + Unpin,
    {
        if !write_buf.is_empty() {
            stream.write_all(write_buf).await?;
            write_buf.clear();
        }
        Ok(())
    }

    /// Handle a single frontend message
    async fn handle_message(
        &mut self,
        msg: FrontendMessage,
        buf: &mut BytesMut,
    ) -> ProtocolResult<bool> {
        // Until the client has authenticated it may only take part in logging in.
        if !matches!(
            self.state,
            ConnectionState::Ready | ConnectionState::InTransaction
        ) && !matches!(
            msg,
            FrontendMessage::Startup { .. }
                | FrontendMessage::Password { .. }
                | FrontendMessage::SASLInitialResponse { .. }
                | FrontendMessage::SASLResponse { .. }
                | FrontendMessage::SSLRequest
                | FrontendMessage::Terminate
        ) {
            self.send_fatal(buf, "28000", "the connection is not authenticated");
            return Ok(false);
        }
        let keep_open = self.handle_message_authenticated(msg, buf).await?;
        Ok(keep_open && !self.close_after)
    }

    async fn handle_message_authenticated(
        &mut self,
        msg: FrontendMessage,
        buf: &mut BytesMut,
    ) -> ProtocolResult<bool> {
        match msg {
            FrontendMessage::Startup {
                protocol_version,
                parameters,
            } => {
                if matches!(self.state, ConnectionState::Initial) {
                    self.handle_startup(protocol_version, parameters, buf)
                        .await?;
                }
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
                self.handle_describe(target, &name, buf).await?;
            }
            FrontendMessage::Close { target, name } => {
                self.handle_close(target, &name, buf)?;
            }
            FrontendMessage::Sync => {
                BackendMessage::ReadyForQuery {
                    status: self.txn_status(),
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
            FrontendMessage::SASLInitialResponse { mechanism, data } => {
                self.handle_sasl_initial_response(&mechanism, data, buf)
                    .await?;
            }
            FrontendMessage::SASLResponse { data } => {
                self.handle_sasl_response(data, buf).await?;
            }
            FrontendMessage::FunctionCall { .. } => {
                // Function call support is minimal/stubbed
            }
            FrontendMessage::CopyData { .. }
            | FrontendMessage::CopyDone
            | FrontendMessage::CopyFail { .. } => {
                // Copy protocol not fully supported by server yet
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

        // PostgreSQL 18 Protocol Version Negotiation
        // Protocol version format: major * 65536 + minor (e.g., 3.0 = 196608, 3.2 = 196610)
        let major = protocol_version >> 16;
        let minor = protocol_version & 0xFFFF;

        // We support protocol 3.0 (fully) and 3.2 (partially - message types defined)
        // If client requests protocol > 3.0, we negotiate down to 3.0
        const SUPPORTED_MAJOR: i32 = 3;
        const SUPPORTED_MINOR: i32 = 0;

        // Check for unrecognized protocol options (those starting with _pq_.)
        let unrecognized_options: Vec<String> = parameters
            .keys()
            .filter(|k| k.starts_with("_pq_."))
            .cloned()
            .collect();

        // Send NegotiateProtocolVersion if needed (PG18 protocol 3.2 feature)
        if major != SUPPORTED_MAJOR || minor > SUPPORTED_MINOR || !unrecognized_options.is_empty() {
            if major == SUPPORTED_MAJOR && minor > SUPPORTED_MINOR {
                // Client requested a newer minor version, negotiate to our supported version
                info!(
                    "Protocol negotiation: client requested {}.{}, negotiating to {}.{}",
                    major, minor, SUPPORTED_MAJOR, SUPPORTED_MINOR
                );
            }
            if !unrecognized_options.is_empty() {
                info!("Unrecognized protocol options: {:?}", unrecognized_options);
            }
            BackendMessage::NegotiateProtocolVersion {
                newest_minor_version: SUPPORTED_MINOR,
                unrecognized_options,
            }
            .encode(buf);
        }

        self.username = parameters.get("user").cloned();
        self.database = parameters.get("database").cloned();
        self.parameters = parameters;
        self.state = ConnectionState::Authenticating;

        // Development only: create the user with its name as the password.
        if self.auto_register && matches!(self.auth_manager.auth_method(), AuthMethod::ScramSha256)
        {
            if let Some(user) = &self.username {
                if self
                    .auth_manager
                    .user_store()
                    .get_user(user)
                    .await
                    .is_none()
                {
                    self.auth_manager
                        .user_store()
                        .add_user(user.clone(), user.clone(), &AuthMethod::ScramSha256)
                        .await;
                }
            }
        }

        let response = self.auth_manager.get_initial_auth_response();
        BackendMessage::Authentication(response.clone()).encode(buf);

        if let AuthenticationResponse::Ok = response {
            self.finish_authentication(buf);
        }

        Ok(())
    }

    /// Finish authentication and unblock connection
    fn finish_authentication(&mut self, buf: &mut BytesMut) {
        // Parameters libpq/psycopg expect to be reported at startup
        for (name, value) in [
            ("server_version", super::session_compat::SERVER_VERSION),
            ("server_encoding", "UTF8"),
            ("client_encoding", "UTF8"),
            ("DateStyle", "ISO, MDY"),
            ("IntervalStyle", "postgres"),
            ("TimeZone", "UTC"),
            ("integer_datetimes", "on"),
            ("standard_conforming_strings", "on"),
            ("application_name", ""),
            ("is_superuser", "on"),
        ] {
            BackendMessage::ParameterStatus {
                name: name.to_string(),
                value: value.to_string(),
            }
            .encode(buf);
        }

        // Send backend key data (PostgreSQL 18: supports variable-length keys)
        BackendMessage::BackendKeyData {
            process_id: self.process_id,
            secret_key: self.secret_key.clone(),
        }
        .encode(buf);

        // Ready for query
        BackendMessage::ReadyForQuery {
            status: TransactionStatus::Idle,
        }
        .encode(buf);

        self.state = ConnectionState::Ready;
    }

    /// Handle password message
    async fn handle_password(&mut self, password: &str, buf: &mut BytesMut) -> ProtocolResult<()> {
        let username = self.username.clone().unwrap_or_default();
        // In a real implementation we would check the password properly
        let valid = self
            .auth_manager
            .verify_password(&username, password, None)
            .await?;

        if valid {
            BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);
            self.finish_authentication(buf);
            Ok(())
        } else {
            self.auth_failed(buf);
            Ok(())
        }
    }

    /// Handle SASL initial response
    async fn handle_sasl_initial_response(
        &mut self,
        mechanism: &str,
        data: Option<bytes::Bytes>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        if mechanism != "SCRAM-SHA-256" {
            self.auth_failed(buf);
            return Ok(());
        }

        let username = self.username.clone().unwrap_or_default();
        let user_store = self.auth_manager.user_store();
        let user_creds = user_store.get_user(&username).await;

        if let Some(creds) = user_creds {
            if let (Some(stored_key), Some(server_key), Some(salt), Some(iterations)) = (
                creds.scram_stored_key,
                creds.scram_server_key,
                creds.scram_salt,
                creds.scram_iterations,
            ) {
                // Parse client-first-message to extract client nonce
                let client_first = if let Some(d) = &data {
                    String::from_utf8_lossy(d).to_string()
                } else {
                    "".to_string()
                };

                // Extract 'r=' part (nonce)
                // Format: n,,n=user,r=nonce
                let nonce = client_first
                    .split(',')
                    .find(|p| p.starts_with("r="))
                    .map(|p| p.trim_start_matches("r=").to_string());

                if let Some(client_nonce) = nonce {
                    let mut scram = ScramAuth::new(
                        username.clone(),
                        client_nonce,
                        salt,
                        iterations,
                        stored_key,
                        server_key,
                    );

                    let server_first = scram.process_client_first(&client_first)?;
                    self.scram_auth = Some(scram);

                    BackendMessage::Authentication(AuthenticationResponse::SASLContinue {
                        data: bytes::Bytes::from(server_first),
                    })
                    .encode(buf);
                } else {
                    self.auth_failed(buf);
                }
            } else {
                self.auth_failed(buf);
            }
        } else {
            self.auth_failed(buf);
        }
        Ok(())
    }

    /// Handle SASL response
    async fn handle_sasl_response(
        &mut self,
        data: bytes::Bytes,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        if let Some(scram) = self.scram_auth.take() {
            let client_final = String::from_utf8_lossy(&data).to_string();
            match scram.process_client_final(&client_final) {
                Ok(server_final) => {
                    BackendMessage::Authentication(AuthenticationResponse::SASLFinal {
                        data: bytes::Bytes::from(server_final),
                    })
                    .encode(buf);

                    BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);
                    self.finish_authentication(buf);
                }
                Err(_) => self.auth_failed(buf),
            }
        } else {
            self.auth_failed(buf);
        }
        Ok(())
    }

    /// Handle simple query
    async fn handle_query(&mut self, query: &str, buf: &mut BytesMut) -> ProtocolResult<()> {
        info!("Query: {} (database: {:?})", query, self.database);

        if query.trim().is_empty() {
            BackendMessage::EmptyQueryResponse.encode(buf);
            BackendMessage::ReadyForQuery {
                status: TransactionStatus::Idle,
            }
            .encode(buf);
            return Ok(());
        }

        // Set the current database context before executing the query
        if let Some(ref db) = self.database {
            self.query_engine.set_current_database(db).await;
        }

        let outcome = self
            .query_engine
            .execute_multiple_queries_in(&mut self.sql_session, query)
            .await;
        match outcome {
            Ok(results) => {
                for result in results {
                    self.send_query_result(&result, buf);
                }
            }
            Err(e) => {
                let (code, message) = Self::error_parts(&e);
                self.send_error_with_code(buf, &code, &message);
            }
        }
        BackendMessage::ReadyForQuery {
            status: self.txn_status(),
        }
        .encode(buf);

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
            .ok_or_else(|| ProtocolError::PostgresError(format!("Portal not found: {portal}")))?;

        let query = self
            .prepared_statements
            .get(statement_name)
            .ok_or_else(|| {
                ProtocolError::PostgresError(format!("Statement not found: {statement_name}"))
            })?;

        // For now, ignore parameters and execute the query
        // TODO: Implement parameter substitution
        let _ = params;

        let query = query.clone();
        match self
            .query_engine
            .execute_multiple_queries_in(&mut self.sql_session, &query)
            .await
        {
            Ok(results) => {
                for result in &results {
                    self.send_query_result_as(result, buf, false);
                }
            }
            Err(e) => {
                let (code, message) = Self::error_parts(&e);
                self.send_error_with_code(buf, &code, &message);
            }
        }

        Ok(())
    }

    /// Handle describe message: report the parameters and result columns of a statement or portal.
    async fn handle_describe(
        &mut self,
        target: super::messages::DescribeTarget,
        name: &str,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Describe: target={:?}, name={}", target, name);

        let sql = match target {
            super::messages::DescribeTarget::Statement => {
                self.prepared_statements.get(name).cloned()
            }
            super::messages::DescribeTarget::Portal => self
                .portals
                .get(name)
                .and_then(|(statement, _)| self.prepared_statements.get(statement).cloned()),
        };
        let columns = sql.as_deref().and_then(|q| self.query_engine.describe(q));
        if matches!(target, super::messages::DescribeTarget::Statement) {
            BackendMessage::ParameterDescription {
                param_types: vec![],
            }
            .encode(buf);
        }
        match columns {
            Some(cols) if !cols.is_empty() => {
                let fields = cols
                    .into_iter()
                    .map(|(name, oid)| FieldDescription {
                        name,
                        table_oid: 0,
                        column_id: 0,
                        type_oid: oid,
                        type_size: match oid {
                            type_oids::BOOL => 1,
                            type_oids::INT4 => 4,
                            _ => -1,
                        },
                        type_modifier: -1,
                        format: 0,
                    })
                    .collect();
                BackendMessage::RowDescription { fields }.encode(buf);
            }
            _ => BackendMessage::NoData.encode(buf),
        }
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
        self.send_query_result_as(result, buf, true);
    }

    /// Encode a result. `describe` is false for the extended protocol's `Execute`, where the
    /// client already received the row description from `Describe`.
    fn send_query_result_as(&self, result: &QueryResult, buf: &mut BytesMut, describe: bool) {
        match result {
            QueryResult::Select { columns, rows } => {
                let mut fields: Vec<FieldDescription> = Vec::with_capacity(columns.len());
                for (i, col) in columns.iter().enumerate() {
                    let mut oid = type_oids::TEXT;
                    let mut size: i16 = -1;
                    if let Some(first_row) = rows.first() {
                        if let Some(Some(val)) = first_row.get(i) {
                            if val.chars().all(|c| c.is_ascii_digit()) {
                                oid = type_oids::INT4;
                                size = 4;
                            } else if val.parse::<f64>().is_ok() {
                                oid = type_oids::FLOAT8;
                                size = 8;
                            }
                        }
                    }
                    fields.push(FieldDescription {
                        name: col.clone(),
                        table_oid: 0,
                        column_id: 0,
                        type_oid: oid,
                        type_size: size,
                        type_modifier: -1,
                        format: 0,
                    });
                }

                if describe {
                    BackendMessage::RowDescription { fields }.encode(buf);
                }

                for row in rows {
                    let values: Vec<Option<bytes::Bytes>> = row
                        .iter()
                        .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                        .collect();
                    BackendMessage::DataRow { values }.encode(buf);
                }

                BackendMessage::CommandComplete {
                    tag: format!("SELECT {}", rows.len()),
                }
                .encode(buf);
            }
            QueryResult::Insert { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("INSERT 0 {count}"),
                }
                .encode(buf);
            }
            QueryResult::Update { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("UPDATE {count}"),
                }
                .encode(buf);
            }
            QueryResult::Delete { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("DELETE {count}"),
                }
                .encode(buf);
            }
            QueryResult::Merge {
                count,
                rows,
                columns,
            } => {
                // If rows are present (RETURNING clause), we need to send RowDescription and DataRow
                if !rows.is_empty() {
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

                    if describe {
                        BackendMessage::RowDescription { fields }.encode(buf);
                    }

                    for row in rows {
                        let values: Vec<Option<bytes::Bytes>> = row
                            .iter()
                            .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                            .collect();
                        BackendMessage::DataRow { values }.encode(buf);
                    }
                }

                BackendMessage::CommandComplete {
                    tag: format!("MERGE {count}"),
                }
                .encode(buf);
            }
            QueryResult::Typed { columns, rows } => {
                let fields = columns
                    .iter()
                    .map(|(name, oid)| FieldDescription {
                        name: name.clone(),
                        table_oid: 0,
                        column_id: 0,
                        type_oid: *oid,
                        type_size: match *oid {
                            type_oids::BOOL => 1,
                            type_oids::INT4 => 4,
                            _ => -1,
                        },
                        type_modifier: -1,
                        format: 0,
                    })
                    .collect();
                if describe {
                    BackendMessage::RowDescription { fields }.encode(buf);
                }
                for row in rows {
                    let values = row
                        .iter()
                        .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                        .collect();
                    BackendMessage::DataRow { values }.encode(buf);
                }
                BackendMessage::CommandComplete {
                    tag: format!("SELECT {}", rows.len()),
                }
                .encode(buf);
            }
            QueryResult::Returning { tag, columns, rows } => {
                let fields = columns
                    .iter()
                    .map(|(name, oid)| FieldDescription {
                        name: name.clone(),
                        table_oid: 0,
                        column_id: 0,
                        type_oid: *oid,
                        type_size: match *oid {
                            type_oids::BOOL => 1,
                            type_oids::INT4 => 4,
                            _ => -1,
                        },
                        type_modifier: -1,
                        format: 0,
                    })
                    .collect();
                if describe {
                    BackendMessage::RowDescription { fields }.encode(buf);
                }
                for row in rows {
                    let values = row
                        .iter()
                        .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                        .collect();
                    BackendMessage::DataRow { values }.encode(buf);
                }
                BackendMessage::CommandComplete { tag: tag.clone() }.encode(buf);
            }
            QueryResult::Command { tag } => match tag.strip_prefix("ERROR: ") {
                Some(message) => self.send_error(buf, message),
                None => BackendMessage::CommandComplete { tag: tag.clone() }.encode(buf),
            },
            QueryResult::Set { .. } => {
                BackendMessage::CommandComplete {
                    tag: "SET".to_string(),
                }
                .encode(buf);
            }
        }
    }

    /// The transaction status reported in `ReadyForQuery`.
    fn txn_status(&self) -> TransactionStatus {
        match self.sql_session.txn_state() {
            None => TransactionStatus::Idle,
            Some(false) => TransactionStatus::InTransaction,
            Some(true) => TransactionStatus::InFailedTransaction,
        }
    }

    /// SQLSTATE and message for an error. The durable engine tags its errors as `[SQLSTATE] message`.
    fn error_parts(e: &ProtocolError) -> (String, String) {
        if let ProtocolError::PostgresError(m) = e {
            if m.len() > 8 && m.starts_with('[') && m.as_bytes()[6] == b']' && m.is_char_boundary(8)
            {
                return (m[1..6].to_string(), m[8..].to_string());
            }
            return ("XX000".to_string(), m.clone());
        }
        ("XX000".to_string(), e.to_string())
    }

    /// Handle SSL request
    async fn handle_ssl_request(&mut self, buf: &mut BytesMut) -> ProtocolResult<()> {
        // Reject SSL request - send 'N' to indicate SSL not supported
        buf.put_u8(b'N');
        Ok(())
    }

    /// A fatal error ends the connection: it is sent, then the connection is closed.
    fn send_fatal(&mut self, buf: &mut BytesMut, code: &str, message: &str) {
        let mut fields = HashMap::new();
        fields.insert(b'S', "FATAL".to_string());
        fields.insert(b'V', "FATAL".to_string());
        fields.insert(b'C', code.to_string());
        fields.insert(b'M', message.to_string());
        BackendMessage::ErrorResponse { fields }.encode(buf);
        self.close_after = true;
    }

    /// Wrong password and unknown user look the same to the client.
    fn auth_failed(&mut self, buf: &mut BytesMut) {
        let user = self.username.clone().unwrap_or_default();
        self.send_fatal(
            buf,
            "28P01",
            &format!("password authentication failed for user \"{user}\""),
        );
    }

    /// Send error response
    fn send_error(&self, buf: &mut BytesMut, message: &str) {
        self.send_error_with_code(buf, "XX000", message);
    }

    fn send_error_with_code(&self, buf: &mut BytesMut, code: &str, message: &str) {
        let mut fields = HashMap::new();
        fields.insert(b'S', "ERROR".to_string());
        fields.insert(b'C', code.to_string());
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
use rand::RngExt;
impl PostgresWireProtocol {
    /// Generate a random cancel key
    /// PostgreSQL 18 (protocol 3.2): Supports 4-256 bytes
    /// Default: 4 bytes for backward compatibility with protocol 3.0 clients
    fn random_secret_key() -> Vec<u8> {
        let mut key = vec![0u8; 4]; // 4 bytes for compatibility
        rand::rng().fill(&mut key[..]);
        key
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn startup_reports_postgresql_18_server_version() {
        let mut protocol = PostgresWireProtocol::new();
        let mut buf = BytesMut::new();
        protocol.finish_authentication(&mut buf);
        let expected = format!(
            "server_version\0{}\0",
            super::super::session_compat::SERVER_VERSION
        );
        assert!(
            buf.windows(expected.len())
                .any(|w| w == expected.as_bytes()),
            "ParameterStatus for server_version missing from startup: {:?}",
            String::from_utf8_lossy(&buf)
        );
        assert!(
            buf.windows(b"server_version\x0018.0\0".len())
                .any(|w| w == b"server_version\x0018.0\0"),
            "startup must advertise 18.0"
        );
    }
}
