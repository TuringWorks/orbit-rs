//! Enhanced configuration system for multi-protocol Orbit server
//!
//! Supports configuration of multiple protocol servers including:
//! - gRPC actor API
//! - PostgreSQL wire protocol
//! - Redis RESP protocol
//! - REST HTTP API
//! - Cypher (Neo4j-compatible) protocol
//! - MCP (Model Context Protocol)

use orbit_shared::pooling::{LoadBalancingStrategy, PoolTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Main configuration for the Orbit server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitServerConfig {
    /// Server identification
    pub server: ServerConfig,

    /// Actor system configuration
    pub actor_system: ActorSystemConfig,

    /// Protocol server configurations
    pub protocols: ProtocolsConfig,

    /// Security and authentication
    pub security: SecurityConfig,

    /// Performance and resource limits
    pub performance: PerformanceConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Monitoring and metrics
    pub monitoring: MonitoringConfig,

    /// Connection pooling configuration
    pub pooling: PoolingConfig,

    /// Persistence configuration
    pub persistence: Option<PersistenceConfig>,

    /// Storage configuration (tiered and cold storage)
    pub storage: Option<StorageConfig>,
}

/// Server identification and basic settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Unique server/node identifier
    pub node_id: Option<String>,

    /// Default bind address for all servers
    pub bind_address: String,

    /// Environment (development, staging, production)
    pub environment: Environment,

    /// Data directory for persistence
    pub data_dir: PathBuf,

    /// Configuration directory
    pub config_dir: PathBuf,
}

/// Environment type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

/// Actor system specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSystemConfig {
    /// Maximum number of actors per node
    pub max_actors: usize,

    /// Actor mailbox size
    pub mailbox_size: usize,

    /// Actor supervision strategy
    pub supervision_strategy: SupervisionStrategy,

    /// Clustering configuration
    pub cluster: Option<ClusterConfig>,
}

/// Actor supervision strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SupervisionStrategy {
    OneForOne,
    OneForAll,
    RestForOne,
}

/// Cluster configuration for distributed deployments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Seed nodes for cluster discovery
    pub seed_nodes: Vec<String>,

    /// Gossip protocol port
    pub gossip_port: u16,

    /// Cluster heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,

    /// Node failure timeout in milliseconds
    pub failure_timeout_ms: u64,
}

/// Advanced connection pooling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolingConfig {
    /// Enable connection pooling
    pub enabled: bool,

    /// Minimum connections per pool
    pub min_connections: usize,

    /// Maximum connections per pool  
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Idle timeout in seconds
    pub idle_timeout_secs: u64,

    /// Maximum connection lifetime in seconds
    pub max_lifetime_secs: u64,

    /// Health check interval in seconds
    pub health_check_interval_secs: u64,

    /// Load balancing strategy
    pub load_balancing_strategy: LoadBalancingStrategy,

    /// Pool tier
    pub tier: PoolTier,

    /// Enable dynamic pool sizing
    pub enable_dynamic_sizing: bool,

    /// Target utilization for dynamic sizing (0.0-1.0)
    pub target_utilization: f64,

    /// Circuit breaker configuration
    pub circuit_breaker: PoolCircuitBreakerConfig,

    /// Per-protocol pooling overrides
    pub protocol_overrides: HashMap<String, ProtocolPoolConfig>,
}

/// Circuit breaker configuration for connection pools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCircuitBreakerConfig {
    /// Enable circuit breaker
    pub enabled: bool,

    /// Failure threshold before opening circuit
    pub failure_threshold: u32,

    /// Time window for counting failures in seconds
    pub failure_window_secs: u64,

    /// Recovery timeout in seconds
    pub recovery_timeout_secs: u64,

    /// Success threshold to close circuit
    pub success_threshold: u32,

    /// Maximum half-open attempts
    pub half_open_max_calls: u32,
}

/// Per-protocol pool configuration overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolPoolConfig {
    /// Override minimum connections
    pub min_connections: Option<usize>,

    /// Override maximum connections
    pub max_connections: Option<usize>,

    /// Override load balancing strategy
    pub load_balancing_strategy: Option<LoadBalancingStrategy>,

    /// Protocol-specific pool nodes
    pub nodes: Vec<PoolNodeConfig>,
}

/// Pool node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolNodeConfig {
    /// Node identifier
    pub node_id: String,

    /// Node address
    pub address: String,

    /// Maximum connections for this node
    pub max_connections: usize,

    /// Node weight for weighted load balancing
    pub weight: u32,

    /// Node-specific configuration
    pub config: HashMap<String, serde_json::Value>,
}

/// Configuration for all protocol servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolsConfig {
    /// gRPC server configuration
    pub grpc: Option<GrpcConfig>,

    /// PostgreSQL wire protocol server
    pub postgresql: Option<PostgresqlConfig>,

    /// Redis RESP protocol server
    pub redis: Option<RedisConfig>,

    /// HTTP REST API server
    pub rest: Option<RestConfig>,

    /// Cypher (Neo4j-compatible) server
    pub cypher: Option<CypherConfig>,

    /// MCP (Model Context Protocol) server
    pub mcp: Option<McpConfig>,

    /// CQL (Cassandra Query Language) server
    pub cql: Option<CqlConfig>,

    /// MySQL wire protocol server
    pub mysql: Option<MySqlConfig>,

    /// AQL (ArangoDB Query Language) server
    pub aql: Option<AqlConfig>,

    /// OrbitQL query language server
    pub orbitql: Option<OrbitQLConfig>,
}

/// gRPC server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// Enable gRPC server
    pub enabled: bool,

    /// gRPC server port
    pub port: u16,

    /// Maximum concurrent streams
    pub max_concurrent_streams: u32,

    /// Maximum message size
    pub max_message_size: usize,

    /// Keep-alive configuration
    pub keep_alive: Option<KeepAliveConfig>,

    /// TLS configuration
    pub tls: Option<TlsConfig>,
}

/// PostgreSQL server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresqlConfig {
    /// Enable PostgreSQL wire protocol server
    pub enabled: bool,

    /// PostgreSQL server port (default: 5432)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// SQL engine configuration
    pub sql_engine: SqlEngineConfig,

    /// Vector operations configuration
    pub vector_ops: VectorOpsConfig,

    /// PostgreSQL-specific features
    pub features: PostgresqlFeatures,
}

/// Redis server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Enable Redis RESP protocol server
    pub enabled: bool,

    /// Redis server port (default: 6379)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Redis command configuration
    pub commands: RedisCommandConfig,

    /// Vector operations configuration
    pub vector_ops: VectorOpsConfig,

    /// Redis-specific features
    pub features: RedisFeatures,
}

/// REST API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConfig {
    /// Enable REST API server
    pub enabled: bool,

    /// REST API server port
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Request timeout in seconds
    pub request_timeout_secs: u64,

    /// CORS configuration
    pub cors: Option<CorsConfig>,

    /// Rate limiting
    pub rate_limit: Option<RateLimitConfig>,
}

/// Cypher server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CypherConfig {
    /// Enable Cypher server
    pub enabled: bool,

    /// Cypher server port (default: 7474)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Graph engine configuration
    pub graph_engine: GraphEngineConfig,
}

impl Default for CypherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 7687,
            max_connections: 100,
            graph_engine: GraphEngineConfig::default(),
        }
    }
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Enable MCP server
    pub enabled: bool,

    /// MCP server port
    pub port: u16,

    /// Supported MCP version
    pub version: String,

    /// Available tools configuration
    pub tools: McpToolsConfig,
}

/// CQL (Cassandra Query Language) server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CqlConfig {
    /// Enable CQL server
    pub enabled: bool,

    /// CQL server port (default: 9042)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// CQL protocol version (default: 4)
    pub protocol_version: u8,

    /// Enable authentication
    pub authentication_enabled: bool,
}

/// MySQL wire protocol server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlConfig {
    /// Enable MySQL server
    pub enabled: bool,

    /// MySQL server port (default: 3306)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Server version string
    pub server_version: String,

    /// Enable authentication
    pub authentication_enabled: bool,
}

/// AQL (ArangoDB Query Language) server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AqlConfig {
    /// Enable AQL server
    pub enabled: bool,

    /// AQL server port (default: 8529)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Enable authentication
    pub authentication_enabled: bool,
}

impl Default for AqlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8529,
            max_connections: 100,
            connection_timeout_secs: 30,
            authentication_enabled: false,
        }
    }
}

/// OrbitQL query language server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitQLConfig {
    /// Enable OrbitQL server
    pub enabled: bool,

    /// OrbitQL server port (default: 8081)
    pub port: u16,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,

    /// Enable authentication
    pub authentication_enabled: bool,
}

/// SQL engine specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlEngineConfig {
    /// Maximum query complexity
    pub max_query_complexity: usize,

    /// Query timeout in seconds
    pub query_timeout_secs: u64,

    /// Enable query optimization
    pub enable_optimization: bool,

    /// Enable query caching
    pub enable_caching: bool,

    /// Cache size in MB
    pub cache_size_mb: usize,
}

/// Vector operations configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorOpsConfig {
    /// Default similarity metric
    pub default_metric: String,

    /// Maximum vector dimensions
    pub max_dimensions: usize,

    /// Default batch size for vector operations
    pub batch_size: usize,

    /// Enable SIMD optimizations
    pub enable_simd: bool,

    /// Vector index configuration
    pub indexing: VectorIndexConfig,
}

/// Vector indexing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexConfig {
    /// Default index algorithm (hnsw, ivf, brute_force)
    pub default_algorithm: String,

    /// HNSW configuration
    pub hnsw: HnswConfig,

    /// IVF configuration
    pub ivf: IvfConfig,
}

/// HNSW index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Number of connections per layer
    pub m: usize,

    /// Size of dynamic candidate list
    pub ef_construction: usize,

    /// Search parameter
    pub ef_search: usize,
}

/// IVF index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IvfConfig {
    /// Number of clusters
    pub nlist: usize,

    /// Number of clusters to search
    pub nprobe: usize,
}

/// PostgreSQL-specific features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresqlFeatures {
    /// Enable pgvector extension compatibility
    pub enable_pgvector: bool,

    /// Enable PostGIS compatibility
    pub enable_postgis: bool,

    /// Enable JSON/JSONB operations
    pub enable_json: bool,

    /// Enable full-text search
    pub enable_fulltext: bool,

    /// Enable prepared statements
    pub enable_prepared_statements: bool,

    /// Enable transactions
    pub enable_transactions: bool,
}

/// Redis-specific features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisFeatures {
    /// Enable Redis Streams
    pub enable_streams: bool,

    /// Enable Redis Pub/Sub
    pub enable_pubsub: bool,

    /// Enable Redis Modules compatibility
    pub enable_modules: bool,

    /// Enable RedisSearch compatibility
    pub enable_redisearch: bool,

    /// Enable RedisJSON compatibility
    pub enable_redisjson: bool,

    /// Enable RedisGraph compatibility
    pub enable_redisgraph: bool,

    /// Enable clustering mode
    pub enable_cluster: bool,
}

/// Redis command configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCommandConfig {
    /// Enabled command groups
    pub enabled_groups: Vec<String>,

    /// Disabled specific commands
    pub disabled_commands: Vec<String>,

    /// Command timeout in seconds
    pub command_timeout_secs: u64,

    /// Maximum pipeline size
    pub max_pipeline_size: usize,
}

/// Graph engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEngineConfig {
    /// Maximum query complexity
    pub max_query_complexity: usize,

    /// Query timeout in seconds
    pub query_timeout_secs: u64,

    /// Enable query optimization
    pub enable_optimization: bool,

    /// Graph storage backend
    pub storage_backend: String,
}

impl Default for GraphEngineConfig {
    fn default() -> Self {
        Self {
            max_query_complexity: 100,
            query_timeout_secs: 30,
            enable_optimization: true,
            storage_backend: "memory".to_string(),
        }
    }
}

/// MCP tools configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsConfig {
    /// Available tools
    pub available_tools: Vec<String>,

    /// Tool-specific configurations
    pub tool_configs: HashMap<String, serde_json::Value>,
}

/// Keep-alive configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAliveConfig {
    /// Keep-alive interval in seconds
    pub interval_secs: u64,

    /// Keep-alive timeout in seconds
    pub timeout_secs: u64,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS
    pub enabled: bool,

    /// Certificate file path
    pub cert_file: PathBuf,

    /// Private key file path
    pub key_file: PathBuf,

    /// CA certificate file path (optional)
    pub ca_cert_file: Option<PathBuf>,

    /// Require client certificates
    pub require_client_cert: bool,
}

/// CORS configuration for REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    pub allowed_headers: Vec<String>,

    /// Max age for preflight requests
    pub max_age_secs: u64,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute per client
    pub requests_per_minute: usize,

    /// Burst capacity
    pub burst_capacity: usize,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Authentication configuration
    pub authentication: AuthenticationConfig,

    /// Authorization configuration
    pub authorization: AuthorizationConfig,

    /// Encryption configuration
    pub encryption: EncryptionConfig,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    /// Enable authentication
    pub enabled: bool,

    /// Authentication methods
    pub methods: Vec<AuthMethod>,

    /// JWT configuration
    pub jwt: Option<JwtConfig>,

    /// Session configuration
    pub session: Option<SessionConfig>,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Basic,
    Bearer,
    ApiKey,
    OAuth2,
    Jwt,
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// JWT secret key
    pub secret_key: String,

    /// Token expiration time in seconds
    pub expiration_secs: u64,

    /// JWT algorithm
    pub algorithm: String,
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session timeout in seconds
    pub timeout_secs: u64,

    /// Session storage backend
    pub storage: String,
}

/// Authorization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationConfig {
    /// Enable authorization
    pub enabled: bool,

    /// Authorization model (rbac, abac, acl)
    pub model: String,

    /// Default permissions
    pub default_permissions: Vec<String>,
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Encryption at rest
    pub at_rest: Option<EncryptionAtRestConfig>,

    /// Encryption in transit
    pub in_transit: Option<EncryptionInTransitConfig>,
}

/// Encryption at rest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionAtRestConfig {
    /// Enable encryption at rest
    pub enabled: bool,

    /// Encryption algorithm
    pub algorithm: String,

    /// Key management service
    pub kms: Option<KmsConfig>,
}

/// Encryption in transit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionInTransitConfig {
    /// Enable encryption in transit
    pub enabled: bool,

    /// TLS version
    pub tls_version: String,

    /// Cipher suites
    pub cipher_suites: Vec<String>,
}

/// Key management service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmsConfig {
    /// KMS provider (aws, azure, gcp, vault)
    pub provider: String,

    /// KMS endpoint
    pub endpoint: Option<String>,

    /// Key ID
    pub key_id: String,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Memory configuration
    pub memory: MemoryConfig,

    /// I/O configuration
    pub io: IoConfig,

    /// Network configuration
    pub network: NetworkConfig,

    /// CPU configuration
    pub cpu: CpuConfig,
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum memory usage in MB
    pub max_memory_mb: usize,

    /// Memory pool configuration
    pub pool_config: MemoryPoolConfig,

    /// Garbage collection settings
    pub gc_settings: GcSettings,
}

/// Memory pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolConfig {
    /// Initial pool size in MB
    pub initial_size_mb: usize,

    /// Maximum pool size in MB
    pub max_size_mb: usize,

    /// Growth factor
    pub growth_factor: f64,
}

/// Garbage collection settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcSettings {
    /// Enable automatic GC
    pub auto_gc: bool,

    /// GC interval in seconds
    pub interval_secs: u64,

    /// Memory threshold for GC trigger
    pub memory_threshold_pct: f64,
}

/// I/O configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoConfig {
    /// Maximum concurrent I/O operations
    pub max_concurrent_ops: usize,

    /// I/O timeout in seconds
    pub timeout_secs: u64,

    /// Buffer size in KB
    pub buffer_size_kb: usize,

    /// Enable direct I/O
    pub enable_direct_io: bool,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// TCP no delay
    pub tcp_nodelay: bool,

    /// Keep-alive settings
    pub keep_alive: bool,

    /// Socket receive buffer size
    pub recv_buffer_size: Option<usize>,

    /// Socket send buffer size
    pub send_buffer_size: Option<usize>,
}

/// CPU configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuConfig {
    /// Number of worker threads
    pub worker_threads: Option<usize>,

    /// Enable CPU affinity
    pub enable_affinity: bool,

    /// SIMD optimizations
    pub enable_simd: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,

    /// Log format (json, plain)
    pub format: String,

    /// Log outputs
    pub outputs: Vec<LogOutput>,

    /// Component-specific log levels
    pub component_levels: HashMap<String, String>,
}

/// Log output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogOutput {
    /// Output type (stdout, stderr, file, syslog)
    pub output_type: String,

    /// File path (for file output)
    pub file_path: Option<PathBuf>,

    /// Log rotation
    pub rotation: Option<LogRotationConfig>,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Rotation strategy (size, time)
    pub strategy: String,

    /// Maximum file size in MB
    pub max_size_mb: Option<usize>,

    /// Rotation interval
    pub interval: Option<String>,

    /// Maximum number of files to keep
    pub max_files: usize,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Metrics configuration
    pub metrics: MetricsConfig,

    /// Health check configuration
    pub health_checks: HealthCheckConfig,

    /// Tracing configuration
    pub tracing: TracingConfig,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Metrics server port
    pub port: u16,

    /// Metrics format (prometheus, json)
    pub format: String,

    /// Collection interval in seconds
    pub collection_interval_secs: u64,

    /// Retention period in days
    pub retention_days: u32,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Enable health checks
    pub enabled: bool,

    /// Health check port
    pub port: u16,

    /// Check interval in seconds
    pub interval_secs: u64,

    /// Timeout for health checks
    pub timeout_secs: u64,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: bool,

    /// Tracing backend (jaeger, zipkin, otlp)
    pub backend: String,

    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,

    /// Tracing endpoint
    pub endpoint: Option<String>,
}

/// Persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Persistence provider (rocksdb, memory)
    pub provider: String,

    /// Data directory for persistence
    pub data_dir: PathBuf,

    /// Write-Ahead Log configuration
    pub wal: WalConfig,

    /// RocksDB-specific configuration
    pub rocksdb: Option<RocksDbConfig>,
}

/// Write-Ahead Log configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalConfig {
    /// Enable WAL
    pub enabled: bool,

    /// WAL directory
    pub wal_dir: PathBuf,

    /// Sync interval in milliseconds
    pub sync_interval_ms: u64,

    /// Maximum WAL size in MB
    pub max_size_mb: usize,

    /// Enable fsync on every write
    pub fsync_on_write: bool,
}

/// RocksDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocksDbConfig {
    /// Enable WAL in RocksDB
    pub enable_wal: bool,

    /// Max open files
    pub max_open_files: i32,

    /// Write buffer size in MB
    pub write_buffer_size_mb: usize,

    /// Max write buffer number
    pub max_write_buffer_number: u32,

    /// Block cache size in MB
    pub block_cache_size_mb: usize,

    /// Enable compression
    pub enable_compression: bool,

    /// Compression type
    pub compression_type: String,
}

/// Storage configuration for tiered and cold storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Tiered storage configuration
    pub tiered: TieredStorageConfig,

    /// Cold tier configuration
    pub cold_tier: Option<ColdTierConfig>,
}

/// Tiered storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredStorageConfig {
    /// Enable tiered storage
    pub enabled: bool,

    /// Hot to warm threshold in hours
    pub hot_to_warm_threshold_hours: u64,

    /// Warm to cold threshold in days
    pub warm_to_cold_threshold_days: u64,

    /// Enable automatic tiering
    pub auto_tiering: bool,

    /// Enable background migration
    pub background_migration: bool,
}

/// Cold tier configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdTierConfig {
    /// Enable cold tier
    pub enabled: bool,

    /// Backend type (s3, azure, minio)
    pub backend: String,

    /// S3 configuration
    pub s3: Option<S3Config>,

    /// Azure configuration
    pub azure: Option<AzureStorageConfig>,

    /// MinIO configuration
    pub minio: Option<MinioConfig>,
}

/// S3 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,

    /// AWS region
    pub region: String,

    /// Access key ID
    pub access_key_id: Option<String>,

    /// Secret access key
    pub secret_access_key: Option<String>,

    /// Endpoint URL (for S3-compatible services)
    pub endpoint: Option<String>,
}

/// Azure Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureStorageConfig {
    /// Account name
    pub account_name: String,

    /// Container name
    pub container_name: String,

    /// Access key
    pub access_key: Option<String>,

    /// Connection string
    pub connection_string: Option<String>,
}

/// MinIO configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinioConfig {
    /// MinIO endpoint
    pub endpoint: String,

    /// Bucket name
    pub bucket: String,

    /// Access key
    pub access_key: String,

    /// Secret key
    pub secret_key: String,

    /// Use SSL
    pub use_ssl: bool,

    /// Region (optional)
    pub region: Option<String>,
}

impl Default for OrbitServerConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            actor_system: ActorSystemConfig::default(),
            protocols: ProtocolsConfig::default(),
            security: SecurityConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            monitoring: MonitoringConfig::default(),
            pooling: PoolingConfig::default(),
            persistence: Some(PersistenceConfig::default()),
            storage: Some(StorageConfig::default()),
        }
    }
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            provider: "rocksdb".to_string(),
            data_dir: PathBuf::from("./data"),
            wal: WalConfig::default(),
            rocksdb: Some(RocksDbConfig::default()),
        }
    }
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            wal_dir: PathBuf::from("./data/wal"),
            sync_interval_ms: 1000,
            max_size_mb: 1024,
            fsync_on_write: false,
        }
    }
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            enable_wal: true,
            max_open_files: 1000,
            write_buffer_size_mb: 64,
            max_write_buffer_number: 2,
            block_cache_size_mb: 256,
            enable_compression: true,
            compression_type: "lz4".to_string(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            tiered: TieredStorageConfig::default(),
            cold_tier: None,
        }
    }
}

impl Default for TieredStorageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_to_warm_threshold_hours: 48,
            warm_to_cold_threshold_days: 30,
            auto_tiering: true,
            background_migration: true,
        }
    }
}

impl TieredStorageConfig {
    /// Get hot to warm threshold as Duration
    pub fn hot_to_warm_threshold(&self) -> Duration {
        Duration::from_secs(self.hot_to_warm_threshold_hours * 60 * 60)
    }

    /// Get warm to cold threshold as Duration
    pub fn warm_to_cold_threshold(&self) -> Duration {
        Duration::from_secs(self.warm_to_cold_threshold_days * 24 * 60 * 60)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            bind_address: "0.0.0.0".to_string(),
            environment: Environment::Development,
            data_dir: PathBuf::from("./data"),
            config_dir: PathBuf::from("./config"),
        }
    }
}

impl Default for ActorSystemConfig {
    fn default() -> Self {
        Self {
            max_actors: 10_000,
            mailbox_size: 1000,
            supervision_strategy: SupervisionStrategy::OneForOne,
            cluster: None,
        }
    }
}

impl Default for ProtocolsConfig {
    fn default() -> Self {
        Self {
            grpc: Some(GrpcConfig::default()),
            postgresql: Some(PostgresqlConfig::default()),
            redis: Some(RedisConfig::default()),
            rest: Some(RestConfig::default()),
            cypher: None,
            mcp: None,
            cql: Some(CqlConfig::default()),
            mysql: Some(MySqlConfig::default()),
            aql: Some(AqlConfig::default()),
            orbitql: Some(OrbitQLConfig::default()),
        }
    }
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 50051,
            max_concurrent_streams: 1000,
            max_message_size: 4 * 1024 * 1024, // 4MB
            keep_alive: None,
            tls: None,
        }
    }
}

impl Default for PostgresqlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 5432,
            max_connections: 1000,
            connection_timeout_secs: 30,
            sql_engine: SqlEngineConfig::default(),
            vector_ops: VectorOpsConfig::default(),
            features: PostgresqlFeatures::default(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 6379,
            max_connections: 1000,
            connection_timeout_secs: 30,
            commands: RedisCommandConfig::default(),
            vector_ops: VectorOpsConfig::default(),
            features: RedisFeatures::default(),
        }
    }
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8080,
            max_connections: 1000,
            request_timeout_secs: 30,
            cors: None,
            rate_limit: None,
        }
    }
}

impl Default for CqlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 9042,
            max_connections: 1000,
            connection_timeout_secs: 30,
            protocol_version: 4,
            authentication_enabled: false,
        }
    }
}

impl Default for MySqlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 3306,
            max_connections: 1000,
            connection_timeout_secs: 30,
            server_version: "8.0.0-Orbit".to_string(),
            authentication_enabled: false,
        }
    }
}

impl Default for OrbitQLConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8081,
            max_connections: 1000,
            connection_timeout_secs: 30,
            authentication_enabled: false,
        }
    }
}

impl Default for SqlEngineConfig {
    fn default() -> Self {
        Self {
            max_query_complexity: 1000,
            query_timeout_secs: 30,
            enable_optimization: true,
            enable_caching: true,
            cache_size_mb: 256,
        }
    }
}

impl Default for VectorOpsConfig {
    fn default() -> Self {
        Self {
            default_metric: "cosine".to_string(),
            max_dimensions: 4096,
            batch_size: 1000,
            enable_simd: true,
            indexing: VectorIndexConfig::default(),
        }
    }
}

impl Default for VectorIndexConfig {
    fn default() -> Self {
        Self {
            default_algorithm: "hnsw".to_string(),
            hnsw: HnswConfig::default(),
            ivf: IvfConfig::default(),
        }
    }
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 50,
        }
    }
}

impl Default for IvfConfig {
    fn default() -> Self {
        Self {
            nlist: 100,
            nprobe: 10,
        }
    }
}

impl Default for PostgresqlFeatures {
    fn default() -> Self {
        Self {
            enable_pgvector: true,
            enable_postgis: false,
            enable_json: true,
            enable_fulltext: true,
            enable_prepared_statements: true,
            enable_transactions: true,
        }
    }
}

impl Default for RedisFeatures {
    fn default() -> Self {
        Self {
            enable_streams: true,
            enable_pubsub: true,
            enable_modules: false,
            enable_redisearch: true,
            enable_redisjson: true,
            enable_redisgraph: true,
            enable_cluster: false,
        }
    }
}

impl Default for RedisCommandConfig {
    fn default() -> Self {
        Self {
            enabled_groups: vec![
                "generic".to_string(),
                "string".to_string(),
                "list".to_string(),
                "hash".to_string(),
                "set".to_string(),
                "zset".to_string(),
                "vector".to_string(),
                "search".to_string(),
                "json".to_string(),
                "graph".to_string(),
                "stream".to_string(),
                "pubsub".to_string(),
            ],
            disabled_commands: vec![],
            command_timeout_secs: 30,
            max_pipeline_size: 1000,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            authentication: AuthenticationConfig::default(),
            authorization: AuthorizationConfig::default(),
            encryption: EncryptionConfig::default(),
        }
    }
}

impl Default for AuthenticationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            methods: vec![AuthMethod::Basic],
            jwt: None,
            session: None,
        }
    }
}

impl Default for AuthorizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: "rbac".to_string(),
            default_permissions: vec!["read".to_string(), "write".to_string()],
        }
    }
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            at_rest: None,
            in_transit: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            memory: MemoryConfig::default(),
            io: IoConfig::default(),
            network: NetworkConfig::default(),
            cpu: CpuConfig::default(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096,
            pool_config: MemoryPoolConfig::default(),
            gc_settings: GcSettings::default(),
        }
    }
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            initial_size_mb: 256,
            max_size_mb: 2048,
            growth_factor: 1.5,
        }
    }
}

impl Default for GcSettings {
    fn default() -> Self {
        Self {
            auto_gc: true,
            interval_secs: 300,
            memory_threshold_pct: 80.0,
        }
    }
}

impl Default for IoConfig {
    fn default() -> Self {
        Self {
            max_concurrent_ops: 1000,
            timeout_secs: 30,
            buffer_size_kb: 64,
            enable_direct_io: false,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            tcp_nodelay: true,
            keep_alive: true,
            recv_buffer_size: None,
            send_buffer_size: None,
        }
    }
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            worker_threads: None,
            enable_affinity: false,
            enable_simd: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "plain".to_string(),
            outputs: vec![LogOutput {
                output_type: "stdout".to_string(),
                file_path: None,
                rotation: None,
            }],
            component_levels: HashMap::new(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            health_checks: HealthCheckConfig::default(),
            tracing: TracingConfig::default(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 9090,
            format: "prometheus".to_string(),
            collection_interval_secs: 30,
            retention_days: 7,
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8081,
            interval_secs: 30,
            timeout_secs: 5,
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: "jaeger".to_string(),
            sampling_rate: 0.1,
            endpoint: None,
        }
    }
}

impl Default for PoolingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_connections: 5,
            max_connections: 100,
            connection_timeout_secs: 30,
            idle_timeout_secs: 300,
            max_lifetime_secs: 3600,
            health_check_interval_secs: 30,
            load_balancing_strategy: LoadBalancingStrategy::LeastConnections,
            tier: PoolTier::Application,
            enable_dynamic_sizing: true,
            target_utilization: 0.75,
            circuit_breaker: PoolCircuitBreakerConfig::default(),
            protocol_overrides: HashMap::new(),
        }
    }
}

impl Default for PoolCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold: 5,
            failure_window_secs: 60,
            recovery_timeout_secs: 30,
            success_threshold: 3,
            half_open_max_calls: 3,
        }
    }
}

/// Configuration loading and validation
impl OrbitServerConfig {
    /// Load configuration from TOML file
    pub async fn load_from_file(
        path: &std::path::Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to TOML file
    pub async fn save_to_file(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate port numbers don't conflict
        let mut used_ports = std::collections::HashSet::new();

        if let Some(grpc) = &self.protocols.grpc {
            if grpc.enabled && !used_ports.insert(grpc.port) {
                return Err(format!("Port {} is used by multiple services", grpc.port).into());
            }
        }

        if let Some(postgres) = &self.protocols.postgresql {
            if postgres.enabled && !used_ports.insert(postgres.port) {
                return Err(format!("Port {} is used by multiple services", postgres.port).into());
            }
        }

        if let Some(redis) = &self.protocols.redis {
            if redis.enabled && !used_ports.insert(redis.port) {
                return Err(format!("Port {} is used by multiple services", redis.port).into());
            }
        }

        if let Some(rest) = &self.protocols.rest {
            if rest.enabled && !used_ports.insert(rest.port) {
                return Err(format!("Port {} is used by multiple services", rest.port).into());
            }
        }

        if let Some(cql) = &self.protocols.cql {
            if cql.enabled && !used_ports.insert(cql.port) {
                return Err(format!("Port {} is used by multiple services", cql.port).into());
            }
        }

        if let Some(mysql) = &self.protocols.mysql {
            if mysql.enabled && !used_ports.insert(mysql.port) {
                return Err(format!("Port {} is used by multiple services", mysql.port).into());
            }
        }

        if !used_ports.insert(self.monitoring.metrics.port) {
            return Err(format!(
                "Metrics port {} is used by another service",
                self.monitoring.metrics.port
            )
            .into());
        }

        if !used_ports.insert(self.monitoring.health_checks.port) {
            return Err(format!(
                "Health check port {} is used by another service",
                self.monitoring.health_checks.port
            )
            .into());
        }

        Ok(())
    }

    /// Get enabled protocol servers
    pub fn enabled_protocols(&self) -> Vec<&str> {
        let mut protocols = Vec::new();

        if self.protocols.grpc.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("grpc");
        }
        if self
            .protocols
            .postgresql
            .as_ref()
            .map_or(false, |c| c.enabled)
        {
            protocols.push("postgresql");
        }
        if self.protocols.redis.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("redis");
        }
        if self.protocols.rest.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("rest");
        }
        if self.protocols.cypher.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("cypher");
        }
        if self.protocols.mcp.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("mcp");
        }
        if self.protocols.cql.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("cql");
        }
        if self.protocols.mysql.as_ref().map_or(false, |c| c.enabled) {
            protocols.push("mysql");
        }

        protocols
    }

    /// Generate default configuration file
    pub fn generate_example_config() -> String {
        toml::to_string_pretty(&Self::default()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrbitServerConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_enabled_protocols() {
        let config = OrbitServerConfig::default();
        let protocols = config.enabled_protocols();
        assert!(protocols.contains(&"grpc"));
        assert!(protocols.contains(&"postgresql"));
        assert!(protocols.contains(&"redis"));
        assert!(protocols.contains(&"rest"));
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = OrbitServerConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: OrbitServerConfig = toml::from_str(&toml_str).unwrap();
        assert!(deserialized.validate().is_ok());
    }
}
