use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// OrbitTransaction represents a managed distributed transaction configuration
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "orbit.turingworks.com",
    version = "v1",
    kind = "OrbitTransaction",
    plural = "orbittransactions",
    namespaced
)]
#[kube(status = "OrbitTransactionStatus")]
#[kube(shortname = "otx")]
pub struct OrbitTransactionSpec {
    /// Target cluster for transaction management
    pub cluster_ref: ClusterReference,

    /// Transaction coordinator configuration
    #[serde(default)]
    pub coordinator: CoordinatorConfig,

    /// Transaction timeout and retry configuration
    #[serde(default)]
    pub timeouts: TimeoutConfig,

    /// Persistence configuration for transaction logs
    #[serde(default)]
    pub persistence: PersistenceConfig,

    /// Recovery and failover configuration
    #[serde(default)]
    pub recovery: RecoveryConfig,

    /// Monitoring and alerting configuration
    #[serde(default)]
    pub monitoring: TransactionMonitoringConfig,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ClusterReference {
    /// Name of the OrbitCluster
    pub name: String,

    /// Namespace of the OrbitCluster (optional, defaults to same namespace)
    pub namespace: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct CoordinatorConfig {
    /// Maximum concurrent transactions
    #[serde(default = "default_max_concurrent_transactions")]
    pub max_concurrent_transactions: u32,

    /// Transaction batch size for optimization
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,

    /// Enable transaction coordinator clustering
    #[serde(default = "default_enable_clustering")]
    pub enable_clustering: bool,

    /// Coordinator heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct TimeoutConfig {
    /// Default transaction timeout in seconds
    #[serde(default = "default_transaction_timeout")]
    pub default_timeout: u32,

    /// Maximum allowed transaction timeout in seconds
    #[serde(default = "default_max_timeout")]
    pub max_timeout: u32,

    /// Prepare phase timeout in seconds
    #[serde(default = "default_prepare_timeout")]
    pub prepare_timeout: u32,

    /// Commit phase timeout in seconds
    #[serde(default = "default_commit_timeout")]
    pub commit_timeout: u32,

    /// Abort phase timeout in seconds
    #[serde(default = "default_abort_timeout")]
    pub abort_timeout: u32,

    /// Maximum retry attempts for failed operations
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Retry backoff interval in milliseconds
    #[serde(default = "default_retry_backoff_ms")]
    pub retry_backoff_ms: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct PersistenceConfig {
    /// Storage backend type
    #[serde(default = "default_storage_backend")]
    pub storage_backend: String,

    /// Database connection string or path
    pub database_url: Option<String>,

    /// Maximum database connections
    #[serde(default = "default_max_db_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u32,

    /// Enable Write-Ahead Logging
    #[serde(default = "default_enable_wal")]
    pub enable_wal: bool,

    /// Log retention period in days
    #[serde(default = "default_log_retention_days")]
    pub log_retention_days: u32,

    /// Enable log compression
    #[serde(default = "default_enable_compression")]
    pub enable_compression: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct RecoveryConfig {
    /// Enable automatic recovery
    #[serde(default = "default_enable_recovery")]
    pub enable_recovery: bool,

    /// Recovery scan interval in seconds
    #[serde(default = "default_recovery_interval")]
    pub recovery_interval: u32,

    /// Maximum recovery attempts per transaction
    #[serde(default = "default_max_recovery_attempts")]
    pub max_recovery_attempts: u32,

    /// Recovery timeout in seconds
    #[serde(default = "default_recovery_timeout")]
    pub recovery_timeout: u32,

    /// Enable coordinator failover
    #[serde(default = "default_enable_failover")]
    pub enable_failover: bool,

    /// Failover detection timeout in seconds
    #[serde(default = "default_failover_timeout")]
    pub failover_timeout: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct TransactionMonitoringConfig {
    /// Enable metrics collection
    #[serde(default = "default_enable_metrics")]
    pub enable_metrics: bool,

    /// Metrics collection interval in seconds
    #[serde(default = "default_metrics_interval")]
    pub metrics_interval: u32,

    /// Enable transaction tracing
    #[serde(default = "default_enable_tracing")]
    pub enable_tracing: bool,

    /// Alert thresholds
    #[serde(default)]
    pub alert_thresholds: AlertThresholds,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct AlertThresholds {
    /// Alert when transaction failure rate exceeds this percentage
    #[serde(default = "default_failure_rate_threshold")]
    pub failure_rate_percentage: f32,

    /// Alert when average transaction duration exceeds this (seconds)
    #[serde(default = "default_duration_threshold")]
    pub duration_threshold_seconds: f32,

    /// Alert when concurrent transactions exceed this number
    #[serde(default = "default_concurrent_threshold")]
    pub concurrent_transactions_threshold: u32,

    /// Alert when recovery attempts exceed this rate per minute
    #[serde(default = "default_recovery_rate_threshold")]
    pub recovery_rate_threshold: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct OrbitTransactionStatus {
    /// Current phase of the transaction system
    pub phase: Option<TransactionPhase>,

    /// Number of active transactions
    pub active_transactions: Option<u32>,

    /// Number of completed transactions
    pub completed_transactions: Option<u64>,

    /// Number of failed transactions
    pub failed_transactions: Option<u64>,

    /// Current coordinator information
    pub coordinator: Option<CoordinatorStatus>,

    /// Conditions affecting the transaction system
    #[serde(default)]
    pub conditions: Vec<TransactionCondition>,

    /// Performance metrics
    pub metrics: Option<TransactionMetrics>,

    /// Last observed generation
    pub observed_generation: Option<i64>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub enum TransactionPhase {
    Pending,
    Initializing,
    Running,
    Recovering,
    Failed,
    Terminating,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct CoordinatorStatus {
    /// Node ID of the current coordinator
    pub node_id: String,

    /// Pod name hosting the coordinator
    pub pod_name: String,

    /// Coordinator started at timestamp
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// Number of transactions coordinated
    pub transactions_coordinated: u64,

    /// Coordinator health status
    pub health_status: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct TransactionCondition {
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
pub struct TransactionMetrics {
    /// Average transaction duration in milliseconds
    pub avg_duration_ms: f64,

    /// Transaction success rate percentage
    pub success_rate: f32,

    /// Transactions per second
    pub transactions_per_second: f32,

    /// Current number of active transactions
    pub current_active: u32,

    /// Peak concurrent transactions
    pub peak_concurrent: u32,

    /// Number of recovery operations
    pub recovery_operations: u64,

    /// Last updated timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

// Default value functions
fn default_max_concurrent_transactions() -> u32 {
    100
}

fn default_batch_size() -> u32 {
    10
}

fn default_enable_clustering() -> bool {
    true
}

fn default_heartbeat_interval() -> u32 {
    30
}

fn default_transaction_timeout() -> u32 {
    300
}

fn default_max_timeout() -> u32 {
    3600
}

fn default_prepare_timeout() -> u32 {
    30
}

fn default_commit_timeout() -> u32 {
    30
}

fn default_abort_timeout() -> u32 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_backoff_ms() -> u32 {
    1000
}

fn default_storage_backend() -> String {
    "sqlite".to_string()
}

fn default_max_db_connections() -> u32 {
    10
}

fn default_connection_timeout() -> u32 {
    30
}

fn default_enable_wal() -> bool {
    true
}

fn default_log_retention_days() -> u32 {
    30
}

fn default_enable_compression() -> bool {
    true
}

fn default_enable_recovery() -> bool {
    true
}

fn default_recovery_interval() -> u32 {
    60
}

fn default_max_recovery_attempts() -> u32 {
    3
}

fn default_recovery_timeout() -> u32 {
    300
}

fn default_enable_failover() -> bool {
    true
}

fn default_failover_timeout() -> u32 {
    180
}

fn default_enable_metrics() -> bool {
    true
}

fn default_metrics_interval() -> u32 {
    30
}

fn default_enable_tracing() -> bool {
    false
}

fn default_failure_rate_threshold() -> f32 {
    5.0
}

fn default_duration_threshold() -> f32 {
    30.0
}

fn default_concurrent_threshold() -> u32 {
    80
}

fn default_recovery_rate_threshold() -> f32 {
    10.0
}
