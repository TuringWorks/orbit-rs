pub mod config;
pub mod cow_btree;
pub mod lsm_tree;
pub mod metrics;
pub mod persistence_factory;
pub mod rocksdb_impl;
pub mod workload;

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use uuid::Uuid;

/// Represents an actor lease in the orbit-rs system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActorLease {
    pub key: ActorKey,
    pub node_id: String,
    pub expires_at: SystemTime,
    pub lease_duration: Duration,
    pub version: u64,
    pub metadata: LeaseMetadata,
}

/// Actor identifier
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ActorKey {
    pub actor_id: Uuid,
    pub actor_type: String,
}

/// Metadata associated with an actor lease
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LeaseMetadata {
    pub created_at: SystemTime,
    pub last_renewed: SystemTime,
    pub renewal_count: u64,
    pub custom_data: serde_json::Value,
}

/// Performance metrics for persistence operations
#[derive(Debug, Clone)]
pub struct PersistenceMetrics {
    pub operation_type: OperationType,
    pub latency: Duration,
    pub memory_used: u64,
    pub disk_bytes_read: u64,
    pub disk_bytes_written: u64,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub enum OperationType {
    Insert,
    Update,
    Get,
    RangeQuery,
    Snapshot,
    Recovery,
}

/// Errors that can occur during persistence operations
#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Corruption detected: {0}")]
    Corruption(String),
    #[error("Key not found: {0:?}")]
    KeyNotFound(ActorKey),
    #[error("Version conflict: expected {expected}, got {actual}")]
    VersionConflict { expected: u64, actual: u64 },
    #[error("Storage full")]
    StorageFull,
}

/// Common interface for all persistence implementations
#[async_trait::async_trait]
pub trait PersistenceProvider: Send + Sync {
    /// Store or update an actor lease
    async fn store_lease(&self, lease: &ActorLease)
        -> Result<PersistenceMetrics, PersistenceError>;

    /// Retrieve an actor lease by key
    async fn get_lease(
        &self,
        key: &ActorKey,
    ) -> Result<(Option<ActorLease>, PersistenceMetrics), PersistenceError>;

    /// Get all leases for a range of actor keys (for cluster coordination)
    async fn range_query(
        &self,
        start: &ActorKey,
        end: &ActorKey,
    ) -> Result<(Vec<ActorLease>, PersistenceMetrics), PersistenceError>;

    /// Create a snapshot of current state
    async fn create_snapshot(&self) -> Result<(String, PersistenceMetrics), PersistenceError>;

    /// Restore from a snapshot
    async fn restore_from_snapshot(
        &self,
        snapshot_id: &str,
    ) -> Result<PersistenceMetrics, PersistenceError>;

    /// Get storage statistics
    async fn get_stats(&self) -> Result<StorageStats, PersistenceError>;

    /// Simulate crash and measure recovery time
    async fn simulate_crash_recovery(&self) -> Result<PersistenceMetrics, PersistenceError>;
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_keys: u64,
    pub total_size_bytes: u64,
    pub memory_usage_bytes: u64,
    pub disk_usage_bytes: u64,
    pub average_key_size: u64,
    pub average_value_size: u64,
}

impl ActorLease {
    pub fn new(
        actor_id: Uuid,
        actor_type: String,
        node_id: String,
        lease_duration: Duration,
    ) -> Self {
        let now = SystemTime::now();
        let key = ActorKey {
            actor_id,
            actor_type,
        };

        Self {
            key,
            node_id,
            expires_at: now + lease_duration,
            lease_duration,
            version: 1,
            metadata: LeaseMetadata {
                created_at: now,
                last_renewed: now,
                renewal_count: 0,
                custom_data: serde_json::json!({}),
            },
        }
    }

    pub fn renew(&mut self, new_duration: Duration) {
        let now = SystemTime::now();
        self.expires_at = now + new_duration;
        self.lease_duration = new_duration;
        self.metadata.last_renewed = now;
        self.metadata.renewal_count += 1;
        self.version += 1;
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    pub fn is_renewal(&self) -> bool {
        self.metadata.renewal_count > 0
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, PersistenceError> {
        Ok(serde_json::to_vec(self)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PersistenceError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

impl ActorKey {
    pub fn to_bytes(&self) -> Vec<u8> {
        // Create a sortable byte representation
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.actor_type.as_bytes());
        bytes.push(0); // separator
        bytes.extend_from_slice(self.actor_id.to_string().as_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PersistenceError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|_| PersistenceError::Corruption("Invalid UTF-8 in actor key".to_string()))?;

        let parts: Vec<&str> = s.splitn(2, '\0').collect();
        if parts.len() != 2 {
            return Err(PersistenceError::Corruption(
                "Invalid actor key format".to_string(),
            ));
        }

        let actor_type = parts[0].to_string();
        let actor_id = Uuid::parse_str(parts[1])
            .map_err(|_| PersistenceError::Corruption("Invalid UUID in actor key".to_string()))?;

        Ok(Self {
            actor_id,
            actor_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_key_serialization() {
        let key = ActorKey {
            actor_id: Uuid::new_v4(),
            actor_type: "test_actor".to_string(),
        };

        let bytes = key.to_bytes();
        let restored = ActorKey::from_bytes(&bytes).unwrap();

        assert_eq!(key, restored);
    }

    #[test]
    fn test_actor_lease_operations() {
        let mut lease = ActorLease::new(
            Uuid::new_v4(),
            "test_actor".to_string(),
            "node1".to_string(),
            Duration::from_secs(300),
        );

        assert_eq!(lease.version, 1);
        assert_eq!(lease.metadata.renewal_count, 0);
        assert!(!lease.is_renewal());

        lease.renew(Duration::from_secs(600));

        assert_eq!(lease.version, 2);
        assert_eq!(lease.metadata.renewal_count, 1);
        assert!(lease.is_renewal());
    }
}
