use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// OrbitCluster represents a complete Orbit-RS cluster deployment
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "orbit.turingworks.com",
    version = "v1",
    kind = "OrbitCluster",
    plural = "orbitclusters",
    namespaced
)]
#[kube(status = "OrbitClusterStatus")]
#[kube(shortname = "oc")]
pub struct OrbitClusterSpec {
    /// Number of Orbit server replicas
    #[serde(default = "default_replicas")]
    pub replicas: i32,

    /// Container image configuration
    pub image: ImageSpec,

    /// Cluster configuration
    #[serde(default)]
    pub cluster: ClusterConfig,

    /// Transaction system configuration
    #[serde(default)]
    pub transactions: TransactionConfig,

    /// Resource requirements
    #[serde(default)]
    pub resources: ResourceRequirements,

    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,

    /// Persistence backend configuration
    #[serde(default)]
    pub persistence: PersistenceConfig,

    /// Service configuration
    #[serde(default)]
    pub service: ServiceConfig,

    /// Monitoring and observability
    #[serde(default)]
    pub monitoring: MonitoringConfig,

    /// Additional environment variables
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ImageSpec {
    /// Container image repository
    pub repository: String,

    /// Image tag
    #[serde(default = "default_image_tag")]
    pub tag: String,

    /// Image pull policy
    #[serde(default = "default_pull_policy")]
    pub pull_policy: String,

    /// Image pull secrets
    #[serde(default)]
    pub pull_secrets: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct ClusterConfig {
    /// Discovery mode (kubernetes, etcd, dns)
    #[serde(default = "default_discovery_mode")]
    pub discovery_mode: String,

    /// Election method (kubernetes, raft)
    #[serde(default = "default_election_method")]
    pub election_method: String,

    /// Lease duration in seconds
    #[serde(default = "default_lease_duration")]
    pub lease_duration: u32,

    /// Lease renew interval in seconds
    #[serde(default = "default_renew_interval")]
    pub lease_renew_interval: u32,

    /// Enable Raft fallback
    #[serde(default = "default_raft_fallback")]
    pub enable_raft_fallback: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct TransactionConfig {
    /// Database path for transaction log
    #[serde(default = "default_db_path")]
    pub database_path: String,

    /// Maximum database connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Enable WAL journaling
    #[serde(default = "default_enable_wal")]
    pub enable_wal: bool,

    /// Recovery timeout in seconds
    #[serde(default = "default_recovery_timeout")]
    pub recovery_timeout: u32,

    /// Maximum recovery attempts
    #[serde(default = "default_max_recovery_attempts")]
    pub max_recovery_attempts: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct ResourceRequirements {
    /// CPU requests
    #[serde(default = "default_cpu_request")]
    pub cpu_request: String,

    /// Memory requests
    #[serde(default = "default_memory_request")]
    pub memory_request: String,

    /// CPU limits
    #[serde(default = "default_cpu_limit")]
    pub cpu_limit: String,

    /// Memory limits
    #[serde(default = "default_memory_limit")]
    pub memory_limit: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct StorageConfig {
    /// Storage class name
    pub storage_class: Option<String>,

    /// Storage size
    #[serde(default = "default_storage_size")]
    pub size: String,

    /// Access mode
    #[serde(default = "default_access_mode")]
    pub access_mode: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct ServiceConfig {
    /// Service type (ClusterIP, LoadBalancer, NodePort)
    #[serde(default = "default_service_type")]
    pub service_type: String,

    /// gRPC port
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,

    /// Health check port
    #[serde(default = "default_health_port")]
    pub health_port: u16,

    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// Service annotations
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct MonitoringConfig {
    /// Enable Prometheus metrics
    #[serde(default = "default_enable_metrics")]
    pub enabled: bool,

    /// Enable ServiceMonitor
    #[serde(default)]
    pub service_monitor: bool,

    /// Metrics scrape interval
    #[serde(default = "default_scrape_interval")]
    pub scrape_interval: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct PersistenceConfig {
    /// Persistence backend type
    #[serde(default = "default_persistence_backend")]
    pub backend: String,

    /// Memory backend configuration
    #[serde(default)]
    pub memory: Option<MemoryPersistenceConfig>,

    /// Local storage backend configuration (COW B+Tree, LSM-Tree, RocksDB)
    #[serde(default)]
    pub local: Option<LocalStorageConfig>,

    /// Cloud storage configuration (S3, Azure, GCP)
    #[serde(default)]
    pub cloud: Option<CloudStorageConfig>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct MemoryPersistenceConfig {
    /// Maximum entries in memory
    #[serde(default = "default_memory_max_entries")]
    pub max_entries: u64,

    /// Enable disk backup
    #[serde(default)]
    pub enable_disk_backup: bool,

    /// Backup file path
    #[serde(default = "default_memory_backup_path")]
    pub backup_path: String,

    /// Backup sync interval in seconds
    #[serde(default = "default_memory_sync_interval")]
    pub sync_interval: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LocalStorageConfig {
    /// Storage backend type (cow_btree, lsm_tree, rocksdb)
    pub backend_type: String,

    /// Data directory path
    #[serde(default = "default_local_data_dir")]
    pub data_dir: String,

    /// Enable compression
    #[serde(default = "default_enable_compression")]
    pub enable_compression: bool,

    /// Write buffer size in bytes
    #[serde(default = "default_write_buffer_size")]
    pub write_buffer_size: u64,

    /// Cache size in bytes
    #[serde(default = "default_cache_size")]
    pub cache_size: u64,

    /// Backend-specific configuration
    #[serde(default)]
    pub config: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct CloudStorageConfig {
    /// Cloud provider (s3, azure, gcp)
    pub provider: String,

    /// Storage endpoint URL
    pub endpoint: Option<String>,

    /// Region/location
    pub region: Option<String>,

    /// Bucket/container name
    pub bucket: String,

    /// Object prefix
    #[serde(default = "default_cloud_prefix")]
    pub prefix: String,

    /// Connection timeout in seconds
    #[serde(default = "default_cloud_timeout")]
    pub connection_timeout: u32,

    /// Retry count
    #[serde(default = "default_cloud_retry_count")]
    pub retry_count: u32,

    /// Enable SSL/TLS
    #[serde(default = "default_enable_ssl")]
    pub enable_ssl: bool,

    /// Credentials secret name
    pub credentials_secret: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct OrbitClusterStatus {
    /// Current phase of the cluster
    pub phase: Option<ClusterPhase>,

    /// Number of ready replicas
    pub ready_replicas: Option<i32>,

    /// Current replica count
    pub replicas: Option<i32>,

    /// Conditions affecting the cluster
    #[serde(default)]
    pub conditions: Vec<ClusterCondition>,

    /// Leader node information
    pub leader: Option<LeaderInfo>,

    /// Last observed generation
    pub observed_generation: Option<i64>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub enum ClusterPhase {
    Pending,
    Creating,
    Running,
    Scaling,
    Updating,
    Failed,
    Terminating,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ClusterCondition {
    /// Type of condition
    pub condition_type: String,

    /// Status of the condition
    pub status: String,

    /// Last transition time
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,

    /// Reason for the condition
    pub reason: Option<String>,

    /// Human-readable message
    pub message: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LeaderInfo {
    /// Node ID of the current leader
    pub node_id: String,

    /// Pod name of the leader
    pub pod_name: String,

    /// Elected at timestamp
    pub elected_at: chrono::DateTime<chrono::Utc>,

    /// Lease expiry time
    pub lease_expires_at: chrono::DateTime<chrono::Utc>,
}

// Default value functions
fn default_replicas() -> i32 {
    3
}

fn default_image_tag() -> String {
    "latest".to_string()
}

fn default_pull_policy() -> String {
    "IfNotPresent".to_string()
}

fn default_discovery_mode() -> String {
    "kubernetes".to_string()
}

fn default_election_method() -> String {
    "kubernetes".to_string()
}

fn default_lease_duration() -> u32 {
    30
}

fn default_renew_interval() -> u32 {
    10
}

fn default_raft_fallback() -> bool {
    true
}

fn default_db_path() -> String {
    "/app/data/orbit_transactions.db".to_string()
}

fn default_max_connections() -> u32 {
    10
}

fn default_enable_wal() -> bool {
    true
}

fn default_recovery_timeout() -> u32 {
    300
}

fn default_max_recovery_attempts() -> u32 {
    3
}

fn default_cpu_request() -> String {
    "250m".to_string()
}

fn default_memory_request() -> String {
    "512Mi".to_string()
}

fn default_cpu_limit() -> String {
    "1000m".to_string()
}

fn default_memory_limit() -> String {
    "2Gi".to_string()
}

fn default_storage_size() -> String {
    "10Gi".to_string()
}

fn default_access_mode() -> String {
    "ReadWriteOnce".to_string()
}

fn default_service_type() -> String {
    "ClusterIP".to_string()
}

fn default_grpc_port() -> u16 {
    50051
}

fn default_health_port() -> u16 {
    8080
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_enable_metrics() -> bool {
    true
}

fn default_scrape_interval() -> String {
    "30s".to_string()
}

// Persistence configuration defaults
fn default_persistence_backend() -> String {
    "memory".to_string()
}

fn default_memory_max_entries() -> u64 {
    1000000
}

fn default_memory_backup_path() -> String {
    "/app/data/orbit_backup.json".to_string()
}

fn default_memory_sync_interval() -> u32 {
    300
}

fn default_local_data_dir() -> String {
    "/app/data".to_string()
}

fn default_enable_compression() -> bool {
    true
}

fn default_write_buffer_size() -> u64 {
    134217728 // 128MB
}

fn default_cache_size() -> u64 {
    268435456 // 256MB
}

fn default_cloud_prefix() -> String {
    "orbit".to_string()
}

fn default_cloud_timeout() -> u32 {
    30
}

fn default_cloud_retry_count() -> u32 {
    3
}

fn default_enable_ssl() -> bool {
    true
}
