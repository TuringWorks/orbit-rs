//! Flight SQL session management
//!
//! Manages client sessions, prepared statements, transactions, and LIVE queries

use super::messages::*;
use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Flight SQL session for a connected client
#[derive(Debug)]
pub struct FlightSession {
    /// Session ID
    pub session_id: String,
    /// Authenticated user
    pub user: Option<String>,
    /// Current database/schema
    pub database: Option<String>,
    /// Session properties
    pub properties: HashMap<String, String>,
    /// Active transaction
    pub transaction: Option<TransactionHandle>,
    /// Prepared statements
    pub prepared_statements: HashMap<[u8; 16], PreparedStatementHandle>,
    /// Active LIVE query subscriptions
    pub live_queries: HashMap<[u8; 16], LiveQueryHandle>,
    /// Session creation time
    pub created_at: u64,
    /// Last activity time
    pub last_activity: u64,
}

impl FlightSession {
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
            live_queries: HashMap::new(),
            created_at: now,
            last_activity: now,
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

    /// Set a session property
    pub fn set_property(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(key.into(), value.into());
        self.touch();
    }

    /// Get a session property
    pub fn get_property(&self, key: &str) -> Option<&String> {
        self.properties.get(key)
    }

    /// Update last activity time
    pub fn touch(&mut self) {
        self.last_activity = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Check if session has an active transaction
    pub fn has_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    /// Begin a transaction
    pub fn begin_transaction(
        &mut self,
        isolation_level: IsolationLevel,
    ) -> Result<[u8; 16], FlightSqlError> {
        if self.transaction.is_some() {
            return Err(FlightSqlError::transaction_error(
                "Transaction already active",
            ));
        }

        let tx = TransactionHandle::new(isolation_level);
        let id = tx.transaction_id;
        self.transaction = Some(tx);
        self.touch();
        Ok(id)
    }

    /// Commit the current transaction
    pub fn commit_transaction(&mut self) -> Result<[u8; 16], FlightSqlError> {
        match self.transaction.take() {
            Some(tx) => {
                self.touch();
                Ok(tx.transaction_id)
            }
            None => Err(FlightSqlError::transaction_error("No active transaction")),
        }
    }

    /// Rollback the current transaction
    pub fn rollback_transaction(&mut self) -> Result<[u8; 16], FlightSqlError> {
        match self.transaction.take() {
            Some(tx) => {
                self.touch();
                Ok(tx.transaction_id)
            }
            None => Err(FlightSqlError::transaction_error("No active transaction")),
        }
    }

    /// Create a savepoint
    pub fn create_savepoint(&mut self, name: &str) -> Result<[u8; 16], FlightSqlError> {
        match &mut self.transaction {
            Some(tx) => {
                tx.add_savepoint(name);
                // Generate savepoint ID
                let mut savepoint_id = [0u8; 16];
                for byte in &mut savepoint_id {
                    *byte = rand::random();
                }
                self.touch();
                Ok(savepoint_id)
            }
            None => Err(FlightSqlError::transaction_error(
                "No active transaction for savepoint",
            )),
        }
    }

    /// Release a savepoint
    pub fn release_savepoint(&mut self, name: &str) -> Result<(), FlightSqlError> {
        match &mut self.transaction {
            Some(tx) => {
                if let Some(pos) = tx.savepoints.iter().position(|s| s == name) {
                    tx.savepoints.remove(pos);
                    self.touch();
                    Ok(())
                } else {
                    Err(FlightSqlError::not_found(format!(
                        "Savepoint '{}' not found",
                        name
                    )))
                }
            }
            None => Err(FlightSqlError::transaction_error("No active transaction")),
        }
    }

    /// Rollback to a savepoint
    pub fn rollback_to_savepoint(&mut self, name: &str) -> Result<(), FlightSqlError> {
        match &mut self.transaction {
            Some(tx) => {
                if let Some(pos) = tx.savepoints.iter().position(|s| s == name) {
                    // Remove all savepoints after this one
                    tx.savepoints.truncate(pos + 1);
                    self.touch();
                    Ok(())
                } else {
                    Err(FlightSqlError::not_found(format!(
                        "Savepoint '{}' not found",
                        name
                    )))
                }
            }
            None => Err(FlightSqlError::transaction_error("No active transaction")),
        }
    }

    /// Create a prepared statement
    pub fn create_prepared_statement(
        &mut self,
        query: &str,
    ) -> Result<PreparedStatementHandle, FlightSqlError> {
        let ps = PreparedStatementHandle::new(query);
        let handle = ps.handle;
        self.prepared_statements.insert(handle, ps.clone());
        self.touch();
        Ok(ps)
    }

    /// Get a prepared statement
    pub fn get_prepared_statement(&self, handle: &[u8; 16]) -> Option<&PreparedStatementHandle> {
        self.prepared_statements.get(handle)
    }

    /// Close a prepared statement
    pub fn close_prepared_statement(&mut self, handle: &[u8; 16]) -> bool {
        self.touch();
        self.prepared_statements.remove(handle).is_some()
    }

    /// Subscribe to a LIVE query
    pub fn subscribe_live_query(&mut self, query: &str) -> LiveQueryHandle {
        let lq = LiveQueryHandle::new(query);
        let id = lq.subscription_id;
        self.live_queries.insert(id, lq.clone());
        self.touch();
        lq
    }

    /// Unsubscribe from a LIVE query
    pub fn unsubscribe_live_query(&mut self, subscription_id: &[u8; 16]) -> bool {
        self.touch();
        self.live_queries.remove(subscription_id).is_some()
    }

    /// Get a LIVE query subscription
    pub fn get_live_query(&self, subscription_id: &[u8; 16]) -> Option<&LiveQueryHandle> {
        self.live_queries.get(subscription_id)
    }

    /// Check if session is expired (default: 1 hour timeout)
    pub fn is_expired(&self, timeout_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now - self.last_activity > timeout_secs
    }
}

impl Default for FlightSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Session manager for all Flight SQL connections
pub struct SessionManager {
    sessions: RwLock<HashMap<String, Arc<RwLock<FlightSession>>>>,
    session_timeout_secs: u64,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(session_timeout_secs: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            session_timeout_secs,
        }
    }

    /// Create a new session
    pub async fn create_session(&self) -> Arc<RwLock<FlightSession>> {
        let session = Arc::new(RwLock::new(FlightSession::new()));
        let session_id = session.read().await.session_id.clone();
        self.sessions
            .write()
            .await
            .insert(session_id, session.clone());
        session
    }

    /// Get an existing session
    pub async fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<FlightSession>>> {
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

    /// Get active session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get all session IDs
    pub async fn session_ids(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(3600) // 1 hour default timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = FlightSession::new();
        assert!(!session.session_id.is_empty());
        assert!(session.user.is_none());
        assert!(session.database.is_none());
        assert!(!session.has_transaction());
    }

    #[test]
    fn test_session_transaction() {
        let mut session = FlightSession::new();

        // Begin transaction
        let tx_id = session
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        assert!(session.has_transaction());

        // Create savepoint
        let sp_id = session.create_savepoint("sp1").unwrap();
        assert!(!sp_id.iter().all(|&b| b == 0));

        // Cannot begin another transaction
        assert!(session
            .begin_transaction(IsolationLevel::Serializable)
            .is_err());

        // Commit
        let committed_id = session.commit_transaction().unwrap();
        assert_eq!(committed_id, tx_id);
        assert!(!session.has_transaction());
    }

    #[test]
    fn test_prepared_statements() {
        let mut session = FlightSession::new();

        // Create prepared statement
        let ps = session
            .create_prepared_statement("SELECT * FROM users WHERE id = ?")
            .unwrap();
        assert_eq!(ps.query, "SELECT * FROM users WHERE id = ?");

        // Get prepared statement
        let retrieved = session.get_prepared_statement(&ps.handle);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().query, ps.query);

        // Close prepared statement
        assert!(session.close_prepared_statement(&ps.handle));
        assert!(session.get_prepared_statement(&ps.handle).is_none());
    }

    #[test]
    fn test_live_queries() {
        let mut session = FlightSession::new();

        // Subscribe to LIVE query
        let lq = session.subscribe_live_query("LIVE SELECT * FROM orders");
        assert!(session.get_live_query(&lq.subscription_id).is_some());

        // Unsubscribe
        assert!(session.unsubscribe_live_query(&lq.subscription_id));
        assert!(session.get_live_query(&lq.subscription_id).is_none());
    }

    #[tokio::test]
    async fn test_session_manager() {
        let manager = SessionManager::new(3600);

        // Create session
        let session = manager.create_session().await;
        let session_id = session.read().await.session_id.clone();

        // Get session
        let retrieved = manager.get_session(&session_id).await;
        assert!(retrieved.is_some());

        // Session count
        assert_eq!(manager.session_count().await, 1);

        // Remove session
        assert!(manager.remove_session(&session_id).await);
        assert!(manager.get_session(&session_id).await.is_none());
    }
}
