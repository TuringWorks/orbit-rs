//! Storage abstraction layer for PostgreSQL wire protocol
//!
//! This module provides a pluggable storage backend interface that can work with
//! different storage engines including in-memory, LSM-tree, and distributed storage.

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::{
    executor::{ExtensionDefinition, IndexSchema, SchemaDefinition, TableSchema, ViewSchema},
    types::SqlValue,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub mod memory;
// TODO: Re-enable LSM storage once orbit-shared dependencies are properly resolved
// pub mod lsm;

/// Configuration for storage backend selection
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum StorageBackendConfig {
    /// Pure in-memory storage (default, non-durable)
    #[default]
    Memory,
    /// LSM-Tree based durable storage
    Lsm {
        data_dir: String,
        memtable_size_limit: usize,
        max_memtables: usize,
        bloom_filter_fp_rate: f64,
        enable_compaction: bool,
        compaction_threshold: usize,
    },
    /// Distributed cluster storage
    Cluster {
        cluster_id: String,
        node_id: String,
        data_dir: String,
        replication_factor: usize,
    },
}

/// Storage backend metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageMetrics {
    pub read_operations: u64,
    pub write_operations: u64,
    pub delete_operations: u64,
    pub read_latency_avg: f64,
    pub write_latency_avg: f64,
    pub delete_latency_avg: f64,
    pub error_count: u64,
    pub memory_usage_bytes: u64,
    pub disk_usage_bytes: u64,
    pub cache_hit_rate: f64,
    pub compaction_count: u64,
}

/// Transaction context for storage operations
#[derive(Debug, Clone)]
pub struct StorageTransaction {
    pub id: String,
    pub isolation_level: String,
    pub read_timestamp: std::time::Instant,
    pub write_buffer: HashMap<String, Vec<u8>>,
}

/// Abstraction for PostgreSQL table storage operations
#[async_trait]
pub trait TableStorage: Send + Sync {
    /// Initialize the storage backend
    async fn initialize(&self) -> ProtocolResult<()>;

    /// Shutdown the storage backend gracefully
    async fn shutdown(&self) -> ProtocolResult<()>;

    /// Get current storage metrics
    async fn metrics(&self) -> StorageMetrics;

    /// Begin a new transaction
    async fn begin_transaction(&self) -> ProtocolResult<StorageTransaction>;

    /// Commit a transaction
    async fn commit_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()>;

    /// Rollback a transaction
    async fn rollback_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()>;

    // Schema Operations

    /// Store table schema definition
    async fn store_table_schema(
        &self,
        schema: &TableSchema,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get table schema by name
    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>>;

    /// List all table schemas
    async fn list_table_schemas(&self) -> ProtocolResult<Vec<TableSchema>>;

    /// Remove table schema
    async fn remove_table_schema(
        &self,
        table_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool>;

    // Data Operations

    /// Insert row data into a table
    async fn insert_row(
        &self,
        table_name: &str,
        row: &HashMap<String, SqlValue>,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Insert multiple rows into a table
    async fn insert_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, SqlValue>],
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get all rows from a table
    async fn get_table_data(
        &self,
        table_name: &str,
    ) -> ProtocolResult<Vec<HashMap<String, SqlValue>>>;

    /// Update rows in a table based on a condition (for now, update all rows)
    async fn update_rows(
        &self,
        table_name: &str,
        updates: &HashMap<String, SqlValue>,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize>;

    /// Delete rows from a table based on a condition (for now, delete all rows)
    async fn delete_rows(
        &self,
        table_name: &str,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize>;

    /// Clear all data from a table (but keep schema)
    async fn truncate_table(
        &self,
        table_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    // Index Operations

    /// Store index definition
    async fn store_index(
        &self,
        index: &IndexSchema,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get index by name
    async fn get_index(&self, index_name: &str) -> ProtocolResult<Option<IndexSchema>>;

    /// List all indexes for a table
    async fn list_table_indexes(&self, table_name: &str) -> ProtocolResult<Vec<IndexSchema>>;

    /// Remove index
    async fn remove_index(
        &self,
        index_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool>;

    // View Operations

    /// Store view definition
    async fn store_view(
        &self,
        view: &ViewSchema,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get view by name
    async fn get_view(&self, view_name: &str) -> ProtocolResult<Option<ViewSchema>>;

    /// List all views
    async fn list_views(&self) -> ProtocolResult<Vec<ViewSchema>>;

    /// Remove view
    async fn remove_view(
        &self,
        view_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool>;

    // Schema Operations

    /// Store schema definition
    async fn store_schema(
        &self,
        schema: &SchemaDefinition,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get schema by name
    async fn get_schema(&self, schema_name: &str) -> ProtocolResult<Option<SchemaDefinition>>;

    /// List all schemas
    async fn list_schemas(&self) -> ProtocolResult<Vec<SchemaDefinition>>;

    /// Remove schema
    async fn remove_schema(
        &self,
        schema_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool>;

    // Extension Operations

    /// Store extension definition
    async fn store_extension(
        &self,
        extension: &ExtensionDefinition,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get extension by name
    async fn get_extension(
        &self,
        extension_name: &str,
    ) -> ProtocolResult<Option<ExtensionDefinition>>;

    /// List all extensions
    async fn list_extensions(&self) -> ProtocolResult<Vec<ExtensionDefinition>>;

    /// Remove extension
    async fn remove_extension(
        &self,
        extension_name: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool>;

    // Configuration Operations

    /// Store configuration setting
    async fn store_setting(
        &self,
        key: &str,
        value: &str,
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()>;

    /// Get configuration setting
    async fn get_setting(&self, key: &str) -> ProtocolResult<Option<String>>;

    /// List all settings
    async fn list_settings(&self) -> ProtocolResult<HashMap<String, String>>;

    // Maintenance Operations

    /// Force a checkpoint (flush all pending data to disk)
    async fn checkpoint(&self) -> ProtocolResult<()>;

    /// Perform compaction (LSM-tree specific)
    async fn compact(&self) -> ProtocolResult<()>;

    /// Estimate storage size in bytes
    async fn storage_size(&self) -> ProtocolResult<u64>;
}

/// Factory for creating storage backends
pub struct StorageBackendFactory;

impl StorageBackendFactory {
    /// Create a new storage backend based on configuration
    pub async fn create_backend(
        config: &StorageBackendConfig,
    ) -> ProtocolResult<Arc<dyn TableStorage>> {
        match config {
            StorageBackendConfig::Memory => Ok(Arc::new(memory::MemoryTableStorage::new())),
            StorageBackendConfig::Lsm { .. } => {
                // TODO: Re-enable LSM storage once dependencies are resolved
                Err(ProtocolError::Other(
                    "LSM storage backend temporarily disabled".to_string(),
                ))
            }
            StorageBackendConfig::Cluster { .. } => {
                // For now, return an error as cluster storage isn't implemented yet
                Err(ProtocolError::Other(
                    "Cluster storage backend not yet implemented".to_string(),
                ))
            }
        }
    }
}

/// Helper trait for serializing storage data
pub trait StorageSerializable {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>>;
    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self>
    where
        Self: Sized;
}

// Implement for common types
impl StorageSerializable for TableSchema {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })
    }
}

impl StorageSerializable for HashMap<String, SqlValue> {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        // Convert SqlValue::Jsonb to a compact binary representation (CBOR) wrapped in an object
        // so that deserialization can distinguish binary-encoded jsonb from normal json string.
        use crate::postgres_wire::jsonb;
        use serde_json::Value as JsonValue;

        let mut map = serde_json::Map::new();
        for (k, v) in self.iter() {
            match v {
                SqlValue::Jsonb(jv) => {
                    // Encode as base64 of CBOR bytes and mark as __jsonb_b64
                    let bytes = jsonb::encode_jsonb(jv)?;
                    let b64 = base64::encode(&bytes);
                    let wrapper = JsonValue::Object(serde_json::Map::from_iter(vec![
                        ("__jsonb_b64".to_string(), JsonValue::String(b64)),
                    ]));
                    map.insert(k.clone(), wrapper);
                }
                SqlValue::Json(jv) => {
                    map.insert(k.clone(), JsonValue::from(jv.clone()));
                }
                _ => {
                    // Fallback to serde_json conversion for other SqlValue types
                    let s = v.to_postgres_string();
                    map.insert(k.clone(), JsonValue::String(s));
                }
            }
        }

        serde_json::to_vec(&JsonValue::Object(map))
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        use crate::postgres_wire::jsonb;
        use serde_json::Value as JsonValue;

        let v: JsonValue = serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })?;

        let mut result = HashMap::new();
        if let JsonValue::Object(map) = v {
            for (k, val) in map.into_iter() {
                if let JsonValue::Object(inner) = val {
                    if let Some(JsonValue::String(b64)) = inner.get("__jsonb_b64") {
                        let bytes = base64::decode(b64).map_err(|e| {
                            ProtocolError::SerializationError(format!("base64 decode failed: {}", e))
                        })?;
                        let jv = jsonb::decode_jsonb(&bytes)?;
                        result.insert(k, SqlValue::Jsonb(jv));
                        continue;
                    }
                }

                // Otherwise treat as string value stored via to_postgres_string
                match val {
                    JsonValue::String(s) => result.insert(k, SqlValue::Text(s)),
                    JsonValue::Null => result.insert(k, SqlValue::Null),
                    other => result.insert(k, SqlValue::Text(other.to_string())),
                };
            }
        }

        Ok(result)
    }
}

impl StorageSerializable for IndexSchema {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })
    }
}

impl StorageSerializable for ViewSchema {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })
    }
}

impl StorageSerializable for SchemaDefinition {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })
    }
}

impl StorageSerializable for ExtensionDefinition {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })
    }
}

impl StorageSerializable for Vec<HashMap<String, SqlValue>> {
    fn to_storage_bytes(&self) -> ProtocolResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| ProtocolError::SerializationError(format!("Serialization failed: {}", e)))
    }

    fn from_storage_bytes(bytes: &[u8]) -> ProtocolResult<Self> {
        serde_json::from_slice(bytes).map_err(|e| {
            ProtocolError::SerializationError(format!("Deserialization failed: {}", e))
        })
    }
}
