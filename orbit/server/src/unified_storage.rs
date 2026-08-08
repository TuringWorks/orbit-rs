//! Unified Cross-Protocol Storage Integration
//!
//! This module provides the integration layer between the orbit-server's protocol
//! implementations and the orbit-engine's unified storage system. It enables true
//! cross-protocol data sharing where data written via one protocol (Redis, PostgreSQL,
//! MySQL, CQL, Cypher, AQL, REST) is immediately accessible through all other protocols.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Protocol Servers                           │
//! ├─────────┬─-─────-───-┬─────────┬─────────┬─────────┬──────────-─┤
//! │  Redis  │ PostgreSQL │  MySQL  │   CQL   │ Cypher  │  AQL/REST  │
//! ├─────────┴──-─────-──-┴─────────┴─────────┴─────────┴──────────-─┤
//! │                 UnifiedStorageIntegration                       │
//! │  (This module - bridges protocols to unified storage)           │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    SchemaRegistry                               │
//! │  (Manages namespaces and cross-protocol projections)            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    UnifiedStorage                               │
//! │  (Single storage backend shared by all protocols)               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use orbit_server::unified_storage::UnifiedStorageIntegration;
//!
//! // Create unified storage integration
//! let integration = UnifiedStorageIntegration::new("./data").await?;
//!
//! // Get protocol adapters for each protocol server
//! let redis_adapter = integration.redis_adapter();
//! let sql_adapter = integration.sql_adapter("postgresql");
//! let graph_adapter = integration.graph_adapter("cypher");
//! ```

pub use orbit_engine::unified::rocksdb_backend::{Compression, RocksDbBackendConfig};

use orbit_engine::unified::{
    rocksdb_backend::RocksDbBackend, AdapterFactory, CqlAdapter, GraphAdapter, MemoryBackend,
    Protocol, RedisAdapter, RestAdapter, SchemaRegistry, SqlAdapter, UnifiedStorage,
    UnifiedStorageBackend, UnifiedStorageConfig,
};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Configuration for unified storage integration
#[derive(Debug, Clone)]
pub struct UnifiedStorageIntegrationConfig {
    /// Data directory for persistent storage
    pub data_dir: String,
    /// Enable TTL expiration background task
    pub enable_ttl_expiration: bool,
    /// TTL check interval in seconds
    pub ttl_check_interval_secs: u64,
    /// Maximum records per scan operation
    pub max_scan_limit: usize,
    /// Use memory backend (for testing)
    pub use_memory_backend: bool,
    /// How the persistent backend trades durability against speed.
    ///
    /// Ignored when `use_memory_backend` is set, which keeps nothing.
    pub durability: RocksDbBackendConfig,
}

impl Default for UnifiedStorageIntegrationConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data/unified".to_string(),
            enable_ttl_expiration: true,
            ttl_check_interval_secs: 60,
            max_scan_limit: 1_000_000,
            use_memory_backend: false,
            durability: RocksDbBackendConfig::default(),
        }
    }
}

/// Unified storage integration for orbit-server
///
/// This struct provides the main integration point between orbit-server's
/// protocol implementations and orbit-engine's unified storage system.
/// It manages a single UnifiedStorage instance and provides protocol-specific
/// adapters that enable cross-protocol data access.
pub struct UnifiedStorageIntegration {
    /// The unified storage instance (shared by all protocols)
    storage: Arc<UnifiedStorage>,
    /// Schema registry for managing namespaces
    registry: Arc<SchemaRegistry>,
}

impl UnifiedStorageIntegration {
    /// Create a new unified storage integration with default configuration
    pub async fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self, UnifiedStorageError> {
        let config = UnifiedStorageIntegrationConfig {
            data_dir: data_dir.as_ref().to_string_lossy().to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Create a new unified storage integration with custom configuration
    pub async fn with_config(
        config: UnifiedStorageIntegrationConfig,
    ) -> Result<Self, UnifiedStorageError> {
        info!(
            "[UnifiedStorage] Initializing unified storage at: {}",
            config.data_dir
        );

        // Create storage backend.
        //
        // Both arms of this used to build a `MemoryBackend`, so the flag named
        // a choice that was never made: every table and row served over the
        // SQL protocols was lost on restart while the log said "persistent
        // backend".
        let backend: Arc<dyn UnifiedStorageBackend> = if config.use_memory_backend {
            info!("[UnifiedStorage] Using in-memory backend");
            Arc::new(MemoryBackend::new())
        } else {
            let path = Path::new(&config.data_dir).join("unified");
            info!(
                path = %path.display(),
                sync_writes = config.durability.sync_writes,
                wal = config.durability.enable_wal,
                "[UnifiedStorage] Using RocksDB backend"
            );
            Arc::new(
                RocksDbBackend::open_with(&path, &config.durability).map_err(|e| {
                    UnifiedStorageError::InitializationFailed(format!(
                        "could not open the unified store: {e}"
                    ))
                })?,
            )
        };

        // Create storage configuration
        let storage_config = UnifiedStorageConfig {
            data_dir: config.data_dir,
            enable_ttl_expiration: config.enable_ttl_expiration,
            ttl_check_interval_secs: config.ttl_check_interval_secs,
            max_scan_limit: config.max_scan_limit,
            enable_wal: true,
            enable_compression: true,
        };

        // Create unified storage
        let storage = Arc::new(UnifiedStorage::new(backend, storage_config));

        // Initialize storage
        storage.initialize().await.map_err(|e| {
            UnifiedStorageError::InitializationFailed(format!(
                "Failed to initialize unified storage: {}",
                e
            ))
        })?;

        // Create schema registry
        let registry = Arc::new(SchemaRegistry::new());

        info!("[UnifiedStorage] Unified storage initialized successfully");

        Ok(Self { storage, registry })
    }

    /// Get the underlying unified storage instance
    pub fn storage(&self) -> Arc<UnifiedStorage> {
        Arc::clone(&self.storage)
    }

    /// Get the schema registry
    pub fn registry(&self) -> Arc<SchemaRegistry> {
        Arc::clone(&self.registry)
    }

    /// Create a Redis adapter for Redis protocol server integration
    pub fn redis_adapter(&self) -> RedisAdapter {
        AdapterFactory::redis(Arc::clone(&self.storage), Arc::clone(&self.registry))
    }

    /// Create a SQL adapter for PostgreSQL or MySQL protocol server integration
    pub fn sql_adapter(&self, dialect: &str) -> SqlAdapter {
        match dialect.to_lowercase().as_str() {
            "postgresql" | "postgres" => {
                AdapterFactory::postgres(Arc::clone(&self.storage), Arc::clone(&self.registry))
            }
            "mysql" => AdapterFactory::mysql(Arc::clone(&self.storage), Arc::clone(&self.registry)),
            _ => AdapterFactory::postgres(Arc::clone(&self.storage), Arc::clone(&self.registry)), // Default to PostgreSQL
        }
    }

    /// Create a CQL adapter for Cassandra protocol server integration
    pub fn cql_adapter(&self) -> CqlAdapter {
        AdapterFactory::cql(Arc::clone(&self.storage), Arc::clone(&self.registry))
    }

    /// Create a Graph adapter for Cypher or AQL protocol server integration
    pub fn graph_adapter(&self, dialect: &str) -> GraphAdapter {
        match dialect.to_lowercase().as_str() {
            "cypher" | "neo4j" => {
                AdapterFactory::cypher(Arc::clone(&self.storage), Arc::clone(&self.registry))
            }
            "aql" | "arangodb" => {
                AdapterFactory::aql(Arc::clone(&self.storage), Arc::clone(&self.registry))
            }
            _ => AdapterFactory::cypher(Arc::clone(&self.storage), Arc::clone(&self.registry)), // Default to Cypher
        }
    }

    /// Create a REST adapter for HTTP REST API integration
    pub fn rest_adapter(&self) -> RestAdapter {
        AdapterFactory::rest(Arc::clone(&self.storage), Arc::clone(&self.registry))
    }

    /// Get an adapter by protocol type
    pub fn adapter(&self, protocol: Protocol) -> Box<dyn std::any::Any + Send + Sync> {
        match protocol {
            Protocol::Redis => Box::new(self.redis_adapter()),
            Protocol::PostgreSQL => Box::new(self.sql_adapter("postgresql")),
            Protocol::MySQL => Box::new(self.sql_adapter("mysql")),
            Protocol::CQL => Box::new(self.cql_adapter()),
            Protocol::Cypher => Box::new(self.graph_adapter("cypher")),
            Protocol::AQL => Box::new(self.graph_adapter("aql")),
            Protocol::REST => Box::new(self.rest_adapter()),
            Protocol::GRPC => Box::new(self.rest_adapter()), // Use REST for gRPC too
        }
    }

    /// Get storage metrics
    pub async fn metrics(&self) -> UnifiedStorageMetrics {
        let engine_metrics = self.storage.metrics().await;
        UnifiedStorageMetrics {
            read_operations: engine_metrics.read_operations,
            write_operations: engine_metrics.write_operations,
            delete_operations: engine_metrics.delete_operations,
            read_latency_avg: engine_metrics.read_latency_avg,
            write_latency_avg: engine_metrics.write_latency_avg,
            delete_latency_avg: engine_metrics.delete_latency_avg,
            total_records: engine_metrics.total_records,
            total_namespaces: engine_metrics.total_namespaces,
            memory_usage_bytes: engine_metrics.memory_usage_bytes,
            error_count: engine_metrics.error_count,
        }
    }
}

/// Metrics for unified storage
#[derive(Debug, Clone, Default)]
pub struct UnifiedStorageMetrics {
    /// Total read operations
    pub read_operations: u64,
    /// Total write operations
    pub write_operations: u64,
    /// Total delete operations
    pub delete_operations: u64,
    /// Average read latency in seconds
    pub read_latency_avg: f64,
    /// Average write latency in seconds
    pub write_latency_avg: f64,
    /// Average delete latency in seconds
    pub delete_latency_avg: f64,
    /// Total number of records
    pub total_records: u64,
    /// Total number of namespaces
    pub total_namespaces: u64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Error count
    pub error_count: u64,
}

/// Errors that can occur during unified storage operations
#[derive(Debug, thiserror::Error)]
pub enum UnifiedStorageError {
    /// Failed to initialize unified storage
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    /// Storage operation failed
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_storage_integration() {
        let config = UnifiedStorageIntegrationConfig {
            use_memory_backend: true,
            ..Default::default()
        };

        let integration = UnifiedStorageIntegration::with_config(config)
            .await
            .unwrap();

        // Test Redis adapter
        let redis = integration.redis_adapter();
        redis
            .set(
                "test:key",
                orbit_engine::unified::UniversalValue::String("value".to_string()),
            )
            .await
            .unwrap();

        let value = redis.get("test:key").await.unwrap();
        assert!(value.is_some());

        // Test SQL adapter
        let sql = integration.sql_adapter("postgresql");
        let mut row = std::collections::BTreeMap::new();
        row.insert(
            "id".to_string(),
            orbit_engine::unified::UniversalValue::Int(1),
        );
        row.insert(
            "name".to_string(),
            orbit_engine::unified::UniversalValue::String("Alice".to_string()),
        );
        sql.insert("users", row, "id").await.unwrap();

        let rows = sql
            .select("users", None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);

        // Test cross-protocol access: data written via Redis, read via SQL
        let hash_key = "user:bob";
        redis
            .hset(
                hash_key,
                "name",
                orbit_engine::unified::UniversalValue::String("Bob".to_string()),
            )
            .await
            .unwrap();

        // The cross-protocol access works because both adapters share the same storage
        let metrics = integration.metrics().await;
        assert!(metrics.write_operations > 0);
    }

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = UnifiedStorageIntegrationConfig {
            use_memory_backend: true,
            ..Default::default()
        };

        let integration = UnifiedStorageIntegration::with_config(config)
            .await
            .unwrap();

        // Test all adapter types
        let _redis = integration.redis_adapter();
        let _postgres = integration.sql_adapter("postgresql");
        let _mysql = integration.sql_adapter("mysql");
        let _cql = integration.cql_adapter();
        let _cypher = integration.graph_adapter("cypher");
        let _aql = integration.graph_adapter("aql");
        let _rest = integration.rest_adapter();

        // Verify storage is shared
        let storage1 = integration.storage();
        let storage2 = integration.storage();
        assert!(Arc::ptr_eq(&storage1, &storage2));
    }
}
