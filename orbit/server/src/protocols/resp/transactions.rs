// Redis transaction support (MULTI/EXEC/DISCARD/WATCH/UNWATCH)
//
// Implements Redis transaction commands for atomic execution of command sequences.
// Transactions in Redis provide:
// - Atomicity: All commands execute together or none execute
// - Isolation: Commands queued during MULTI are not executed until EXEC
// - Optimistic locking: WATCH allows conditional execution based on key changes

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::resp::types::RespValue;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Transaction state for a client connection
#[derive(Debug, Clone)]
pub struct TransactionState {
    /// Whether we're in a MULTI block
    pub in_transaction: bool,
    /// Queued commands to execute on EXEC
    pub queued_commands: Vec<QueuedCommand>,
    /// Keys being watched for changes
    pub watched_keys: HashSet<String>,
    /// Whether any watched key has been modified
    pub watch_broken: bool,
}

/// A command queued during MULTI
#[derive(Debug, Clone)]
pub struct QueuedCommand {
    pub command: String,
    pub args: Vec<RespValue>,
}

impl TransactionState {
    pub fn new() -> Self {
        Self {
            in_transaction: false,
            queued_commands: Vec::new(),
            watched_keys: HashSet::new(),
            watch_broken: false,
        }
    }

    /// Start a transaction (MULTI command)
    pub fn start_transaction(&mut self) -> ProtocolResult<RespValue> {
        if self.in_transaction {
            return Err(ProtocolError::RespError(
                "ERR MULTI calls can not be nested".to_string(),
            ));
        }

        self.in_transaction = true;
        self.queued_commands.clear();
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// Queue a command for execution
    pub fn queue_command(&mut self, command: String, args: Vec<RespValue>) -> RespValue {
        if !self.in_transaction {
            // This shouldn't happen if the protocol handler is correct
            return RespValue::Error("ERR QUEUED without MULTI".to_string());
        }

        self.queued_commands.push(QueuedCommand { command, args });
        RespValue::SimpleString("QUEUED".to_string())
    }

/// Discard the transaction (DISCARD command)
    pub fn discard_transaction(&mut self) -> ProtocolResult<RespValue> {
        if !self.in_transaction {
            return Err(ProtocolError::RespError(
                "ERR DISCARD without MULTI".to_string(),
            ));
        }

        self.in_transaction = false;
        self.queued_commands.clear();
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// Watch keys for changes (WATCH command)
    pub fn watch_keys(&mut self, keys: Vec<String>) -> ProtocolResult<RespValue> {
        if self.in_transaction {
            return Err(ProtocolError::RespError(
                "ERR WATCH inside MULTI is not allowed".to_string(),
            ));
        }

        for key in keys {
            self.watched_keys.insert(key);
        }
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// Unwatch all keys (UNWATCH command)
    pub fn unwatch_all(&mut self) -> RespValue {
        self.watched_keys.clear();
        self.watch_broken = false;
        RespValue::SimpleString("OK".to_string())
    }

    /// Mark a key as modified (called when any watched key changes)
    pub fn mark_key_modified(&mut self, key: &str) {
        if self.watched_keys.contains(key) {
            self.watch_broken = true;
        }
    }

    /// Check if transaction can proceed (no watched keys modified)
    pub fn can_execute(&self) -> bool {
        !self.watch_broken
    }

    /// Reset transaction state after EXEC
    pub fn reset_after_exec(&mut self) {
        self.in_transaction = false;
        self.queued_commands.clear();
        self.watched_keys.clear();
        self.watch_broken = false;
    }
}

/// Global transaction manager to track key modifications across connections
pub struct TransactionManager {
    /// Map of connection ID to transaction state
    transactions: Arc<RwLock<HashMap<String, TransactionState>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create transaction state for a connection
    pub async fn get_or_create(&self, conn_id: &str) -> TransactionState {
        let mut txns = self.transactions.write().await;
        txns.entry(conn_id.to_string())
            .or_insert_with(TransactionState::new)
            .clone()
    }

    /// Update transaction state for a connection
    pub async fn update(&self, conn_id: &str, state: TransactionState) {
        let mut txns = self.transactions.write().await;
        txns.insert(conn_id.to_string(), state);
    }

    /// Notify all connections watching a key that it has been modified
    pub async fn notify_key_modified(&self, key: &str) {
        let mut txns = self.transactions.write().await;
        for state in txns.values_mut() {
            state.mark_key_modified(key);
        }
    }

    /// Remove transaction state for a connection (on disconnect)
    pub async fn remove(&self, conn_id: &str) {
        let mut txns = self.transactions.write().await;
        txns.remove(conn_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_lifecycle() {
        let mut state = TransactionState::new();

        // Start transaction
        assert!(!state.in_transaction);
        let result = state.start_transaction();
        assert!(result.is_ok());
        assert!(state.in_transaction);

        // Queue commands
        state.queue_command("SET".to_string(), vec![]);
        state.queue_command("GET".to_string(), vec![]);
        assert_eq!(state.queued_commands.len(), 2);

        // Discard
        let result = state.discard_transaction();
        assert!(result.is_ok());
        assert!(!state.in_transaction);
        assert_eq!(state.queued_commands.len(), 0);
    }

    #[test]
    fn test_watch_functionality() {
        let mut state = TransactionState::new();

        // Watch keys
        let result = state.watch_keys(vec!["key1".to_string(), "key2".to_string()]);
        assert!(result.is_ok());
        assert_eq!(state.watched_keys.len(), 2);
        assert!(state.can_execute());

        // Modify watched key
        state.mark_key_modified("key1");
        assert!(!state.can_execute());

        // Unwatch
        state.unwatch_all();
        assert_eq!(state.watched_keys.len(), 0);
        assert!(!state.watch_broken);
    }

    #[test]
    fn test_nested_multi_error() {
        let mut state = TransactionState::new();
        state.start_transaction().unwrap();

        // Try to start another transaction
        let result = state.start_transaction();
        assert!(result.is_err());
    }

    #[test]
    fn test_watch_inside_multi_error() {
        let mut state = TransactionState::new();
        state.start_transaction().unwrap();

        // Try to WATCH inside MULTI
        let result = state.watch_keys(vec!["key1".to_string()]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transaction_manager() {
        let manager = TransactionManager::new();

        // Get state for connection
        let state = manager.get_or_create("conn1").await;
        assert!(!state.in_transaction);

        // Update state
        let mut new_state = state.clone();
        new_state.start_transaction().unwrap();
        manager.update("conn1", new_state).await;

        // Verify update
        let retrieved = manager.get_or_create("conn1").await;
        assert!(retrieved.in_transaction);

        // Remove
        manager.remove("conn1").await;
    }
}
