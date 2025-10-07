//! Provider-based persistence system for Orbit server
//!
//! This module provides a pluggable storage architecture that supports multiple
//! persistence backends including cloud storage, distributed databases, and
//! specialized storage systems.

use async_trait::async_trait;
use orbit_shared::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod cloud;
pub mod memory;
// pub mod distributed; // TODO: File missing
pub mod config;
pub mod dynamic;
// pub mod health; // TODO: File missing
// pub mod migration; // TODO: File missing
pub mod cow_btree;
pub mod factory;
pub mod lsm_tree;
pub mod rocksdb;

/// Configuration for different storage providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistenceConfig {
    /// In-memory storage (default, non-persistent)
    Memory(MemoryConfig),
    /// Amazon S3 compatible storage
    S3(S3Config),
    /// Azure Blob Storage
    Azure(AzureConfig),
    /// Google Cloud Storage
    GoogleCloud(GoogleCloudConfig),
    /// etcd distributed key-value store
    Etcd(EtcdConfig),
    /// Redis in-memory data structure store
    Redis(RedisConfig),
    /// Kubernetes-based storage
    Kubernetes(KubernetesConfig),
    /// MinIO object storage
    MinIO(MinIOConfig),
    /// Flash-optimized storage with multipathing
    Flash(FlashConfig),
    /// Composite provider with primary/backup configuration
    Composite(CompositeConfig),
    /// Copy-on-Write B+ Tree with WAL
    CowBTree(crate::persistence::cow_btree::CowBTreeConfig),
    /// LSM-Tree with compaction
    LsmTree(crate::persistence::lsm_tree::LsmTreeConfig),
    /// RocksDB embedded database
    RocksDB(crate::persistence::rocksdb::RocksDbConfig),
}

/// Memory storage configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryConfig {
    /// Maximum number of entries to keep in memory
    pub max_entries: Option<usize>,
    /// Enable persistence to disk for recovery
    pub disk_backup: Option<DiskBackupConfig>,
}

/// S3-compatible storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub prefix: Option<String>,
    pub enable_ssl: bool,
    pub connection_timeout: Option<u64>,
    pub read_timeout: Option<u64>,
    pub write_timeout: Option<u64>,
    pub retry_count: u32,
}

/// Azure Blob Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    pub account_name: String,
    pub account_key: String,
    pub container_name: String,
    pub endpoint: Option<String>,
    pub prefix: Option<String>,
    pub connection_timeout: Option<u64>,
    pub retry_count: u32,
}

/// Google Cloud Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCloudConfig {
    pub project_id: String,
    pub bucket_name: String,
    pub credentials_path: Option<String>,
    pub service_account_key: Option<String>,
    pub prefix: Option<String>,
    pub connection_timeout: Option<u64>,
    pub retry_count: u32,
}

/// etcd configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtcdConfig {
    pub endpoints: Vec<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ca_cert: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
    pub prefix: String,
    pub connection_timeout: Option<u64>,
    pub lease_ttl: u64,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub cluster_mode: bool,
    pub database: Option<u32>,
    pub password: Option<String>,
    pub prefix: String,
    pub connection_timeout: Option<u64>,
    pub pool_size: u32,
    pub retry_count: u32,
}

/// Kubernetes storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesConfig {
    pub namespace: String,
    pub config_map_name: String,
    pub secret_name: Option<String>,
    pub kubeconfig_path: Option<String>,
    pub in_cluster: bool,
}

/// MinIO configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinIOConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub region: Option<String>,
    pub secure: bool,
    pub prefix: Option<String>,
}

/// Flash-optimized storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashConfig {
    pub data_dir: String,
    pub enable_multipathing: bool,
    pub io_depth: u32,
    pub block_size: u32,
    pub cache_size: u64,
    pub compression: CompressionType,
    pub paths: Vec<String>,
}

/// Disk backup configuration for memory provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskBackupConfig {
    pub path: String,
    pub sync_interval: u64,
    pub compression: CompressionType,
}

/// Composite provider configuration with failover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeConfig {
    pub primary: Box<PersistenceConfig>,
    pub backup: Option<Box<PersistenceConfig>>,
    pub sync_interval: u64,
    pub health_check_interval: u64,
    pub failover_threshold: u32,
}

/// Compression types for storage optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Lz4,
    Zstd,
}

/// Health status of a persistence provider
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Metrics for persistence operations
#[derive(Debug, Clone, Default)]
pub struct PersistenceMetrics {
    pub read_operations: u64,
    pub write_operations: u64,
    pub delete_operations: u64,
    pub read_latency_avg: f64,
    pub write_latency_avg: f64,
    pub delete_latency_avg: f64,
    pub error_count: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Transaction context for atomic operations
#[derive(Debug, Clone)]
pub struct TransactionContext {
    pub id: String,
    pub isolation_level: IsolationLevel,
    pub timeout: Option<std::time::Duration>,
}

/// Isolation levels for transactions
#[derive(Debug, Clone)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

/// Core persistence provider trait
#[async_trait]
pub trait PersistenceProvider: Send + Sync {
    /// Initialize the provider
    async fn initialize(&self) -> OrbitResult<()>;

    /// Shutdown the provider gracefully
    async fn shutdown(&self) -> OrbitResult<()>;

    /// Check health status
    async fn health_check(&self) -> ProviderHealth;

    /// Get current metrics
    async fn metrics(&self) -> PersistenceMetrics;

    /// Begin a transaction
    async fn begin_transaction(&self, context: TransactionContext) -> OrbitResult<String>;

    /// Commit a transaction
    async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()>;

    /// Rollback a transaction
    async fn rollback_transaction(&self, transaction_id: &str) -> OrbitResult<()>;
}

/// Provider for storing addressable directory information
#[async_trait]
pub trait AddressableDirectoryProvider: PersistenceProvider {
    /// Store an addressable lease
    async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()>;

    /// Retrieve an addressable lease
    async fn get_lease(
        &self,
        reference: &AddressableReference,
    ) -> OrbitResult<Option<AddressableLease>>;

    /// Update an existing lease
    async fn update_lease(&self, lease: &AddressableLease) -> OrbitResult<()>;

    /// Remove a lease
    async fn remove_lease(&self, reference: &AddressableReference) -> OrbitResult<bool>;

    /// List all leases for a node
    async fn list_node_leases(&self, node_id: &NodeId) -> OrbitResult<Vec<AddressableLease>>;

    /// List all active leases
    async fn list_all_leases(&self) -> OrbitResult<Vec<AddressableLease>>;

    /// Remove expired leases
    async fn cleanup_expired_leases(&self) -> OrbitResult<u64>;

    /// Bulk operations for efficiency
    async fn store_leases_bulk(&self, leases: &[AddressableLease]) -> OrbitResult<()>;
    async fn remove_leases_bulk(&self, references: &[AddressableReference]) -> OrbitResult<u64>;
}

/// Provider for storing cluster node information
#[async_trait]
pub trait ClusterNodeProvider: PersistenceProvider {
    /// Store node information
    async fn store_node(&self, node: &NodeInfo) -> OrbitResult<()>;

    /// Retrieve node information
    async fn get_node(&self, node_id: &NodeId) -> OrbitResult<Option<NodeInfo>>;

    /// Update node information
    async fn update_node(&self, node: &NodeInfo) -> OrbitResult<()>;

    /// Remove a node
    async fn remove_node(&self, node_id: &NodeId) -> OrbitResult<bool>;

    /// List all active nodes
    async fn list_active_nodes(&self) -> OrbitResult<Vec<NodeInfo>>;

    /// List all nodes (including inactive)
    async fn list_all_nodes(&self) -> OrbitResult<Vec<NodeInfo>>;

    /// Remove expired nodes
    async fn cleanup_expired_nodes(&self) -> OrbitResult<u64>;

    /// Update node lease
    async fn renew_node_lease(&self, node_id: &NodeId, lease: &NodeLease) -> OrbitResult<()>;
}

/// Registry for managing multiple persistence providers
pub struct PersistenceProviderRegistry {
    addressable_providers: Arc<RwLock<HashMap<String, Arc<dyn AddressableDirectoryProvider>>>>,
    cluster_providers: Arc<RwLock<HashMap<String, Arc<dyn ClusterNodeProvider>>>>,
    default_addressable: Arc<RwLock<Option<String>>>,
    default_cluster: Arc<RwLock<Option<String>>>,
}

impl PersistenceProviderRegistry {
    pub fn new() -> Self {
        Self {
            addressable_providers: Arc::new(RwLock::new(HashMap::new())),
            cluster_providers: Arc::new(RwLock::new(HashMap::new())),
            default_addressable: Arc::new(RwLock::new(None)),
            default_cluster: Arc::new(RwLock::new(None)),
        }
    }

    /// Register an addressable directory provider
    pub async fn register_addressable_provider(
        &self,
        name: String,
        provider: Arc<dyn AddressableDirectoryProvider>,
        is_default: bool,
    ) -> OrbitResult<()> {
        let mut providers = self.addressable_providers.write().await;
        providers.insert(name.clone(), provider);

        if is_default {
            let mut default = self.default_addressable.write().await;
            *default = Some(name);
        }

        Ok(())
    }

    /// Register a cluster node provider
    pub async fn register_cluster_provider(
        &self,
        name: String,
        provider: Arc<dyn ClusterNodeProvider>,
        is_default: bool,
    ) -> OrbitResult<()> {
        let mut providers = self.cluster_providers.write().await;
        providers.insert(name.clone(), provider);

        if is_default {
            let mut default = self.default_cluster.write().await;
            *default = Some(name);
        }

        Ok(())
    }

    /// Get the default addressable directory provider
    pub async fn get_default_addressable_provider(
        &self,
    ) -> OrbitResult<Arc<dyn AddressableDirectoryProvider>> {
        let default_name = {
            let default = self.default_addressable.read().await;
            default.clone().ok_or_else(|| {
                OrbitError::configuration("No default addressable provider configured")
            })?
        };

        let providers = self.addressable_providers.read().await;
        providers.get(&default_name).cloned().ok_or_else(|| {
            OrbitError::configuration(format!(
                "Default addressable provider '{}' not found",
                default_name
            ))
        })
    }

    /// Get the default cluster node provider
    pub async fn get_default_cluster_provider(&self) -> OrbitResult<Arc<dyn ClusterNodeProvider>> {
        let default_name = {
            let default = self.default_cluster.read().await;
            default.clone().ok_or_else(|| {
                OrbitError::configuration("No default cluster provider configured")
            })?
        };

        let providers = self.cluster_providers.read().await;
        providers.get(&default_name).cloned().ok_or_else(|| {
            OrbitError::configuration(format!(
                "Default cluster provider '{}' not found",
                default_name
            ))
        })
    }

    /// Get a specific addressable provider by name
    pub async fn get_addressable_provider(
        &self,
        name: &str,
    ) -> OrbitResult<Arc<dyn AddressableDirectoryProvider>> {
        let providers = self.addressable_providers.read().await;
        providers.get(name).cloned().ok_or_else(|| {
            OrbitError::configuration(format!("Addressable provider '{}' not found", name))
        })
    }

    /// Get a specific cluster provider by name
    pub async fn get_cluster_provider(
        &self,
        name: &str,
    ) -> OrbitResult<Arc<dyn ClusterNodeProvider>> {
        let providers = self.cluster_providers.read().await;
        providers.get(name).cloned().ok_or_else(|| {
            OrbitError::configuration(format!("Cluster provider '{}' not found", name))
        })
    }

    /// List all registered provider names
    pub async fn list_providers(&self) -> (Vec<String>, Vec<String>) {
        let addressable = self.addressable_providers.read().await;
        let cluster = self.cluster_providers.read().await;

        (
            addressable.keys().cloned().collect(),
            cluster.keys().cloned().collect(),
        )
    }
}

impl Default for PersistenceProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
