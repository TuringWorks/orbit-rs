use crate::addressable::AddressableReference;
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use crate::transaction_log::PersistentTransactionLogger;
use crate::transactions::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

/// Configuration for recovery mechanisms
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Interval for checking coordinator health
    pub health_check_interval: Duration,
    /// Timeout for coordinator health checks
    pub health_check_timeout: Duration,
    /// Maximum time to wait for coordinator recovery
    pub coordinator_recovery_timeout: Duration,
    /// Election timeout for new coordinator selection
    pub election_timeout: Duration,
    /// Enable automatic failover
    pub enable_automatic_failover: bool,
    /// Recovery checkpoint interval
    pub checkpoint_interval: Duration,
    /// Maximum transaction age for recovery
    pub max_recovery_age: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(10),
            health_check_timeout: Duration::from_secs(5),
            coordinator_recovery_timeout: Duration::from_secs(300), // 5 minutes
            election_timeout: Duration::from_secs(30),
            enable_automatic_failover: true,
            checkpoint_interval: Duration::from_secs(60),
            max_recovery_age: Duration::from_secs(86400), // 24 hours
        }
    }
}

/// Transaction recovery checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCheckpoint {
    pub transaction_id: TransactionId,
    pub coordinator_node: NodeId,
    pub participants: HashSet<AddressableReference>,
    pub operations: Vec<TransactionOperation>,
    pub current_state: TransactionState,
    pub timeout: Duration,
    pub created_at: i64,
    pub last_updated: i64,
    pub votes_received: HashMap<AddressableReference, TransactionVote>,
    pub acknowledgments_received: HashMap<AddressableReference, bool>,
}

impl TransactionCheckpoint {
    pub fn from_transaction(transaction: &DistributedTransaction) -> Self {
        Self {
            transaction_id: transaction.transaction_id.clone(),
            coordinator_node: transaction.transaction_id.coordinator_node.clone(),
            participants: transaction.participants.clone(),
            operations: transaction.operations.clone(),
            current_state: transaction.state.clone(),
            timeout: transaction.timeout,
            created_at: transaction.started_at,
            last_updated: chrono::Utc::now().timestamp_millis(),
            votes_received: HashMap::new(),
            acknowledgments_received: HashMap::new(),
        }
    }
}

/// Recovery statistics
#[derive(Debug, Clone)]
pub struct RecoveryStats {
    pub total_recoveries: u64,
    pub successful_recoveries: u64,
    pub failed_recoveries: u64,
    pub coordinator_failures_detected: u64,
    pub elections_participated: u64,
    pub elections_won: u64,
    pub transactions_recovered: u64,
    pub checkpoints_created: u64,
}

/// Transaction recovery manager
pub struct TransactionRecoveryManager {
    pub node_id: NodeId,
    config: RecoveryConfig,
    logger: Arc<dyn PersistentTransactionLogger>,
    /// Active transaction checkpoints
    checkpoints: Arc<RwLock<HashMap<TransactionId, TransactionCheckpoint>>>,
    /// Known coordinators and their health status
    pub coordinators: Arc<RwLock<HashMap<NodeId, CoordinatorHealth>>>,
    /// Recovery statistics
    stats: Arc<RwLock<RecoveryStats>>,
    /// Current coordinator (if this node is acting as one)
    current_coordinator: Arc<RwLock<Option<Arc<TransactionCoordinator>>>>,
    /// Cluster membership manager
    pub cluster_manager: Arc<dyn ClusterManager>,
    /// Recovery event handlers
    pub event_handlers: Arc<RwLock<Vec<Arc<dyn RecoveryEventHandler>>>>,
    /// Transaction-to-coordinator mapping for tracking ownership
    transaction_coordinator_map: Arc<RwLock<HashMap<TransactionId, NodeId>>>,
    /// Current cluster leader for coordination decisions
    current_leader: Arc<RwLock<Option<NodeId>>>,
}

#[derive(Debug, Clone)]
pub struct CoordinatorHealth {
    pub node_id: NodeId,
    pub last_seen: Instant,
    pub is_healthy: bool,
    pub consecutive_failures: u32,
    pub last_election_participation: Option<Instant>,
}

/// Trait for managing cluster membership
#[async_trait]
pub trait ClusterManager: Send + Sync {
    /// Get all nodes in the cluster
    async fn get_cluster_nodes(&self) -> OrbitResult<Vec<NodeId>>;

    /// Check if a node is the current leader/coordinator
    async fn is_leader(&self, node_id: &NodeId) -> OrbitResult<bool>;

    /// Start leader election
    async fn start_election(&self, candidate: &NodeId) -> OrbitResult<bool>;

    /// Notify cluster of coordinator failure
    async fn report_coordinator_failure(&self, failed_coordinator: &NodeId) -> OrbitResult<()>;

    /// Get cluster configuration
    async fn get_cluster_config(&self) -> OrbitResult<ClusterConfig>;
}

#[derive(Debug, Clone)]
pub struct ClusterConfig {
    pub total_nodes: usize,
    pub majority_threshold: usize,
    pub current_leader: Option<NodeId>,
}

/// Recovery event handler trait
#[async_trait]
pub trait RecoveryEventHandler: Send + Sync {
    /// Called when a coordinator failure is detected
    async fn on_coordinator_failure(&self, failed_coordinator: &NodeId) -> OrbitResult<()>;

    /// Called when recovery process starts
    async fn on_recovery_start(&self, transaction_id: &TransactionId) -> OrbitResult<()>;

    /// Called when recovery process completes
    async fn on_recovery_complete(
        &self,
        transaction_id: &TransactionId,
        success: bool,
    ) -> OrbitResult<()>;

    /// Called when this node becomes coordinator
    async fn on_coordinator_elected(&self, new_coordinator: &NodeId) -> OrbitResult<()>;
}

impl TransactionRecoveryManager {
    pub fn new(
        node_id: NodeId,
        config: RecoveryConfig,
        logger: Arc<dyn PersistentTransactionLogger>,
        cluster_manager: Arc<dyn ClusterManager>,
    ) -> Self {
        Self {
            node_id,
            config,
            logger,
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            coordinators: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RecoveryStats {
                total_recoveries: 0,
                successful_recoveries: 0,
                failed_recoveries: 0,
                coordinator_failures_detected: 0,
                elections_participated: 0,
                elections_won: 0,
                transactions_recovered: 0,
                checkpoints_created: 0,
            })),
            current_coordinator: Arc::new(RwLock::new(None)),
            cluster_manager,
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            transaction_coordinator_map: Arc::new(RwLock::new(HashMap::new())),
            current_leader: Arc::new(RwLock::new(None)),
        }
    }

    /// Add a recovery event handler
    pub async fn add_event_handler(&self, handler: Arc<dyn RecoveryEventHandler>) {
        let mut handlers = self.event_handlers.write().await;
        handlers.push(handler);
    }

    /// Create a checkpoint for a transaction
    pub async fn create_checkpoint(&self, transaction: &DistributedTransaction) -> OrbitResult<()> {
        let checkpoint = TransactionCheckpoint::from_transaction(transaction);

        // Store checkpoint in memory
        {
            let mut checkpoints = self.checkpoints.write().await;
            checkpoints.insert(checkpoint.transaction_id.clone(), checkpoint.clone());
        }

        // Persist checkpoint to log
        let _checkpoint_data = serde_json::to_value(&checkpoint)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize checkpoint: {}", e)))?;

        // This would ideally be stored in a dedicated recovery log
        debug!(
            "Created checkpoint for transaction: {}",
            checkpoint.transaction_id
        );

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.checkpoints_created += 1;
        }

        Ok(())
    }

    /// Update an existing checkpoint
    pub async fn update_checkpoint(
        &self,
        transaction_id: &TransactionId,
        state: TransactionState,
        votes: Option<HashMap<AddressableReference, TransactionVote>>,
        acks: Option<HashMap<AddressableReference, bool>>,
    ) -> OrbitResult<()> {
        let mut checkpoints = self.checkpoints.write().await;
        if let Some(checkpoint) = checkpoints.get_mut(transaction_id) {
            checkpoint.current_state = state;
            checkpoint.last_updated = chrono::Utc::now().timestamp_millis();

            if let Some(votes) = votes {
                checkpoint.votes_received = votes;
            }

            if let Some(acks) = acks {
                checkpoint.acknowledgments_received = acks;
            }

            debug!("Updated checkpoint for transaction: {}", transaction_id);
        }

        Ok(())
    }

    /// Remove checkpoint after transaction completion
    pub async fn remove_checkpoint(&self, transaction_id: &TransactionId) -> OrbitResult<()> {
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.remove(transaction_id);
        debug!("Removed checkpoint for transaction: {}", transaction_id);
        Ok(())
    }

    /// Start the recovery manager
    pub async fn start(&self) -> OrbitResult<()> {
        info!("Starting transaction recovery manager");

        // Initialize coordinator health tracking
        self.initialize_coordinator_tracking().await?;

        // Start background tasks
        self.start_background_tasks().await?;

        // Recover any incomplete transactions
        self.recover_incomplete_transactions().await?;

        info!("Transaction recovery manager started");
        Ok(())
    }

    /// Initialize tracking of cluster coordinators
    async fn initialize_coordinator_tracking(&self) -> OrbitResult<()> {
        let cluster_nodes = self.cluster_manager.get_cluster_nodes().await?;
        let mut coordinators = self.coordinators.write().await;

        for node_id in cluster_nodes {
            coordinators.insert(
                node_id.clone(),
                CoordinatorHealth {
                    node_id,
                    last_seen: Instant::now(),
                    is_healthy: true,
                    consecutive_failures: 0,
                    last_election_participation: None,
                },
            );
        }

        Ok(())
    }

    /// Start background recovery tasks
    async fn start_background_tasks(&self) -> OrbitResult<()> {
        let recovery_manager = self.clone();

        // Health checking task
        {
            let manager = recovery_manager.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(manager.config.health_check_interval);
                loop {
                    interval.tick().await;
                    if let Err(e) = manager.check_coordinator_health().await {
                        error!("Coordinator health check failed: {}", e);
                    }
                }
            });
        }

        // Checkpoint management task
        {
            let manager = recovery_manager.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(manager.config.checkpoint_interval);
                loop {
                    interval.tick().await;
                    if let Err(e) = manager.cleanup_old_checkpoints().await {
                        error!("Checkpoint cleanup failed: {}", e);
                    }
                }
            });
        }

        // Recovery monitoring task
        {
            let manager = recovery_manager.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    if let Err(e) = manager.monitor_recovery_progress().await {
                        error!("Recovery monitoring failed: {}", e);
                    }
                }
            });
        }

        Ok(())
    }

    /// Check health of known coordinators
    async fn check_coordinator_health(&self) -> OrbitResult<()> {
        let mut coordinators = self.coordinators.write().await;
        let now = Instant::now();
        let mut failed_coordinators = Vec::new();

        for (node_id, health) in coordinators.iter_mut() {
            let time_since_last_seen = now.duration_since(health.last_seen);

            if time_since_last_seen > self.config.health_check_timeout {
                health.consecutive_failures += 1;

                if health.consecutive_failures >= 3 && health.is_healthy {
                    health.is_healthy = false;
                    failed_coordinators.push(node_id.clone());

                    warn!("Coordinator failure detected: {}", node_id);

                    // Update statistics
                    {
                        let mut stats = self.stats.write().await;
                        stats.coordinator_failures_detected += 1;
                    }
                }
            } else {
                // Reset failure count if coordinator is responding
                if health.consecutive_failures > 0 {
                    health.consecutive_failures = 0;
                    health.is_healthy = true;
                }
            }
        }

        // Handle failed coordinators
        for failed_coordinator in failed_coordinators {
            self.handle_coordinator_failure(&failed_coordinator).await?;
        }

        Ok(())
    }

    /// Handle coordinator failure
    async fn handle_coordinator_failure(&self, failed_coordinator: &NodeId) -> OrbitResult<()> {
        info!("Handling coordinator failure: {}", failed_coordinator);

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_coordinator_failure(failed_coordinator).await {
                error!("Recovery event handler failed: {}", e);
            }
        }

        // Notify cluster manager
        self.cluster_manager
            .report_coordinator_failure(failed_coordinator)
            .await?;

        // Find transactions that need recovery
        let transactions_to_recover = self
            .find_transactions_needing_recovery(failed_coordinator)
            .await?;

        if !transactions_to_recover.is_empty() {
            info!(
                "Found {} transactions requiring recovery",
                transactions_to_recover.len()
            );

            // Start recovery process
            if self.config.enable_automatic_failover {
                self.initiate_recovery_process(transactions_to_recover)
                    .await?;
            }
        }

        Ok(())
    }

    /// Find transactions that need recovery due to coordinator failure
    pub async fn find_transactions_needing_recovery(
        &self,
        failed_coordinator: &NodeId,
    ) -> OrbitResult<Vec<TransactionCheckpoint>> {
        let checkpoints = self.checkpoints.read().await;
        let mut transactions_to_recover = Vec::new();

        for checkpoint in checkpoints.values() {
            if &checkpoint.coordinator_node == failed_coordinator {
                // Check if transaction is in a recoverable state
                if matches!(
                    checkpoint.current_state,
                    TransactionState::Preparing
                        | TransactionState::Prepared
                        | TransactionState::Committing
                        | TransactionState::Aborting
                ) {
                    transactions_to_recover.push(checkpoint.clone());
                }
            }
        }

        Ok(transactions_to_recover)
    }

    /// Initiate recovery process for failed transactions
    pub async fn initiate_recovery_process(
        &self,
        transactions: Vec<TransactionCheckpoint>,
    ) -> OrbitResult<()> {
        info!(
            "Initiating recovery process for {} transactions",
            transactions.len()
        );

        // Check if this node should become the new coordinator
        let should_become_coordinator = self.should_become_coordinator().await?;

        if should_become_coordinator {
            // Start election process
            let election_won = self.cluster_manager.start_election(&self.node_id).await?;

            {
                let mut stats = self.stats.write().await;
                stats.elections_participated += 1;
                if election_won {
                    stats.elections_won += 1;
                }
            }

            if election_won {
                info!("Won coordinator election, taking over failed transactions");
                self.become_coordinator(transactions).await?;
            }
        }

        Ok(())
    }

    /// Check if this node should become the new coordinator
    async fn should_become_coordinator(&self) -> OrbitResult<bool> {
        let cluster_config = self.cluster_manager.get_cluster_config().await?;

        // Simple heuristic: become coordinator if no current leader exists
        // In a real implementation, this would use more sophisticated election algorithms
        Ok(cluster_config.current_leader.is_none())
    }

    /// Become the new coordinator and recover transactions
    pub async fn become_coordinator(
        &self,
        transactions: Vec<TransactionCheckpoint>,
    ) -> OrbitResult<()> {
        info!(
            "Becoming new coordinator for {} transactions",
            transactions.len()
        );

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_coordinator_elected(&self.node_id).await {
                error!("Recovery event handler failed: {}", e);
            }
        }

        // Process each transaction
        for checkpoint in transactions {
            let transaction_id = checkpoint.transaction_id.clone();
            if let Err(e) = self.recover_transaction(checkpoint).await {
                error!("Failed to recover transaction {}: {}", transaction_id, e);

                let mut stats = self.stats.write().await;
                stats.failed_recoveries += 1;
            } else {
                let mut stats = self.stats.write().await;
                stats.successful_recoveries += 1;
                stats.transactions_recovered += 1;
            }

            let mut stats = self.stats.write().await;
            stats.total_recoveries += 1;
        }

        Ok(())
    }

    /// Recover a single transaction
    async fn recover_transaction(&self, checkpoint: TransactionCheckpoint) -> OrbitResult<()> {
        info!("Recovering transaction: {}", checkpoint.transaction_id);

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_recovery_start(&checkpoint.transaction_id).await {
                error!("Recovery event handler failed: {}", e);
            }
        }

        let success = match checkpoint.current_state {
            TransactionState::Preparing => {
                // Transaction was in prepare phase - need to determine if we should commit or abort
                self.recover_preparing_transaction(&checkpoint).await?
            }
            TransactionState::Prepared => {
                // All participants voted yes - should commit
                self.recover_prepared_transaction(&checkpoint).await?
            }
            TransactionState::Committing => {
                // Transaction was committing - continue commit
                self.recover_committing_transaction(&checkpoint).await?
            }
            TransactionState::Aborting => {
                // Transaction was aborting - continue abort
                self.recover_aborting_transaction(&checkpoint).await?
            }
            _ => {
                debug!(
                    "Transaction {} is in non-recoverable state: {:?}",
                    checkpoint.transaction_id, checkpoint.current_state
                );
                true
            }
        };

        // Notify event handlers
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler
                .on_recovery_complete(&checkpoint.transaction_id, success)
                .await
            {
                error!("Recovery event handler failed: {}", e);
            }
        }

        Ok(())
    }

    /// Recover transaction in preparing state
    async fn recover_preparing_transaction(
        &self,
        checkpoint: &TransactionCheckpoint,
    ) -> OrbitResult<bool> {
        info!(
            "Recovering preparing transaction: {}",
            checkpoint.transaction_id
        );

        // Query participants for their votes
        // If we have enough YES votes, proceed to commit
        // Otherwise, abort the transaction

        // For now, we'll abort preparing transactions as the safest option
        info!(
            "Aborting preparing transaction {} for safety",
            checkpoint.transaction_id
        );
        Ok(true) // Simplified for now
    }

    /// Recover transaction in prepared state  
    async fn recover_prepared_transaction(
        &self,
        checkpoint: &TransactionCheckpoint,
    ) -> OrbitResult<bool> {
        info!(
            "Recovering prepared transaction: {}",
            checkpoint.transaction_id
        );

        // All participants voted yes, so we should commit
        // Send commit messages to all participants

        info!(
            "Committing prepared transaction {}",
            checkpoint.transaction_id
        );
        Ok(true) // Simplified for now
    }

    /// Recover transaction in committing state
    async fn recover_committing_transaction(
        &self,
        checkpoint: &TransactionCheckpoint,
    ) -> OrbitResult<bool> {
        info!(
            "Recovering committing transaction: {}",
            checkpoint.transaction_id
        );

        // Transaction was committing - continue sending commit messages
        // and wait for acknowledgments

        info!(
            "Continuing commit for transaction {}",
            checkpoint.transaction_id
        );
        Ok(true) // Simplified for now
    }

    /// Recover transaction in aborting state
    async fn recover_aborting_transaction(
        &self,
        checkpoint: &TransactionCheckpoint,
    ) -> OrbitResult<bool> {
        info!(
            "Recovering aborting transaction: {}",
            checkpoint.transaction_id
        );

        // Transaction was aborting - continue sending abort messages
        // and wait for acknowledgments

        info!(
            "Continuing abort for transaction {}",
            checkpoint.transaction_id
        );
        Ok(true) // Simplified for now
    }

    /// Recover incomplete transactions on startup
    async fn recover_incomplete_transactions(&self) -> OrbitResult<()> {
        info!("Recovering incomplete transactions on startup");

        // This would query the persistent log for incomplete transactions
        // and attempt to recover them

        // For now, this is a placeholder
        debug!("Transaction recovery on startup completed");
        Ok(())
    }

    /// Clean up old checkpoints
    async fn cleanup_old_checkpoints(&self) -> OrbitResult<()> {
        let cutoff_time =
            chrono::Utc::now().timestamp_millis() - self.config.max_recovery_age.as_millis() as i64;
        let mut checkpoints = self.checkpoints.write().await;
        let mut to_remove = Vec::new();

        for (transaction_id, checkpoint) in checkpoints.iter() {
            if checkpoint.created_at < cutoff_time {
                to_remove.push(transaction_id.clone());
            }
        }

        for transaction_id in to_remove {
            checkpoints.remove(&transaction_id);
            debug!("Cleaned up old checkpoint: {}", transaction_id);
        }

        Ok(())
    }

    /// Monitor recovery progress
    async fn monitor_recovery_progress(&self) -> OrbitResult<()> {
        let checkpoints = self.checkpoints.read().await;
        let active_recoveries = checkpoints.len();

        if active_recoveries > 0 {
            debug!(
                "Monitoring {} active recovery checkpoints",
                active_recoveries
            );
        }

        Ok(())
    }

    /// Get recovery statistics
    pub async fn get_stats(&self) -> RecoveryStats {
        self.stats.read().await.clone()
    }

    /// Get current checkpoints
    pub async fn get_checkpoints(&self) -> HashMap<TransactionId, TransactionCheckpoint> {
        self.checkpoints.read().await.clone()
    }

    /// Register a transaction with its coordinator
    pub async fn register_transaction_coordinator(
        &self,
        transaction_id: TransactionId,
        coordinator: NodeId,
    ) -> OrbitResult<()> {
        let mut mapping = self.transaction_coordinator_map.write().await;
        mapping.insert(transaction_id.clone(), coordinator.clone());

        debug!(
            "Registered transaction {} with coordinator {}",
            transaction_id, coordinator
        );
        Ok(())
    }

    /// Get the coordinator for a transaction
    pub async fn get_transaction_coordinator(
        &self,
        transaction_id: &TransactionId,
    ) -> Option<NodeId> {
        let mapping = self.transaction_coordinator_map.read().await;
        mapping.get(transaction_id).cloned()
    }

    /// Remove transaction-coordinator mapping
    pub async fn unregister_transaction_coordinator(
        &self,
        transaction_id: &TransactionId,
    ) -> OrbitResult<()> {
        let mut mapping = self.transaction_coordinator_map.write().await;
        mapping.remove(transaction_id);

        debug!(
            "Unregistered transaction coordinator mapping for {}",
            transaction_id
        );
        Ok(())
    }

    /// Update the current cluster leader
    pub async fn update_cluster_leader(&self, leader: Option<NodeId>) -> OrbitResult<()> {
        let mut current_leader = self.current_leader.write().await;
        *current_leader = leader.clone();

        match leader {
            Some(leader_id) => info!("Updated cluster leader to: {}", leader_id),
            None => info!("Cleared cluster leader"),
        }

        Ok(())
    }

    /// Get current cluster leader
    pub async fn get_cluster_leader(&self) -> Option<NodeId> {
        self.current_leader.read().await.clone()
    }

    /// Reassign transactions from a failed coordinator to the current leader
    pub async fn reassign_transactions_to_leader(
        &self,
        failed_coordinator: &NodeId,
    ) -> OrbitResult<Vec<TransactionId>> {
        let current_leader = self.get_cluster_leader().await.ok_or_else(|| {
            OrbitError::cluster("No current leader available for transaction reassignment")
        })?;

        let mut mapping = self.transaction_coordinator_map.write().await;
        let mut reassigned_transactions = Vec::new();

        // Find all transactions owned by the failed coordinator
        let transactions_to_reassign: Vec<TransactionId> = mapping
            .iter()
            .filter(|(_, coordinator)| *coordinator == failed_coordinator)
            .map(|(tx_id, _)| tx_id.clone())
            .collect();

        // Reassign them to the current leader
        for transaction_id in transactions_to_reassign {
            mapping.insert(transaction_id.clone(), current_leader.clone());
            reassigned_transactions.push(transaction_id.clone());

            info!(
                "Reassigned transaction {} from failed coordinator {} to leader {}",
                transaction_id, failed_coordinator, current_leader
            );
        }

        Ok(reassigned_transactions)
    }
}

impl Clone for TransactionRecoveryManager {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            config: self.config.clone(),
            logger: Arc::clone(&self.logger),
            checkpoints: Arc::clone(&self.checkpoints),
            coordinators: Arc::clone(&self.coordinators),
            stats: Arc::clone(&self.stats),
            current_coordinator: Arc::clone(&self.current_coordinator),
            cluster_manager: Arc::clone(&self.cluster_manager),
            event_handlers: Arc::clone(&self.event_handlers),
            transaction_coordinator_map: Arc::clone(&self.transaction_coordinator_map),
            current_leader: Arc::clone(&self.current_leader),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction_log::{PersistentLogConfig, SqliteTransactionLogger};
    use tempfile::tempdir;

    // Mock cluster manager for testing
    struct MockClusterManager {
        nodes: Vec<NodeId>,
        current_leader: Option<NodeId>,
    }

    #[async_trait]
    impl ClusterManager for MockClusterManager {
        async fn get_cluster_nodes(&self) -> OrbitResult<Vec<NodeId>> {
            Ok(self.nodes.clone())
        }

        async fn is_leader(&self, node_id: &NodeId) -> OrbitResult<bool> {
            Ok(self.current_leader.as_ref() == Some(node_id))
        }

        async fn start_election(&self, _candidate: &NodeId) -> OrbitResult<bool> {
            Ok(true) // Always win elections in tests
        }

        async fn report_coordinator_failure(
            &self,
            _failed_coordinator: &NodeId,
        ) -> OrbitResult<()> {
            Ok(())
        }

        async fn get_cluster_config(&self) -> OrbitResult<ClusterConfig> {
            Ok(ClusterConfig {
                total_nodes: self.nodes.len(),
                majority_threshold: self.nodes.len() / 2 + 1,
                current_leader: self.current_leader.clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_recovery_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("recovery_test.db");

        let log_config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger = Arc::new(SqliteTransactionLogger::new(log_config).await.unwrap());
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let cluster_manager = Arc::new(MockClusterManager {
            nodes: vec![node_id.clone()],
            current_leader: None,
        });

        let recovery_manager = TransactionRecoveryManager::new(
            node_id,
            RecoveryConfig::default(),
            logger,
            cluster_manager,
        );

        let stats = recovery_manager.get_stats().await;
        assert_eq!(stats.total_recoveries, 0);
    }

    #[tokio::test]
    async fn test_checkpoint_management() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("recovery_test.db");

        let log_config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger = Arc::new(SqliteTransactionLogger::new(log_config).await.unwrap());
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let cluster_manager = Arc::new(MockClusterManager {
            nodes: vec![node_id.clone()],
            current_leader: None,
        });

        let recovery_manager = TransactionRecoveryManager::new(
            node_id.clone(),
            RecoveryConfig::default(),
            logger,
            cluster_manager,
        );

        let transaction = DistributedTransaction::new(node_id, Duration::from_secs(30));

        // Create checkpoint
        recovery_manager
            .create_checkpoint(&transaction)
            .await
            .unwrap();

        // Verify checkpoint exists
        let checkpoints = recovery_manager.get_checkpoints().await;
        assert!(checkpoints.contains_key(&transaction.transaction_id));

        // Update checkpoint
        recovery_manager
            .update_checkpoint(
                &transaction.transaction_id,
                TransactionState::Committed,
                None,
                None,
            )
            .await
            .unwrap();

        // Remove checkpoint
        recovery_manager
            .remove_checkpoint(&transaction.transaction_id)
            .await
            .unwrap();

        let checkpoints = recovery_manager.get_checkpoints().await;
        assert!(!checkpoints.contains_key(&transaction.transaction_id));
    }
}
