//! Provider-based persistence system for Orbit server
//!
//! This module provides a pluggable storage architecture that supports multiple
//! persistence backends including cloud storage, distributed databases, and
//! specialized storage systems.

use async_trait::async_trait;
use orbit_shared::{
    AddressableLease, AddressableReference, NodeId, NodeInfo, NodeLease, OrbitError, OrbitResult,
};
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
pub mod tikv;

/// Configuration for different storage providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistenceConfig {
    /// In-memory storage (default, non-persistent)
    Memory(MemoryConfig),
    /// Amazon S3 compatible storage
    S3(S3Config),
    /// AWS S3 with enhanced GPU and compute optimizations
    AWSS3(AWSS3Config),
    /// Digital Ocean Spaces object storage
    DigitalOceanSpaces(DigitalOceanSpacesConfig),
    /// Azure Blob Storage
    Azure(AzureConfig),
    /// Google Cloud Storage
    GoogleCloud(GoogleCloudConfig),
    /// GCP Storage with enhanced compute integration
    GCPStorage(GCPStorageConfig),
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
    /// TiKV distributed transactional key-value store
    TiKV(crate::persistence::tikv::TiKVConfig),
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

/// Digital Ocean Spaces storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalOceanSpacesConfig {
    /// Digital Ocean Spaces endpoint (e.g., "nyc3.digitaloceanspaces.com")
    pub endpoint: String,
    /// Digital Ocean region (e.g., "nyc3", "sfo3", "ams3", "sgp1", "fra1")
    pub region: String,
    /// Spaces bucket/space name
    pub space_name: String,
    /// Digital Ocean Spaces access key
    pub access_key_id: String,
    /// Digital Ocean Spaces secret key
    pub secret_access_key: String,
    /// Optional path prefix for organization
    pub prefix: Option<String>,
    /// Enable SSL/TLS (recommended: true)
    pub enable_ssl: bool,
    /// Enable CDN acceleration for reads
    pub enable_cdn: bool,
    /// CDN endpoint URL (if CDN is enabled)
    pub cdn_endpoint: Option<String>,
    /// Connection timeout in seconds
    pub connection_timeout: Option<u64>,
    /// Read timeout in seconds
    pub read_timeout: Option<u64>,
    /// Write timeout in seconds
    pub write_timeout: Option<u64>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Enable server-side encryption
    pub enable_encryption: bool,
    /// Custom metadata tags for cost tracking and organization
    pub tags: std::collections::HashMap<String, String>,
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

/// AWS S3 configuration with enhanced GPU and compute optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AWSS3Config {
    /// AWS region (e.g., "us-east-1", "us-west-2")
    pub region: String,
    /// S3 bucket name
    pub bucket: String,
    /// AWS access key ID
    pub access_key_id: String,
    /// AWS secret access key
    pub secret_access_key: String,
    /// Optional session token for temporary credentials
    pub session_token: Option<String>,
    /// Optional path prefix for organization
    pub prefix: Option<String>,
    /// Enable SSL/TLS (recommended: true)
    pub enable_ssl: bool,
    /// Connection timeout in seconds
    pub connection_timeout: Option<u64>,
    /// Read timeout in seconds
    pub read_timeout: Option<u64>,
    /// Write timeout in seconds
    pub write_timeout: Option<u64>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// S3 storage class optimization
    pub storage_class: S3StorageClass,
    /// Enable S3 Transfer Acceleration for GPU workloads
    pub enable_transfer_acceleration: bool,
    /// Enable multipart uploads for large GPU datasets
    pub multipart_upload_threshold: u64, // bytes
    /// Part size for multipart uploads
    pub multipart_part_size: u64, // bytes
    /// Maximum concurrent uploads for GPU data
    pub max_concurrent_uploads: u32,
    /// Enable server-side encryption
    pub enable_encryption: bool,
    /// KMS key ID for encryption (optional)
    pub kms_key_id: Option<String>,
    /// S3 Intelligent Tiering for cost optimization
    pub enable_intelligent_tiering: bool,
    /// GPU-specific optimizations
    pub gpu_optimizations: AWSS3GPUOptimizations,
    /// EC2 instance role ARN (for cross-account access)
    pub instance_role_arn: Option<String>,
    /// VPC endpoint configuration for private access
    pub vpc_endpoint: Option<String>,
    /// Custom metadata tags for cost tracking
    pub tags: std::collections::HashMap<String, String>,
}

/// AWS S3 storage classes for different performance/cost profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum S3StorageClass {
    /// Standard storage for frequently accessed data
    Standard,
    /// Reduced redundancy (deprecated, for compatibility)
    ReducedRedundancy,
    /// Standard Infrequent Access
    StandardIA,
    /// One Zone Infrequent Access
    OneZoneIA,
    /// Glacier for archival
    Glacier,
    /// Deep Archive for long-term archival
    DeepArchive,
    /// Intelligent Tiering (automatic optimization)
    IntelligentTiering,
}

/// GPU-specific optimizations for AWS S3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AWSS3GPUOptimizations {
    /// Optimize for GPU training data access patterns
    pub optimize_for_training: bool,
    /// Use S3 Express One Zone for ultra-low latency (GPU inference)
    pub use_express_one_zone: bool,
    /// Enable S3 Select for partial data retrieval
    pub enable_s3_select: bool,
    /// Prefetch data for GPU batch processing
    pub enable_prefetch: bool,
    /// Prefetch buffer size in bytes
    pub prefetch_buffer_size: u64,
    /// Parallel download threads for large datasets
    pub parallel_download_threads: u32,
    /// Enable compression for neural network models
    pub compress_models: bool,
    /// Use CloudFront for global GPU cluster access
    pub use_cloudfront: bool,
    /// CloudFront distribution ID
    pub cloudfront_distribution_id: Option<String>,
}

/// GCP Storage configuration with enhanced compute integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCPStorageConfig {
    /// GCP project ID
    pub project_id: String,
    /// Cloud Storage bucket name
    pub bucket_name: String,
    /// Service account key JSON (base64 encoded)
    pub service_account_key: Option<String>,
    /// Path to service account key file
    pub credentials_path: Option<String>,
    /// GCP region for the bucket
    pub region: String,
    /// Optional path prefix for organization
    pub prefix: Option<String>,
    /// Connection timeout in seconds
    pub connection_timeout: Option<u64>,
    /// Read timeout in seconds
    pub read_timeout: Option<u64>,
    /// Write timeout in seconds
    pub write_timeout: Option<u64>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Storage class optimization
    pub storage_class: GCPStorageClass,
    /// Enable resumable uploads for large files
    pub enable_resumable_uploads: bool,
    /// Resumable upload threshold in bytes
    pub resumable_upload_threshold: u64,
    /// Maximum concurrent uploads
    pub max_concurrent_uploads: u32,
    /// Enable customer-managed encryption
    pub enable_encryption: bool,
    /// Cloud KMS key name for encryption
    pub kms_key_name: Option<String>,
    /// GPU-specific optimizations
    pub gpu_optimizations: GCPStorageGPUOptimizations,
    /// Use Google Cloud CDN for global access
    pub enable_cdn: bool,
    /// Enable Cloud Storage Transfer Service
    pub enable_transfer_service: bool,
    /// Labels for cost tracking and organization
    pub labels: std::collections::HashMap<String, String>,
}

/// GCP Cloud Storage classes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GCPStorageClass {
    /// Standard storage
    Standard,
    /// Nearline storage (30-day minimum)
    Nearline,
    /// Coldline storage (90-day minimum)
    Coldline,
    /// Archive storage (365-day minimum)
    Archive,
}

/// GPU-specific optimizations for GCP Storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCPStorageGPUOptimizations {
    /// Optimize for TPU/GPU training workloads
    pub optimize_for_ml_training: bool,
    /// Use regional buckets for GPU clusters
    pub use_regional_buckets: bool,
    /// Enable parallel composite uploads
    pub enable_parallel_composite: bool,
    /// Composite upload threshold in bytes
    pub composite_upload_threshold: u64,
    /// Enable gzip compression for model files
    pub enable_gzip_compression: bool,
    /// Use Cloud Storage FUSE for direct file system access
    pub enable_fuse_mount: bool,
    /// FUSE mount point for GPU containers
    pub fuse_mount_point: Option<String>,
    /// Enable automatic data locality optimization
    pub enable_data_locality: bool,
    /// Use Google Cloud Storage Insights for optimization
    pub enable_insights: bool,
    /// Prefetch configuration for batch processing
    pub prefetch_config: Option<GCPPrefetchConfig>,
}

/// Prefetch configuration for GCP Storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCPPrefetchConfig {
    /// Enable prefetching
    pub enabled: bool,
    /// Prefetch buffer size in bytes
    pub buffer_size: u64,
    /// Number of prefetch threads
    pub thread_count: u32,
    /// Prefetch patterns (file extensions or patterns)
    pub patterns: Vec<String>,
}

/// TiKV configuration for distributed storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TiKVConfig {
    /// List of PD (Placement Driver) endpoints
    pub pd_endpoints: Vec<String>,
    /// Connection timeout in seconds
    pub connection_timeout: Option<u64>,
    /// Request timeout in seconds
    pub request_timeout: Option<u64>,
    /// Maximum number of connections in the pool
    pub max_connections: Option<u32>,
    /// Enable pessimistic transactions (default: optimistic)
    pub enable_pessimistic_txn: bool,
    /// Transaction timeout in seconds
    pub txn_timeout: Option<u64>,
    /// Enable async commit for better performance
    pub enable_async_commit: bool,
    /// Enable one-phase commit optimization
    pub enable_one_pc: bool,
    /// Batch size for bulk operations
    pub batch_size: usize,
    /// Region cache size
    pub region_cache_size: usize,
    /// Coprocessor pool size
    pub coprocessor_pool_size: usize,
    /// Enable TLS/SSL
    pub enable_tls: bool,
    /// CA certificate path for TLS
    pub ca_cert_path: Option<String>,
    /// Client certificate path for mTLS
    pub client_cert_path: Option<String>,
    /// Client private key path for mTLS
    pub client_key_path: Option<String>,
    /// Key prefix for organizing data
    pub key_prefix: String,
    /// Enable compression for stored data
    pub enable_compression: bool,
    /// Maximum retries for failed operations
    pub max_retries: u32,
    /// Retry backoff delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for TiKVConfig {
    fn default() -> Self {
        Self {
            pd_endpoints: vec!["127.0.0.1:2379".to_string()],
            connection_timeout: Some(30),
            request_timeout: Some(10),
            max_connections: Some(10),
            enable_pessimistic_txn: false,
            txn_timeout: Some(30),
            enable_async_commit: true,
            enable_one_pc: true,
            batch_size: 1000,
            region_cache_size: 1000,
            coprocessor_pool_size: 8,
            enable_tls: false,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
            key_prefix: "orbit".to_string(),
            enable_compression: true,
            max_retries: 3,
            retry_delay_ms: 100,
        }
    }
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
