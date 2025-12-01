//! Unified Storage Layer
//!
//! This module provides the core storage backend that all protocols share.
//! It stores data in a protocol-agnostic format (UniversalValue) and provides
//! a single source of truth for all data operations.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      UnifiedStorage                              │
//! │                                                                  │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
//! │  │  Namespace   │  │    Index     │  │  TTL/Expiration      │   │
//! │  │   Manager    │  │   Manager    │  │     Manager          │   │
//! │  └──────────────┘  └──────────────┘  └──────────────────────┘   │
//! │                                                                  │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │                 Storage Backend Trait                     │   │
//! │  │          (Memory, RocksDB, or other backends)             │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Format
//!
//! - Records: `data:{namespace}:{key}`
//! - Namespace schemas: `schema:{namespace}`
//! - Secondary indexes: `idx:{namespace}:{field}:{value}:{key}`
//! - TTL tracking: `ttl:{expiration_timestamp}:{namespace}:{key}`

use super::types::{RecordId, RecordMetadata, UniversalRecord, UniversalResult, UniversalValue};
use super::operations::{FilterExpression, SortOrder, UniversalOperation};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors that can occur during unified storage operations
#[derive(Error, Debug, Clone)]
pub enum UnifiedStorageError {
    /// Record not found
    #[error("Record not found: {namespace}:{key}")]
    NotFound {
        /// Namespace of the missing record
        namespace: String,
        /// Key of the missing record
        key: String,
    },

    /// Namespace already exists
    #[error("Namespace already exists: {0}")]
    NamespaceExists(String),

    /// Namespace not found
    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),

    /// Version conflict during optimistic locking
    #[error("Version conflict: expected {expected}, found {actual}")]
    VersionConflict {
        /// Expected version
        expected: u64,
        /// Actual version found
        actual: u64,
    },

    /// Key already exists (for conditional insert)
    #[error("Key already exists: {namespace}:{key}")]
    KeyExists {
        /// Namespace
        namespace: String,
        /// Key that already exists
        key: String,
    },

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Backend storage error
    #[error("Storage backend error: {0}")]
    Backend(String),

    /// Transaction error
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Result type for unified storage operations
pub type UnifiedStorageResult<T> = Result<T, UnifiedStorageError>;

/// Configuration for unified storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedStorageConfig {
    /// Data directory path (for persistent backends)
    pub data_dir: String,
    /// Enable TTL expiration background task
    pub enable_ttl_expiration: bool,
    /// TTL expiration check interval in seconds
    pub ttl_check_interval_secs: u64,
    /// Maximum records per scan operation
    pub max_scan_limit: usize,
    /// Enable write-ahead logging (for durability)
    pub enable_wal: bool,
    /// Enable compression
    pub enable_compression: bool,
}

impl Default for UnifiedStorageConfig {
    fn default() -> Self {
        Self {
            data_dir: "./orbit_unified_data".to_string(),
            enable_ttl_expiration: true,
            ttl_check_interval_secs: 60,
            max_scan_limit: 10000,
            enable_wal: true,
            enable_compression: true,
        }
    }
}

/// Storage metrics for monitoring
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
    /// Memory usage in bytes (for memory backend)
    pub memory_usage_bytes: u64,
    /// Error count
    pub error_count: u64,
}

/// Trait for storage backends
///
/// This trait abstracts the actual storage mechanism, allowing different
/// backends like in-memory storage or RocksDB.
#[async_trait]
pub trait UnifiedStorageBackend: Send + Sync {
    /// Initialize the backend
    async fn initialize(&self) -> UnifiedStorageResult<()>;

    /// Shutdown the backend gracefully
    async fn shutdown(&self) -> UnifiedStorageResult<()>;

    /// Get a raw value by key
    async fn get(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>>;

    /// Put a raw value
    async fn put(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()>;

    /// Delete a key
    async fn delete(&self, key: &str) -> UnifiedStorageResult<bool>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> UnifiedStorageResult<bool>;

    /// Scan keys with a prefix
    async fn scan_prefix(&self, prefix: &str, limit: Option<usize>) -> UnifiedStorageResult<Vec<(String, Vec<u8>)>>;

    /// Batch put operation
    async fn put_batch(&self, entries: Vec<(String, Vec<u8>)>) -> UnifiedStorageResult<()>;

    /// Batch delete operation
    async fn delete_batch(&self, keys: Vec<String>) -> UnifiedStorageResult<u64>;

    /// Get current metrics
    async fn metrics(&self) -> UnifiedStorageMetrics;
}

/// In-memory storage backend for testing and development
pub struct MemoryBackend {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    metrics: Arc<RwLock<UnifiedStorageMetrics>>,
}

impl MemoryBackend {
    /// Create a new memory backend
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(UnifiedStorageMetrics::default())),
        }
    }

    async fn update_metrics(&self, operation: &str, success: bool) {
        let mut metrics = self.metrics.write().await;
        match operation {
            "read" => metrics.read_operations += 1,
            "write" => metrics.write_operations += 1,
            "delete" => metrics.delete_operations += 1,
            _ => {}
        }
        if !success {
            metrics.error_count += 1;
        }
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UnifiedStorageBackend for MemoryBackend {
    async fn initialize(&self) -> UnifiedStorageResult<()> {
        tracing::info!("Memory backend initialized");
        Ok(())
    }

    async fn shutdown(&self) -> UnifiedStorageResult<()> {
        let mut data = self.data.write().await;
        data.clear();
        tracing::info!("Memory backend shutdown");
        Ok(())
    }

    async fn get(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>> {
        let data = self.data.read().await;
        let result = data.get(key).cloned();
        self.update_metrics("read", true).await;
        Ok(result)
    }

    async fn put(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value.to_vec());
        self.update_metrics("write", true).await;

        // Update total records metric
        let mut metrics = self.metrics.write().await;
        metrics.total_records = data.len() as u64;
        metrics.memory_usage_bytes = data.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() as u64;

        Ok(())
    }

    async fn delete(&self, key: &str) -> UnifiedStorageResult<bool> {
        let mut data = self.data.write().await;
        let removed = data.remove(key).is_some();
        self.update_metrics("delete", true).await;

        // Update total records metric
        let mut metrics = self.metrics.write().await;
        metrics.total_records = data.len() as u64;

        Ok(removed)
    }

    async fn exists(&self, key: &str) -> UnifiedStorageResult<bool> {
        let data = self.data.read().await;
        self.update_metrics("read", true).await;
        Ok(data.contains_key(key))
    }

    async fn scan_prefix(&self, prefix: &str, limit: Option<usize>) -> UnifiedStorageResult<Vec<(String, Vec<u8>)>> {
        let data = self.data.read().await;
        let mut results: Vec<_> = data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Sort for deterministic ordering
        results.sort_by(|a, b| a.0.cmp(&b.0));

        if let Some(limit) = limit {
            results.truncate(limit);
        }

        self.update_metrics("read", true).await;
        Ok(results)
    }

    async fn put_batch(&self, entries: Vec<(String, Vec<u8>)>) -> UnifiedStorageResult<()> {
        let mut data = self.data.write().await;
        for (key, value) in entries {
            data.insert(key, value);
        }
        self.update_metrics("write", true).await;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.total_records = data.len() as u64;
        metrics.memory_usage_bytes = data.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() as u64;

        Ok(())
    }

    async fn delete_batch(&self, keys: Vec<String>) -> UnifiedStorageResult<u64> {
        let mut data = self.data.write().await;
        let mut count = 0u64;
        for key in keys {
            if data.remove(&key).is_some() {
                count += 1;
            }
        }
        self.update_metrics("delete", true).await;

        // Update total records metric
        let mut metrics = self.metrics.write().await;
        metrics.total_records = data.len() as u64;

        Ok(count)
    }

    async fn metrics(&self) -> UnifiedStorageMetrics {
        self.metrics.read().await.clone()
    }
}

/// Unified storage layer that all protocols share
///
/// This is the main entry point for all data operations. Protocol adapters
/// translate their native commands into UniversalOperations and execute
/// them through this storage layer.
pub struct UnifiedStorage {
    backend: Arc<dyn UnifiedStorageBackend>,
    config: UnifiedStorageConfig,
}

impl UnifiedStorage {
    /// Create a new unified storage with the given backend
    pub fn new(backend: Arc<dyn UnifiedStorageBackend>, config: UnifiedStorageConfig) -> Self {
        Self { backend, config }
    }

    /// Create a new unified storage with memory backend (for testing)
    pub fn with_memory_backend() -> Self {
        Self {
            backend: Arc::new(MemoryBackend::new()),
            config: UnifiedStorageConfig::default(),
        }
    }

    /// Initialize the storage
    pub async fn initialize(&self) -> UnifiedStorageResult<()> {
        self.backend.initialize().await
    }

    /// Shutdown the storage
    pub async fn shutdown(&self) -> UnifiedStorageResult<()> {
        self.backend.shutdown().await
    }

    /// Execute a universal operation
    pub async fn execute(&self, operation: UniversalOperation) -> UnifiedStorageResult<UniversalResult> {
        match operation {
            UniversalOperation::Get { namespace, key } => {
                self.get(&namespace, &key).await
            }
            UniversalOperation::Put { namespace, key, value, ttl, if_not_exists, if_version } => {
                self.put(&namespace, &key, value, ttl, if_not_exists, if_version).await
            }
            UniversalOperation::Delete { namespace, key } => {
                self.delete(&namespace, &key).await
            }
            UniversalOperation::Exists { namespace, key } => {
                self.exists(&namespace, &key).await
            }
            UniversalOperation::MultiGet { namespace, keys } => {
                self.multi_get(&namespace, &keys).await
            }
            UniversalOperation::MultiPut { records } => {
                self.multi_put(records).await
            }
            UniversalOperation::MultiDelete { namespace, keys } => {
                self.multi_delete(&namespace, &keys).await
            }
            UniversalOperation::Scan { namespace, filter, limit, offset, order_by, projection } => {
                self.scan(&namespace, filter, limit, offset, order_by, projection).await
            }
            UniversalOperation::ScanKeys { namespace, pattern, limit, cursor: _ } => {
                self.scan_keys(&namespace, pattern, limit).await
            }
            UniversalOperation::Count { namespace, filter } => {
                self.count(&namespace, filter).await
            }
            UniversalOperation::GetField { namespace, key, field } => {
                self.get_field(&namespace, &key, &field).await
            }
            UniversalOperation::SetField { namespace, key, field, value } => {
                self.set_field(&namespace, &key, &field, value).await
            }
            UniversalOperation::DeleteField { namespace, key, field } => {
                self.delete_field(&namespace, &key, &field).await
            }
            UniversalOperation::IncrementField { namespace, key, field, delta } => {
                self.increment_field(&namespace, &key, &field, delta).await
            }
            UniversalOperation::ListPushFront { namespace, key, values } => {
                self.list_push_front(&namespace, &key, values).await
            }
            UniversalOperation::ListPushBack { namespace, key, values } => {
                self.list_push_back(&namespace, &key, values).await
            }
            UniversalOperation::ListPopFront { namespace, key, count } => {
                self.list_pop_front(&namespace, &key, count).await
            }
            UniversalOperation::ListPopBack { namespace, key, count } => {
                self.list_pop_back(&namespace, &key, count).await
            }
            UniversalOperation::ListRange { namespace, key, start, stop } => {
                self.list_range(&namespace, &key, start, stop).await
            }
            UniversalOperation::ListLength { namespace, key } => {
                self.list_length(&namespace, &key).await
            }
            UniversalOperation::SetAdd { namespace, key, members } => {
                self.set_add(&namespace, &key, members).await
            }
            UniversalOperation::SetRemove { namespace, key, members } => {
                self.set_remove(&namespace, &key, members).await
            }
            UniversalOperation::SetIsMember { namespace, key, member } => {
                self.set_is_member(&namespace, &key, member).await
            }
            UniversalOperation::SetMembers { namespace, key } => {
                self.set_members(&namespace, &key).await
            }
            UniversalOperation::SortedSetAdd { namespace, key, members } => {
                self.sorted_set_add(&namespace, &key, members).await
            }
            UniversalOperation::SortedSetRangeByScore { namespace, key, min, max, limit, offset } => {
                self.sorted_set_range_by_score(&namespace, &key, min, max, limit, offset).await
            }
            UniversalOperation::SortedSetRangeByRank { namespace, key, start, stop, with_scores } => {
                self.sorted_set_range_by_rank(&namespace, &key, start, stop, with_scores).await
            }
            UniversalOperation::SetTTL { namespace, key, ttl } => {
                self.set_ttl(&namespace, &key, ttl).await
            }
            UniversalOperation::GetTTL { namespace, key } => {
                self.get_ttl(&namespace, &key).await
            }
            UniversalOperation::RemoveTTL { namespace, key } => {
                self.remove_ttl(&namespace, &key).await
            }
            _ => {
                Err(UnifiedStorageError::InvalidOperation(
                    format!("Operation not yet implemented: {:?}", std::mem::discriminant(&operation))
                ))
            }
        }
    }

    // ============================================================================
    // Core CRUD Operations
    // ============================================================================

    fn data_key(namespace: &str, key: &str) -> String {
        format!("data:{}:{}", namespace, key)
    }

    /// Get a record by namespace and key
    pub async fn get(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        match self.backend.get(&storage_key).await? {
            Some(data) => {
                let record: UniversalRecord = serde_json::from_slice(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

                // Check if record has expired
                if record.is_expired() {
                    // Delete expired record
                    self.backend.delete(&storage_key).await?;
                    Ok(UniversalResult::Empty)
                } else {
                    Ok(UniversalResult::Record(record))
                }
            }
            None => Ok(UniversalResult::Empty),
        }
    }

    /// Put a record
    pub async fn put(
        &self,
        namespace: &str,
        key: &str,
        value: UniversalValue,
        ttl: Option<Duration>,
        if_not_exists: bool,
        if_version: Option<u64>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        // Check existing record for conditional operations
        let existing = self.backend.get(&storage_key).await?;

        if if_not_exists && existing.is_some() {
            return Err(UnifiedStorageError::KeyExists {
                namespace: namespace.to_string(),
                key: key.to_string(),
            });
        }

        let metadata = if let Some(data) = &existing {
            let mut existing_record: UniversalRecord = serde_json::from_slice(data)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

            // Check version for optimistic locking
            if let Some(expected_version) = if_version {
                if existing_record.metadata.version != expected_version {
                    return Err(UnifiedStorageError::VersionConflict {
                        expected: expected_version,
                        actual: existing_record.metadata.version,
                    });
                }
            }

            // Update existing metadata
            existing_record.metadata.touch();
            if let Some(ttl_duration) = ttl {
                let now = chrono::Utc::now().timestamp_millis();
                existing_record.metadata.ttl = Some(now + ttl_duration.as_millis() as i64);
            }
            existing_record.metadata
        } else {
            // Create new metadata
            if let Some(ttl_duration) = ttl {
                RecordMetadata::with_ttl("unified", ttl_duration.as_millis() as i64)
            } else {
                RecordMetadata::new("unified")
            }
        };

        let record = UniversalRecord {
            id: RecordId::new(namespace, key),
            value,
            metadata,
        };

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Ok)
    }

    /// Delete a record
    pub async fn delete(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);
        let removed = self.backend.delete(&storage_key).await?;

        if removed {
            Ok(UniversalResult::Count(1))
        } else {
            Ok(UniversalResult::Count(0))
        }
    }

    /// Check if a key exists
    pub async fn exists(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        if self.backend.exists(&storage_key).await? {
            // Also check if it's expired
            if let Some(data) = self.backend.get(&storage_key).await? {
                let record: UniversalRecord = serde_json::from_slice(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;
                if record.is_expired() {
                    self.backend.delete(&storage_key).await?;
                    return Ok(UniversalResult::Value(UniversalValue::Bool(false)));
                }
            }
            Ok(UniversalResult::Value(UniversalValue::Bool(true)))
        } else {
            Ok(UniversalResult::Value(UniversalValue::Bool(false)))
        }
    }

    // ============================================================================
    // Batch Operations
    // ============================================================================

    /// Get multiple records
    pub async fn multi_get(&self, namespace: &str, keys: &[String]) -> UnifiedStorageResult<UniversalResult> {
        let mut records = Vec::with_capacity(keys.len());

        for key in keys {
            let result = self.get(namespace, key).await?;
            if let UniversalResult::Record(record) = result {
                records.push(record);
            }
        }

        Ok(UniversalResult::Records(records))
    }

    /// Put multiple records
    pub async fn multi_put(&self, records: Vec<(RecordId, UniversalValue)>) -> UnifiedStorageResult<UniversalResult> {
        let mut entries = Vec::with_capacity(records.len());

        for (id, value) in records {
            let record = UniversalRecord {
                id: id.clone(),
                value,
                metadata: RecordMetadata::new("unified"),
            };

            let serialized = serde_json::to_vec(&record)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

            entries.push((Self::data_key(&id.namespace, &id.key), serialized));
        }

        self.backend.put_batch(entries).await?;

        Ok(UniversalResult::Ok)
    }

    /// Delete multiple records
    pub async fn multi_delete(&self, namespace: &str, keys: &[String]) -> UnifiedStorageResult<UniversalResult> {
        let storage_keys: Vec<String> = keys
            .iter()
            .map(|k| Self::data_key(namespace, k))
            .collect();

        let count = self.backend.delete_batch(storage_keys).await?;

        Ok(UniversalResult::Count(count))
    }

    // ============================================================================
    // Scan Operations
    // ============================================================================

    /// Scan records in a namespace
    pub async fn scan(
        &self,
        namespace: &str,
        filter: Option<FilterExpression>,
        limit: Option<usize>,
        offset: Option<usize>,
        order_by: Option<Vec<(String, SortOrder)>>,
        projection: Vec<String>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let prefix = format!("data:{}:", namespace);
        let max_limit = limit.unwrap_or(self.config.max_scan_limit);

        // Fetch all records matching prefix
        let entries = self.backend.scan_prefix(&prefix, None).await?;

        let mut records: Vec<UniversalRecord> = Vec::new();

        for (_, data) in entries {
            let record: UniversalRecord = serde_json::from_slice(&data)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

            // Skip expired records
            if record.is_expired() {
                continue;
            }

            // Apply filter
            if let Some(ref filter_expr) = filter {
                if !self.matches_filter(&record, filter_expr) {
                    continue;
                }
            }

            records.push(record);
        }

        // Apply ordering
        if let Some(ref order_fields) = order_by {
            records.sort_by(|a, b| {
                for (field, order) in order_fields {
                    let val_a = a.get_field(field);
                    let val_b = b.get_field(field);

                    let cmp = match (val_a, val_b) {
                        (Some(UniversalValue::Int(a)), Some(UniversalValue::Int(b))) => a.cmp(b),
                        (Some(UniversalValue::Float(a)), Some(UniversalValue::Float(b))) => {
                            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Some(UniversalValue::String(a)), Some(UniversalValue::String(b))) => a.cmp(b),
                        _ => std::cmp::Ordering::Equal,
                    };

                    let cmp = match order {
                        SortOrder::Ascending => cmp,
                        SortOrder::Descending => cmp.reverse(),
                    };

                    if cmp != std::cmp::Ordering::Equal {
                        return cmp;
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        // Apply offset and limit
        let offset = offset.unwrap_or(0);
        let records: Vec<_> = records.into_iter().skip(offset).take(max_limit).collect();

        // Apply projection
        let records = if !projection.is_empty() {
            records
                .into_iter()
                .map(|mut record| {
                    if let UniversalValue::Map(ref map) = record.value {
                        let projected: BTreeMap<String, UniversalValue> = map
                            .iter()
                            .filter(|(k, _)| projection.contains(k))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        record.value = UniversalValue::Map(projected);
                    }
                    record
                })
                .collect()
        } else {
            records
        };

        Ok(UniversalResult::Records(records))
    }

    /// Scan keys in a namespace
    pub async fn scan_keys(
        &self,
        namespace: &str,
        pattern: Option<String>,
        limit: Option<usize>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let prefix = format!("data:{}:", namespace);
        let entries = self.backend.scan_prefix(&prefix, limit).await?;

        let keys: Vec<UniversalValue> = entries
            .into_iter()
            .filter_map(|(key, data)| {
                // Extract the key from the storage key
                let record_key = key.strip_prefix(&prefix)?;

                // Apply pattern matching if provided
                if let Some(ref pattern) = pattern {
                    if !self.matches_pattern(record_key, pattern) {
                        return None;
                    }
                }

                // Check if record is not expired
                if let Ok(record) = serde_json::from_slice::<UniversalRecord>(&data) {
                    if record.is_expired() {
                        return None;
                    }
                }

                Some(UniversalValue::String(record_key.to_string()))
            })
            .collect();

        Ok(UniversalResult::Values(keys))
    }

    /// Count records in a namespace
    pub async fn count(&self, namespace: &str, filter: Option<FilterExpression>) -> UnifiedStorageResult<UniversalResult> {
        let prefix = format!("data:{}:", namespace);
        let entries = self.backend.scan_prefix(&prefix, None).await?;

        let mut count = 0u64;

        for (_, data) in entries {
            let record: UniversalRecord = serde_json::from_slice(&data)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

            if record.is_expired() {
                continue;
            }

            if let Some(ref filter_expr) = filter {
                if !self.matches_filter(&record, filter_expr) {
                    continue;
                }
            }

            count += 1;
        }

        Ok(UniversalResult::Count(count))
    }

    // ============================================================================
    // Field-Level Operations
    // ============================================================================

    /// Get a specific field from a record
    pub async fn get_field(&self, namespace: &str, key: &str, field: &str) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let Some(value) = record.get_field(field) {
                    Ok(UniversalResult::Value(value.clone()))
                } else {
                    Ok(UniversalResult::Empty)
                }
            }
            _ => Ok(UniversalResult::Empty),
        }
    }

    /// Set a specific field in a record
    pub async fn set_field(
        &self,
        namespace: &str,
        key: &str,
        field: &str,
        value: UniversalValue,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => {
                // Create new record with Map value
                UniversalRecord::new(namespace, key, UniversalValue::Map(BTreeMap::new()), "unified")
            }
        };

        // Ensure value is a Map
        let map = match &mut record.value {
            UniversalValue::Map(m) => m,
            _ => {
                // Convert to Map
                let mut new_map = BTreeMap::new();
                new_map.insert("_value".to_string(), record.value.clone());
                record.value = UniversalValue::Map(new_map);
                if let UniversalValue::Map(m) = &mut record.value {
                    m
                } else {
                    unreachable!()
                }
            }
        };

        map.insert(field.to_string(), value);
        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Ok)
    }

    /// Delete a field from a record
    pub async fn delete_field(&self, namespace: &str, key: &str, field: &str) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => return Ok(UniversalResult::Count(0)),
        };

        if let UniversalValue::Map(ref mut m) = record.value {
            let removed = m.remove(field).is_some();
            if removed {
                record.metadata.touch();

                let serialized = serde_json::to_vec(&record)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

                self.backend.put(&storage_key, &serialized).await?;

                return Ok(UniversalResult::Count(1));
            }
        }

        Ok(UniversalResult::Count(0))
    }

    /// Increment a numeric field
    pub async fn increment_field(
        &self,
        namespace: &str,
        key: &str,
        field: &str,
        delta: i64,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => {
                UniversalRecord::new(namespace, key, UniversalValue::Map(BTreeMap::new()), "unified")
            }
        };

        let new_value = if let UniversalValue::Map(ref mut m) = record.value {
            let current = m.get(field).and_then(|v| v.as_int()).unwrap_or(0);
            let new_value = current + delta;
            m.insert(field.to_string(), UniversalValue::Int(new_value));
            new_value
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot increment field on non-Map value".to_string()
            ));
        };

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Value(UniversalValue::Int(new_value)))
    }

    // ============================================================================
    // List Operations
    // ============================================================================

    /// Push values to the front of a list
    pub async fn list_push_front(
        &self,
        namespace: &str,
        key: &str,
        values: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => {
                UniversalRecord::new(namespace, key, UniversalValue::List(Vec::new()), "unified")
            }
        };

        let length = if let UniversalValue::List(ref mut list) = record.value {
            for value in values.into_iter().rev() {
                list.insert(0, value);
            }
            list.len()
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot push to non-List value".to_string()
            ));
        };

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Count(length as u64))
    }

    /// Push values to the back of a list
    pub async fn list_push_back(
        &self,
        namespace: &str,
        key: &str,
        values: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => {
                UniversalRecord::new(namespace, key, UniversalValue::List(Vec::new()), "unified")
            }
        };

        let length = if let UniversalValue::List(ref mut list) = record.value {
            list.extend(values);
            list.len()
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot push to non-List value".to_string()
            ));
        };

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Count(length as u64))
    }

    /// Pop values from the front of a list
    pub async fn list_pop_front(
        &self,
        namespace: &str,
        key: &str,
        count: Option<usize>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => return Ok(UniversalResult::Empty),
        };

        let popped = if let UniversalValue::List(ref mut list) = record.value {
            let count = count.unwrap_or(1);
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                if list.is_empty() {
                    break;
                }
                values.push(list.remove(0));
            }
            values
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot pop from non-List value".to_string()
            ));
        };

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        if popped.len() == 1 {
            Ok(UniversalResult::Value(popped.into_iter().next().unwrap()))
        } else {
            Ok(UniversalResult::Values(popped))
        }
    }

    /// Pop values from the back of a list
    pub async fn list_pop_back(
        &self,
        namespace: &str,
        key: &str,
        count: Option<usize>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => return Ok(UniversalResult::Empty),
        };

        let popped = if let UniversalValue::List(ref mut list) = record.value {
            let count = count.unwrap_or(1);
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                if let Some(value) = list.pop() {
                    values.push(value);
                } else {
                    break;
                }
            }
            values
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot pop from non-List value".to_string()
            ));
        };

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        if popped.len() == 1 {
            Ok(UniversalResult::Value(popped.into_iter().next().unwrap()))
        } else {
            Ok(UniversalResult::Values(popped))
        }
    }

    /// Get a range of list elements
    pub async fn list_range(
        &self,
        namespace: &str,
        key: &str,
        start: i64,
        stop: i64,
    ) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(ref list) = record.value {
                    let len = list.len() as i64;

                    // Handle negative indices (like Redis)
                    let start = if start < 0 { (len + start).max(0) } else { start };
                    let stop = if stop < 0 { len + stop + 1 } else { stop + 1 };

                    let start = start.max(0) as usize;
                    let stop = stop.max(0) as usize;

                    let values: Vec<UniversalValue> = list
                        .iter()
                        .skip(start)
                        .take(stop.saturating_sub(start))
                        .cloned()
                        .collect();

                    Ok(UniversalResult::Values(values))
                } else {
                    Err(UnifiedStorageError::InvalidOperation(
                        "Cannot get range from non-List value".to_string()
                    ))
                }
            }
            _ => Ok(UniversalResult::Values(Vec::new())),
        }
    }

    /// Get list length
    pub async fn list_length(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(ref list) = record.value {
                    Ok(UniversalResult::Count(list.len() as u64))
                } else {
                    Err(UnifiedStorageError::InvalidOperation(
                        "Cannot get length of non-List value".to_string()
                    ))
                }
            }
            _ => Ok(UniversalResult::Count(0)),
        }
    }

    // ============================================================================
    // Set Operations
    // ============================================================================

    /// Add members to a set
    pub async fn set_add(
        &self,
        namespace: &str,
        key: &str,
        members: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => {
                UniversalRecord::new(namespace, key, UniversalValue::Set(Vec::new()), "unified")
            }
        };

        let mut added = 0u64;

        if let UniversalValue::Set(ref mut set) = record.value {
            for member in members {
                if !set.contains(&member) {
                    set.push(member);
                    added += 1;
                }
            }
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot add to non-Set value".to_string()
            ));
        }

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Count(added))
    }

    /// Remove members from a set
    pub async fn set_remove(
        &self,
        namespace: &str,
        key: &str,
        members: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => return Ok(UniversalResult::Count(0)),
        };

        let mut removed = 0u64;

        if let UniversalValue::Set(ref mut set) = record.value {
            for member in &members {
                if let Some(pos) = set.iter().position(|m| m == member) {
                    set.remove(pos);
                    removed += 1;
                }
            }
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot remove from non-Set value".to_string()
            ));
        }

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Count(removed))
    }

    /// Check if a member is in a set
    pub async fn set_is_member(
        &self,
        namespace: &str,
        key: &str,
        member: UniversalValue,
    ) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Set(ref set) = record.value {
                    let is_member = set.contains(&member);
                    Ok(UniversalResult::Value(UniversalValue::Bool(is_member)))
                } else {
                    Err(UnifiedStorageError::InvalidOperation(
                        "Cannot check membership on non-Set value".to_string()
                    ))
                }
            }
            _ => Ok(UniversalResult::Value(UniversalValue::Bool(false))),
        }
    }

    /// Get all members of a set
    pub async fn set_members(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Set(set) = record.value {
                    Ok(UniversalResult::Values(set))
                } else {
                    Err(UnifiedStorageError::InvalidOperation(
                        "Cannot get members of non-Set value".to_string()
                    ))
                }
            }
            _ => Ok(UniversalResult::Values(Vec::new())),
        }
    }

    // ============================================================================
    // Sorted Set Operations
    // ============================================================================

    /// Add members with scores to a sorted set
    pub async fn sorted_set_add(
        &self,
        namespace: &str,
        key: &str,
        members: Vec<(UniversalValue, f64)>,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => {
                UniversalRecord::new(namespace, key, UniversalValue::SortedSet(Vec::new()), "unified")
            }
        };

        let mut added = 0u64;

        if let UniversalValue::SortedSet(ref mut sorted_set) = record.value {
            for (member, score) in members {
                // Check if member already exists
                if let Some(pos) = sorted_set.iter().position(|(m, _)| m == &member) {
                    // Update score
                    sorted_set[pos].1 = score;
                } else {
                    // Add new member
                    sorted_set.push((member, score));
                    added += 1;
                }
            }
            // Sort by score
            sorted_set.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            return Err(UnifiedStorageError::InvalidOperation(
                "Cannot add to non-SortedSet value".to_string()
            ));
        }

        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Count(added))
    }

    /// Get members by score range
    pub async fn sorted_set_range_by_score(
        &self,
        namespace: &str,
        key: &str,
        min: f64,
        max: f64,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::SortedSet(ref sorted_set) = record.value {
                    let values: Vec<UniversalValue> = sorted_set
                        .iter()
                        .filter(|(_, score)| *score >= min && *score <= max)
                        .skip(offset.unwrap_or(0))
                        .take(limit.unwrap_or(usize::MAX))
                        .map(|(member, _)| member.clone())
                        .collect();

                    Ok(UniversalResult::Values(values))
                } else {
                    Err(UnifiedStorageError::InvalidOperation(
                        "Cannot range on non-SortedSet value".to_string()
                    ))
                }
            }
            _ => Ok(UniversalResult::Values(Vec::new())),
        }
    }

    /// Get members by rank range
    pub async fn sorted_set_range_by_rank(
        &self,
        namespace: &str,
        key: &str,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::SortedSet(ref sorted_set) = record.value {
                    let len = sorted_set.len() as i64;

                    // Handle negative indices
                    let start = if start < 0 { (len + start).max(0) } else { start };
                    let stop = if stop < 0 { len + stop + 1 } else { stop + 1 };

                    let start = start.max(0) as usize;
                    let stop = stop.max(0) as usize;

                    if with_scores {
                        let values: Vec<UniversalValue> = sorted_set
                            .iter()
                            .skip(start)
                            .take(stop.saturating_sub(start))
                            .flat_map(|(member, score)| {
                                vec![member.clone(), UniversalValue::Float(*score)]
                            })
                            .collect();
                        Ok(UniversalResult::Values(values))
                    } else {
                        let values: Vec<UniversalValue> = sorted_set
                            .iter()
                            .skip(start)
                            .take(stop.saturating_sub(start))
                            .map(|(member, _)| member.clone())
                            .collect();
                        Ok(UniversalResult::Values(values))
                    }
                } else {
                    Err(UnifiedStorageError::InvalidOperation(
                        "Cannot range on non-SortedSet value".to_string()
                    ))
                }
            }
            _ => Ok(UniversalResult::Values(Vec::new())),
        }
    }

    // ============================================================================
    // TTL Operations
    // ============================================================================

    /// Set TTL on a key
    pub async fn set_ttl(
        &self,
        namespace: &str,
        key: &str,
        ttl: Duration,
    ) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => return Ok(UniversalResult::Value(UniversalValue::Bool(false))),
        };

        let now = chrono::Utc::now().timestamp_millis();
        record.metadata.ttl = Some(now + ttl.as_millis() as i64);
        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Value(UniversalValue::Bool(true)))
    }

    /// Get remaining TTL
    pub async fn get_ttl(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        match self.get(namespace, key).await? {
            UniversalResult::Record(record) => {
                if let Some(ttl) = record.metadata.ttl {
                    let now = chrono::Utc::now().timestamp_millis();
                    let remaining = (ttl - now) / 1000; // Return in seconds
                    Ok(UniversalResult::Value(UniversalValue::Int(remaining.max(-1))))
                } else {
                    // -1 means no TTL set (Redis convention)
                    Ok(UniversalResult::Value(UniversalValue::Int(-1)))
                }
            }
            _ => {
                // -2 means key doesn't exist (Redis convention)
                Ok(UniversalResult::Value(UniversalValue::Int(-2)))
            }
        }
    }

    /// Remove TTL from a key (make it persistent)
    pub async fn remove_ttl(&self, namespace: &str, key: &str) -> UnifiedStorageResult<UniversalResult> {
        let storage_key = Self::data_key(namespace, key);

        let mut record = match self.backend.get(&storage_key).await? {
            Some(data) => {
                serde_json::from_slice::<UniversalRecord>(&data)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?
            }
            None => return Ok(UniversalResult::Value(UniversalValue::Bool(false))),
        };

        let had_ttl = record.metadata.ttl.is_some();
        record.metadata.ttl = None;
        record.metadata.touch();

        let serialized = serde_json::to_vec(&record)
            .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;

        self.backend.put(&storage_key, &serialized).await?;

        Ok(UniversalResult::Value(UniversalValue::Bool(had_ttl)))
    }

    // ============================================================================
    // Helper Methods
    // ============================================================================

    /// Check if a record matches a filter expression
    fn matches_filter(&self, record: &UniversalRecord, filter: &FilterExpression) -> bool {
        match filter {
            FilterExpression::Eq(field, value) => {
                record.get_field(field).map_or(false, |v| v == value)
            }
            FilterExpression::Ne(field, value) => {
                record.get_field(field).map_or(true, |v| v != value)
            }
            FilterExpression::Gt(field, value) => {
                self.compare_values(record.get_field(field), Some(value)) == Some(std::cmp::Ordering::Greater)
            }
            FilterExpression::Gte(field, value) => {
                matches!(self.compare_values(record.get_field(field), Some(value)), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))
            }
            FilterExpression::Lt(field, value) => {
                self.compare_values(record.get_field(field), Some(value)) == Some(std::cmp::Ordering::Less)
            }
            FilterExpression::Lte(field, value) => {
                matches!(self.compare_values(record.get_field(field), Some(value)), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))
            }
            FilterExpression::In(field, values) => {
                record.get_field(field).map_or(false, |v| values.contains(v))
            }
            FilterExpression::NotIn(field, values) => {
                record.get_field(field).map_or(true, |v| !values.contains(v))
            }
            FilterExpression::Between(field, min, max) => {
                if let Some(v) = record.get_field(field) {
                    matches!(self.compare_values(Some(v), Some(min)), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))
                        && matches!(self.compare_values(Some(v), Some(max)), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))
                } else {
                    false
                }
            }
            FilterExpression::Like(field, pattern) => {
                record.get_field(field)
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| self.matches_like_pattern(s, pattern))
            }
            FilterExpression::ILike(field, pattern) => {
                record.get_field(field)
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| self.matches_like_pattern(&s.to_lowercase(), &pattern.to_lowercase()))
            }
            FilterExpression::StartsWith(field, prefix) => {
                record.get_field(field)
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s.starts_with(prefix))
            }
            FilterExpression::EndsWith(field, suffix) => {
                record.get_field(field)
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s.ends_with(suffix))
            }
            FilterExpression::Contains(field, substring) => {
                record.get_field(field)
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s.contains(substring))
            }
            FilterExpression::IsNull(field) => {
                record.get_field(field).map_or(true, |v| v.is_null())
            }
            FilterExpression::IsNotNull(field) => {
                record.get_field(field).map_or(false, |v| !v.is_null())
            }
            FilterExpression::And(left, right) => {
                self.matches_filter(record, left) && self.matches_filter(record, right)
            }
            FilterExpression::Or(left, right) => {
                self.matches_filter(record, left) || self.matches_filter(record, right)
            }
            FilterExpression::Not(expr) => {
                !self.matches_filter(record, expr)
            }
            _ => {
                // Other filters not yet implemented
                true
            }
        }
    }

    /// Compare two UniversalValues
    fn compare_values(&self, a: Option<&UniversalValue>, b: Option<&UniversalValue>) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Some(UniversalValue::Int(a)), Some(UniversalValue::Int(b))) => Some(a.cmp(b)),
            (Some(UniversalValue::Float(a)), Some(UniversalValue::Float(b))) => a.partial_cmp(b),
            (Some(UniversalValue::String(a)), Some(UniversalValue::String(b))) => Some(a.cmp(b)),
            (Some(UniversalValue::Int(a)), Some(UniversalValue::Float(b))) => (*a as f64).partial_cmp(b),
            (Some(UniversalValue::Float(a)), Some(UniversalValue::Int(b))) => a.partial_cmp(&(*b as f64)),
            _ => None,
        }
    }

    /// Match SQL LIKE pattern
    fn matches_like_pattern(&self, s: &str, pattern: &str) -> bool {
        // Simple LIKE implementation: % = any sequence, _ = single char
        let regex_pattern = pattern
            .replace('%', ".*")
            .replace('_', ".");
        regex::Regex::new(&format!("^{}$", regex_pattern))
            .map(|re| re.is_match(s))
            .unwrap_or(false)
    }

    /// Match glob pattern (like Redis KEYS)
    fn matches_pattern(&self, s: &str, pattern: &str) -> bool {
        // Simple glob: * = any sequence, ? = single char
        let regex_pattern = pattern
            .replace('*', ".*")
            .replace('?', ".");
        regex::Regex::new(&format!("^{}$", regex_pattern))
            .map(|re| re.is_match(s))
            .unwrap_or(false)
    }

    /// Get storage metrics
    pub async fn metrics(&self) -> UnifiedStorageMetrics {
        self.backend.metrics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_crud() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Put
        let result = storage.put(
            "users",
            "alice",
            UniversalValue::String("Alice".to_string()),
            None,
            false,
            None,
        ).await.unwrap();
        assert!(matches!(result, UniversalResult::Ok));

        // Get
        let result = storage.get("users", "alice").await.unwrap();
        match result {
            UniversalResult::Record(record) => {
                assert_eq!(record.id.namespace, "users");
                assert_eq!(record.id.key, "alice");
                assert_eq!(record.value, UniversalValue::String("Alice".to_string()));
            }
            _ => panic!("Expected Record result"),
        }

        // Exists
        let result = storage.exists("users", "alice").await.unwrap();
        assert!(matches!(result, UniversalResult::Value(UniversalValue::Bool(true))));

        // Delete
        let result = storage.delete("users", "alice").await.unwrap();
        assert!(matches!(result, UniversalResult::Count(1)));

        // Verify deleted
        let result = storage.exists("users", "alice").await.unwrap();
        assert!(matches!(result, UniversalResult::Value(UniversalValue::Bool(false))));
    }

    #[tokio::test]
    async fn test_map_operations() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Set field
        storage.set_field("users", "alice", "name", UniversalValue::String("Alice".to_string())).await.unwrap();
        storage.set_field("users", "alice", "age", UniversalValue::Int(30)).await.unwrap();

        // Get field
        let result = storage.get_field("users", "alice", "name").await.unwrap();
        assert!(matches!(result, UniversalResult::Value(UniversalValue::String(ref s)) if s == "Alice"));

        // Increment field
        let result = storage.increment_field("users", "alice", "age", 5).await.unwrap();
        assert!(matches!(result, UniversalResult::Value(UniversalValue::Int(35))));

        // Delete field
        let result = storage.delete_field("users", "alice", "name").await.unwrap();
        assert!(matches!(result, UniversalResult::Count(1)));
    }

    #[tokio::test]
    async fn test_list_operations() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Push back
        storage.list_push_back("lists", "mylist", vec![
            UniversalValue::String("a".to_string()),
            UniversalValue::String("b".to_string()),
        ]).await.unwrap();

        // Push front
        storage.list_push_front("lists", "mylist", vec![
            UniversalValue::String("x".to_string()),
        ]).await.unwrap();

        // Range
        let result = storage.list_range("lists", "mylist", 0, -1).await.unwrap();
        match result {
            UniversalResult::Values(values) => {
                assert_eq!(values.len(), 3);
                assert_eq!(values[0], UniversalValue::String("x".to_string()));
                assert_eq!(values[1], UniversalValue::String("a".to_string()));
                assert_eq!(values[2], UniversalValue::String("b".to_string()));
            }
            _ => panic!("Expected Values result"),
        }

        // Length
        let result = storage.list_length("lists", "mylist").await.unwrap();
        assert!(matches!(result, UniversalResult::Count(3)));
    }

    #[tokio::test]
    async fn test_set_operations() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Add members
        let result = storage.set_add("sets", "myset", vec![
            UniversalValue::String("a".to_string()),
            UniversalValue::String("b".to_string()),
            UniversalValue::String("c".to_string()),
        ]).await.unwrap();
        assert!(matches!(result, UniversalResult::Count(3)));

        // Is member
        let result = storage.set_is_member("sets", "myset", UniversalValue::String("a".to_string())).await.unwrap();
        assert!(matches!(result, UniversalResult::Value(UniversalValue::Bool(true))));

        // Remove member
        let result = storage.set_remove("sets", "myset", vec![
            UniversalValue::String("a".to_string()),
        ]).await.unwrap();
        assert!(matches!(result, UniversalResult::Count(1)));

        // Members
        let result = storage.set_members("sets", "myset").await.unwrap();
        match result {
            UniversalResult::Values(values) => {
                assert_eq!(values.len(), 2);
            }
            _ => panic!("Expected Values result"),
        }
    }

    #[tokio::test]
    async fn test_sorted_set_operations() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Add members with scores
        storage.sorted_set_add("zsets", "myzset", vec![
            (UniversalValue::String("a".to_string()), 1.0),
            (UniversalValue::String("b".to_string()), 2.0),
            (UniversalValue::String("c".to_string()), 3.0),
        ]).await.unwrap();

        // Range by score
        let result = storage.sorted_set_range_by_score("zsets", "myzset", 1.0, 2.5, None, None).await.unwrap();
        match result {
            UniversalResult::Values(values) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0], UniversalValue::String("a".to_string()));
                assert_eq!(values[1], UniversalValue::String("b".to_string()));
            }
            _ => panic!("Expected Values result"),
        }
    }

    #[tokio::test]
    async fn test_conditional_put() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Put with if_not_exists
        storage.put("test", "key1", UniversalValue::Int(1), None, true, None).await.unwrap();

        // Second put should fail
        let result = storage.put("test", "key1", UniversalValue::Int(2), None, true, None).await;
        assert!(matches!(result, Err(UnifiedStorageError::KeyExists { .. })));

        // Verify original value
        let result = storage.get("test", "key1").await.unwrap();
        match result {
            UniversalResult::Record(record) => {
                assert_eq!(record.value, UniversalValue::Int(1));
            }
            _ => panic!("Expected Record result"),
        }
    }

    #[tokio::test]
    async fn test_scan_with_filter() {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();

        // Create test data
        for i in 1..=10 {
            let mut map = BTreeMap::new();
            map.insert("id".to_string(), UniversalValue::Int(i));
            map.insert("name".to_string(), UniversalValue::String(format!("user{}", i)));
            storage.put(
                "users",
                &format!("user{}", i),
                UniversalValue::Map(map),
                None,
                false,
                None,
            ).await.unwrap();
        }

        // Scan with filter
        let filter = FilterExpression::Gt("id".to_string(), UniversalValue::Int(5));
        let result = storage.scan("users", Some(filter), None, None, None, vec![]).await.unwrap();

        match result {
            UniversalResult::Records(records) => {
                assert_eq!(records.len(), 5);
            }
            _ => panic!("Expected Records result"),
        }
    }
}
