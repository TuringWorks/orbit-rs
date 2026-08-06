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
    /// Parameter type OIDs supplied by `Parse`, per prepared statement.
    ///
    /// Needed to answer `Describe(Statement)`, which must report one type per
    /// parameter before the row description.
    statement_param_types: HashMap<String, Vec<i32>>,
    portals: HashMap<String, (String, Vec<Option<bytes::Bytes>>)>,
    auth_manager: AuthManager,
    scram_auth: Option<ScramAuth>,
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
            statement_param_types: HashMap::new(),
            portals: HashMap::new(),
            auth_manager,
            scram_auth: None,
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
            statement_param_types: HashMap::new(),
            portals: HashMap::new(),
            auth_manager,
            scram_auth: None,
        }
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
                self.handle_describe(target, &name, buf).await?;
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

        // Auto-register user for SCRAM testing if needed
        if matches!(self.auth_manager.auth_method(), AuthMethod::ScramSha256) {
            if let Some(user) = &self.username {
                if self
                    .auth_manager
                    .user_store()
                    .get_user(user)
                    .await
                    .is_none()
                {
                    // Auto-create user with password same as username for testing
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
            self.send_error(buf, "Password authentication failed");
            Ok(()) // Don't terminate, just error? Usually terminate on auth fail.
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
            self.send_error(buf, "Unsupported SASL mechanism");
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
                    self.send_error(buf, "Invalid SCRAM client-first-message: missing nonce");
                }
            } else {
                self.send_error(buf, "User not configured for SCRAM");
            }
        } else {
            self.send_error(buf, "Authentication failed");
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
                Err(e) => {
                    self.send_error(buf, &format!("SCRAM authentication failed: {}", e));
                }
            }
        } else {
            self.send_error(buf, "Protocol error: SASL response without initial step");
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

        match self.query_engine.execute_multiple_queries(query).await {
            Ok(results) => {
                for result in results {
                    self.send_query_result(&result, buf);
                }
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
        param_types: Vec<i32>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Parse: name={}, query={}", statement_name, query);

        self.prepared_statements
            .insert(statement_name.to_string(), query.to_string());

        // A client may send fewer type OIDs than the statement has placeholders,
        // leaving the rest to be inferred. This engine treats every parameter as
        // text, so unspecified entries are filled in as such rather than left
        // out — `Describe` must report one type per parameter.
        let placeholders = Self::count_placeholders(query);
        let mut param_types = param_types;
        param_types.resize(
            param_types.len().max(placeholders),
            super::messages::type_oids::TEXT,
        );
        // OID 0 means "unspecified"; answer with the type actually used.
        for oid in &mut param_types {
            if *oid == 0 {
                *oid = super::messages::type_oids::TEXT;
            }
        }
        self.statement_param_types
            .insert(statement_name.to_string(), param_types);

        BackendMessage::ParseComplete.encode(buf);
        Ok(())
    }

    /// Highest `$n` placeholder appearing in `query`.
    ///
    /// Counts distinct positions rather than occurrences: `$1 AND $1` is one
    /// parameter. Text inside single-quoted literals is skipped so a `$1` in a
    /// string is not mistaken for a placeholder.
    fn count_placeholders(query: &str) -> usize {
        let bytes = query.as_bytes();
        let mut highest = 0usize;
        let mut index = 0usize;
        let mut in_literal = false;

        while index < bytes.len() {
            match bytes[index] {
                b'\'' => {
                    in_literal = !in_literal;
                    index += 1;
                }
                b'$' if !in_literal => {
                    let start = index + 1;
                    let end = start
                        + bytes[start..]
                            .iter()
                            .take_while(|b| b.is_ascii_digit())
                            .count();
                    if end > start {
                        if let Ok(n) = query[start..end].parse::<usize>() {
                            highest = highest.max(n);
                        }
                    }
                    index = end.max(index + 1);
                }
                _ => index += 1,
            }
        }

        highest
    }

    /// Handle bind message
    fn handle_bind(
        &mut self,
        portal: &str,
        statement: &str,
        param_formats: Vec<i16>,
        params: Vec<Option<bytes::Bytes>>,
        _result_formats: Vec<i16>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Bind: portal={}, statement={}", portal, statement);

        // Format codes are per-parameter, or a single code covering all of them,
        // or empty meaning all text.
        let is_binary = |index: usize| match param_formats.len() {
            0 => false,
            1 => param_formats[0] == 1,
            _ => param_formats.get(index).is_some_and(|f| *f == 1),
        };

        let declared = self
            .statement_param_types
            .get(statement)
            .cloned()
            .unwrap_or_default();

        // Parameters are stored as text, because that is what substitution into
        // the statement needs. Binary values are decoded here, where the
        // declared type says how to read the bytes.
        let mut decoded = Vec::with_capacity(params.len());
        for (index, value) in params.into_iter().enumerate() {
            let Some(raw) = value else {
                decoded.push(None);
                continue;
            };

            if !is_binary(index) {
                decoded.push(Some(raw));
                continue;
            }

            let type_oid = declared.get(index).copied().unwrap_or(type_oids::TEXT);
            match Self::decode_binary_parameter(&raw, type_oid) {
                Ok(text) => decoded.push(Some(bytes::Bytes::from(text))),
                Err(e) => {
                    self.send_error(buf, &format!("parameter ${}: {e}", index + 1));
                    return Ok(());
                }
            }
        }

        self.portals
            .insert(portal.to_string(), (statement.to_string(), decoded));

        BackendMessage::BindComplete.encode(buf);
        Ok(())
    }

    /// Render a binary-format parameter as the text the engine works with.
    ///
    /// PostgreSQL's binary encodings are big-endian and fixed width for the
    /// scalar types. A type this does not know is rejected rather than guessed
    /// at, because misreading the bytes would substitute a wrong value into the
    /// statement without any error.
    fn decode_binary_parameter(raw: &[u8], type_oid: i32) -> Result<String, String> {
        fn fixed<const N: usize>(raw: &[u8], type_name: &str) -> Result<[u8; N], String> {
            raw.try_into()
                .map_err(|_| format!("expected {N} bytes for {type_name}, got {}", raw.len()))
        }

        match type_oid {
            type_oids::BOOL => match raw {
                [0] => Ok("false".to_string()),
                [1] => Ok("true".to_string()),
                _ => Err("expected a single 0 or 1 byte for bool".to_string()),
            },
            type_oids::INT2 => Ok(i16::from_be_bytes(fixed::<2>(raw, "int2")?).to_string()),
            type_oids::INT4 => Ok(i32::from_be_bytes(fixed::<4>(raw, "int4")?).to_string()),
            type_oids::INT8 => Ok(i64::from_be_bytes(fixed::<8>(raw, "int8")?).to_string()),
            type_oids::FLOAT4 => Ok(f32::from_be_bytes(fixed::<4>(raw, "float4")?).to_string()),
            type_oids::FLOAT8 => Ok(f64::from_be_bytes(fixed::<8>(raw, "float8")?).to_string()),
            // These are already UTF-8 on the wire in both formats.
            type_oids::TEXT | type_oids::VARCHAR | type_oids::JSON | type_oids::UUID => {
                std::str::from_utf8(raw)
                    .map(str::to_string)
                    .map_err(|_| "value is not valid UTF-8".to_string())
            }
            other => Err(format!(
                "binary format is not supported for type OID {other}; send this parameter in \
                 text format"
            )),
        }
    }

    /// Substitute bound parameters into `$n` placeholders.
    ///
    /// Values are quoted as SQL literals, so a parameter containing a quote or a
    /// semicolon becomes data rather than syntax. Placeholders inside string
    /// literals are left alone.
    ///
    /// # Errors
    /// Returns an error when the statement references a parameter that was not
    /// bound; executing with a literal `$n` left in place would silently return
    /// the wrong rows.
    fn bind_parameters(
        query: &str,
        params: &[Option<bytes::Bytes>],
    ) -> ProtocolResult<String> {
        let bytes = query.as_bytes();
        let mut out = String::with_capacity(query.len());
        let mut index = 0usize;
        let mut in_literal = false;

        while index < bytes.len() {
            let ch = bytes[index];

            if ch == b'\'' {
                in_literal = !in_literal;
                out.push('\'');
                index += 1;
                continue;
            }

            if ch != b'$' || in_literal {
                out.push(bytes[index] as char);
                index += 1;
                continue;
            }

            let start = index + 1;
            let end = start
                + bytes[start..]
                    .iter()
                    .take_while(|b| b.is_ascii_digit())
                    .count();

            if end == start {
                out.push('$');
                index += 1;
                continue;
            }

            let position: usize = query[start..end].parse().map_err(|_| {
                ProtocolError::PostgresError(format!(
                    "invalid parameter placeholder ${}",
                    &query[start..end]
                ))
            })?;

            let value = params.get(position.wrapping_sub(1)).ok_or_else(|| {
                ProtocolError::PostgresError(format!(
                    "statement references ${position} but only {} parameter(s) were bound",
                    params.len()
                ))
            })?;

            match value {
                None => out.push_str("NULL"),
                Some(raw) => {
                    let text = std::str::from_utf8(raw).map_err(|_| {
                        ProtocolError::PostgresError(format!(
                            "parameter ${position} is not valid UTF-8 text"
                        ))
                    })?;
                    out.push('\'');
                    // Doubling is how a single quote is escaped in a SQL literal.
                    out.push_str(&text.replace('\'', "''"));
                    out.push('\'');
                }
            }

            index = end;
        }

        Ok(out)
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

        let bound = match Self::bind_parameters(query, params) {
            Ok(bound) => bound,
            Err(e) => {
                self.send_error(buf, &e.to_string());
                return Ok(());
            }
        };

        match self.query_engine.execute_query(&bound).await {
            Ok(result) => {
                // Extended protocol: the row description was already sent in
                // response to Describe, and repeating it here is a protocol
                // violation. Only the rows and the command tag belong on Execute.
                self.send_query_result_without_description(&result, buf);
            }
            Err(e) => {
                self.send_error(buf, &e.to_string());
            }
        }

        Ok(())
    }

    /// Handle describe message
    /// Handle a Describe message.
    ///
    /// The extended query protocol requires:
    ///
    /// * `Describe(Statement)` → `ParameterDescription`, then `RowDescription`
    ///   or `NoData`;
    /// * `Describe(Portal)` → `RowDescription` or `NoData`.
    ///
    /// Sending only `NoData` — as this did — leaves every conforming driver
    /// reading a `ParameterDescription` it never receives, which is why
    /// `prepare()` failed with "unexpected message from server".
    async fn handle_describe(
        &mut self,
        target: super::messages::DescribeTarget,
        name: &str,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        use super::messages::DescribeTarget;

        debug!("Describe: target={:?}, name={}", target, name);

        let sql = match target {
            DescribeTarget::Statement => self.prepared_statements.get(name).cloned(),
            DescribeTarget::Portal => self
                .portals
                .get(name)
                .and_then(|(statement, _)| self.prepared_statements.get(statement).cloned()),
        };

        let Some(sql) = sql else {
            self.send_error(
                buf,
                &match target {
                    DescribeTarget::Statement => format!("Statement not found: {name}"),
                    DescribeTarget::Portal => format!("Portal not found: {name}"),
                },
            );
            return Ok(());
        };

        // Parameter types belong only to a statement description; a portal's
        // parameters are already bound.
        if matches!(target, DescribeTarget::Statement) {
            // A type the client declared in Parse is authoritative — it says how
            // that client will serialise the value. Where it declared nothing,
            // the type is inferred from how the parameter is used, so callers
            // are not forced to stringify every value.
            let declared = self
                .statement_param_types
                .get(name)
                .cloned()
                .unwrap_or_default();
            let inferred = self.query_engine.describe_parameters(&sql).await?;

            let param_types: Vec<i32> = (0..declared.len().max(inferred.len()))
                .map(|i| match declared.get(i).copied() {
                    Some(oid) if oid != type_oids::TEXT => oid,
                    _ => inferred.get(i).copied().unwrap_or(type_oids::TEXT),
                })
                .collect();

            // Remember what was advertised: Bind decodes binary values with it.
            self.statement_param_types
                .insert(name.to_string(), param_types.clone());

            BackendMessage::ParameterDescription { param_types }.encode(buf);
        }

        let description = self.query_engine.describe_statement(&sql).await?;

        if description.returns_rows() {
            let fields = description
                .columns
                .iter()
                .map(|column| FieldDescription {
                    name: column.name.clone(),
                    table_oid: 0,
                    column_id: 0,
                    type_oid: column.type_oid,
                    type_size: -1,
                    type_modifier: -1,
                    format: 0,
                })
                .collect();
            BackendMessage::RowDescription { fields }.encode(buf);
        } else {
            BackendMessage::NoData.encode(buf);
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
        // Simple protocol: the row description precedes the rows.
        if let QueryResult::Select { columns, .. } | QueryResult::Merge { columns, .. } = result {
            // Every value this engine holds is text, and it is sent in text
            // format. Types were previously guessed from the characters of the
            // first row, which labelled a column of '01234' as int4 and made
            // conforming clients decode it as 1234 — losing the leading zero.
            // Advertising text describes what is actually on the wire.
            let fields: Vec<FieldDescription> = columns
                .iter()
                .map(|col| FieldDescription {
                    name: col.clone(),
                    table_oid: 0,
                    column_id: 0,
                    type_oid: type_oids::TEXT,
                    type_size: -1,
                    type_modifier: -1,
                    format: 0,
                })
                .collect();
            BackendMessage::RowDescription { fields }.encode(buf);
        }

        self.send_query_result_without_description(result, buf);
    }

    /// Send rows and the command tag, without a row description.
    ///
    /// This is what `Execute` must send: in the extended query protocol the row
    /// description belongs to `Describe`, and repeating it here makes
    /// conforming clients fail.
    fn send_query_result_without_description(&self, result: &QueryResult, buf: &mut BytesMut) {
        match result {
            QueryResult::Select { columns: _, rows } => {
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

                    BackendMessage::RowDescription { fields }.encode(buf);

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
            QueryResult::Set { .. } => {
                BackendMessage::CommandComplete {
                    tag: "SET".to_string(),
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
mod extended_protocol_tests {
    use super::*;

    fn param(text: &str) -> Option<bytes::Bytes> {
        Some(bytes::Bytes::from(text.to_string()))
    }

    #[test]
    fn placeholders_are_counted_by_highest_position_not_occurrences() {
        assert_eq!(PostgresWireProtocol::count_placeholders("SELECT 1"), 0);
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT * FROM t WHERE a = $1 AND b = $1"),
            1
        );
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT * FROM t WHERE a = $2 AND b = $1"),
            2
        );
    }

    #[test]
    fn a_dollar_inside_a_string_literal_is_not_a_placeholder() {
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT '$1' FROM t"),
            0
        );
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT '$5' FROM t WHERE a = $1"),
            1
        );
    }

    #[test]
    fn parameters_are_substituted_in_position_order() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE a = $1 AND b = $2",
            &[param("one"), param("two")],
        )
        .expect("both parameters are bound");
        assert_eq!(bound, "SELECT * FROM t WHERE a = 'one' AND b = 'two'");
    }

    #[test]
    fn a_repeated_placeholder_uses_the_same_value_each_time() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT $1, $1", &[param("x")]).expect("bound");
        assert_eq!(bound, "SELECT 'x', 'x'");
    }

    #[test]
    fn a_null_parameter_becomes_sql_null_not_an_empty_string() {
        let bound = PostgresWireProtocol::bind_parameters("SELECT $1", &[None]).expect("bound");
        assert_eq!(bound, "SELECT NULL");
    }

    /// A parameter is data. Quotes and statement terminators inside one must not
    /// become syntax.
    #[test]
    fn quotes_in_a_parameter_are_escaped_rather_than_ending_the_literal() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE name = $1",
            &[param("O'Brien")],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT * FROM t WHERE name = 'O''Brien'");
    }

    #[test]
    fn an_injection_attempt_in_a_parameter_stays_inside_the_literal() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE name = $1",
            &[param("x'; DROP TABLE users; --")],
        )
        .expect("bound");
        assert_eq!(
            bound,
            "SELECT * FROM t WHERE name = 'x''; DROP TABLE users; --'"
        );
        // Exactly one literal: an odd number of quotes would mean the value
        // escaped into statement syntax.
        assert_eq!(bound.matches('\'').count() % 2, 0);
    }

    #[test]
    fn a_placeholder_inside_a_literal_is_left_untouched() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT '$1', $1", &[param("v")]).expect("bound");
        assert_eq!(bound, "SELECT '$1', 'v'");
    }

    /// Running with a literal `$1` still in the statement would quietly return
    /// the wrong rows, so an unbound reference must fail loudly.
    #[test]
    fn referencing_an_unbound_parameter_is_an_error() {
        let error = PostgresWireProtocol::bind_parameters("SELECT $2", &[param("only-one")])
            .expect_err("only one parameter was bound");
        assert!(
            error.to_string().contains("$2"),
            "the message should name the missing parameter: {error}"
        );
    }

    #[test]
    fn a_statement_without_placeholders_is_unchanged() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT 1 FROM t", &[]).expect("bound");
        assert_eq!(bound, "SELECT 1 FROM t");
    }

    #[test]
    fn binary_scalars_decode_to_their_text_form() {
        use super::super::messages::type_oids;

        let decode = PostgresWireProtocol::decode_binary_parameter;
        assert_eq!(decode(&2i32.to_be_bytes(), type_oids::INT4).unwrap(), "2");
        assert_eq!(decode(&(-7i64).to_be_bytes(), type_oids::INT8).unwrap(), "-7");
        assert_eq!(decode(&300i16.to_be_bytes(), type_oids::INT2).unwrap(), "300");
        assert_eq!(decode(&[1], type_oids::BOOL).unwrap(), "true");
        assert_eq!(decode(&[0], type_oids::BOOL).unwrap(), "false");
        assert_eq!(decode(b"hello", type_oids::TEXT).unwrap(), "hello");
        assert_eq!(
            decode(&1.5f64.to_be_bytes(), type_oids::FLOAT8).unwrap(),
            "1.5"
        );
    }

    /// Misreading the bytes would substitute a wrong value with no error, so a
    /// wrong-width payload and an unknown type must both be refused.
    #[test]
    fn malformed_or_unknown_binary_parameters_are_refused() {
        use super::super::messages::type_oids;

        let decode = PostgresWireProtocol::decode_binary_parameter;
        assert!(decode(&[1, 2], type_oids::INT4).is_err(), "wrong width");
        assert!(decode(&[2], type_oids::BOOL).is_err(), "not 0 or 1");
        assert!(decode(&[0; 8], type_oids::BYTEA).is_err(), "unsupported type");
    }

    #[test]
    fn describing_a_parameterised_statement_neutralises_placeholders() {
        use crate::protocols::postgres_wire::query_engine::QueryEngine;

        assert_eq!(
            QueryEngine::placeholders_as_null_for_test("SELECT a FROM t WHERE b = $1 AND c = $22"),
            "SELECT a FROM t WHERE b = NULL AND c = NULL"
        );
        assert_eq!(
            QueryEngine::placeholders_as_null_for_test("SELECT '$1' FROM t"),
            "SELECT '$1' FROM t"
        );
    }

    #[test]
    fn comparisons_pair_a_placeholder_with_the_column_beside_it() {
        use crate::protocols::postgres_wire::query_engine::QueryEngine;

        let pairs = QueryEngine::placeholder_comparisons_for_test(
            "SELECT * FROM t WHERE id = $1 AND score >= $2 AND name LIKE $3",
        );
        assert_eq!(
            pairs,
            vec![
                (1, "id".to_string()),
                (2, "score".to_string()),
                (3, "name".to_string())
            ]
        );
    }

    #[test]
    fn a_placeholder_that_is_not_compared_to_a_column_is_left_alone() {
        use crate::protocols::postgres_wire::query_engine::QueryEngine;

        assert!(QueryEngine::placeholder_comparisons_for_test("SELECT $1").is_empty());
    }
}
