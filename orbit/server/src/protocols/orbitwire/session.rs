//! OrbitWire session management
//!
//! Manages client sessions, streams, and subscriptions

use super::messages::{IsolationLevel, RowDescriptionMessage};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// OrbitWire session for a connected client
#[derive(Debug)]
pub struct OrbitWireSession {
    /// Session ID
    pub session_id: String,
    /// Authenticated user
    pub user: Option<String>,
    /// Current database
    pub database: Option<String>,
    /// Session properties
    pub properties: HashMap<String, String>,
    /// Active transaction
    pub transaction: Option<TransactionState>,
    /// Prepared statements
    pub prepared_statements: HashMap<String, PreparedStatement>,
    /// Active LIVE query subscriptions
    pub live_subscriptions: HashMap<[u8; 16], LiveSubscription>,
    /// Active streams
    pub streams: HashMap<u32, StreamState>,
    /// Next stream ID
    next_stream_id: AtomicU32,
    /// Session creation time
    pub created_at: u64,
    /// Last activity time
    pub last_activity: u64,
    /// Client capabilities
    pub capabilities: Vec<String>,
}

/// Transaction state
#[derive(Debug, Clone)]
pub struct TransactionState {
    pub transaction_id: [u8; 16],
    pub isolation_level: IsolationLevel,
    pub read_only: bool,
    pub savepoints: Vec<SavepointState>,
    pub started_at: u64,
}

/// Savepoint state
#[derive(Debug, Clone)]
pub struct SavepointState {
    pub id: [u8; 16],
    pub name: String,
    pub created_at: u64,
}

/// Prepared statement
#[derive(Debug, Clone)]
pub struct PreparedStatement {
    pub name: String,
    pub query: String,
    pub parameter_types: Vec<u8>,
    pub result_schema: Option<RowDescriptionMessage>,
    pub created_at: u64,
}

/// LIVE query subscription
#[derive(Debug, Clone)]
pub struct LiveSubscription {
    pub subscription_id: [u8; 16],
    pub query: String,
    pub stream_id: u32,
    pub diff_mode: bool,
    pub schema: Option<RowDescriptionMessage>,
    pub last_event_id: u64,
    pub created_at: u64,
}

/// Stream state
#[derive(Debug, Clone)]
pub struct StreamState {
    pub stream_id: u32,
    pub stream_type: StreamType,
    pub state: StreamStatus,
    pub created_at: u64,
}

/// Stream type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Query,
    PreparedStatement,
    LiveSubscription,
    Transaction,
}

/// Stream status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStatus {
    Active,
    Paused,
    Completed,
    Error,
}

impl OrbitWireSession {
    /// Create a new session
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            user: None,
            database: None,
            properties: HashMap::new(),
            transaction: None,
            prepared_statements: HashMap::new(),
            live_subscriptions: HashMap::new(),
            streams: HashMap::new(),
            next_stream_id: AtomicU32::new(1),
            created_at: now,
            last_activity: now,
            capabilities: Vec::new(),
        }
    }

    /// Set the authenticated user
    pub fn set_user(&mut self, user: impl Into<String>) {
        self.user = Some(user.into());
        self.touch();
    }

    /// Set the current database
    pub fn set_database(&mut self, database: impl Into<String>) {
        self.database = Some(database.into());
        self.touch();
    }

    /// Set client capabilities
    pub fn set_capabilities(&mut self, capabilities: Vec<String>) {
        self.capabilities = capabilities;
        self.touch();
    }

    /// Update last activity time
    pub fn touch(&mut self) {
        self.last_activity = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Allocate a new stream ID
    pub fn next_stream_id(&self) -> u32 {
        self.next_stream_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Create a new stream
    pub fn create_stream(&mut self, stream_type: StreamType) -> u32 {
        let stream_id = self.next_stream_id();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.streams.insert(
            stream_id,
            StreamState {
                stream_id,
                stream_type,
                state: StreamStatus::Active,
                created_at: now,
            },
        );

        self.touch();
        stream_id
    }

    /// Complete a stream
    pub fn complete_stream(&mut self, stream_id: u32) {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            stream.state = StreamStatus::Completed;
        }
        self.touch();
    }

    /// Remove a stream
    pub fn remove_stream(&mut self, stream_id: u32) -> bool {
        self.touch();
        self.streams.remove(&stream_id).is_some()
    }

    /// Begin a transaction
    pub fn begin_transaction(
        &mut self,
        isolation_level: IsolationLevel,
        read_only: bool,
    ) -> Result<[u8; 16], SessionError> {
        if self.transaction.is_some() {
            return Err(SessionError::TransactionActive);
        }

        let mut transaction_id = [0u8; 16];
        for byte in &mut transaction_id {
            *byte = rand::random();
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.transaction = Some(TransactionState {
            transaction_id,
            isolation_level,
            read_only,
            savepoints: Vec::new(),
            started_at: now,
        });

        self.touch();
        Ok(transaction_id)
    }

    /// Commit the current transaction
    pub fn commit_transaction(&mut self) -> Result<[u8; 16], SessionError> {
        match self.transaction.take() {
            Some(tx) => {
                self.touch();
                Ok(tx.transaction_id)
            }
            None => Err(SessionError::NoTransaction),
        }
    }

    /// Rollback the current transaction
    pub fn rollback_transaction(&mut self) -> Result<[u8; 16], SessionError> {
        match self.transaction.take() {
            Some(tx) => {
                self.touch();
                Ok(tx.transaction_id)
            }
            None => Err(SessionError::NoTransaction),
        }
    }

    /// Create a savepoint
    pub fn create_savepoint(&mut self, name: impl Into<String>) -> Result<[u8; 16], SessionError> {
        let tx = self
            .transaction
            .as_mut()
            .ok_or(SessionError::NoTransaction)?;

        let mut savepoint_id = [0u8; 16];
        for byte in &mut savepoint_id {
            *byte = rand::random();
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        tx.savepoints.push(SavepointState {
            id: savepoint_id,
            name: name.into(),
            created_at: now,
        });

        self.touch();
        Ok(savepoint_id)
    }

    /// Release a savepoint
    pub fn release_savepoint(&mut self, name: &str) -> Result<[u8; 16], SessionError> {
        let tx = self
            .transaction
            .as_mut()
            .ok_or(SessionError::NoTransaction)?;

        if let Some(pos) = tx.savepoints.iter().position(|sp| sp.name == name) {
            let sp = tx.savepoints.remove(pos);
            self.touch();
            Ok(sp.id)
        } else {
            Err(SessionError::SavepointNotFound)
        }
    }

    /// Rollback to a savepoint
    pub fn rollback_to_savepoint(&mut self, name: &str) -> Result<[u8; 16], SessionError> {
        let tx = self
            .transaction
            .as_mut()
            .ok_or(SessionError::NoTransaction)?;

        if let Some(pos) = tx.savepoints.iter().position(|sp| sp.name == name) {
            // Remove all savepoints after this one
            tx.savepoints.truncate(pos + 1);
            let id = tx.savepoints[pos].id;
            self.touch();
            Ok(id)
        } else {
            Err(SessionError::SavepointNotFound)
        }
    }

    /// Create a prepared statement
    pub fn create_prepared_statement(
        &mut self,
        name: impl Into<String>,
        query: impl Into<String>,
    ) -> Result<&PreparedStatement, SessionError> {
        let name = name.into();
        let query = query.into();

        if self.prepared_statements.contains_key(&name) {
            return Err(SessionError::PreparedStatementExists);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.prepared_statements.insert(
            name.clone(),
            PreparedStatement {
                name: name.clone(),
                query,
                parameter_types: Vec::new(),
                result_schema: None,
                created_at: now,
            },
        );

        self.touch();
        Ok(self.prepared_statements.get(&name).unwrap())
    }

    /// Get a prepared statement
    pub fn get_prepared_statement(&self, name: &str) -> Option<&PreparedStatement> {
        self.prepared_statements.get(name)
    }

    /// Close a prepared statement
    pub fn close_prepared_statement(&mut self, name: &str) -> bool {
        self.touch();
        self.prepared_statements.remove(name).is_some()
    }

    /// Subscribe to a LIVE query
    pub fn subscribe_live_query(
        &mut self,
        query: impl Into<String>,
        diff_mode: bool,
    ) -> LiveSubscription {
        let mut subscription_id = [0u8; 16];
        for byte in &mut subscription_id {
            *byte = rand::random();
        }

        let stream_id = self.create_stream(StreamType::LiveSubscription);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let subscription = LiveSubscription {
            subscription_id,
            query: query.into(),
            stream_id,
            diff_mode,
            schema: None,
            last_event_id: 0,
            created_at: now,
        };

        self.live_subscriptions
            .insert(subscription_id, subscription.clone());
        self.touch();
        subscription
    }

    /// Unsubscribe from a LIVE query
    pub fn unsubscribe_live_query(&mut self, subscription_id: &[u8; 16]) -> bool {
        if let Some(sub) = self.live_subscriptions.remove(subscription_id) {
            self.remove_stream(sub.stream_id);
            self.touch();
            true
        } else {
            false
        }
    }

    /// Get a LIVE subscription
    pub fn get_live_subscription(&self, subscription_id: &[u8; 16]) -> Option<&LiveSubscription> {
        self.live_subscriptions.get(subscription_id)
    }

    /// Check if session has active transaction
    pub fn has_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    /// Check if session is expired
    pub fn is_expired(&self, timeout_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now - self.last_activity > timeout_secs
    }

    /// Get active stream count
    pub fn active_stream_count(&self) -> usize {
        self.streams
            .values()
            .filter(|s| s.state == StreamStatus::Active)
            .count()
    }
}

impl Default for OrbitWireSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Session errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    TransactionActive,
    NoTransaction,
    SavepointNotFound,
    PreparedStatementExists,
    PreparedStatementNotFound,
    SubscriptionNotFound,
    StreamNotFound,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::TransactionActive => write!(f, "Transaction already active"),
            SessionError::NoTransaction => write!(f, "No active transaction"),
            SessionError::SavepointNotFound => write!(f, "Savepoint not found"),
            SessionError::PreparedStatementExists => write!(f, "Prepared statement already exists"),
            SessionError::PreparedStatementNotFound => write!(f, "Prepared statement not found"),
            SessionError::SubscriptionNotFound => write!(f, "Subscription not found"),
            SessionError::StreamNotFound => write!(f, "Stream not found"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Session manager for all OrbitWire connections
pub struct OrbitWireSessionManager {
    sessions: RwLock<HashMap<String, Arc<RwLock<OrbitWireSession>>>>,
    session_timeout_secs: u64,
}

impl OrbitWireSessionManager {
    /// Create a new session manager
    pub fn new(session_timeout_secs: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            session_timeout_secs,
        }
    }

    /// Create a new session
    pub async fn create_session(&self) -> Arc<RwLock<OrbitWireSession>> {
        let session = Arc::new(RwLock::new(OrbitWireSession::new()));
        let session_id = session.read().await.session_id.clone();
        self.sessions
            .write()
            .await
            .insert(session_id, session.clone());
        session
    }

    /// Get an existing session
    pub async fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<OrbitWireSession>>> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Remove a session
    pub async fn remove_session(&self, session_id: &str) -> bool {
        self.sessions.write().await.remove(session_id).is_some()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let mut expired = Vec::new();

        for (id, session) in sessions.iter() {
            if session.read().await.is_expired(self.session_timeout_secs) {
                expired.push(id.clone());
            }
        }

        for id in &expired {
            sessions.remove(id);
        }

        expired.len()
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

impl Default for OrbitWireSessionManager {
    fn default() -> Self {
        Self::new(3600) // 1 hour default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = OrbitWireSession::new();
        assert!(!session.session_id.is_empty());
        assert!(session.user.is_none());
        assert!(!session.has_transaction());
    }

    #[test]
    fn test_transaction_lifecycle() {
        let mut session = OrbitWireSession::new();

        // Begin transaction
        let tx_id = session
            .begin_transaction(IsolationLevel::ReadCommitted, false)
            .unwrap();
        assert!(session.has_transaction());

        // Create savepoint
        let _sp_id = session.create_savepoint("sp1").unwrap();
        let _sp_id = session.create_savepoint("sp1").unwrap();

        // Cannot begin another transaction
        assert!(session
            .begin_transaction(IsolationLevel::Serializable, false)
            .is_err());

        // Commit
        let committed_id = session.commit_transaction().unwrap();
        assert_eq!(committed_id, tx_id);
        assert!(!session.has_transaction());
    }

    #[test]
    fn test_prepared_statements() {
        let mut session = OrbitWireSession::new();

        // Create
        session
            .create_prepared_statement("stmt1", "SELECT * FROM users WHERE id = $1")
            .unwrap();

        // Get
        let ps = session.get_prepared_statement("stmt1");
        assert!(ps.is_some());
        assert_eq!(ps.unwrap().query, "SELECT * FROM users WHERE id = $1");

        // Cannot create duplicate
        assert!(session
            .create_prepared_statement("stmt1", "SELECT 1")
            .is_err());

        // Close
        assert!(session.close_prepared_statement("stmt1"));
        assert!(session.get_prepared_statement("stmt1").is_none());
    }

    #[test]
    fn test_live_subscriptions() {
        let mut session = OrbitWireSession::new();

        // Subscribe
        let sub = session.subscribe_live_query("LIVE SELECT * FROM orders", true);
        assert!(session
            .get_live_subscription(&sub.subscription_id)
            .is_some());

        // Unsubscribe
        assert!(session.unsubscribe_live_query(&sub.subscription_id));
        assert!(session
            .get_live_subscription(&sub.subscription_id)
            .is_none());
    }

    #[test]
    fn test_streams() {
        let mut session = OrbitWireSession::new();

        // Create stream
        let stream_id = session.create_stream(StreamType::Query);
        assert!(session.streams.contains_key(&stream_id));
        assert_eq!(session.active_stream_count(), 1);

        // Complete stream
        session.complete_stream(stream_id);
        assert_eq!(session.active_stream_count(), 0);

        // Remove stream
        assert!(session.remove_stream(stream_id));
        assert!(!session.streams.contains_key(&stream_id));
    }
}
