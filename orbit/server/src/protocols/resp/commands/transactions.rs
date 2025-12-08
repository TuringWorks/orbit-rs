//! Redis transaction commands (MULTI/EXEC/DISCARD/WATCH/UNWATCH)
//!
//! Implements Redis transaction commands for atomic execution of command sequences.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::resp::simple_local::SimpleLocalRegistry;
use crate::protocols::resp::transactions::{TransactionManager, TransactionState};
use crate::protocols::resp::types::RespValue;
use async_trait::async_trait;
use orbit_client::OrbitClient;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Transaction commands handler
pub struct TransactionCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
    /// Transaction manager for global key watch notifications
    transaction_manager: Arc<TransactionManager>,
    /// Per-connection transaction state (keyed by connection ID)
    /// Note: The main handler will need to pass connection context
    connection_states: Arc<RwLock<std::collections::HashMap<String, TransactionState>>>,
}

impl TransactionCommands {
    /// Create a new transaction commands handler
    pub fn new(orbit_client: Arc<OrbitClient>, local_registry: Arc<SimpleLocalRegistry>) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
            transaction_manager: Arc::new(TransactionManager::new()),
            connection_states: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Create a new transaction commands handler with shared transaction manager
    pub fn with_transaction_manager(
        orbit_client: Arc<OrbitClient>,
        local_registry: Arc<SimpleLocalRegistry>,
        transaction_manager: Arc<TransactionManager>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
            transaction_manager,
            connection_states: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get the transaction manager for external notifications
    pub fn transaction_manager(&self) -> &Arc<TransactionManager> {
        &self.transaction_manager
    }

    /// Get or create transaction state for a connection
    pub async fn get_state(&self, conn_id: &str) -> TransactionState {
        let states = self.connection_states.read().await;
        states
            .get(conn_id)
            .cloned()
            .unwrap_or_else(TransactionState::new)
    }

    /// Update transaction state for a connection
    pub async fn set_state(&self, conn_id: &str, state: TransactionState) {
        let mut states = self.connection_states.write().await;
        states.insert(conn_id.to_string(), state);
    }

    /// Check if connection is in a transaction
    pub async fn is_in_transaction(&self, conn_id: &str) -> bool {
        let states = self.connection_states.read().await;
        states
            .get(conn_id)
            .map(|s| s.in_transaction)
            .unwrap_or(false)
    }

    /// Remove connection state (on disconnect)
    pub async fn remove_connection(&self, conn_id: &str) {
        let mut states = self.connection_states.write().await;
        states.remove(conn_id);
        self.transaction_manager.remove(conn_id).await;
    }

    /// Handle MULTI command
    async fn cmd_multi(&self, conn_id: &str) -> ProtocolResult<RespValue> {
        let mut state = self.get_state(conn_id).await;
        let result = state.start_transaction()?;
        self.set_state(conn_id, state).await;
        Ok(result)
    }

    /// Handle EXEC command
    /// Note: This returns the queued commands for the caller to execute
    pub async fn cmd_exec(
        &self,
        conn_id: &str,
    ) -> ProtocolResult<(bool, Vec<(String, Vec<RespValue>)>)> {
        let state = self.get_state(conn_id).await;

        if !state.in_transaction {
            return Err(ProtocolError::RespError(
                "ERR EXEC without MULTI".to_string(),
            ));
        }

        // Check if watch was broken
        if !state.can_execute() {
            // Clear state and return nil
            let mut new_state = state.clone();
            new_state.reset_after_exec();
            self.set_state(conn_id, new_state).await;
            return Ok((false, vec![])); // Returns nil array (watch broken)
        }

        // Extract queued commands
        let commands: Vec<(String, Vec<RespValue>)> = state
            .queued_commands
            .iter()
            .map(|q| (q.command.clone(), q.args.clone()))
            .collect();

        // Reset state after exec
        let mut new_state = state.clone();
        new_state.reset_after_exec();
        self.set_state(conn_id, new_state).await;

        Ok((true, commands))
    }

    /// Handle DISCARD command
    async fn cmd_discard(&self, conn_id: &str) -> ProtocolResult<RespValue> {
        let mut state = self.get_state(conn_id).await;
        let result = state.discard_transaction()?;
        self.set_state(conn_id, state).await;
        Ok(result)
    }

    /// Handle WATCH command
    async fn cmd_watch(&self, conn_id: &str, keys: Vec<String>) -> ProtocolResult<RespValue> {
        let mut state = self.get_state(conn_id).await;
        let result = state.watch_keys(keys)?;
        self.set_state(conn_id, state.clone()).await;
        // Also register with global manager
        self.transaction_manager.update(conn_id, state).await;
        Ok(result)
    }

    /// Handle UNWATCH command
    async fn cmd_unwatch(&self, conn_id: &str) -> RespValue {
        let mut state = self.get_state(conn_id).await;
        let result = state.unwatch_all();
        self.set_state(conn_id, state.clone()).await;
        // Also update global manager
        self.transaction_manager.update(conn_id, state).await;
        result
    }

    /// Queue a command during MULTI transaction
    pub async fn queue_command(
        &self,
        conn_id: &str,
        command: String,
        args: Vec<RespValue>,
    ) -> RespValue {
        let mut state = self.get_state(conn_id).await;
        let result = state.queue_command(command, args);
        self.set_state(conn_id, state).await;
        result
    }

    /// Notify that a key has been modified (for WATCH support)
    pub async fn notify_key_modified(&self, key: &str) {
        // Update local states
        let mut states = self.connection_states.write().await;
        for state in states.values_mut() {
            state.mark_key_modified(key);
        }
        // Also notify global manager
        self.transaction_manager.notify_key_modified(key).await;
    }
}

#[async_trait]
impl CommandHandler for TransactionCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // For transaction commands, we need a connection ID
        // The default connection ID is used when not passed through context
        let conn_id = "default";

        match command_name {
            "MULTI" => {
                self.validate_arg_count(command_name, args, 0)?;
                self.cmd_multi(conn_id).await
            }
            "EXEC" => {
                self.validate_arg_count(command_name, args, 0)?;
                // EXEC needs special handling by the server to execute queued commands
                // Return an error for now - proper execution handled by server
                let (can_exec, commands) = self.cmd_exec(conn_id).await?;
                if !can_exec {
                    // Watch was broken
                    Ok(RespValue::Null)
                } else if commands.is_empty() {
                    // No commands queued
                    Ok(RespValue::Array(vec![]))
                } else {
                    // This shouldn't be reached in normal flow - server intercepts EXEC
                    // Return placeholder indicating commands need execution
                    Ok(RespValue::SimpleString(format!(
                        "EXEC_PENDING:{}",
                        commands.len()
                    )))
                }
            }
            "DISCARD" => {
                self.validate_arg_count(command_name, args, 0)?;
                self.cmd_discard(conn_id).await
            }
            "WATCH" => {
                if args.is_empty() {
                    return Err(ProtocolError::RespError(
                        "ERR wrong number of arguments for 'watch' command".to_string(),
                    ));
                }
                let keys: Vec<String> = args.iter().filter_map(|v| v.as_string()).collect();
                if keys.is_empty() {
                    return Err(ProtocolError::RespError(
                        "ERR wrong number of arguments for 'watch' command".to_string(),
                    ));
                }
                self.cmd_watch(conn_id, keys).await
            }
            "UNWATCH" => {
                self.validate_arg_count(command_name, args, 0)?;
                Ok(self.cmd_unwatch(conn_id).await)
            }
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown command '{}'",
                command_name.to_lowercase()
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["MULTI", "EXEC", "DISCARD", "WATCH", "UNWATCH"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_handler() -> TransactionCommands {
        let client_config = orbit_client::OrbitClientConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        let orbit_client = OrbitClient::new_offline(client_config).await.unwrap();
        let local_registry = Arc::new(SimpleLocalRegistry::new());
        TransactionCommands::new(Arc::new(orbit_client), local_registry)
    }

    #[tokio::test]
    async fn test_multi_exec_basic() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Start transaction
        let result = handler.cmd_multi(conn_id).await;
        assert!(result.is_ok());
        assert!(handler.is_in_transaction(conn_id).await);

        // Queue some commands
        handler
            .queue_command(conn_id, "SET".to_string(), vec![])
            .await;
        handler
            .queue_command(conn_id, "GET".to_string(), vec![])
            .await;

        // Execute
        let (can_exec, commands) = handler.cmd_exec(conn_id).await.unwrap();
        assert!(can_exec);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].0, "SET");
        assert_eq!(commands[1].0, "GET");

        // Transaction should be cleared
        assert!(!handler.is_in_transaction(conn_id).await);
    }

    #[tokio::test]
    async fn test_multi_discard() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Start transaction
        handler.cmd_multi(conn_id).await.unwrap();
        assert!(handler.is_in_transaction(conn_id).await);

        // Queue command
        handler
            .queue_command(conn_id, "SET".to_string(), vec![])
            .await;

        // Discard
        let result = handler.cmd_discard(conn_id).await;
        assert!(result.is_ok());
        assert!(!handler.is_in_transaction(conn_id).await);
    }

    #[tokio::test]
    async fn test_watch_broken() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Watch a key
        handler
            .cmd_watch(conn_id, vec!["mykey".to_string()])
            .await
            .unwrap();

        // Start transaction
        handler.cmd_multi(conn_id).await.unwrap();

        // Simulate key modification
        handler.notify_key_modified("mykey").await;

        // Queue command
        handler
            .queue_command(conn_id, "SET".to_string(), vec![])
            .await;

        // Execute - should fail due to watch broken
        let (can_exec, commands) = handler.cmd_exec(conn_id).await.unwrap();
        assert!(!can_exec);
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn test_unwatch() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Watch a key
        handler
            .cmd_watch(conn_id, vec!["mykey".to_string()])
            .await
            .unwrap();

        // Unwatch
        handler.cmd_unwatch(conn_id).await;

        // Modify key shouldn't affect transaction now
        handler.notify_key_modified("mykey").await;

        // Start and execute transaction
        handler.cmd_multi(conn_id).await.unwrap();
        handler
            .queue_command(conn_id, "SET".to_string(), vec![])
            .await;
        let (can_exec, _) = handler.cmd_exec(conn_id).await.unwrap();
        assert!(can_exec);
    }

    #[tokio::test]
    async fn test_nested_multi_error() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Start first transaction
        handler.cmd_multi(conn_id).await.unwrap();

        // Try to start nested transaction
        let result = handler.cmd_multi(conn_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exec_without_multi() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Try EXEC without MULTI
        let result = handler.cmd_exec(conn_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_discard_without_multi() {
        let handler = create_test_handler().await;
        let conn_id = "test_conn";

        // Try DISCARD without MULTI
        let result = handler.cmd_discard(conn_id).await;
        assert!(result.is_err());
    }
}
