//! Data synchronization coordinator for cluster-wide consistency
//!
//! This module coordinates data synchronization across cluster nodes,
//! ensuring eventual consistency and handling network partitions.

use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use crate::replication::ReplicationSlotManager;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Synchronization mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SyncMode {
    /// Full synchronization of all data
    Full,
    /// Incremental sync based on LSN
    Incremental,
    /// Selective sync of specific keys
    Selective { keys: Vec<String> },
}

/// Data synchronization status
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    /// Not started
    Idle,
    /// Synchronization in progress
    InProgress,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed { error: String },
}

/// Sync operation details
#[derive(Debug, Clone)]
pub struct SyncOperation {
    pub operation_id: String,
    pub source_node: NodeId,
    pub target_node: NodeId,
    pub mode: SyncMode,
    pub status: SyncStatus,
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
    pub bytes_transferred: u64,
    pub operations_synced: u64,
}

/// Sync statistics
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub total_syncs: u64,
    pub successful_syncs: u64,
    pub failed_syncs: u64,
    pub total_bytes_transferred: u64,
    pub total_operations_synced: u64,
    pub average_sync_duration_ms: u64,
}

/// Configuration for data synchronization
#[derive(Debug, Clone)]
pub struct DataSyncConfig {
    /// Maximum concurrent synchronization operations
    pub max_concurrent_syncs: usize,
    /// Sync timeout
    pub sync_timeout: Duration,
    /// Batch size for incremental sync
    pub batch_size: usize,
    /// Enable compression for data transfer
    pub enable_compression: bool,
    /// Retry count on failure
    pub max_retries: u32,
}

impl Default for DataSyncConfig {
    fn default() -> Self {
        Self {
            max_concurrent_syncs: 5,
            sync_timeout: Duration::from_secs(300),
            batch_size: 1000,
            enable_compression: true,
            max_retries: 3,
        }
    }
}

/// Tracks synchronization state for nodes
#[derive(Debug, Clone)]
struct NodeSyncState {
    node_id: NodeId,
    last_sync_lsn: u64,
    last_sync_time: Option<Instant>,
    is_syncing: bool,
    consecutive_failures: u32,
}

/// Data synchronization coordinator
pub struct DataSyncCoordinator {
    config: DataSyncConfig,
    /// Replication manager for tracking LSNs
    replication_manager: Arc<ReplicationSlotManager>,
    /// Active sync operations
    active_syncs: Arc<RwLock<HashMap<String, SyncOperation>>>,
    /// Node sync states
    node_states: Arc<RwLock<HashMap<NodeId, NodeSyncState>>>,
    /// Statistics
    stats: Arc<RwLock<SyncStats>>,
}

impl DataSyncCoordinator {
    /// Create a new data sync coordinator
    pub fn new(
        config: DataSyncConfig,
        replication_manager: Arc<ReplicationSlotManager>,
    ) -> Self {
        Self {
            config,
            replication_manager,
            active_syncs: Arc::new(RwLock::new(HashMap::new())),
            node_states: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SyncStats::default())),
        }
    }

    /// Register a node for synchronization tracking
    pub async fn register_node(&self, node_id: NodeId) -> OrbitResult<()> {
        let mut states = self.node_states.write().await;
        
        if !states.contains_key(&node_id) {
            info!("Registering node for sync tracking: {}", node_id);
            states.insert(
                node_id.clone(),
                NodeSyncState {
                    node_id,
                    last_sync_lsn: 0,
                    last_sync_time: None,
                    is_syncing: false,
                    consecutive_failures: 0,
                },
            );
        }
        
        Ok(())
    }

    /// Unregister a node
    pub async fn unregister_node(&self, node_id: &NodeId) -> OrbitResult<()> {
        let mut states = self.node_states.write().await;
        states.remove(node_id);
        info!("Unregistered node from sync tracking: {}", node_id);
        Ok(())
    }

    /// Start synchronization from source to target node
    pub async fn start_sync(
        &self,
        source_node: NodeId,
        target_node: NodeId,
        mode: SyncMode,
    ) -> OrbitResult<String> {
        // Check if we're at max concurrent syncs
        {
            let active = self.active_syncs.read().await;
            if active.len() >= self.config.max_concurrent_syncs {
                return Err(OrbitError::internal("Maximum concurrent syncs reached"));
            }
        }

        // Check if target is already syncing
        {
            let mut states = self.node_states.write().await;
            if let Some(state) = states.get_mut(&target_node) {
                if state.is_syncing {
                    return Err(OrbitError::internal(format!(
                        "Node {} is already syncing",
                        target_node
                    )));
                }
                state.is_syncing = true;
            }
        }

        let operation_id = format!("sync-{}", uuid::Uuid::new_v4());
        
        let operation = SyncOperation {
            operation_id: operation_id.clone(),
            source_node: source_node.clone(),
            target_node: target_node.clone(),
            mode: mode.clone(),
            status: SyncStatus::InProgress,
            started_at: Instant::now(),
            completed_at: None,
            bytes_transferred: 0,
            operations_synced: 0,
        };

        // Store operation
        {
            let mut active = self.active_syncs.write().await;
            active.insert(operation_id.clone(), operation.clone());
        }

        info!(
            "Starting sync {} from {} to {} with mode: {:?}",
            operation_id, source_node, target_node, mode
        );

        // Start sync in background
        let coordinator = self.clone();
        let op_id = operation_id.clone();
        tokio::spawn(async move {
            coordinator.execute_sync(op_id).await
        });

        Ok(operation_id)
    }

    /// Execute a synchronization operation
    async fn execute_sync(&self, operation_id: String) -> OrbitResult<()> {
        let mut operation = {
            let active = self.active_syncs.read().await;
            active
                .get(&operation_id)
                .cloned()
                .ok_or_else(|| OrbitError::internal("Sync operation not found"))?
        };

        let result = match &operation.mode {
            SyncMode::Full => self.execute_full_sync(&mut operation).await,
            SyncMode::Incremental => self.execute_incremental_sync(&mut operation).await,
            SyncMode::Selective { keys } => {
                let keys_clone = keys.clone();
                self.execute_selective_sync(&mut operation, &keys_clone).await
            }
        };

        // Update operation status
        match result {
            Ok(_) => {
                info!("Sync {} completed successfully", operation_id);
                operation.status = SyncStatus::Completed;
                operation.completed_at = Some(Instant::now());
                
                // Update node state
                {
                    let mut states = self.node_states.write().await;
                    if let Some(state) = states.get_mut(&operation.target_node) {
                        state.is_syncing = false;
                        state.last_sync_time = Some(Instant::now());
                        state.consecutive_failures = 0;
                    }
                }

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.successful_syncs += 1;
                    stats.total_bytes_transferred += operation.bytes_transferred;
                    stats.total_operations_synced += operation.operations_synced;
                }
            }
            Err(e) => {
                error!("Sync {} failed: {}", operation_id, e);
                operation.status = SyncStatus::Failed {
                    error: e.to_string(),
                };
                operation.completed_at = Some(Instant::now());

                // Update node state
                {
                    let mut states = self.node_states.write().await;
                    if let Some(state) = states.get_mut(&operation.target_node) {
                        state.is_syncing = false;
                        state.consecutive_failures += 1;
                    }
                }

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.failed_syncs += 1;
                }
            }
        }

        // Update operation in active syncs
        {
            let mut active = self.active_syncs.write().await;
            active.insert(operation_id.clone(), operation);
        }

        Ok(())
    }

    /// Execute full synchronization
    async fn execute_full_sync(&self, operation: &mut SyncOperation) -> OrbitResult<()> {
        info!("Executing full sync from {} to {}", 
              operation.source_node, operation.target_node);

        // In a real implementation:
        // 1. Get all data from source node
        // 2. Transfer in batches to target
        // 3. Apply writes on target
        // 4. Verify consistency

        // Simulate sync with progress
        for batch in 0..10 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            let batch_size = 100;
            operation.bytes_transferred += batch_size * 1024;
            operation.operations_synced += batch_size;
            
            debug!("Full sync progress: batch {} of 10", batch + 1);
        }

        Ok(())
    }

    /// Execute incremental synchronization
    async fn execute_incremental_sync(&self, operation: &mut SyncOperation) -> OrbitResult<()> {
        info!("Executing incremental sync from {} to {}", 
              operation.source_node, operation.target_node);

        // Get target's last sync LSN
        let target_lsn = {
            let states = self.node_states.read().await;
            states
                .get(&operation.target_node)
                .map(|s| s.last_sync_lsn)
                .unwrap_or(0)
        };

        // Get current LSN from replication manager
        let current_lsn = self.replication_manager.current_lsn().await;
        
        if current_lsn <= target_lsn {
            info!("Target is up to date, no sync needed");
            return Ok(());
        }

        let operations_to_sync = current_lsn - target_lsn;
        info!("Syncing {} operations", operations_to_sync);

        // In a real implementation:
        // 1. Get operations from source between target_lsn and current_lsn
        // 2. Transfer in batches
        // 3. Apply on target
        // 4. Update LSN

        // Simulate incremental sync
        let batches = (operations_to_sync as usize + self.config.batch_size - 1) / self.config.batch_size;
        for batch in 0..batches {
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            let batch_ops = std::cmp::min(self.config.batch_size as u64, operations_to_sync);
            operation.bytes_transferred += batch_ops * 512; // Avg 512 bytes per op
            operation.operations_synced += batch_ops;
            
            debug!("Incremental sync progress: batch {} of {}", batch + 1, batches);
        }

        // Update target's LSN
        {
            let mut states = self.node_states.write().await;
            if let Some(state) = states.get_mut(&operation.target_node) {
                state.last_sync_lsn = current_lsn;
            }
        }

        Ok(())
    }

    /// Execute selective synchronization
    async fn execute_selective_sync(
        &self,
        operation: &mut SyncOperation,
        keys: &[String],
    ) -> OrbitResult<()> {
        info!("Executing selective sync of {} keys from {} to {}", 
              keys.len(), operation.source_node, operation.target_node);

        // In a real implementation:
        // 1. Get data for specified keys from source
        // 2. Transfer to target
        // 3. Apply updates on target

        // Simulate selective sync
        for key in keys {
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            operation.bytes_transferred += 256; // Avg 256 bytes per key
            operation.operations_synced += 1;
            
            debug!("Synced key: {}", key);
        }

        Ok(())
    }

    /// Get status of a sync operation
    pub async fn get_sync_status(&self, operation_id: &str) -> Option<SyncOperation> {
        let active = self.active_syncs.read().await;
        active.get(operation_id).cloned()
    }

    /// Get all active sync operations
    pub async fn get_active_syncs(&self) -> Vec<SyncOperation> {
        let active = self.active_syncs.read().await;
        active.values().cloned().collect()
    }

    /// Cancel a sync operation
    pub async fn cancel_sync(&self, operation_id: &str) -> OrbitResult<()> {
        let mut active = self.active_syncs.write().await;
        
        if let Some(mut operation) = active.get_mut(operation_id) {
            info!("Cancelling sync operation: {}", operation_id);
            operation.status = SyncStatus::Failed {
                error: "Cancelled by user".to_string(),
            };
            operation.completed_at = Some(Instant::now());
            
            // Update node state
            let mut states = self.node_states.write().await;
            if let Some(state) = states.get_mut(&operation.target_node) {
                state.is_syncing = false;
            }
        }
        
        Ok(())
    }

    /// Get synchronization statistics
    pub async fn get_stats(&self) -> SyncStats {
        let mut stats = self.stats.read().await.clone();
        stats.total_syncs = stats.successful_syncs + stats.failed_syncs;
        
        if stats.successful_syncs > 0 {
            // Calculate average sync duration (simplified)
            stats.average_sync_duration_ms = 1000; // Placeholder
        }
        
        stats
    }

    /// Clean up completed sync operations
    pub async fn cleanup_completed(&self, older_than: Duration) {
        let mut active = self.active_syncs.write().await;
        let now = Instant::now();
        
        active.retain(|id, op| {
            let should_keep = match &op.status {
                SyncStatus::InProgress => true,
                SyncStatus::Completed | SyncStatus::Failed { .. } => {
                    if let Some(completed_at) = op.completed_at {
                        now.duration_since(completed_at) < older_than
                    } else {
                        true
                    }
                }
                _ => true,
            };
            
            if !should_keep {
                debug!("Cleaning up completed sync: {}", id);
            }
            
            should_keep
        });
    }

    /// Check nodes that need synchronization
    pub async fn get_nodes_needing_sync(&self) -> Vec<NodeId> {
        let states = self.node_states.read().await;
        let current_lsn = self.replication_manager.current_lsn().await;
        
        states
            .values()
            .filter(|state| {
                !state.is_syncing && 
                (current_lsn > state.last_sync_lsn + 1000 || // More than 1000 ops behind
                 state.last_sync_time.is_none() || // Never synced
                 state.last_sync_time.unwrap().elapsed() > Duration::from_secs(300)) // Last sync > 5 min ago
            })
            .map(|state| state.node_id.clone())
            .collect()
    }
}

impl Clone for DataSyncCoordinator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            replication_manager: Arc::clone(&self.replication_manager),
            active_syncs: Arc::clone(&self.active_syncs),
            node_states: Arc::clone(&self.node_states),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replication::{ReplicationConfig, ReplicationSlotManager};

    fn create_test_coordinator() -> DataSyncCoordinator {
        let repl_mgr = Arc::new(ReplicationSlotManager::new(ReplicationConfig::default()));
        DataSyncCoordinator::new(DataSyncConfig::default(), repl_mgr)
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = create_test_coordinator();
        let stats = coordinator.get_stats().await;
        assert_eq!(stats.total_syncs, 0);
    }

    #[tokio::test]
    async fn test_register_node() {
        let coordinator = create_test_coordinator();
        let node_id = NodeId::new("node-1".to_string(), "default".to_string());
        
        coordinator.register_node(node_id.clone()).await.unwrap();
        
        let states = coordinator.node_states.read().await;
        assert!(states.contains_key(&node_id));
    }

    #[tokio::test]
    async fn test_start_sync() {
        let coordinator = create_test_coordinator();
        let source = NodeId::new("source".to_string(), "default".to_string());
        let target = NodeId::new("target".to_string(), "default".to_string());
        
        coordinator.register_node(source.clone()).await.unwrap();
        coordinator.register_node(target.clone()).await.unwrap();
        
        let operation_id = coordinator
            .start_sync(source, target, SyncMode::Incremental)
            .await
            .unwrap();
        
        assert!(!operation_id.is_empty());
        
        // Give sync time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let operation = coordinator.get_sync_status(&operation_id).await;
        assert!(operation.is_some());
    }

    #[tokio::test]
    async fn test_get_nodes_needing_sync() {
        let coordinator = create_test_coordinator();
        let node1 = NodeId::new("node-1".to_string(), "default".to_string());
        let node2 = NodeId::new("node-2".to_string(), "default".to_string());
        
        coordinator.register_node(node1.clone()).await.unwrap();
        coordinator.register_node(node2.clone()).await.unwrap();
        
        // Both nodes should need sync (never synced)
        let nodes = coordinator.get_nodes_needing_sync().await;
        assert_eq!(nodes.len(), 2);
    }
}
