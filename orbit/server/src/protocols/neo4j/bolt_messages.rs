//! Bolt Protocol Message Handlers
//!
//! This module implements all Bolt protocol message handlers according to the
//! Neo4j Bolt Protocol specification.

use crate::protocols::cypher::graph_engine::{GraphEngine, QueryResult};
use crate::protocols::error::ProtocolResult;
use crate::protocols::graph_database::PersistentGraphStorage;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, info, warn};

/// Transaction state for Bolt connections
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    /// No active transaction (auto-commit mode)
    None,
    /// Explicit transaction in progress
    Active { tx_id: u64 },
    /// Transaction failed, awaiting rollback
    Failed { tx_id: u64 },
}

/// Authentication state
#[derive(Debug, Clone)]
pub struct AuthState {
    /// Whether the client is authenticated
    pub authenticated: bool,
    /// Username/principal
    pub principal: Option<String>,
    /// Authentication scheme used
    pub scheme: Option<String>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            authenticated: false,
            principal: None,
            scheme: None,
        }
    }
}

/// Connection state for a Bolt session
pub struct BoltConnectionState {
    /// Current transaction state
    pub transaction_state: TransactionState,
    /// Authentication state
    pub auth_state: AuthState,
    /// Pending query results
    pub pending_results: Option<QueryResult>,
    /// Current result cursor position
    pub result_cursor: usize,
    /// Connection ID
    pub connection_id: String,
    /// Server version
    pub server_version: String,
}

impl BoltConnectionState {
    /// Create a new connection state
    pub fn new(connection_id: String) -> Self {
        Self {
            transaction_state: TransactionState::None,
            auth_state: AuthState::default(),
            pending_results: None,
            result_cursor: 0,
            connection_id,
            server_version: format!("Orbit-Neo4j/{}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Reset connection state
    pub fn reset(&mut self) {
        self.transaction_state = TransactionState::None;
        self.pending_results = None;
        self.result_cursor = 0;
    }
}

/// Bolt message handler
pub struct BoltMessageHandler {
    engine: Arc<GraphEngine<PersistentGraphStorage>>,
}

impl BoltMessageHandler {
    /// Create a new message handler
    pub fn new(engine: Arc<GraphEngine<PersistentGraphStorage>>) -> Self {
        Self { engine }
    }

    /// Handle HELLO message
    ///
    /// HELLO is the first message sent by the client to initialize the connection.
    /// It includes authentication credentials and client metadata.
    ///
    /// Request structure:
    /// - user_agent: String
    /// - auth_token: Map { scheme, principal, credentials, ... }
    /// - routing: Optional Map (for cluster routing)
    ///
    /// Response: SUCCESS with server metadata
    pub async fn handle_hello<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        user_agent: String,
        auth_token: HashMap<String, Value>,
        _routing: Option<HashMap<String, Value>>,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        info!("HELLO from client: {}", user_agent);

        // Extract authentication info
        let scheme = auth_token
            .get("scheme")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let principal = auth_token
            .get("principal")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // For now, accept all authentication (TODO: implement proper auth)
        state.auth_state.authenticated = true;
        state.auth_state.principal = principal.clone();
        state.auth_state.scheme = Some(scheme.to_string());

        debug!(
            "Authenticated user: {:?} with scheme: {}",
            principal, scheme
        );

        // Send SUCCESS with server metadata
        let mut metadata = HashMap::new();
        metadata.insert(
            "server".to_string(),
            Value::String(state.server_version.clone()),
        );
        metadata.insert(
            "connection_id".to_string(),
            Value::String(state.connection_id.clone()),
        );

        protocol.send_success(stream, metadata).await
    }

    /// Handle LOGON message
    ///
    /// LOGON allows re-authentication on an existing connection.
    pub async fn handle_logon<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        auth_token: HashMap<String, Value>,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        let scheme = auth_token
            .get("scheme")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let principal = auth_token
            .get("principal")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Update authentication state
        state.auth_state.authenticated = true;
        state.auth_state.principal = principal;
        state.auth_state.scheme = Some(scheme.to_string());

        protocol.send_success(stream, HashMap::new()).await
    }

    /// Handle LOGOFF message
    ///
    /// LOGOFF invalidates the current authentication.
    pub async fn handle_logoff<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        state.auth_state = AuthState::default();
        protocol.send_success(stream, HashMap::new()).await
    }

    /// Handle RUN message
    ///
    /// RUN executes a Cypher query with optional parameters.
    ///
    /// Request structure:
    /// - query: String (Cypher query)
    /// - parameters: Map (query parameters)
    /// - extra: Map (metadata like timeout, mode, etc.)
    ///
    /// Response: SUCCESS with fields metadata
    pub async fn handle_run<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        query: String,
        parameters: HashMap<String, Value>,
        _extra: HashMap<String, Value>,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        debug!("RUN query: {}", query);
        debug!("Parameters: {:?}", parameters);

        // Check authentication
        if !state.auth_state.authenticated {
            return protocol
                .send_failure(
                    stream,
                    "Neo.ClientError.Security.Unauthorized",
                    "Not authenticated",
                )
                .await;
        }

        // Execute query
        match self.engine.execute_query(&query).await {
            Ok(result) => {
                debug!("Query executed successfully, {} rows", result.rows.len());

                // Store results for PULL
                state.pending_results = Some(result);
                state.result_cursor = 0;

                // Send SUCCESS with fields metadata
                let mut metadata = HashMap::new();
                if let Some(ref results) = state.pending_results {
                    let fields: Vec<Value> = results
                        .columns
                        .iter()
                        .map(|c| Value::String(c.clone()))
                        .collect();
                    metadata.insert("fields".to_string(), Value::Array(fields));
                }

                protocol.send_success(stream, metadata).await
            }
            Err(e) => {
                warn!("Query execution failed: {}", e);
                state.pending_results = None;
                protocol
                    .send_failure(
                        stream,
                        "Neo.ClientError.Statement.SyntaxError",
                        &e.to_string(),
                    )
                    .await
            }
        }
    }

    /// Handle PULL message
    ///
    /// PULL fetches query results in batches.
    ///
    /// Request structure:
    /// - n: Optional i64 (number of records to fetch, -1 for all)
    /// - qid: Optional i64 (query ID for multiple concurrent queries)
    ///
    /// Response: RECORD messages followed by SUCCESS
    pub async fn handle_pull<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        n: Option<i64>,
        _qid: Option<i64>,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        let batch_size = match n {
            Some(-1) | None => usize::MAX, // Fetch all
            Some(n) if n > 0 => n as usize,
            _ => {
                return protocol
                    .send_failure(
                        stream,
                        "Neo.ClientError.Request.Invalid",
                        "Invalid batch size",
                    )
                    .await;
            }
        };

        if let Some(ref results) = state.pending_results {
            let mut sent_count = 0;

            // Send rows
            for row in results.rows.iter().skip(state.result_cursor) {
                if sent_count >= batch_size {
                    break;
                }

                let values: Vec<Value> = row
                    .iter()
                    .map(|v| match v {
                        Some(s) => Value::String(s.clone()),
                        None => Value::Null,
                    })
                    .collect();

                protocol.send_record(stream, values).await?;
                sent_count += 1;
            }

            state.result_cursor += sent_count;

            // If no rows but nodes are present, send nodes as records
            if sent_count == 0 && !results.nodes.is_empty() {
                for node in results.nodes.iter().skip(state.result_cursor) {
                    if sent_count >= batch_size {
                        break;
                    }

                    // Convert node to simple string representation for now
                    // TODO: Proper Node structure encoding
                    let node_str = format!("Node({}: {:?})", node.id, node.labels);
                    protocol
                        .send_record(stream, vec![Value::String(node_str)])
                        .await?;
                    sent_count += 1;
                }
                state.result_cursor += sent_count;
            }

            // Check if there are more results
            let has_more = state.result_cursor < results.rows.len().max(results.nodes.len());

            let mut metadata = HashMap::new();
            metadata.insert("has_more".to_string(), Value::Bool(has_more));

            if !has_more {
                // Clear results when done
                state.pending_results = None;
                state.result_cursor = 0;
            }

            protocol.send_success(stream, metadata).await
        } else {
            // No pending results
            protocol.send_success(stream, HashMap::new()).await
        }
    }

    /// Handle DISCARD message
    ///
    /// DISCARD discards pending query results.
    pub async fn handle_discard<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        _n: Option<i64>,
        _qid: Option<i64>,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        state.pending_results = None;
        state.result_cursor = 0;
        protocol.send_success(stream, HashMap::new()).await
    }

    /// Handle BEGIN message
    ///
    /// BEGIN starts an explicit transaction.
    pub async fn handle_begin<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        _extra: HashMap<String, Value>,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        match state.transaction_state {
            TransactionState::None => {
                // Start new transaction
                let tx_id = rand::random::<u64>();
                state.transaction_state = TransactionState::Active { tx_id };

                debug!("Started transaction {}", tx_id);

                // TODO: Actually begin transaction in storage layer
                protocol.send_success(stream, HashMap::new()).await
            }
            _ => {
                protocol
                    .send_failure(
                        stream,
                        "Neo.ClientError.Transaction.TransactionStartFailed",
                        "Transaction already active",
                    )
                    .await
            }
        }
    }

    /// Handle COMMIT message
    ///
    /// COMMIT commits the active transaction.
    pub async fn handle_commit<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        match state.transaction_state {
            TransactionState::Active { tx_id } => {
                debug!("Committing transaction {}", tx_id);

                // TODO: Actually commit transaction in storage layer
                state.transaction_state = TransactionState::None;

                // Return bookmark (optional)
                let mut metadata = HashMap::new();
                metadata.insert(
                    "bookmark".to_string(),
                    Value::String(format!("bookmark:{}", tx_id)),
                );

                protocol.send_success(stream, metadata).await
            }
            TransactionState::Failed { .. } => {
                protocol
                    .send_failure(
                        stream,
                        "Neo.ClientError.Transaction.TransactionCommitFailed",
                        "Transaction is in failed state",
                    )
                    .await
            }
            TransactionState::None => {
                protocol
                    .send_failure(
                        stream,
                        "Neo.ClientError.Transaction.TransactionCommitFailed",
                        "No active transaction",
                    )
                    .await
            }
        }
    }

    /// Handle ROLLBACK message
    ///
    /// ROLLBACK rolls back the active transaction.
    pub async fn handle_rollback<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        match state.transaction_state {
            TransactionState::Active { tx_id } | TransactionState::Failed { tx_id } => {
                debug!("Rolling back transaction {}", tx_id);

                // TODO: Actually rollback transaction in storage layer
                state.transaction_state = TransactionState::None;

                protocol.send_success(stream, HashMap::new()).await
            }
            TransactionState::None => {
                protocol
                    .send_failure(
                        stream,
                        "Neo.ClientError.Transaction.TransactionRollbackFailed",
                        "No active transaction",
                    )
                    .await
            }
        }
    }

    /// Handle RESET message
    ///
    /// RESET resets the connection state, rolling back any active transaction.
    pub async fn handle_reset<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        stream: &mut S,
        protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<()> {
        // Rollback active transaction if any
        if let TransactionState::Active { tx_id } | TransactionState::Failed { tx_id } =
            state.transaction_state
        {
            debug!("RESET: Rolling back transaction {}", tx_id);
            // TODO: Actually rollback in storage
        }

        state.reset();
        protocol.send_success(stream, HashMap::new()).await
    }

    /// Handle GOODBYE message
    ///
    /// GOODBYE cleanly closes the connection.
    pub async fn handle_goodbye<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        state: &mut BoltConnectionState,
        _stream: &mut S,
        _protocol: &mut impl BoltProtocolWriter,
    ) -> ProtocolResult<bool> {
        info!("Client sent GOODBYE, closing connection");

        // Cleanup any active transaction
        if let TransactionState::Active { tx_id } | TransactionState::Failed { tx_id } =
            state.transaction_state
        {
            debug!("GOODBYE: Rolling back transaction {}", tx_id);
            // TODO: Actually rollback in storage
        }

        state.reset();
        Ok(false) // Signal to close connection
    }
}

/// Trait for writing Bolt protocol messages
///
/// This trait abstracts the protocol writer to allow for testing
#[async_trait::async_trait]
pub trait BoltProtocolWriter: Send + Sync {
    /// Send SUCCESS message
    async fn send_success<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        metadata: HashMap<String, Value>,
    ) -> ProtocolResult<()>;

    /// Send RECORD message
    async fn send_record<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        fields: Vec<Value>,
    ) -> ProtocolResult<()>;

    /// Send FAILURE message
    async fn send_failure<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
        code: &str,
        message: &str,
    ) -> ProtocolResult<()>;

    /// Send IGNORED message
    async fn send_ignored<S: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        stream: &mut S,
    ) -> ProtocolResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_state_transitions() {
        let mut state = BoltConnectionState::new("test-conn".to_string());

        // Initial state
        assert_eq!(state.transaction_state, TransactionState::None);

        // Begin transaction
        state.transaction_state = TransactionState::Active { tx_id: 123 };
        assert!(matches!(
            state.transaction_state,
            TransactionState::Active { .. }
        ));

        // Reset clears transaction
        state.reset();
        assert_eq!(state.transaction_state, TransactionState::None);
    }

    #[test]
    fn test_auth_state() {
        let mut state = BoltConnectionState::new("test-conn".to_string());

        // Initially not authenticated
        assert!(!state.auth_state.authenticated);

        // Authenticate
        state.auth_state.authenticated = true;
        state.auth_state.principal = Some("alice".to_string());
        state.auth_state.scheme = Some("basic".to_string());

        assert!(state.auth_state.authenticated);
        assert_eq!(state.auth_state.principal, Some("alice".to_string()));
    }
}
