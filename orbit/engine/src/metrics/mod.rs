//! Metrics and observability for the storage engine

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Storage metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageMetrics {
    /// Total number of reads
    pub reads: u64,

    /// Total number of writes
    pub writes: u64,

    /// Total number of deletes
    pub deletes: u64,

    /// Total bytes read
    pub bytes_read: u64,

    /// Total bytes written
    pub bytes_written: u64,

    /// Number of hot tier operations
    pub hot_tier_ops: u64,

    /// Number of warm tier operations
    pub warm_tier_ops: u64,

    /// Number of cold tier operations
    pub cold_tier_ops: u64,

    /// Number of tier migrations
    pub tier_migrations: u64,

    /// Average read latency
    pub avg_read_latency_ms: f64,

    /// Average write latency
    pub avg_write_latency_ms: f64,
}

/// Transaction metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionMetrics {
    /// Total number of transactions started
    pub started: u64,

    /// Total number of committed transactions
    pub committed: u64,

    /// Total number of aborted transactions
    pub aborted: u64,

    /// Total number of conflicts detected
    pub conflicts: u64,

    /// Total number of deadlocks detected
    pub deadlocks: u64,

    /// Average transaction duration
    pub avg_duration_ms: f64,
}

/// Cluster metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterMetrics {
    /// Number of nodes in cluster
    pub node_count: usize,

    /// Number of healthy nodes
    pub healthy_nodes: usize,

    /// Current Raft term
    pub raft_term: u64,

    /// Current leader node ID
    pub leader_node_id: Option<String>,

    /// Number of log entries
    pub log_entries: u64,

    /// Number of snapshots
    pub snapshots: u64,

    /// Replication lag (max across all followers)
    pub max_replication_lag_ms: u64,
}

/// Query execution metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryMetrics {
    /// Total number of queries executed
    pub queries_executed: u64,

    /// Total number of point lookups
    pub point_lookups: u64,

    /// Total number of scans
    pub scans: u64,

    /// Total number of aggregations
    pub aggregations: u64,

    /// Average query execution time
    pub avg_execution_time_ms: f64,

    /// Number of queries using SIMD
    pub simd_queries: u64,
}

/// Combined engine metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EngineMetrics {
    /// Storage metrics
    pub storage: StorageMetrics,

    /// Transaction metrics
    pub transaction: TransactionMetrics,

    /// Cluster metrics
    pub cluster: ClusterMetrics,

    /// Query metrics
    pub query: QueryMetrics,

    /// Uptime
    pub uptime: Duration,
}

impl EngineMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
