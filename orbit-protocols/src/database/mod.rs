//! Database Actor System for PostgreSQL/pgvector/TimescaleDB
//!
//! This module provides a comprehensive actor-based system for database operations,
//! including PostgreSQL, pgvector, and TimescaleDB functionality with full ACID
//! compliance, connection pooling, and distributed processing capabilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub mod actors;
pub mod factory;
pub mod messages;
pub mod storage;
pub mod testing;

/// Re-export main actor types
pub use actors::{PostgresActor, TimescaleActor, VectorDatabaseActor};
pub use factory::DatabaseActorFactory;
pub use messages::{DatabaseMessage, DatabaseResponse, QueryRequest, QueryResponse};

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection parameters
    pub postgres: PostgresConfig,
    /// Vector database configuration
    pub vector: VectorConfig,
    /// TimescaleDB configuration
    pub timescale: TimescaleConfig,
    /// Connection pool settings
    pub pool: ConnectionPoolConfig,
    /// Performance tuning settings
    pub performance: PerformanceConfig,
}

/// PostgreSQL specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub schema: Option<String>,
    pub ssl_mode: SslMode,
    pub application_name: String,
    pub timezone: String,
    pub statement_timeout: Duration,
    pub lock_timeout: Duration,
    pub idle_in_transaction_timeout: Duration,
}

/// Vector database configuration for pgvector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    /// Default vector dimension
    pub default_dimension: usize,
    /// Index types to use
    pub default_index_type: VectorIndexType,
    /// IVFFLAT configuration
    pub ivfflat_lists: u32,
    /// HNSW configuration
    pub hnsw_m: u32,
    pub hnsw_ef_construction: u32,
    pub hnsw_ef_search: u32,
    /// Similarity search settings
    pub similarity_threshold: f32,
    pub max_results: usize,
    /// Vector storage format
    pub storage_format: VectorStorageFormat,
}

/// TimescaleDB specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimescaleConfig {
    /// Default chunk time interval
    pub chunk_time_interval: Duration,
    /// Compression settings
    pub enable_compression: bool,
    pub compression_after: Duration,
    pub compression_segment_by: Vec<String>,
    /// Retention policy
    pub retention_policy: Option<Duration>,
    /// Continuous aggregates
    pub enable_continuous_aggregates: bool,
    pub aggregate_refresh_interval: Duration,
    /// Background job settings
    pub enable_background_jobs: bool,
    pub job_schedule_interval: Duration,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub test_before_acquire: bool,
    pub fair_queue: bool,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Query optimization settings
    pub enable_prepared_statements: bool,
    pub statement_cache_size: usize,
    pub query_timeout: Duration,
    /// Memory settings
    pub work_mem: String,
    pub shared_buffers: String,
    pub effective_cache_size: String,
    /// Parallel processing
    pub max_parallel_workers: u32,
    pub max_parallel_workers_per_gather: u32,
    /// Logging and monitoring
    pub log_slow_queries: bool,
    pub slow_query_threshold: Duration,
    pub enable_query_stats: bool,
}

/// SSL modes for PostgreSQL connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

/// Vector index types supported by pgvector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorIndexType {
    /// IVFFLAT index for large datasets
    Ivfflat,
    /// HNSW index for high accuracy
    Hnsw,
    /// Brute force (no index) for small datasets
    BruteForce,
}

/// Vector storage format options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorStorageFormat {
    /// Full precision vectors
    Vector,
    /// Half precision vectors (float16)
    HalfVector,
    /// Sparse vectors
    SparseVector,
    /// Binary vectors
    BinaryVector,
}

/// Actor lifecycle events specific to database actors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseActorEvent {
    /// Connection established
    Connected {
        database_name: String,
        connection_count: u32,
    },
    /// Connection lost
    Disconnected {
        database_name: String,
        error: String,
    },
    /// Transaction started
    TransactionStarted {
        transaction_id: String,
        isolation_level: IsolationLevel,
    },
    /// Transaction committed
    TransactionCommitted {
        transaction_id: String,
        duration_ms: u64,
    },
    /// Transaction rolled back
    TransactionRolledBack {
        transaction_id: String,
        reason: String,
    },
    /// Query executed
    QueryExecuted {
        query_hash: String,
        duration_ms: u64,
        rows_affected: Option<u64>,
    },
    /// Performance metric updated
    MetricsUpdated { metrics: DatabaseMetrics },
}

/// Transaction isolation levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

/// Database performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetrics {
    /// Connection pool metrics
    pub pool_size: u32,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub pending_requests: u32,
    /// Query performance metrics
    pub queries_per_second: f64,
    pub average_query_time_ms: f64,
    pub slow_queries_count: u64,
    /// Transaction metrics
    pub active_transactions: u32,
    pub committed_transactions: u64,
    pub rollback_count: u64,
    /// Error metrics
    pub error_rate: f64,
    pub last_error: Option<String>,
    /// Cache metrics
    pub cache_hit_ratio: f64,
    pub cache_size_bytes: u64,
    /// Vector-specific metrics
    pub vector_searches_per_second: Option<f64>,
    pub average_vector_search_time_ms: Option<f64>,
    /// TimescaleDB-specific metrics
    pub chunks_created: Option<u64>,
    pub compression_ratio: Option<f64>,
    pub hypertables_count: Option<u32>,
}

/// Query execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryContext {
    /// Optional transaction ID if running within a transaction
    pub transaction_id: Option<String>,
    /// Query timeout
    pub timeout: Duration,
    /// Whether to use prepared statements
    pub use_prepared: bool,
    /// Query tags for monitoring
    pub tags: HashMap<String, String>,
    /// Client information
    pub client_info: Option<ClientInfo>,
}

/// Client connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_id: String,
    pub application_name: String,
    pub ip_address: String,
    pub connected_at: DateTime<Utc>,
    pub protocol_version: String,
}

/// Error types for database operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseError {
    /// Connection errors
    ConnectionFailed { database: String, error: String },
    /// Query execution errors
    QueryFailed {
        query: String,
        error: String,
        error_code: Option<String>,
    },
    /// Transaction errors
    TransactionFailed {
        transaction_id: String,
        operation: String,
        error: String,
    },
    /// Schema errors
    SchemaError {
        operation: String,
        object_name: String,
        error: String,
    },
    /// Vector operation errors
    VectorOperationFailed {
        operation: String,
        dimension: Option<usize>,
        error: String,
    },
    /// TimescaleDB errors
    TimescaleOperationFailed {
        operation: String,
        hypertable: Option<String>,
        error: String,
    },
    /// Actor system errors
    ActorError {
        actor_type: String,
        actor_id: String,
        error: String,
    },
    /// Configuration errors
    ConfigurationError { setting: String, error: String },
    /// Timeout errors
    TimeoutError { operation: String, timeout_ms: u64 },
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            postgres: PostgresConfig::default(),
            vector: VectorConfig::default(),
            timescale: TimescaleConfig::default(),
            pool: ConnectionPoolConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "orbit".to_string(),
            username: "postgres".to_string(),
            password: "".to_string(),
            schema: None,
            ssl_mode: SslMode::Prefer,
            application_name: "orbit-rs".to_string(),
            timezone: "UTC".to_string(),
            statement_timeout: Duration::from_secs(30),
            lock_timeout: Duration::from_secs(10),
            idle_in_transaction_timeout: Duration::from_secs(300),
        }
    }
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            default_dimension: 384,
            default_index_type: VectorIndexType::Hnsw,
            ivfflat_lists: 1000,
            hnsw_m: 16,
            hnsw_ef_construction: 64,
            hnsw_ef_search: 40,
            similarity_threshold: 0.7,
            max_results: 100,
            storage_format: VectorStorageFormat::Vector,
        }
    }
}

impl Default for TimescaleConfig {
    fn default() -> Self {
        Self {
            chunk_time_interval: Duration::from_secs(86400), // 1 day
            enable_compression: true,
            compression_after: Duration::from_secs(604800), // 1 week
            compression_segment_by: vec!["series_id".to_string()],
            retention_policy: Some(Duration::from_secs(31536000)), // 1 year
            enable_continuous_aggregates: true,
            aggregate_refresh_interval: Duration::from_secs(3600), // 1 hour
            enable_background_jobs: true,
            job_schedule_interval: Duration::from_secs(86400), // 1 day
        }
    }
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 20,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600), // 10 minutes
            max_lifetime: Duration::from_secs(3600), // 1 hour
            test_before_acquire: true,
            fair_queue: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_prepared_statements: true,
            statement_cache_size: 1000,
            query_timeout: Duration::from_secs(60),
            work_mem: "64MB".to_string(),
            shared_buffers: "256MB".to_string(),
            effective_cache_size: "1GB".to_string(),
            max_parallel_workers: 8,
            max_parallel_workers_per_gather: 2,
            log_slow_queries: true,
            slow_query_threshold: Duration::from_millis(1000),
            enable_query_stats: true,
        }
    }
}

impl Default for QueryContext {
    fn default() -> Self {
        Self {
            transaction_id: None,
            timeout: Duration::from_secs(30),
            use_prepared: true,
            tags: HashMap::new(),
            client_info: None,
        }
    }
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::ConnectionFailed { database, error } => {
                write!(f, "Connection to database '{}' failed: {}", database, error)
            }
            DatabaseError::QueryFailed {
                query,
                error,
                error_code,
            } => {
                write!(
                    f,
                    "Query failed: {} (code: {:?}). Query: {}",
                    error, error_code, query
                )
            }
            DatabaseError::TransactionFailed {
                transaction_id,
                operation,
                error,
            } => {
                write!(
                    f,
                    "Transaction {} operation '{}' failed: {}",
                    transaction_id, operation, error
                )
            }
            DatabaseError::SchemaError {
                operation,
                object_name,
                error,
            } => {
                write!(
                    f,
                    "Schema operation '{}' on '{}' failed: {}",
                    operation, object_name, error
                )
            }
            DatabaseError::VectorOperationFailed {
                operation,
                dimension,
                error,
            } => {
                write!(
                    f,
                    "Vector operation '{}' failed (dim: {:?}): {}",
                    operation, dimension, error
                )
            }
            DatabaseError::TimescaleOperationFailed {
                operation,
                hypertable,
                error,
            } => {
                write!(
                    f,
                    "TimescaleDB operation '{}' failed (table: {:?}): {}",
                    operation, hypertable, error
                )
            }
            DatabaseError::ActorError {
                actor_type,
                actor_id,
                error,
            } => {
                write!(f, "Actor {}:{} error: {}", actor_type, actor_id, error)
            }
            DatabaseError::ConfigurationError { setting, error } => {
                write!(f, "Configuration error for '{}': {}", setting, error)
            }
            DatabaseError::TimeoutError {
                operation,
                timeout_ms,
            } => {
                write!(
                    f,
                    "Operation '{}' timed out after {}ms",
                    operation, timeout_ms
                )
            }
        }
    }
}

impl std::error::Error for DatabaseError {}

/// Utility functions for database operations
pub mod utils {
    use super::*;

    /// Generate a unique transaction ID
    pub fn generate_transaction_id() -> String {
        format!("tx_{}", Uuid::new_v4())
    }

    /// Generate a query hash for caching and monitoring
    pub fn generate_query_hash(sql: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        sql.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Create default query context with optional transaction
    pub fn create_query_context(transaction_id: Option<String>) -> QueryContext {
        QueryContext {
            transaction_id,
            ..Default::default()
        }
    }

    /// Validate vector dimension
    pub fn validate_vector_dimension(dimension: usize) -> Result<(), String> {
        if dimension == 0 {
            return Err("Vector dimension cannot be zero".to_string());
        }
        if dimension > 16000 {
            return Err("Vector dimension too large (max 16000)".to_string());
        }
        Ok(())
    }

    /// Parse PostgreSQL connection string
    pub fn parse_connection_string(conn_str: &str) -> Result<PostgresConfig, String> {
        // Simplified parser - in production use a proper PostgreSQL URL parser
        let mut config = PostgresConfig::default();

        for part in conn_str.split_whitespace() {
            if let Some((key, value)) = part.split_once('=') {
                match key.to_lowercase().as_str() {
                    "host" => config.host = value.to_string(),
                    "port" => {
                        config.port = value.parse().map_err(|e| format!("Invalid port: {}", e))?
                    }
                    "dbname" | "database" => config.database = value.to_string(),
                    "user" | "username" => config.username = value.to_string(),
                    "password" => config.password = value.to_string(),
                    "sslmode" => {
                        config.ssl_mode = match value.to_lowercase().as_str() {
                            "disable" => SslMode::Disable,
                            "allow" => SslMode::Allow,
                            "prefer" => SslMode::Prefer,
                            "require" => SslMode::Require,
                            "verify-ca" => SslMode::VerifyCa,
                            "verify-full" => SslMode::VerifyFull,
                            _ => return Err(format!("Invalid SSL mode: {}", value)),
                        };
                    }
                    _ => {} // Ignore unknown parameters
                }
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configs() {
        let config = DatabaseConfig::default();
        assert_eq!(config.postgres.host, "localhost");
        assert_eq!(config.postgres.port, 5432);
        assert_eq!(config.vector.default_dimension, 384);
        assert!(config.timescale.enable_compression);
        assert_eq!(config.pool.min_connections, 2);
        assert_eq!(config.pool.max_connections, 20);
    }

    #[test]
    fn test_query_hash_generation() {
        let sql1 = "SELECT * FROM users WHERE id = 1";
        let sql2 = "SELECT * FROM users WHERE id = 2";

        let hash1 = utils::generate_query_hash(sql1);
        let hash2 = utils::generate_query_hash(sql2);

        assert_ne!(hash1, hash2);
        assert_eq!(hash1, utils::generate_query_hash(sql1)); // Should be consistent
    }

    #[test]
    fn test_vector_dimension_validation() {
        assert!(utils::validate_vector_dimension(384).is_ok());
        assert!(utils::validate_vector_dimension(0).is_err());
        assert!(utils::validate_vector_dimension(20000).is_err());
    }

    #[test]
    fn test_connection_string_parsing() {
        let conn_str = "host=localhost port=5432 dbname=test user=postgres password=secret";
        let config = utils::parse_connection_string(conn_str).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "test");
        assert_eq!(config.username, "postgres");
        assert_eq!(config.password, "secret");
    }
}
