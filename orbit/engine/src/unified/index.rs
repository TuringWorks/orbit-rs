//! Secondary Index Management for Unified Storage
//!
//! This module provides secondary index support for the unified storage layer,
//! enabling efficient queries on non-primary-key fields.
//!
//! # Index Key Format
//!
//! Secondary indexes are stored with the following key format:
//! ```text
//! idx:{namespace}:{index_name}:{field_value}:{record_key}
//! ```
//!
//! # Supported Index Types
//!
//! - `BTree`: Standard B-tree index for range queries and equality
//! - `Hash`: Hash index for fast equality lookups
//! - `FullText`: Full-text search index (uses inverted index)
//! - `Geospatial`: Geospatial index for location queries
//! - `Vector`: Vector similarity search index
//!
//! # Example
//!
//! ```rust,ignore
//! use orbit_engine::unified::index::SecondaryIndexManager;
//!
//! let index_manager = SecondaryIndexManager::new(storage.clone());
//!
//! // Create index on email field
//! index_manager.create_index("users", "idx_email", vec!["email"], IndexType::Hash, false).await?;
//!
//! // Query by email
//! let records = index_manager.query_by_index("users", "idx_email", "alice@example.com").await?;
//! ```

use super::operations::IndexDefinition;
use super::storage::{UnifiedStorage, UnifiedStorageResult};
use super::types::{UniversalRecord, UniversalResult, UniversalValue};
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Secondary index entry stored in the backend
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// The namespace this index belongs to
    pub namespace: String,
    /// The index name
    pub index_name: String,
    /// The indexed field value
    pub field_value: String,
    /// The primary key of the record
    pub record_key: String,
}

impl IndexEntry {
    /// Create a new index entry
    pub fn new(namespace: &str, index_name: &str, field_value: &str, record_key: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            index_name: index_name.to_string(),
            field_value: field_value.to_string(),
            record_key: record_key.to_string(),
        }
    }

    /// Convert to storage key
    pub fn to_storage_key(&self) -> String {
        format!(
            "idx:{}:{}:{}:{}",
            self.namespace, self.index_name, self.field_value, self.record_key
        )
    }

    /// Parse from storage key
    pub fn from_storage_key(key: &str) -> Option<Self> {
        let parts: Vec<&str> = key.splitn(5, ':').collect();
        if parts.len() == 5 && parts[0] == "idx" {
            Some(Self {
                namespace: parts[1].to_string(),
                index_name: parts[2].to_string(),
                field_value: parts[3].to_string(),
                record_key: parts[4].to_string(),
            })
        } else {
            None
        }
    }
}

/// Manager for secondary indexes
pub struct SecondaryIndexManager {
    /// Reference to the unified storage
    storage: Arc<UnifiedStorage>,
    /// Cache of index definitions per namespace
    index_cache: Arc<RwLock<std::collections::HashMap<String, Vec<IndexDefinition>>>>,
}

impl SecondaryIndexManager {
    /// Create a new secondary index manager
    pub fn new(storage: Arc<UnifiedStorage>) -> Self {
        Self {
            storage,
            index_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Register index definitions for a namespace
    pub async fn register_indexes(&self, namespace: &str, indexes: Vec<IndexDefinition>) {
        let mut cache = self.index_cache.write().await;
        cache.insert(namespace.to_string(), indexes);
    }

    /// Get registered indexes for a namespace
    pub async fn get_indexes(&self, namespace: &str) -> Vec<IndexDefinition> {
        let cache = self.index_cache.read().await;
        cache.get(namespace).cloned().unwrap_or_default()
    }

    /// Build index entries for a record
    pub fn build_index_entries(
        &self,
        record: &UniversalRecord,
        indexes: &[IndexDefinition],
    ) -> Vec<IndexEntry> {
        let mut entries = Vec::new();

        for index in indexes {
            // Build composite key from all indexed fields
            let field_values: Vec<String> = index
                .fields
                .iter()
                .filter_map(|field| {
                    record.get_field(field).map(|v| self.value_to_index_key(v))
                })
                .collect();

            // Only create index entry if all fields have values
            if field_values.len() == index.fields.len() {
                let composite_value = field_values.join(":");
                entries.push(IndexEntry::new(
                    &record.id.namespace,
                    &index.name,
                    &composite_value,
                    &record.id.key,
                ));
            }
        }

        entries
    }

    /// Convert a value to an index key string
    fn value_to_index_key(&self, value: &UniversalValue) -> String {
        match value {
            UniversalValue::Null => "null".to_string(),
            UniversalValue::Bool(b) => b.to_string(),
            UniversalValue::Int(i) => format!("{:020}", i), // Zero-padded for proper ordering
            UniversalValue::Float(f) => format!("{:020.10}", f),
            UniversalValue::String(s) => s.clone(),
            UniversalValue::Bytes(b) => {
                // Simple hex encoding without external crate
                b.iter().map(|byte| format!("{:02x}", byte)).collect()
            }
            UniversalValue::Timestamp(ts) => format!("{:020}", ts),
            _ => serde_json::to_string(value).unwrap_or_default(),
        }
    }

    /// Index a record (called during put operations)
    pub async fn index_record(
        &self,
        record: &UniversalRecord,
    ) -> UnifiedStorageResult<usize> {
        let indexes = self.get_indexes(&record.id.namespace).await;
        if indexes.is_empty() {
            return Ok(0);
        }

        let entries = self.build_index_entries(record, &indexes);
        let mut indexed_count = 0;

        for entry in entries {
            // Store index entry using namespace-based key
            // Key format: {namespace}:{index_name}:{field_value}:{record_key}
            let index_key = format!(
                "{}:{}:{}",
                entry.index_name, entry.field_value, entry.record_key
            );
            self.storage
                .put(
                    &format!("__idx__{}", entry.namespace),
                    &index_key,
                    UniversalValue::String(record.id.key.clone()),
                    None,
                    false,
                    None,
                )
                .await?;
            indexed_count += 1;
        }

        Ok(indexed_count)
    }

    /// Remove index entries for a record (called during delete operations)
    pub async fn unindex_record(
        &self,
        namespace: &str,
        key: &str,
        old_record: Option<&UniversalRecord>,
    ) -> UnifiedStorageResult<usize> {
        let indexes = self.get_indexes(namespace).await;
        if indexes.is_empty() {
            return Ok(0);
        }

        let idx_namespace = format!("__idx__{}", namespace);

        // If we have the old record, use it to build exact index entries to delete
        if let Some(record) = old_record {
            let entries = self.build_index_entries(record, &indexes);
            let mut removed_count = 0;

            for entry in entries {
                let index_key = format!(
                    "{}:{}:{}",
                    entry.index_name, entry.field_value, entry.record_key
                );
                self.storage.delete(&idx_namespace, &index_key).await?;
                removed_count += 1;
            }

            return Ok(removed_count);
        }

        // Otherwise, scan for index entries containing this key
        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(format!("*:{}", key)), None)
            .await?;

        let mut removed_count = 0;
        if let UniversalResult::Values(keys) = result {
            for k in keys {
                if let UniversalValue::String(key_str) = k {
                    if key_str.ends_with(&format!(":{}", key)) {
                        self.storage.delete(&idx_namespace, &key_str).await?;
                        removed_count += 1;
                    }
                }
            }
        }

        Ok(removed_count)
    }

    /// Query records by index
    pub async fn query_by_index(
        &self,
        namespace: &str,
        index_name: &str,
        field_value: &str,
    ) -> UnifiedStorageResult<Vec<String>> {
        let idx_namespace = format!("__idx__{}", namespace);
        // Pattern: {index_name}:{field_value}:*
        let pattern = format!("{}:{}:*", index_name, field_value);
        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(pattern), None)
            .await?;

        let mut keys = Vec::new();
        if let UniversalResult::Values(index_keys) = result {
            for k in index_keys {
                if let UniversalValue::String(key_str) = k {
                    // Key format: {index_name}:{field_value}:{record_key}
                    // Extract record_key from the end
                    let parts: Vec<&str> = key_str.rsplitn(2, ':').collect();
                    if parts.len() == 2 {
                        keys.push(parts[0].to_string());
                    }
                }
            }
        }

        Ok(keys)
    }

    /// Query records by index range (for BTree indexes)
    pub async fn query_by_index_range(
        &self,
        namespace: &str,
        index_name: &str,
        min_value: Option<&str>,
        max_value: Option<&str>,
    ) -> UnifiedStorageResult<Vec<String>> {
        let idx_namespace = format!("__idx__{}", namespace);
        // Scan all index entries for this index
        let pattern = format!("{}:*", index_name);
        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(pattern), None)
            .await?;

        let mut keys = BTreeSet::new();
        if let UniversalResult::Values(index_keys) = result {
            for k in index_keys {
                if let UniversalValue::String(key_str) = k {
                    // Key format: {index_name}:{field_value}:{record_key}
                    let parts: Vec<&str> = key_str.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        let field_value = parts[1];
                        let record_key = parts[2];

                        // Check if value is in range
                        let in_range = match (min_value, max_value) {
                            (Some(min), Some(max)) => field_value >= min && field_value <= max,
                            (Some(min), None) => field_value >= min,
                            (None, Some(max)) => field_value <= max,
                            (None, None) => true,
                        };

                        if in_range {
                            keys.insert(record_key.to_string());
                        }
                    }
                }
            }
        }

        Ok(keys.into_iter().collect())
    }

    /// Check if a unique index constraint would be violated
    pub async fn check_unique_constraint(
        &self,
        namespace: &str,
        index_name: &str,
        field_value: &str,
        exclude_key: Option<&str>,
    ) -> UnifiedStorageResult<bool> {
        let keys = self.query_by_index(namespace, index_name, field_value).await?;

        match exclude_key {
            Some(exclude) => Ok(keys.iter().any(|k| k != exclude)),
            None => Ok(!keys.is_empty()),
        }
    }

    /// Rebuild all indexes for a namespace (for maintenance)
    pub async fn rebuild_indexes(&self, namespace: &str) -> UnifiedStorageResult<usize> {
        let indexes = self.get_indexes(namespace).await;
        if indexes.is_empty() {
            return Ok(0);
        }

        let idx_namespace = format!("__idx__{}", namespace);

        // First, delete all existing index entries for this namespace
        let result = self.storage.scan_keys(&idx_namespace, None, None).await?;

        if let UniversalResult::Values(keys) = result {
            for k in keys {
                if let UniversalValue::String(key_str) = k {
                    self.storage.delete(&idx_namespace, &key_str).await?;
                }
            }
        }

        // Now scan all records and rebuild indexes
        let records_result = self
            .storage
            .scan(namespace, None, None, None, None, vec![])
            .await?;

        let mut indexed_count = 0;
        if let UniversalResult::Records(records) = records_result {
            for record in records {
                indexed_count += self.index_record(&record).await?;
            }
        }

        Ok(indexed_count)
    }

    /// Get index statistics
    pub async fn get_index_stats(
        &self,
        namespace: &str,
        index_name: &str,
    ) -> UnifiedStorageResult<IndexStats> {
        let idx_namespace = format!("__idx__{}", namespace);
        let pattern = format!("{}:*", index_name);
        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(pattern), None)
            .await?;

        let mut entry_count = 0;
        let mut unique_values = std::collections::HashSet::new();

        if let UniversalResult::Values(keys) = result {
            for k in keys {
                if let UniversalValue::String(key_str) = k {
                    // Key format: {index_name}:{field_value}:{record_key}
                    let parts: Vec<&str> = key_str.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        entry_count += 1;
                        unique_values.insert(parts[1].to_string());
                    }
                }
            }
        }

        Ok(IndexStats {
            namespace: namespace.to_string(),
            index_name: index_name.to_string(),
            entry_count,
            unique_value_count: unique_values.len(),
            selectivity: if entry_count > 0 {
                unique_values.len() as f64 / entry_count as f64
            } else {
                0.0
            },
        })
    }
}

/// Statistics for an index
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Namespace the index belongs to
    pub namespace: String,
    /// Name of the index
    pub index_name: String,
    /// Total number of index entries
    pub entry_count: usize,
    /// Number of unique indexed values
    pub unique_value_count: usize,
    /// Selectivity (unique_values / total_entries), higher is better
    pub selectivity: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified::operations::IndexType;
    use crate::unified::types::RecordId;
    use std::collections::BTreeMap;

    async fn setup() -> (Arc<UnifiedStorage>, SecondaryIndexManager) {
        let storage = Arc::new(UnifiedStorage::with_memory_backend());
        storage.initialize().await.unwrap();
        let index_manager = SecondaryIndexManager::new(storage.clone());
        (storage, index_manager)
    }

    #[tokio::test]
    async fn test_index_entry_key_format() {
        let entry = IndexEntry::new("users", "idx_email", "alice@example.com", "user123");
        let key = entry.to_storage_key();
        assert_eq!(key, "idx:users:idx_email:alice@example.com:user123");

        let parsed = IndexEntry::from_storage_key(&key).unwrap();
        assert_eq!(parsed.namespace, "users");
        assert_eq!(parsed.index_name, "idx_email");
        assert_eq!(parsed.field_value, "alice@example.com");
        assert_eq!(parsed.record_key, "user123");
    }

    #[tokio::test]
    async fn test_index_record() {
        let (storage, index_manager) = setup().await;

        // Register index
        let indexes = vec![IndexDefinition {
            name: "idx_email".to_string(),
            fields: vec!["email".to_string()],
            unique: true,
            index_type: IndexType::Hash,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create and store record
        let mut fields = BTreeMap::new();
        fields.insert(
            "email".to_string(),
            UniversalValue::String("alice@example.com".to_string()),
        );
        fields.insert(
            "name".to_string(),
            UniversalValue::String("Alice".to_string()),
        );

        let record = UniversalRecord {
            id: RecordId::new("users", "alice"),
            value: UniversalValue::Map(fields),
            metadata: super::super::types::RecordMetadata::new("test"),
        };

        // Store the record first
        storage
            .put(
                "users",
                "alice",
                record.value.clone(),
                None,
                false,
                None,
            )
            .await
            .unwrap();

        // Index the record
        let count = index_manager.index_record(&record).await.unwrap();
        assert_eq!(count, 1);

        // Query by index
        let keys = index_manager
            .query_by_index("users", "idx_email", "alice@example.com")
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "alice");
    }

    #[tokio::test]
    async fn test_composite_index() {
        let (_storage, index_manager) = setup().await;

        // Register composite index
        let indexes = vec![IndexDefinition {
            name: "idx_name_age".to_string(),
            fields: vec!["name".to_string(), "age".to_string()],
            unique: false,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create record with both fields
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            UniversalValue::String("Alice".to_string()),
        );
        fields.insert("age".to_string(), UniversalValue::Int(30));

        let record = UniversalRecord {
            id: RecordId::new("users", "alice"),
            value: UniversalValue::Map(fields),
            metadata: super::super::types::RecordMetadata::new("test"),
        };

        // Index the record
        let count = index_manager.index_record(&record).await.unwrap();
        assert_eq!(count, 1);

        // Query composite value (name:age)
        let keys = index_manager
            .query_by_index("users", "idx_name_age", "Alice:00000000000000000030")
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "alice");
    }

    #[tokio::test]
    async fn test_unique_constraint() {
        let (storage, index_manager) = setup().await;

        // Register unique index
        let indexes = vec![IndexDefinition {
            name: "idx_email".to_string(),
            fields: vec!["email".to_string()],
            unique: true,
            index_type: IndexType::Hash,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create and index first record
        let mut fields1 = BTreeMap::new();
        fields1.insert(
            "email".to_string(),
            UniversalValue::String("alice@example.com".to_string()),
        );

        let record1 = UniversalRecord {
            id: RecordId::new("users", "alice"),
            value: UniversalValue::Map(fields1),
            metadata: super::super::types::RecordMetadata::new("test"),
        };

        storage
            .put("users", "alice", record1.value.clone(), None, false, None)
            .await
            .unwrap();
        index_manager.index_record(&record1).await.unwrap();

        // Check unique constraint - should be violated
        let violated = index_manager
            .check_unique_constraint("users", "idx_email", "alice@example.com", None)
            .await
            .unwrap();
        assert!(violated);

        // Check unique constraint with exclude - should not be violated
        let violated = index_manager
            .check_unique_constraint("users", "idx_email", "alice@example.com", Some("alice"))
            .await
            .unwrap();
        assert!(!violated);
    }

    #[tokio::test]
    async fn test_index_stats() {
        let (storage, index_manager) = setup().await;

        // Register index
        let indexes = vec![IndexDefinition {
            name: "idx_city".to_string(),
            fields: vec!["city".to_string()],
            unique: false,
            index_type: IndexType::Hash,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create and index multiple records
        for (key, city) in [("alice", "NYC"), ("bob", "NYC"), ("charlie", "LA")] {
            let mut fields = BTreeMap::new();
            fields.insert(
                "city".to_string(),
                UniversalValue::String(city.to_string()),
            );

            let record = UniversalRecord {
                id: RecordId::new("users", key),
                value: UniversalValue::Map(fields),
                metadata: super::super::types::RecordMetadata::new("test"),
            };

            storage
                .put("users", key, record.value.clone(), None, false, None)
                .await
                .unwrap();
            index_manager.index_record(&record).await.unwrap();
        }

        // Get stats
        let stats = index_manager
            .get_index_stats("users", "idx_city")
            .await
            .unwrap();

        assert_eq!(stats.entry_count, 3);
        assert_eq!(stats.unique_value_count, 2); // NYC and LA
        assert!(stats.selectivity > 0.6 && stats.selectivity < 0.7); // 2/3 ≈ 0.667
    }
}
