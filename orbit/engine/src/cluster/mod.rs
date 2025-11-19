//! Clustering and distributed consensus
//!
//! Provides Raft consensus, replication, and cluster coordination for distributed deployments.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, EngineResult};
use crate::metrics::ClusterMetrics;

// Module declarations
pub mod consensus;
pub mod manager;
pub mod recovery;
pub mod replication;
pub mod transport;

// Re-exports
pub use consensus::RaftConsensus;
pub use manager::ClusterManager;
pub use replication::ReplicationManager;

/// Node identifier
pub type NodeId = String;

/// Cluster coordinator trait
#[async_trait]
pub trait ClusterCoordinator: Send + Sync {
    /// Get the current leader node ID
    async fn get_leader(&self) -> Option<NodeId>;

    /// Check if this node is the leader
    async fn is_leader(&self) -> bool;

    /// Get all cluster nodes
    async fn get_cluster_nodes(&self) -> Vec<NodeId>;

    /// Check if cluster has quorum
    async fn has_quorum(&self) -> bool;

    /// Get cluster metrics
    async fn metrics(&self) -> ClusterMetrics;

    /// Add a node to the cluster
    async fn add_node(&self, node_id: NodeId, address: String) -> EngineResult<()>;

    /// Remove a node from the cluster
    async fn remove_node(&self, node_id: NodeId) -> EngineResult<()>;
}

/// Node health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealth {
    /// Node is healthy
    Healthy,
    /// Node is degraded but operational
    Degraded,
    /// Node is unhealthy
    Unhealthy,
    /// Node is unknown/unreachable
    Unknown,
}

/// Replication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    /// Replication factor (number of replicas)
    pub replication_factor: usize,
    /// Enable synchronous replication
    pub sync_replication: bool,
    /// Replication timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            sync_replication: false,
            timeout_ms: 5000,
        }
    }
}
