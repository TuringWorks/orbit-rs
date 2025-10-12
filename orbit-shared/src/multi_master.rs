//! Multi-master clustering with conflict resolution
//!
//! This module provides multi-master replication support where multiple nodes
//! can accept writes simultaneously, with automatic conflict detection and resolution.

use crate::cdc::CdcEvent;
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use crate::replication::{ReplicationSlot, ReplicationSlotManager};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Conflict resolution strategy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConflictResolutionStrategy {
    /// Last write wins based on timestamp
    LastWriteWins,
    /// First write wins
    FirstWriteWins,
    /// Use highest node ID as tiebreaker
    NodeIdTiebreaker,
    /// Custom resolution based on data versioning
    Versioned,
    /// Manual resolution required
    Manual,
}

impl Default for ConflictResolutionStrategy {
    fn default() -> Self {
        Self::LastWriteWins
    }
}

/// Represents a write operation in the multi-master cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteOperation {
    pub operation_id: String,
    pub node_id: NodeId,
    pub timestamp: u64,
    pub lsn: u64,
    pub key: String,
    pub value: Vec<u8>,
    pub version: u64,
}

/// Detected conflict between writes
#[derive(Debug, Clone)]
pub struct Conflict {
    pub key: String,
    pub operations: Vec<WriteOperation>,
    pub detected_at: u64,
}

/// Result of conflict resolution
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub winning_operation: WriteOperation,
    pub strategy_used: ConflictResolutionStrategy,
    pub resolved_at: u64,
}

/// Multi-master cluster configuration
#[derive(Debug, Clone)]
pub struct MultiMasterConfig {
    /// Conflict resolution strategy
    pub conflict_strategy: ConflictResolutionStrategy,
    /// Maximum time window for conflict detection (ms)
    pub conflict_window_ms: u64,
    /// Enable automatic conflict resolution
    pub auto_resolve: bool,
    /// Maximum number of pending conflicts
    pub max_pending_conflicts: usize,
    /// Sync interval between masters
    pub sync_interval: Duration,
}

impl Default for MultiMasterConfig {
    fn default() -> Self {
        Self {
            conflict_strategy: ConflictResolutionStrategy::LastWriteWins,
            conflict_window_ms: 1000,
            auto_resolve: true,
            max_pending_conflicts: 1000,
            sync_interval: Duration::from_millis(100),
        }
    }
}

/// Tracks write operations for conflict detection
#[derive(Debug)]
struct WriteTracker {
    /// Recent writes indexed by key
    writes: HashMap<String, VecDeque<WriteOperation>>,
    /// Maximum operations to track per key
    max_operations_per_key: usize,
}

impl WriteTracker {
    fn new(max_operations_per_key: usize) -> Self {
        Self {
            writes: HashMap::new(),
            max_operations_per_key,
        }
    }

    fn add_write(&mut self, write: WriteOperation) {
        let key_writes = self.writes.entry(write.key.clone()).or_insert_with(VecDeque::new);
        
        key_writes.push_back(write);
        
        // Keep only recent writes
        while key_writes.len() > self.max_operations_per_key {
            key_writes.pop_front();
        }
    }

    fn get_writes_for_key(&self, key: &str) -> Vec<WriteOperation> {
        self.writes.get(key).map(|q| q.iter().cloned().collect()).unwrap_or_default()
    }

    fn detect_conflicts(&self, window_ms: u64) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        
        for (key, writes) in &self.writes {
            let recent_writes: Vec<_> = writes
                .iter()
                .filter(|w| now.saturating_sub(w.timestamp) <= window_ms)
                .cloned()
                .collect();
            
            // Conflict if multiple writes from different nodes in window
            if recent_writes.len() > 1 {
                let unique_nodes: std::collections::HashSet<_> = 
                    recent_writes.iter().map(|w| &w.node_id).collect();
                
                if unique_nodes.len() > 1 {
                    conflicts.push(Conflict {
                        key: key.clone(),
                        operations: recent_writes,
                        detected_at: now,
                    });
                }
            }
        }
        
        conflicts
    }
}

/// Multi-master cluster manager
pub struct MultiMasterCluster {
    /// Configuration
    config: MultiMasterConfig,
    /// Master nodes in the cluster
    master_nodes: Arc<RwLock<Vec<NodeId>>>,
    /// This node's ID
    local_node_id: NodeId,
    /// Write tracker for conflict detection
    write_tracker: Arc<RwLock<WriteTracker>>,
    /// Pending conflicts awaiting resolution
    pending_conflicts: Arc<RwLock<VecDeque<Conflict>>>,
    /// Replication manager for syncing data
    replication_manager: Arc<ReplicationSlotManager>,
}

impl MultiMasterCluster {
    /// Create a new multi-master cluster
    pub fn new(
        config: MultiMasterConfig,
        local_node_id: NodeId,
        replication_manager: Arc<ReplicationSlotManager>,
    ) -> Self {
        Self {
            config,
            master_nodes: Arc::new(RwLock::new(Vec::new())),
            local_node_id,
            write_tracker: Arc::new(RwLock::new(WriteTracker::new(100))),
            pending_conflicts: Arc::new(RwLock::new(VecDeque::new())),
            replication_manager,
        }
    }

    /// Add a master node to the cluster
    pub async fn add_master(&self, node_id: NodeId) -> OrbitResult<()> {
        let mut masters = self.master_nodes.write().await;
        
        if !masters.contains(&node_id) {
            info!("Adding master node to cluster: {}", node_id);
            masters.push(node_id);
        }
        
        Ok(())
    }

    /// Remove a master node from the cluster
    pub async fn remove_master(&self, node_id: &NodeId) -> OrbitResult<()> {
        let mut masters = self.master_nodes.write().await;
        masters.retain(|n| n != node_id);
        info!("Removed master node from cluster: {}", node_id);
        Ok(())
    }

    /// Get all master nodes
    pub async fn get_masters(&self) -> Vec<NodeId> {
        self.master_nodes.read().await.clone()
    }

    /// Record a write operation
    pub async fn record_write(&self, write: WriteOperation) -> OrbitResult<()> {
        debug!("Recording write operation: {} on key: {}", write.operation_id, write.key);
        
        let mut tracker = self.write_tracker.write().await;
        tracker.add_write(write);
        
        Ok(())
    }

    /// Detect conflicts in recent writes
    pub async fn detect_conflicts(&self) -> Vec<Conflict> {
        let tracker = self.write_tracker.read().await;
        let conflicts = tracker.detect_conflicts(self.config.conflict_window_ms);
        
        if !conflicts.is_empty() {
            warn!("Detected {} conflicts", conflicts.len());
        }
        
        conflicts
    }

    /// Resolve a conflict using the configured strategy
    pub async fn resolve_conflict(&self, conflict: Conflict) -> OrbitResult<ConflictResolution> {
        info!("Resolving conflict for key: {} with {} operations", 
              conflict.key, conflict.operations.len());
        
        let winning_operation = match self.config.conflict_strategy {
            ConflictResolutionStrategy::LastWriteWins => {
                self.resolve_last_write_wins(&conflict)?
            }
            ConflictResolutionStrategy::FirstWriteWins => {
                self.resolve_first_write_wins(&conflict)?
            }
            ConflictResolutionStrategy::NodeIdTiebreaker => {
                self.resolve_node_id_tiebreaker(&conflict)?
            }
            ConflictResolutionStrategy::Versioned => {
                self.resolve_versioned(&conflict)?
            }
            ConflictResolutionStrategy::Manual => {
                return Err(OrbitError::internal("Manual conflict resolution required"));
            }
        };
        
        Ok(ConflictResolution {
            winning_operation,
            strategy_used: self.config.conflict_strategy.clone(),
            resolved_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
        })
    }

    /// Resolve using last write wins
    fn resolve_last_write_wins(&self, conflict: &Conflict) -> OrbitResult<WriteOperation> {
        conflict.operations
            .iter()
            .max_by_key(|op| op.timestamp)
            .cloned()
            .ok_or_else(|| OrbitError::internal("No operations in conflict"))
    }

    /// Resolve using first write wins
    fn resolve_first_write_wins(&self, conflict: &Conflict) -> OrbitResult<WriteOperation> {
        conflict.operations
            .iter()
            .min_by_key(|op| op.timestamp)
            .cloned()
            .ok_or_else(|| OrbitError::internal("No operations in conflict"))
    }

    /// Resolve using node ID tiebreaker
    fn resolve_node_id_tiebreaker(&self, conflict: &Conflict) -> OrbitResult<WriteOperation> {
        // First try by timestamp, then use node ID as tiebreaker
        let mut ops = conflict.operations.clone();
        ops.sort_by(|a, b| {
            match a.timestamp.cmp(&b.timestamp) {
                std::cmp::Ordering::Equal => {
                    // Use node ID as tiebreaker
                    a.node_id.to_string().cmp(&b.node_id.to_string())
                }
                other => other.reverse(), // Most recent first
            }
        });
        
        ops.first()
            .cloned()
            .ok_or_else(|| OrbitError::internal("No operations in conflict"))
    }

    /// Resolve using version numbers
    fn resolve_versioned(&self, conflict: &Conflict) -> OrbitResult<WriteOperation> {
        conflict.operations
            .iter()
            .max_by_key(|op| op.version)
            .cloned()
            .ok_or_else(|| OrbitError::internal("No operations in conflict"))
    }

    /// Process pending conflicts
    pub async fn process_conflicts(&self) -> OrbitResult<Vec<ConflictResolution>> {
        let conflicts = self.detect_conflicts().await;
        let mut resolutions = Vec::new();
        
        if self.config.auto_resolve {
            for conflict in conflicts {
                match self.resolve_conflict(conflict).await {
                    Ok(resolution) => {
                        info!("Conflict resolved for key: {}", resolution.winning_operation.key);
                        resolutions.push(resolution);
                    }
                    Err(e) => {
                        warn!("Failed to resolve conflict: {}", e);
                    }
                }
            }
        } else {
            // Add to pending queue
            let mut pending = self.pending_conflicts.write().await;
            for conflict in conflicts {
                if pending.len() < self.config.max_pending_conflicts {
                    pending.push_back(conflict);
                } else {
                    warn!("Pending conflicts queue full, dropping conflict");
                }
            }
        }
        
        Ok(resolutions)
    }

    /// Synchronize data with other masters
    pub async fn sync_with_masters(&self) -> OrbitResult<()> {
        let masters = self.get_masters().await;
        
        info!("Synchronizing with {} master nodes", masters.len());
        
        for master in masters {
            if master != self.local_node_id {
                self.sync_with_node(&master).await?;
            }
        }
        
        Ok(())
    }

    /// Synchronize with a specific node
    async fn sync_with_node(&self, node_id: &NodeId) -> OrbitResult<()> {
        debug!("Syncing with node: {}", node_id);
        
        // In a real implementation, this would:
        // 1. Get latest LSN from remote node
        // 2. Request missing operations
        // 3. Apply operations locally
        // 4. Resolve any conflicts
        // 5. Update replication slot
        
        Ok(())
    }

    /// Start background conflict detection and resolution
    pub async fn start_background_tasks(&self) -> OrbitResult<()> {
        let cluster = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cluster.config.sync_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = cluster.process_conflicts().await {
                    warn!("Failed to process conflicts: {}", e);
                }
                
                if let Err(e) = cluster.sync_with_masters().await {
                    warn!("Failed to sync with masters: {}", e);
                }
            }
        });
        
        Ok(())
    }

    /// Get pending conflicts count
    pub async fn pending_conflicts_count(&self) -> usize {
        self.pending_conflicts.read().await.len()
    }

    /// Get configuration
    pub fn config(&self) -> &MultiMasterConfig {
        &self.config
    }
}

impl Clone for MultiMasterCluster {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            master_nodes: Arc::clone(&self.master_nodes),
            local_node_id: self.local_node_id.clone(),
            write_tracker: Arc::clone(&self.write_tracker),
            pending_conflicts: Arc::clone(&self.pending_conflicts),
            replication_manager: Arc::clone(&self.replication_manager),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replication::ReplicationConfig;

    fn create_write_op(node_id: &str, key: &str, timestamp: u64) -> WriteOperation {
        WriteOperation {
            operation_id: format!("op-{}", timestamp),
            node_id: NodeId::new(node_id.to_string(), "default".to_string()),
            timestamp,
            lsn: timestamp,
            key: key.to_string(),
            value: vec![1, 2, 3],
            version: 1,
        }
    }

    #[tokio::test]
    async fn test_multi_master_creation() {
        let config = MultiMasterConfig::default();
        let node_id = NodeId::new("master-1".to_string(), "default".to_string());
        let repl_mgr = Arc::new(ReplicationSlotManager::new(ReplicationConfig::default()));
        
        let cluster = MultiMasterCluster::new(config, node_id, repl_mgr);
        assert_eq!(cluster.get_masters().await.len(), 0);
    }

    #[tokio::test]
    async fn test_add_remove_masters() {
        let config = MultiMasterConfig::default();
        let node_id = NodeId::new("master-1".to_string(), "default".to_string());
        let repl_mgr = Arc::new(ReplicationSlotManager::new(ReplicationConfig::default()));
        let cluster = MultiMasterCluster::new(config, node_id.clone(), repl_mgr);
        
        let master2 = NodeId::new("master-2".to_string(), "default".to_string());
        cluster.add_master(master2.clone()).await.unwrap();
        assert_eq!(cluster.get_masters().await.len(), 1);
        
        cluster.remove_master(&master2).await.unwrap();
        assert_eq!(cluster.get_masters().await.len(), 0);
    }

    #[tokio::test]
    async fn test_conflict_detection() {
        let config = MultiMasterConfig {
            conflict_window_ms: 5000,
            ..Default::default()
        };
        let node_id = NodeId::new("master-1".to_string(), "default".to_string());
        let repl_mgr = Arc::new(ReplicationSlotManager::new(ReplicationConfig::default()));
        let cluster = MultiMasterCluster::new(config, node_id, repl_mgr);
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        
        // Add conflicting writes
        let write1 = create_write_op("master-1", "key1", now);
        let write2 = create_write_op("master-2", "key1", now + 100);
        
        cluster.record_write(write1).await.unwrap();
        cluster.record_write(write2).await.unwrap();
        
        let conflicts = cluster.detect_conflicts().await;
        assert!(!conflicts.is_empty());
    }

    #[tokio::test]
    async fn test_last_write_wins_resolution() {
        let config = MultiMasterConfig {
            conflict_strategy: ConflictResolutionStrategy::LastWriteWins,
            ..Default::default()
        };
        let node_id = NodeId::new("master-1".to_string(), "default".to_string());
        let repl_mgr = Arc::new(ReplicationSlotManager::new(ReplicationConfig::default()));
        let cluster = MultiMasterCluster::new(config, node_id, repl_mgr);
        
        let conflict = Conflict {
            key: "test-key".to_string(),
            operations: vec![
                create_write_op("master-1", "test-key", 100),
                create_write_op("master-2", "test-key", 200),
                create_write_op("master-3", "test-key", 150),
            ],
            detected_at: 300,
        };
        
        let resolution = cluster.resolve_conflict(conflict).await.unwrap();
        assert_eq!(resolution.winning_operation.timestamp, 200);
    }

    #[tokio::test]
    async fn test_first_write_wins_resolution() {
        let config = MultiMasterConfig {
            conflict_strategy: ConflictResolutionStrategy::FirstWriteWins,
            ..Default::default()
        };
        let node_id = NodeId::new("master-1".to_string(), "default".to_string());
        let repl_mgr = Arc::new(ReplicationSlotManager::new(ReplicationConfig::default()));
        let cluster = MultiMasterCluster::new(config, node_id, repl_mgr);
        
        let conflict = Conflict {
            key: "test-key".to_string(),
            operations: vec![
                create_write_op("master-1", "test-key", 100),
                create_write_op("master-2", "test-key", 200),
                create_write_op("master-3", "test-key", 150),
            ],
            detected_at: 300,
        };
        
        let resolution = cluster.resolve_conflict(conflict).await.unwrap();
        assert_eq!(resolution.winning_operation.timestamp, 100);
    }
}
