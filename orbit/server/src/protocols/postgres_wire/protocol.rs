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
use super::notifications::{NotificationHub, SessionNotifications};
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
    /// Result format codes requested by `Bind`, per portal.
    ///
    /// A client that asked for binary results cannot read text ones: it decodes
    /// by width and fails with "failed to fill whole buffer". Ignoring these
    /// codes only appeared to work while every column was advertised as text.
    portal_result_formats: HashMap<String, Vec<i16>>,
    /// Column type OIDs most recently described for a statement, so `Execute`
    /// knows how to encode each value.
    statement_columns: HashMap<String, Vec<i32>>,
    /// Whether the COPY being started came from a simple query.
    copy_in_is_simple: bool,
    /// The `COPY ... FROM STDIN` currently in progress, if any.
    ///
    /// While set, the session is in copy-in mode and the client is streaming
    /// CopyData messages rather than ordinary queries.
    copy_in: Option<CopyInState>,
    /// Shared LISTEN/NOTIFY registry, and this session's end of it.
    notifications: Arc<NotificationHub>,
    session_notifications: SessionNotifications,
    /// Rows of a partially fetched portal, so a second `Execute` resumes rather
    /// than re-running the statement.
    portal_rows: HashMap<String, PortalRows>,
    auth_manager: AuthManager,
    scram_auth: Option<ScramAuth>,
    /// Writes issued inside the current transaction block.
    writes_in_transaction: u64,
    /// Contents of each table as it stood when the transaction block first
    /// wrote to it.
    ///
    /// Storage applies writes as they run, so undoing them means putting the
    /// table back. A snapshot is taken once per table per block, before its
    /// first write, and discarded on COMMIT.
    transaction_snapshots: HashMap<String, Vec<super::persistent_storage::TableRow>>,
    /// Whether this session is inside a transaction block, and whether that
    /// block has already failed.
    ///
    /// Reported in every `ReadyForQuery`. It used to be hardcoded to `Idle`,
    /// which told drivers no transaction was ever open — so a driver could not
    /// tell a committed statement from one queued in an aborted block.
    transaction: TransactionState,
}

/// An in-progress `COPY ... FROM STDIN`.
struct CopyInState {
    /// Whether the copy was started by a simple query, which owes the client a
    /// ReadyForQuery when the stream ends.
    simple_protocol: bool,
    table: String,
    /// Column names the data is being loaded into, in order.
    columns: Vec<String>,
    /// Whether each column takes an unquoted literal, by position.
    ///
    /// A COPY field is text on the wire but must be written into the statement
    /// the way its column expects: quoting `9` for an integer column makes the
    /// insert fail, and the failure arrives mid-stream where the client is not
    /// expecting a message at all.
    numeric_columns: Vec<bool>,
    /// Bytes received but not yet terminated by a newline.
    ///
    /// CopyData messages are chunks of a byte stream, not rows: a row can be
    /// split across two of them.
    partial: Vec<u8>,
    rows: u64,
    /// First row failure, reported when the stream ends.
    failure: Option<String>,
}

/// A portal's result set and how much of it has been delivered.
struct PortalRows {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
    sent: usize,
}

/// Transaction state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionState {
    /// No transaction block open; each statement commits on its own.
    Idle,
    /// Inside a transaction block that is still good.
    Open,
    /// Inside a transaction block that has hit an error. Every statement is
    /// rejected until ROLLBACK.
    Failed,
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
            portal_result_formats: HashMap::new(),
            statement_columns: HashMap::new(),
            portal_rows: HashMap::new(),
            auth_manager,
            scram_auth: None,
            transaction: TransactionState::Idle,
            writes_in_transaction: 0,
            transaction_snapshots: HashMap::new(),
            copy_in: None,
            copy_in_is_simple: false,
            notifications: NotificationHub::new(),
            session_notifications: SessionNotifications::new(),
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
            portal_result_formats: HashMap::new(),
            statement_columns: HashMap::new(),
            portal_rows: HashMap::new(),
            auth_manager,
            scram_auth: None,
            transaction: TransactionState::Idle,
            writes_in_transaction: 0,
            transaction_snapshots: HashMap::new(),
            copy_in: None,
            copy_in_is_simple: false,
            notifications: NotificationHub::new(),
            session_notifications: SessionNotifications::new(),
        }
    }

    /// Share a notification registry with the other sessions on this server.
    ///
    /// Without this each connection gets its own hub, so `NOTIFY` on one
    /// connection can never reach a `LISTEN` on another — which is the only
    /// thing the feature is for.
    #[must_use]
    pub fn with_notification_hub(mut self, hub: Arc<NotificationHub>) -> Self {
        self.notifications = hub;
        self
    }

    /// Transaction status to report in `ReadyForQuery`.
    fn transaction_status(&self) -> TransactionStatus {
        match self.transaction {
            TransactionState::Idle => TransactionStatus::Idle,
            TransactionState::Open => TransactionStatus::InTransaction,
            TransactionState::Failed => TransactionStatus::InFailedTransaction,
        }
    }

    /// Update the transaction state from a statement about to run.
    ///
    /// Recognises the transaction-control statements themselves; everything
    /// else leaves the state alone.
    fn note_statement(&mut self, sql: &str) {
        let head: String = sql
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_uppercase();

        match head.as_str() {
            "BEGIN" | "START" => {
                self.transaction = TransactionState::Open;
                self.writes_in_transaction = 0;
            }
            "COMMIT" | "END" | "ROLLBACK" | "ABORT" => {
                self.transaction = TransactionState::Idle;
                self.writes_in_transaction = 0;
            }
            // Writes are counted so a later ROLLBACK can report what it cannot
            // undo.
            "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "COPY" | "TRUNCATE"
                if self.transaction != TransactionState::Idle =>
            {
                self.writes_in_transaction += 1;
            }
            _ => {}
        }
    }

    /// Snapshot the table a statement is about to write, if a transaction
    /// block is open and this is its first write to that table.
    ///
    /// Taken before the statement runs, because afterwards the previous
    /// contents are gone.
    async fn snapshot_before_write(&mut self, sql: &str) {
        if self.transaction == TransactionState::Idle {
            return;
        }
        let Some(table) = QueryEngine::write_target_table(sql) else {
            return;
        };
        if self.transaction_snapshots.contains_key(&table) {
            return;
        }

        match self.query_engine.snapshot_table(&table).await {
            Ok(Some(rows)) => {
                self.transaction_snapshots.insert(table, rows);
            }
            Ok(None) => {}
            // Recorded but not fatal: the statement still runs, and the
            // rollback will report that it could not restore this table.
            Err(e) => tracing::warn!("could not snapshot '{table}' for rollback: {e}"),
        }
    }

    /// Put every snapshotted table back, undoing the block's writes.
    ///
    /// Returns the tables that could not be restored.
    async fn restore_snapshots(&mut self) -> Vec<String> {
        let snapshots = std::mem::take(&mut self.transaction_snapshots);
        let mut failed = Vec::new();

        for (table, rows) in snapshots {
            if let Err(e) = self.query_engine.restore_table(&table, rows).await {
                tracing::error!("rollback could not restore '{table}': {e}");
                failed.push(table);
            }
        }
        failed
    }

    /// Apply a transaction-control statement's effect on the undo snapshots.
    ///
    /// `ROLLBACK` puts every table the block wrote back to how it stood before
    /// its first write, which is what makes the block atomic. `COMMIT` simply
    /// drops the snapshots.
    ///
    /// This gives atomicity, not isolation: writes are visible to other
    /// sessions as they happen, and a concurrent writer's changes to the same
    /// table would be reverted along with this block's.
    async fn apply_transaction_control(&mut self, query: &str, buf: &mut BytesMut) {
        let head: String = query
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        match head.as_str() {
            "ROLLBACK" | "ABORT" => {
                let failed = self.restore_snapshots().await;
                if failed.is_empty() {
                    return;
                }
                // Partly undone: saying so beats reporting a clean rollback.
                let mut fields = HashMap::new();
                fields.insert(b'S', "WARNING".to_string());
                fields.insert(b'C', "25000".to_string());
                fields.insert(
                    b'M',
                    format!(
                        "ROLLBACK could not restore {}: those changes remain applied",
                        failed.join(", ")
                    ),
                );
                BackendMessage::NoticeResponse { fields }.encode(buf);
            }
            // The block's writes stand; nothing to put back.
            "COMMIT" | "END" => self.transaction_snapshots.clear(),
            _ => {}
        }
    }

    /// Record that a statement failed.
    ///
    /// Inside a transaction block this poisons it: PostgreSQL rejects every
    /// later statement until the block is rolled back.
    fn note_failure(&mut self) {
        if self.transaction == TransactionState::Open {
            self.transaction = TransactionState::Failed;
        }
    }

    /// Handle a generic connection stream (TCP or TLS)
    pub async fn handle_connection<S>(&mut self, stream: S) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        self.handle_connection_with_buffer(stream, BytesMut::new())
            .await
    }

    /// Handle a connection whose first bytes have already been read.
    ///
    /// TLS negotiation happens before this point and has to read the client's
    /// first 8 bytes to know what was asked for. When those turn out to belong
    /// to the startup message instead, they are passed back in here so the
    /// message can be parsed whole.
    pub async fn handle_connection_with_buffer<S>(
        &mut self,
        mut stream: S,
        prefix: BytesMut,
    ) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        info!("New PostgreSQL client connection");

        let mut read_buf = BytesMut::with_capacity(8192);
        read_buf.extend_from_slice(&prefix);
        let mut write_buf = BytesMut::with_capacity(8192);

        // The prefix may already hold a complete startup message.
        if !read_buf.is_empty() {
            match self
                .process_pending_messages(&mut stream, &mut read_buf, &mut write_buf)
                .await?
            {
                ConnectionLoopResult::Continue => {}
                ConnectionLoopResult::ClientDisconnected
                | ConnectionLoopResult::ClientTerminated => return Ok(()),
            }
        }

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
                    self.notifications
                        .disconnect(self.session_notifications.id)
                        .await;
                    return Ok(());
                }
            }
        }

        // A session that has gone must not stay in the registry.
        self.notifications
            .disconnect(self.session_notifications.id)
            .await;

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
        // Wait for either the client to say something or a notification to
        // arrive. Reading alone would hold a notification until the client
        // happened to send a message, which for an idle listener is never —
        // and an idle listener is the whole point of LISTEN.
        let n = loop {
            tokio::select! {
                read = stream.read_buf(read_buf) => break read?,
                Some(notification) = self.session_notifications.receiver.recv() => {
                    BackendMessage::NotificationResponse {
                        process_id: notification.process_id,
                        channel: notification.channel,
                        payload: notification.payload,
                    }
                    .encode(write_buf);
                    self.flush_write_buffer(stream, write_buf).await?;
                }
            }
        };

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
                    // The protocol requires a ReadyForQuery after an error
                    // before the client may send anything else. Without it the
                    // client waits for a message that never comes and the
                    // session appears to have died — one bad statement took
                    // the whole connection down.
                    BackendMessage::ReadyForQuery {
                        status: self.transaction_status(),
                    }
                    .encode(write_buf);
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
                // While a copy-in stream is open the backend ignores Sync: the
                // client sends one straight after Execute and is not expecting
                // a reply until CopyDone. Answering it here put a
                // ReadyForQuery into the middle of the stream, which is the
                // "unexpected message from server" the client then reported.
                if self.copy_in.is_none() {
                    self.deliver_pending_notifications(buf);
                    BackendMessage::ReadyForQuery {
                        status: self.transaction_status(),
                    }
                    .encode(buf);
                }
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
            FrontendMessage::CopyData { data } => {
                self.handle_copy_data(&data, buf).await?;
            }
            FrontendMessage::CopyDone => {
                self.handle_copy_done(buf).await?;
            }
            FrontendMessage::CopyFail { message } => {
                // The client is abandoning the load; nothing already applied is
                // undone, matching a non-transactional COPY.
                let simple = self.copy_in.take().is_some_and(|s| s.simple_protocol);
                self.send_error(buf, &format!("COPY from stdin failed: {message}"));
                self.finish_copy_statement(simple, buf);
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

        self.copy_in_is_simple = true;
        if self.handle_copy_statement(query, buf, true).await.is_some() {
            return Ok(());
        }

        self.snapshot_before_write(query).await;

        if let Some(tag) = self.handle_notification_statement(query).await {
            BackendMessage::CommandComplete { tag }.encode(buf);
            self.deliver_pending_notifications(buf);
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        if query.trim().is_empty() {
            BackendMessage::EmptyQueryResponse.encode(buf);
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
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
                self.apply_transaction_control(query, buf).await;
                self.note_statement(query);
                self.deliver_pending_notifications(buf);
                BackendMessage::ReadyForQuery {
                    status: self.transaction_status(),
                }
                .encode(buf);
            }
            Err(e) => {
                self.send_error(buf, &e.to_string());
                self.note_failure();
                BackendMessage::ReadyForQuery {
                    status: self.transaction_status(),
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

    /// Whether a value of this type is written into SQL without quotes.
    fn is_unquoted_literal_type(type_oid: i32) -> bool {
        matches!(
            type_oid,
            type_oids::INT2
                | type_oids::INT4
                | type_oids::INT8
                | type_oids::FLOAT4
                | type_oids::FLOAT8
                | type_oids::BOOL
        )
    }

    /// Whether `text` is safe to splice in unquoted.
    ///
    /// Belt and braces: the type says the value should be bare, but the bytes
    /// come from the client. Anything that is not plainly a number or a boolean
    /// is quoted instead, so a hostile value cannot become syntax.
    fn is_safe_bare_literal(text: &str) -> bool {
        if matches!(
            text.to_ascii_lowercase().as_str(),
            "true" | "false" | "t" | "f"
        ) {
            return true;
        }

        !text.is_empty()
            && text.parse::<f64>().is_ok()
            && text
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
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
        result_formats: Vec<i16>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Bind: portal={}, statement={}", portal, statement);
        self.portal_result_formats
            .insert(portal.to_string(), result_formats);

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

        // Re-binding a portal restarts it, so any partially delivered result
        // from a previous execution is discarded.
        self.portal_rows.remove(portal);
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
        param_types: &[i32],
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

                    let type_oid = param_types
                        .get(position - 1)
                        .copied()
                        .unwrap_or(type_oids::TEXT);

                    if Self::is_unquoted_literal_type(type_oid)
                        && Self::is_safe_bare_literal(text)
                    {
                        // A numeric or boolean parameter has to be emitted
                        // bare: quoting it turns `id = $1` into `id = '1'`,
                        // which compares an integer column against a string
                        // and silently matches nothing.
                        out.push_str(text);
                    } else {
                        out.push('\'');
                        // Doubling is how a single quote is escaped in a SQL literal.
                        out.push_str(&text.replace('\'', "''"));
                        out.push('\'');
                    }
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
        max_rows: i32,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Execute: portal={}, max_rows={}", portal, max_rows);

        // A portal already drained by a previous Execute returns nothing more.
        if let Some(remaining) = self.portal_rows.get(portal) {
            let already_sent = remaining.sent;
            let rows = remaining.rows.clone();
            let columns = remaining.columns.clone();
            return Ok(self.send_portal_page(portal, &columns, &rows, already_sent, max_rows, buf));
        }

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

        let param_types = self
            .statement_param_types
            .get(statement_name)
            .cloned()
            .unwrap_or_default();

        let bound = match Self::bind_parameters(query, params, &param_types) {
            Ok(bound) => bound,
            Err(e) => {
                self.send_error(buf, &e.to_string());
                return Ok(());
            }
        };

        self.copy_in_is_simple = false;
        if self.handle_copy_statement(&bound, buf, false).await.is_some() {
            return Ok(());
        }

        let bound_for_state = bound.clone();
        let portal_name = portal.to_string();
        match self.query_engine.execute_query(&bound).await {
            Ok(QueryResult::Select { columns, rows })
            | Ok(QueryResult::Merge { columns, rows, .. }) => {
                self.note_statement(&bound_for_state);
                // Remembered so a later Execute on the same portal continues
                // where this one stopped, which is how every driver implements
                // a cursor with a fetch size.
                self.portal_rows.insert(
                    portal_name.clone(),
                    PortalRows {
                        columns: columns.clone(),
                        rows: rows.clone(),
                        sent: 0,
                    },
                );
                self.send_portal_page(&portal_name, &columns, &rows, 0, max_rows, buf);
            }
            Ok(result) => {
                // Extended protocol: the row description was already sent in
                // response to Describe, and repeating it here is a protocol
                // violation. Only the rows and the command tag belong on Execute.
                self.send_query_result_without_description(&result, buf);
                self.note_statement(&bound_for_state);
            }
            Err(e) => {
                self.send_error(buf, &e.to_string());
                self.note_failure();
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

        if matches!(target, DescribeTarget::Statement) || description.returns_rows() {
            // Remembered for Execute, which must encode each value in the
            // format the client asked for and therefore needs its type.
            let statement_name = match target {
                DescribeTarget::Statement => name.to_string(),
                DescribeTarget::Portal => self
                    .portals
                    .get(name)
                    .map(|(statement, _)| statement.clone())
                    .unwrap_or_default(),
            };
            self.statement_columns.insert(
                statement_name,
                description.columns.iter().map(|c| c.type_oid).collect(),
            );
        }

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
                self.portal_rows.remove(name);
                self.portal_result_formats.remove(name);
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

    /// Handle `LISTEN`, `UNLISTEN` and `NOTIFY`, returning the command tag.
    ///
    /// Returns `None` for anything else, which then runs as an ordinary
    /// statement. These are handled here rather than in the SQL engine because
    /// they act on the connection, not on stored data.
    async fn handle_notification_statement(&mut self, query: &str) -> Option<String> {
        let trimmed = query.trim().trim_end_matches(';').trim();
        let (head, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((head, rest)) => (head.to_ascii_uppercase(), rest.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };

        match head.as_str() {
            "LISTEN" if !rest.is_empty() => {
                self.notifications
                    .listen(
                        rest,
                        self.session_notifications.id,
                        self.session_notifications.sender.clone(),
                    )
                    .await;
                Some("LISTEN".to_string())
            }
            "UNLISTEN" => {
                let channel = (rest != "*" && !rest.is_empty()).then_some(rest);
                self.notifications
                    .unlisten(channel, self.session_notifications.id)
                    .await;
                Some("UNLISTEN".to_string())
            }
            "NOTIFY" if !rest.is_empty() => {
                // `NOTIFY channel` or `NOTIFY channel, 'payload'`.
                let (channel, payload) = match rest.split_once(',') {
                    Some((channel, payload)) => (
                        channel.trim(),
                        payload.trim().trim_matches('\'').to_string(),
                    ),
                    None => (rest, String::new()),
                };
                self.notifications
                    .notify(channel, &payload, self.process_id)
                    .await;
                Some("NOTIFY".to_string())
            }
            _ => None,
        }
    }

    /// Write any notifications waiting for this session.
    ///
    /// The protocol allows a NotificationResponse between messages, so they are
    /// flushed at the points the session is already writing.
    fn deliver_pending_notifications(&mut self, buf: &mut BytesMut) {
        while let Ok(notification) = self.session_notifications.receiver.try_recv() {
            BackendMessage::NotificationResponse {
                process_id: notification.process_id,
                channel: notification.channel,
                payload: notification.payload,
            }
            .encode(buf);
        }
    }


    /// Start a `COPY` statement, or return `None` if this is not one.
    ///
    /// Text format only: the binary format needs per-type encoders this engine
    /// does not have, and accepting it while writing text would corrupt the
    /// stream rather than fail.
    async fn handle_copy_statement(
        &mut self,
        query: &str,
        buf: &mut BytesMut,
        send_ready: bool,
    ) -> Option<()> {
        let trimmed = query.trim().trim_end_matches(';').trim();
        if !trimmed.to_ascii_uppercase().starts_with("COPY ") {
            return None;
        }

        let upper = trimmed.to_ascii_uppercase();
        if upper.contains(" BINARY") || upper.contains("FORMAT BINARY") {
            self.send_error(buf, "COPY BINARY is not supported; use the text format");
            self.finish_copy_statement(send_ready, buf);
            return Some(());
        }

        // `COPY <table> [(cols)] TO STDOUT` / `FROM STDIN`
        let body = trimmed[5..].trim();
        let to_stdout = upper.contains(" TO ");
        let split_at = if to_stdout {
            upper.find(" TO ")
        } else {
            upper.find(" FROM ")
        };
        let Some(split_at) = split_at else {
            self.send_error(buf, "COPY requires TO STDOUT or FROM STDIN");
            self.finish_copy_statement(send_ready, buf);
            return Some(());
        };

        let target = trimmed[5..split_at].trim();
        let (table, columns) = match target.split_once('(') {
            Some((table, cols)) => (
                table.trim().to_string(),
                cols.trim_end_matches(')')
                    .split(',')
                    .map(|c| c.trim().to_string())
                    .collect::<Vec<_>>(),
            ),
            None => (target.to_string(), Vec::new()),
        };
        let _ = body;

        if to_stdout {
            self.copy_table_to_stdout(&table, &columns, buf).await;
            self.finish_copy_statement(send_ready, buf);
        } else {
            self.begin_copy_from_stdin(&table, columns, buf).await;
        }
        Some(())
    }

    /// Close out a COPY statement on the simple query path.
    ///
    /// The extended protocol sends ReadyForQuery in response to Sync instead,
    /// so sending one here too would leave the client a message ahead.
    fn finish_copy_statement(&mut self, send_ready: bool, buf: &mut BytesMut) {
        if send_ready {
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
        }
    }

    /// Stream a table to the client as `COPY ... TO STDOUT` text.
    async fn copy_table_to_stdout(
        &mut self,
        table: &str,
        columns: &[String],
        buf: &mut BytesMut,
    ) {
        let projection = if columns.is_empty() {
            "*".to_string()
        } else {
            columns.join(", ")
        };

        let result = self
            .query_engine
            .execute_query(&format!("SELECT {projection} FROM {table}"))
            .await;

        let (column_count, rows) = match result {
            Ok(QueryResult::Select { columns, rows }) => (columns.len(), rows),
            Ok(_) => (0, Vec::new()),
            Err(e) => {
                self.send_error(buf, &e.to_string());
                return;
            }
        };

        BackendMessage::CopyOutResponse {
            format: 0,
            column_formats: vec![0; column_count],
        }
        .encode(buf);

        for row in &rows {
            let line = row
                .iter()
                .map(|value| match value {
                    // `\N` is how the text format spells NULL, and is why a
                    // literal backslash has to be escaped.
                    None => "\\N".to_string(),
                    Some(text) => text
                        .replace('\\', "\\\\")
                        .replace('\t', "\\t")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r"),
                })
                .collect::<Vec<_>>()
                .join("\t");
            BackendMessage::CopyData {
                data: bytes::Bytes::from(format!("{line}\n")),
            }
            .encode(buf);
        }

        BackendMessage::CopyDone.encode(buf);
        BackendMessage::CommandComplete {
            tag: format!("COPY {}", rows.len()),
        }
        .encode(buf);
    }

    /// Put the session into copy-in mode and invite the client to stream.
    async fn begin_copy_from_stdin(
        &mut self,
        table: &str,
        columns: Vec<String>,
        buf: &mut BytesMut,
    ) {
        // Column order has to be known before the first row arrives; when the
        // statement did not name any, the table's own order is used.
        let schema = match self.query_engine.table_schema(table).await {
            Ok(schema) => schema,
            Err(e) => {
                self.send_error(buf, &e.to_string());
                return;
            }
        };

        let columns = if columns.is_empty() {
            match &schema {
                Some(schema) => schema.columns.iter().map(|c| c.name.clone()).collect(),
                None => {
                    self.send_error(buf, &format!("Table '{table}' does not exist"));
                    return;
                }
            }
        } else {
            columns
        };

        use crate::protocols::postgres_wire::persistent_storage::ColumnType;
        let numeric_columns: Vec<bool> = columns
            .iter()
            .map(|name| {
                schema.as_ref().is_some_and(|schema| {
                    schema
                        .columns
                        .iter()
                        .find(|c| c.name.eq_ignore_ascii_case(name))
                        .is_some_and(|c| {
                            matches!(
                                c.data_type,
                                ColumnType::Serial
                                    | ColumnType::Integer
                                    | ColumnType::BigInt
                                    | ColumnType::Double
                                    | ColumnType::Boolean
                            )
                        })
                })
            })
            .collect();

        BackendMessage::CopyInResponse {
            format: 0,
            column_formats: vec![0; columns.len()],
        }
        .encode(buf);

        self.copy_in = Some(CopyInState {
            simple_protocol: self.copy_in_is_simple,
            table: table.to_string(),
            numeric_columns,
            columns,
            partial: Vec::new(),
            rows: 0,
            failure: None,
        });
    }

    /// Consume one chunk of copy-in data.
    async fn handle_copy_data(
        &mut self,
        data: &[u8],
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        if self.copy_in.is_none() {
            self.send_error(buf, "CopyData received while not in copy-in mode");
            return Ok(());
        }

        // Chunks split rows arbitrarily, so only whole lines are consumed and
        // the remainder is carried to the next message.
        let mut pending = {
            let state = self.copy_in.as_mut().expect("checked above");
            state.partial.extend_from_slice(data);
            std::mem::take(&mut state.partial)
        };

        let mut consumed = 0usize;
        while let Some(newline) = pending[consumed..].iter().position(|b| *b == b'\n') {
            let line_end = consumed + newline;
            let line = String::from_utf8_lossy(&pending[consumed..line_end]).into_owned();
            consumed = line_end + 1;
            self.insert_copy_line(line.trim_end_matches('\r')).await?;
        }

        pending.drain(..consumed);
        if let Some(state) = self.copy_in.as_mut() {
            state.partial = pending;
        }
        Ok(())
    }

    /// Insert one text-format COPY line.
    async fn insert_copy_line(&mut self, line: &str) -> ProtocolResult<()> {
        // The end-of-data marker is a line containing only `\.`.
        if line.is_empty() || line == "\\." {
            return Ok(());
        }

        let Some(state) = self.copy_in.as_ref() else {
            return Ok(());
        };

        let values: Vec<String> = line
            .split('\t')
            .enumerate()
            .map(|(index, field)| {
                if field == "\\N" {
                    return "NULL".to_string();
                }
                let unescaped = field
                    .replace("\\t", "\t")
                    .replace("\\n", "\n")
                    .replace("\\r", "\r")
                    .replace("\\\\", "\\");

                // Bare only where the column takes a bare literal *and* the
                // value really is one; anything else is quoted so a stray field
                // cannot become syntax.
                let numeric = state.numeric_columns.get(index).copied().unwrap_or(false);
                if numeric && Self::is_safe_bare_literal(&unescaped) {
                    unescaped
                } else {
                    format!("'{}'", unescaped.replace('\'', "''"))
                }
            })
            .collect();

        let statement = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            state.table,
            state.columns.join(", "),
            values.join(", ")
        );

        match self.query_engine.execute_query(&statement).await {
            Ok(_) => {
                if let Some(state) = self.copy_in.as_mut() {
                    state.rows += 1;
                }
                Ok(())
            }
            // Recorded rather than returned: an error raised here would be
            // written to a client that is streaming data and expecting no
            // messages at all, which desynchronises the connection. It is
            // reported when the stream ends.
            Err(e) => {
                if let Some(state) = self.copy_in.as_mut() {
                    if state.failure.is_none() {
                        state.failure = Some(e.to_string());
                    }
                }
                Ok(())
            }
        }
    }

    /// Finish a copy-in stream.
    async fn handle_copy_done(&mut self, buf: &mut BytesMut) -> ProtocolResult<()> {
        let Some(state) = self.copy_in.take() else {
            self.send_error(buf, "CopyDone received while not in copy-in mode");
            return Ok(());
        };

        match &state.failure {
            Some(failure) => self.send_error(buf, failure),
            None => BackendMessage::CommandComplete {
                tag: format!("COPY {}", state.rows),
            }
            .encode(buf),
        }
        self.finish_copy_statement(state.simple_protocol, buf);
        Ok(())
    }

    /// Send at most `max_rows` rows of a portal, starting at `already_sent`.
    ///
    /// `max_rows == 0` means "no limit", per the protocol. When rows remain
    /// after the limit is reached the reply is `PortalSuspended` rather than
    /// `CommandComplete`: that is what tells the client to ask for the next
    /// page instead of concluding the result set ended.
    fn send_portal_page(
        &mut self,
        portal: &str,
        columns: &[String],
        rows: &[Vec<Option<String>>],
        already_sent: usize,
        max_rows: i32,
        buf: &mut BytesMut,
    ) {
        let _ = columns;
        let limit = if max_rows <= 0 {
            rows.len().saturating_sub(already_sent)
        } else {
            (max_rows as usize).min(rows.len().saturating_sub(already_sent))
        };

        let formats = self
            .portal_result_formats
            .get(portal)
            .cloned()
            .unwrap_or_default();
        let column_types = self
            .portals
            .get(portal)
            .and_then(|(statement, _)| self.statement_columns.get(statement))
            .cloned()
            .unwrap_or_default();

        for row in rows.iter().skip(already_sent).take(limit) {
            let values: Vec<Option<bytes::Bytes>> = row
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let text = value.as_ref()?;
                    let binary = match formats.len() {
                        0 => false,
                        1 => formats[0] == 1,
                        _ => formats.get(index).is_some_and(|f| *f == 1),
                    };
                    if !binary {
                        return Some(bytes::Bytes::from(text.clone()));
                    }
                    let type_oid = column_types.get(index).copied().unwrap_or(type_oids::TEXT);
                    Some(Self::encode_binary_value(text, type_oid))
                })
                .collect();
            BackendMessage::DataRow { values }.encode(buf);
        }

        let sent = already_sent + limit;
        if sent < rows.len() {
            if let Some(state) = self.portal_rows.get_mut(portal) {
                state.sent = sent;
            }
            BackendMessage::PortalSuspended.encode(buf);
        } else {
            self.portal_rows.remove(portal);
            BackendMessage::CommandComplete {
                tag: format!("SELECT {sent}"),
            }
            .encode(buf);
        }
    }

    /// Encode one value in PostgreSQL's binary format for `type_oid`.
    ///
    /// The engine holds every value as text, so this parses and re-encodes.
    /// A value that will not parse as its declared type is sent as its text
    /// bytes: that is what the value actually is, and it keeps a type
    /// mismatch in the catalogue from corrupting unrelated columns in the row.
    fn encode_binary_value(text: &str, type_oid: i32) -> bytes::Bytes {
        fn bytes_of(vector: Vec<u8>) -> bytes::Bytes {
            bytes::Bytes::from(vector)
        }

        match type_oid {
            type_oids::BOOL => {
                let value = matches!(
                    text.to_ascii_lowercase().as_str(),
                    "t" | "true" | "1" | "yes" | "on"
                );
                bytes_of(vec![u8::from(value)])
            }
            type_oids::INT2 => text
                .parse::<i16>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::INT4 => text
                .parse::<i32>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::INT8 => text
                .parse::<i64>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::FLOAT4 => text
                .parse::<f32>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::FLOAT8 => text
                .parse::<f64>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            // text, json, uuid and anything unrecognised are the same bytes in
            // both formats.
            _ => bytes::Bytes::from(text.to_string()),
        }
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
    /// Answer an `SSLRequest` that reached the message loop.
    ///
    /// TLS is negotiated by the listener before this handler ever runs, so an
    /// `SSLRequest` arriving here is a second one on an already-established
    /// session. `N` is the correct answer: whatever transport the connection
    /// has is already fixed.
    async fn handle_ssl_request(&mut self, buf: &mut BytesMut) -> ProtocolResult<()> {
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
            &[],
        )
        .expect("both parameters are bound");
        assert_eq!(bound, "SELECT * FROM t WHERE a = 'one' AND b = 'two'");
    }

    #[test]
    fn a_repeated_placeholder_uses_the_same_value_each_time() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT $1, $1", &[param("x")], &[]).expect("bound");
        assert_eq!(bound, "SELECT 'x', 'x'");
    }

    #[test]
    fn a_null_parameter_becomes_sql_null_not_an_empty_string() {
        let bound = PostgresWireProtocol::bind_parameters("SELECT $1", &[None], &[]).expect("bound");
        assert_eq!(bound, "SELECT NULL");
    }

    /// A parameter is data. Quotes and statement terminators inside one must not
    /// become syntax.
    #[test]
    fn quotes_in_a_parameter_are_escaped_rather_than_ending_the_literal() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE name = $1",
            &[param("O'Brien")],
            &[],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT * FROM t WHERE name = 'O''Brien'");
    }

    #[test]
    fn an_injection_attempt_in_a_parameter_stays_inside_the_literal() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE name = $1",
            &[param("x'; DROP TABLE users; --")],
            &[],
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
            PostgresWireProtocol::bind_parameters("SELECT '$1', $1", &[param("v")], &[]).expect("bound");
        assert_eq!(bound, "SELECT '$1', 'v'");
    }

    /// Running with a literal `$1` still in the statement would quietly return
    /// the wrong rows, so an unbound reference must fail loudly.
    #[test]
    fn referencing_an_unbound_parameter_is_an_error() {
        let error = PostgresWireProtocol::bind_parameters("SELECT $2", &[param("only-one")], &[])
            .expect_err("only one parameter was bound");
        assert!(
            error.to_string().contains("$2"),
            "the message should name the missing parameter: {error}"
        );
    }

    #[test]
    fn a_statement_without_placeholders_is_unchanged() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT 1 FROM t", &[], &[]).expect("bound");
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

    #[test]
    fn numeric_parameters_are_spliced_without_quotes() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE id = $1",
            &[param("42")],
            &[type_oids::INT4],
        )
        .expect("bound");
        // Quoting this would compare an integer column against a string and
        // match nothing.
        assert_eq!(bound, "SELECT * FROM t WHERE id = 42");
    }

    #[test]
    fn boolean_parameters_are_spliced_without_quotes() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT $1", &[param("true")], &[type_oids::BOOL])
                .expect("bound");
        assert_eq!(bound, "SELECT true");
    }

    #[test]
    fn text_parameters_stay_quoted_even_when_they_look_numeric() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT $1",
            &[param("42")],
            &[type_oids::TEXT],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT '42'");
    }

    /// A value claiming to be numeric but carrying SQL must never be spliced
    /// bare, whatever the declared type says.
    #[test]
    fn a_non_numeric_value_declared_numeric_is_still_quoted() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE id = $1",
            &[param("1; DROP TABLE users")],
            &[type_oids::INT4],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT * FROM t WHERE id = '1; DROP TABLE users'");
    }

    #[test]
    fn binary_encoding_matches_postgres_wire_widths() {
        use super::super::messages::type_oids;

        let encode = PostgresWireProtocol::encode_binary_value;
        assert_eq!(encode("5", type_oids::INT4).as_ref(), &5i32.to_be_bytes());
        assert_eq!(encode("-7", type_oids::INT8).as_ref(), &(-7i64).to_be_bytes());
        assert_eq!(encode("300", type_oids::INT2).as_ref(), &300i16.to_be_bytes());
        assert_eq!(encode("1.5", type_oids::FLOAT8).as_ref(), &1.5f64.to_be_bytes());
        assert_eq!(encode("true", type_oids::BOOL).as_ref(), &[1u8]);
        assert_eq!(encode("f", type_oids::BOOL).as_ref(), &[0u8]);
    }

    /// Text is identical in both formats, so it must not be transformed.
    #[test]
    fn text_is_unchanged_by_binary_encoding() {
        use super::super::messages::type_oids;

        assert_eq!(
            PostgresWireProtocol::encode_binary_value("hello", type_oids::TEXT).as_ref(),
            b"hello"
        );
    }

    /// A value that does not parse as its declared type falls back to its own
    /// bytes rather than emitting a wrong-width field that would desynchronise
    /// the client's read of the rest of the row.
    #[test]
    fn an_unparseable_value_falls_back_to_its_text_bytes() {
        use super::super::messages::type_oids;

        assert_eq!(
            PostgresWireProtocol::encode_binary_value("not-a-number", type_oids::INT4).as_ref(),
            b"not-a-number"
        );
    }
}
