//! REST API request/response models with OpenAPI documentation

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Actor creation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateActorRequest {
    /// Actor type (e.g., "GreeterActor", "CounterActor")
    #[schema(example = "GreeterActor")]
    pub actor_type: String,

    /// Actor key (string, int32, int64, or null)
    #[schema(example = json!({"StringKey": {"key": "my-actor"}}))]
    pub key: serde_json::Value,

    /// Initial state (optional)
    #[schema(example = json!({"count": 0}))]
    pub initial_state: Option<serde_json::Value>,
}

/// Actor invocation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InvokeActorRequest {
    /// Method name to invoke
    #[schema(example = "greet")]
    pub method: String,

    /// Method arguments as JSON array
    #[schema(example = json!(["World"]))]
    pub args: Vec<serde_json::Value>,

    /// Optional timeout in milliseconds
    #[schema(example = 5000)]
    pub timeout_ms: Option<u64>,
}

/// Actor state update request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateActorStateRequest {
    /// New state as JSON object
    #[schema(example = json!({"count": 42, "message": "updated"}))]
    pub state: serde_json::Value,

    /// Optional merge strategy: "replace" or "merge"
    #[schema(example = "merge")]
    pub strategy: Option<String>,
}

/// Generic success response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SuccessResponse<T> {
    /// Success flag
    pub success: bool,

    /// Response data
    pub data: T,

    /// Optional message
    pub message: Option<String>,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Error flag
    pub error: bool,

    /// Error code
    #[schema(example = "ACTOR_NOT_FOUND")]
    pub code: String,

    /// Human-readable error message
    #[schema(example = "Actor with ID 'my-actor' not found")]
    pub message: String,

    /// Optional error details
    pub details: Option<serde_json::Value>,
}

/// Actor information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActorInfo {
    /// Actor type
    pub actor_type: String,

    /// Actor key
    pub key: serde_json::Value,

    /// Actor state
    pub state: serde_json::Value,

    /// Node ID where actor is hosted
    pub node_id: Option<String>,

    /// Actor status: "active", "inactive", "deactivating"
    #[schema(example = "active")]
    pub status: String,

    /// Last activity timestamp (ISO 8601)
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_activity: Option<String>,
}

/// Transaction creation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BeginTransactionRequest {
    /// Optional transaction timeout in milliseconds
    #[schema(example = 30000)]
    pub timeout_ms: Option<u64>,

    /// Optional transaction metadata
    pub metadata: Option<serde_json::Value>,
}

/// Transaction operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionOperation {
    /// Target actor type
    pub actor_type: String,

    /// Target actor key
    pub key: serde_json::Value,

    /// Method to invoke
    #[schema(example = "debit")]
    pub method: String,

    /// Method arguments
    #[schema(example = json!([100]))]
    pub args: Vec<serde_json::Value>,

    /// Optional compensation data
    pub compensation: Option<serde_json::Value>,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransactionInfo {
    /// Transaction ID
    pub transaction_id: String,

    /// Transaction status: "preparing", "prepared", "committing", "committed", "aborting", "aborted"
    #[schema(example = "committed")]
    pub status: String,

    /// Operations in this transaction
    pub operations: Vec<TransactionOperation>,

    /// Creation timestamp
    pub created_at: String,

    /// Completion timestamp (if completed)
    pub completed_at: Option<String>,
}

/// Paginated list response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PagedResponse<T> {
    /// Items in current page
    pub items: Vec<T>,

    /// Total number of items
    pub total: usize,

    /// Current page number (0-indexed)
    pub page: usize,

    /// Page size
    pub page_size: usize,

    /// Total number of pages
    pub total_pages: usize,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// Actor state changed
    ActorStateChanged {
        actor_type: String,
        key: serde_json::Value,
        state: serde_json::Value,
    },

    /// Actor activated
    ActorActivated {
        actor_type: String,
        key: serde_json::Value,
        node_id: String,
    },

    /// Actor deactivated
    ActorDeactivated {
        actor_type: String,
        key: serde_json::Value,
    },

    /// Transaction event
    TransactionEvent {
        transaction_id: String,
        status: String,
        message: Option<String>,
    },

    /// System event
    SystemEvent {
        event_type: String,
        data: serde_json::Value,
    },

    /// Subscription acknowledgment
    SubscriptionAck {
        subscription_id: String,
        filters: Vec<String>,
    },

    /// Error message
    Error { code: String, message: String },
}

/// WebSocket subscription request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubscribeRequest {
    /// Event types to subscribe to
    pub event_types: Vec<String>,

    /// Optional filters (actor_type, key patterns, etc.)
    pub filters: Option<serde_json::Value>,
}

/// Natural language query request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NaturalLanguageQueryRequest {
    /// Natural language query
    #[schema(example = "Show me all users from California")]
    pub query: String,

    /// Maximum number of results to return
    #[schema(example = 100)]
    pub limit: Option<usize>,

    /// Maximum number of preview rows
    #[schema(example = 10)]
    pub max_preview_rows: Option<usize>,

    /// Whether to execute the query or just generate SQL
    #[schema(example = true)]
    pub execute: Option<bool>,
}

/// Natural language query response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NaturalLanguageQueryResponse {
    /// Generated SQL query
    pub sql: String,

    /// Query parameters (for parameterized queries)
    pub parameters: Vec<serde_json::Value>,

    /// Query type (read, write, analysis)
    pub query_type: String,

    /// Estimated complexity
    pub complexity: String,

    /// Optimization hints
    pub optimization_hints: Vec<String>,

    /// Query results (if executed)
    pub results: Option<QueryResults>,

    /// Processing metadata
    pub metadata: QueryMetadata,
}

/// Query results
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryResults {
    /// Human-readable summary
    pub summary: String,

    /// Data preview (first N rows)
    pub data_preview: Vec<serde_json::Value>,

    /// Total row count
    pub total_rows: usize,

    /// Whether full result is available in preview
    pub full_result_available: bool,

    /// Continuation token for pagination
    pub continuation_token: Option<String>,

    /// Statistical summary
    pub statistics: Option<serde_json::Value>,

    /// Visualization hints
    pub visualization_hints: Vec<VisualizationHint>,
}

/// Visualization hint
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VisualizationHint {
    /// Visualization type
    pub viz_type: String,

    /// Recommended columns
    pub columns: Vec<String>,

    /// Description
    pub description: String,
}

/// Query metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryMetadata {
    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// NLP processing time in milliseconds
    pub nlp_time_ms: Option<u64>,

    /// SQL generation time in milliseconds
    pub sql_generation_time_ms: Option<u64>,

    /// Query execution time in milliseconds (if executed)
    pub execution_time_ms: Option<u64>,

    /// Result processing time in milliseconds (if executed)
    pub result_processing_time_ms: Option<u64>,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
}

impl<T: Serialize> SuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: Some(message.into()),
        }
    }
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: true,
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            error: true,
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

impl<T> PagedResponse<T> {
    pub fn new(items: Vec<T>, total: usize, page: usize, page_size: usize) -> Self {
        let total_pages = total.div_ceil(page_size);
        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }
}

// ============ SQL Query Endpoint Models ============

/// SQL query execution request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SqlQueryRequest {
    /// SQL query to execute
    #[schema(example = "SELECT * FROM users WHERE status = $1 LIMIT 10")]
    pub query: String,

    /// Query parameters for prepared statements
    #[schema(example = json!(["active"]))]
    pub parameters: Option<Vec<serde_json::Value>>,

    /// Maximum number of rows to return (default: 1000)
    #[schema(example = 1000)]
    pub limit: Option<usize>,

    /// Query timeout in milliseconds (default: 30000)
    #[schema(example = 30000)]
    pub timeout_ms: Option<u64>,

    /// Return query plan instead of executing (EXPLAIN)
    #[schema(example = false)]
    pub explain: Option<bool>,
}

/// SQL query response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SqlQueryResponse {
    /// Column names
    pub columns: Vec<ColumnInfo>,

    /// Query result rows
    pub rows: Vec<Vec<serde_json::Value>>,

    /// Number of rows returned
    pub row_count: usize,

    /// Number of rows affected (for INSERT/UPDATE/DELETE)
    pub rows_affected: Option<u64>,

    /// Query execution time in milliseconds
    pub execution_time_ms: u64,

    /// Whether more rows are available (limit reached)
    pub has_more: bool,

    /// Query plan (if explain=true)
    pub query_plan: Option<serde_json::Value>,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,

    /// Column data type
    #[schema(example = "varchar")]
    pub data_type: String,

    /// Whether column is nullable
    pub nullable: bool,
}

/// Batch SQL query request (for multiple queries)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchSqlQueryRequest {
    /// List of SQL queries to execute
    pub queries: Vec<SqlQueryRequest>,

    /// Whether to run in a transaction
    #[schema(example = true)]
    pub transaction: Option<bool>,

    /// Stop on first error (default: true)
    #[schema(example = true)]
    pub stop_on_error: Option<bool>,
}

/// Batch SQL query response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchSqlQueryResponse {
    /// Results for each query
    pub results: Vec<BatchQueryResult>,

    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,

    /// Number of successful queries
    pub successful: usize,

    /// Number of failed queries
    pub failed: usize,
}

/// Individual query result in batch
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchQueryResult {
    /// Query index (0-based)
    pub index: usize,

    /// Whether query succeeded
    pub success: bool,

    /// Query result (if success)
    pub result: Option<SqlQueryResponse>,

    /// Error message (if failed)
    pub error: Option<String>,
}

/// Table listing response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableInfo {
    /// Table name
    pub name: String,

    /// Schema name
    #[schema(example = "public")]
    pub schema: String,

    /// Table type (TABLE, VIEW, etc.)
    #[schema(example = "TABLE")]
    pub table_type: String,

    /// Estimated row count
    pub estimated_rows: Option<u64>,

    /// Column count
    pub column_count: usize,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DatabaseStats {
    /// Total number of tables
    pub table_count: usize,

    /// Total number of indexes
    pub index_count: usize,

    /// Total database size in bytes
    pub size_bytes: Option<u64>,

    /// Active connections
    pub active_connections: usize,

    /// Server uptime in seconds
    pub uptime_seconds: u64,

    /// Database version
    pub version: String,
}

// ============ Schema Management Models ============

/// Schema information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SchemaInfo {
    /// Schema name
    #[schema(example = "public")]
    pub name: String,

    /// Schema owner
    #[schema(example = "postgres")]
    pub owner: String,

    /// Number of tables in schema
    pub table_count: usize,

    /// Number of views in schema
    pub view_count: usize,
}

/// Table column information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableColumn {
    /// Column name
    #[schema(example = "id")]
    pub name: String,

    /// Column data type
    #[schema(example = "integer")]
    pub data_type: String,

    /// Whether column allows NULL
    pub nullable: bool,

    /// Default value expression
    pub default_value: Option<String>,

    /// Whether column is part of primary key
    pub is_primary_key: bool,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IndexInfo {
    /// Index name
    #[schema(example = "users_pkey")]
    pub name: String,

    /// Columns in the index
    pub columns: Vec<String>,

    /// Whether index enforces uniqueness
    pub unique: bool,

    /// Index type (btree, hash, gin, gist, etc.)
    #[schema(example = "btree")]
    pub index_type: String,
}

/// Detailed table description
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableDescription {
    /// Schema name
    #[schema(example = "public")]
    pub schema: String,

    /// Table name
    #[schema(example = "users")]
    pub name: String,

    /// Table type (TABLE, VIEW, MATERIALIZED VIEW)
    #[schema(example = "TABLE")]
    pub table_type: String,

    /// Column definitions
    pub columns: Vec<TableColumn>,

    /// Primary key columns
    pub primary_key: Option<Vec<String>>,

    /// Table indexes
    pub indexes: Vec<IndexInfo>,

    /// Estimated row count
    pub estimated_rows: Option<u64>,

    /// Table size in bytes
    pub size_bytes: Option<u64>,
}

// ============ Cluster Management Models ============

/// Cluster node information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterNodeInfo {
    /// Unique node identifier
    #[schema(example = "node-1")]
    pub node_id: String,

    /// Node address
    #[schema(example = "192.168.1.10:50051")]
    pub address: String,

    /// Node status (healthy, unhealthy, unreachable)
    #[schema(example = "healthy")]
    pub status: String,

    /// Node role (leader, follower)
    #[schema(example = "leader")]
    pub role: String,

    /// CPU usage percentage
    pub cpu_usage: Option<f64>,

    /// Memory usage percentage
    pub memory_usage: Option<f64>,

    /// Disk usage percentage
    pub disk_usage: Option<f64>,

    /// Node uptime in seconds
    pub uptime_seconds: u64,

    /// Number of actors on this node
    pub actor_count: usize,

    /// Number of active connections
    pub connection_count: usize,
}

/// Cluster status information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterStatus {
    /// Cluster identifier
    #[schema(example = "orbit-cluster-1")]
    pub cluster_id: String,

    /// Overall cluster health
    pub healthy: bool,

    /// Total number of nodes
    pub total_nodes: usize,

    /// Number of healthy nodes
    pub healthy_nodes: usize,

    /// Number of unhealthy nodes
    pub unhealthy_nodes: usize,

    /// Total actors across cluster
    pub total_actors: usize,

    /// Replication factor
    pub replication_factor: usize,

    /// Consistency level
    #[schema(example = "quorum")]
    pub consistency_level: String,
}

// ============ Query History Models ============

/// Query history entry
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QueryHistoryEntry {
    /// Unique query identifier
    pub query_id: String,

    /// SQL query text
    #[schema(example = "SELECT * FROM users WHERE status = 'active'")]
    pub query: String,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Number of rows returned
    pub rows_returned: usize,

    /// Query execution timestamp (ISO 8601)
    pub timestamp: String,

    /// Query status (completed, failed, cancelled)
    #[schema(example = "completed")]
    pub status: String,

    /// User who executed the query
    pub user: Option<String>,
}

// ============ Configuration Models ============

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProtocolConfig {
    /// Protocol name
    #[schema(example = "PostgreSQL")]
    pub name: String,

    /// Port number
    #[schema(example = 5432)]
    pub port: u16,

    /// Whether protocol is enabled
    pub enabled: bool,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorageConfig {
    /// Storage engine type
    #[schema(example = "RocksDB")]
    pub engine: String,

    /// Data directory path
    #[schema(example = "/var/lib/orbit/data")]
    pub data_dir: String,

    /// Cache size in MB
    pub cache_size_mb: usize,

    /// Whether WAL is enabled
    pub wal_enabled: bool,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClusterConfig {
    /// Whether clustering is enabled
    pub enabled: bool,

    /// Node identifier
    #[schema(example = "node-1")]
    pub node_id: String,

    /// Replication factor
    pub replication_factor: usize,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerConfig {
    /// Server version
    pub version: String,

    /// Protocol configurations
    pub protocols: Vec<ProtocolConfig>,

    /// Storage configuration
    pub storage: StorageConfig,

    /// Cluster configuration
    pub cluster: ClusterConfig,
}
