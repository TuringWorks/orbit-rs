//! Secondary Index Manager for Unified Storage
//!
//! This module provides secondary index management for efficient queries on non-primary-key fields.
//! It enables fast lookups like "find all users with email = 'alice@example.com'" without
//! scanning all records.
//!
//! # Index Storage Format
//!
//! Index entries are stored with the key format:
//! - Namespace: `__idx__{namespace}` (e.g., `__idx__users`)
//! - Key: `{index_name}:{field_value}:{record_key}` (e.g., `idx_email:alice@example.com:alice`)
//!
//! This allows efficient prefix scanning for index lookups.

use super::operations::IndexDefinition;
use super::storage::{UnifiedStorage, UnifiedStorageResult};
use super::types::{UniversalRecord, UniversalValue};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// An entry in a secondary index
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// The namespace this index belongs to
    pub namespace: String,
    /// The name of the index
    pub index_name: String,
    /// The field value being indexed
    pub field_value: String,
    /// The key of the record this entry points to
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

    /// Get the storage namespace for this index entry
    pub fn storage_namespace(&self) -> String {
        format!("__idx__{}", self.namespace)
    }

    /// Get the storage key for this index entry
    pub fn storage_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.index_name, self.field_value, self.record_key
        )
    }
}

/// Statistics for an index
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    /// Number of entries in the index
    pub entry_count: u64,
    /// Number of unique values
    pub unique_values: u64,
    /// Index name
    pub name: String,
    /// Fields covered by this index
    pub fields: Vec<String>,
    /// Whether this is a unique index
    pub unique: bool,
}

/// Secondary Index Manager
///
/// Manages secondary indexes for efficient queries on non-primary-key fields.
/// Works with the UnifiedStorage to maintain index consistency.
pub struct SecondaryIndexManager {
    /// Reference to the underlying storage
    storage: Arc<UnifiedStorage>,
    /// Cache of registered indexes per namespace
    index_cache: Arc<RwLock<HashMap<String, Vec<IndexDefinition>>>>,
}

impl SecondaryIndexManager {
    /// Create a new SecondaryIndexManager
    pub fn new(storage: Arc<UnifiedStorage>) -> Self {
        Self {
            storage,
            index_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register indexes for a namespace
    pub async fn register_indexes(&self, namespace: &str, indexes: Vec<IndexDefinition>) {
        let mut cache = self.index_cache.write().await;
        cache.insert(namespace.to_string(), indexes);
    }

    /// Get registered indexes for a namespace
    pub async fn get_indexes(&self, namespace: &str) -> Vec<IndexDefinition> {
        let cache = self.index_cache.read().await;
        cache.get(namespace).cloned().unwrap_or_default()
    }

    /// Build index entries for a record based on registered indexes
    pub fn build_index_entries(
        &self,
        record: &UniversalRecord,
        indexes: &[IndexDefinition],
    ) -> Vec<IndexEntry> {
        let mut entries = Vec::new();

        for index in indexes {
            // For single-field indexes, extract the field value
            if index.fields.len() == 1 {
                let field = &index.fields[0];
                if let Some(value) = record.get_field(field) {
                    let field_value = self.value_to_string(value);
                    entries.push(IndexEntry::new(
                        &record.id.namespace,
                        &index.name,
                        &field_value,
                        &record.id.key,
                    ));
                }
            } else {
                // For composite indexes, concatenate field values
                let field_values: Vec<String> = index
                    .fields
                    .iter()
                    .filter_map(|f| record.get_field(f).map(|v| self.value_to_string(v)))
                    .collect();

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
        }

        entries
    }

    /// Index a record (creates index entries)
    pub async fn index_record(&self, record: &UniversalRecord) -> UnifiedStorageResult<usize> {
        let indexes = self.get_indexes(&record.id.namespace).await;
        if indexes.is_empty() {
            return Ok(0);
        }

        let entries = self.build_index_entries(record, &indexes);
        let count = entries.len();

        for entry in entries {
            // Store index entry: value is the record key for reference
            self.storage
                .put(
                    &entry.storage_namespace(),
                    &entry.storage_key(),
                    UniversalValue::String(entry.record_key.clone()),
                    None,
                    false,
                    None,
                )
                .await?;
        }

        Ok(count)
    }

    /// Remove index entries for a record
    pub async fn unindex_record(
        &self,
        namespace: &str,
        key: &str,
        old_record: Option<&UniversalRecord>,
    ) -> UnifiedStorageResult<usize> {
        // If we have the old record, use it to build entries to delete
        if let Some(record) = old_record {
            let indexes = self.get_indexes(namespace).await;
            let entries = self.build_index_entries(record, &indexes);
            let count = entries.len();

            for entry in entries {
                self.storage
                    .delete(&entry.storage_namespace(), &entry.storage_key())
                    .await?;
            }

            return Ok(count);
        }

        // Otherwise, scan for index entries referencing this key
        let indexes = self.get_indexes(namespace).await;
        let idx_namespace = format!("__idx__{}", namespace);
        let mut deleted = 0;

        for index in &indexes {
            // Scan all entries for this index and find ones pointing to our key
            let prefix = format!("{}:", index.name);
            let result = self
                .storage
                .scan_keys(&idx_namespace, Some(format!("{}*", prefix)), None)
                .await?;

            if let super::types::UniversalResult::Values(keys) = result {
                for key_val in keys {
                    if let UniversalValue::String(idx_key) = key_val {
                        // Check if this entry points to our record
                        if idx_key.ends_with(&format!(":{}", key)) {
                            self.storage.delete(&idx_namespace, &idx_key).await?;
                            deleted += 1;
                        }
                    }
                }
            }
        }

        Ok(deleted)
    }

    /// Query records by index
    pub async fn query_by_index(
        &self,
        namespace: &str,
        index_name: &str,
        field_value: &str,
    ) -> UnifiedStorageResult<Vec<String>> {
        let idx_namespace = format!("__idx__{}", namespace);
        let prefix = format!("{}:{}:", index_name, field_value);

        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(format!("{}*", prefix)), None)
            .await?;

        let mut record_keys = Vec::new();

        if let super::types::UniversalResult::Values(keys) = result {
            for key_val in keys {
                if let UniversalValue::String(idx_key) = key_val {
                    // Extract record key by stripping the known prefix
                    // Index key format: {index_name}:{field_value}:{record_key}
                    if let Some(record_key) = idx_key.strip_prefix(&prefix) {
                        record_keys.push(record_key.to_string());
                    }
                }
            }
        }

        Ok(record_keys)
    }

    /// Query records by index with range
    pub async fn query_by_index_range(
        &self,
        namespace: &str,
        index_name: &str,
        min_value: Option<&str>,
        max_value: Option<&str>,
    ) -> UnifiedStorageResult<Vec<String>> {
        let idx_namespace = format!("__idx__{}", namespace);
        let prefix = format!("{}:", index_name);

        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(format!("{}*", prefix)), None)
            .await?;

        let mut record_keys = Vec::new();

        if let super::types::UniversalResult::Values(keys) = result {
            for key_val in keys {
                if let UniversalValue::String(idx_key) = key_val {
                    // Extract field_value from index key: {index_name}:{field_value}:{record_key}
                    let parts: Vec<&str> = idx_key.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        let field_value = parts[1];
                        let record_key = parts[2];

                        // Check if within range
                        let in_range = match (min_value, max_value) {
                            (Some(min), Some(max)) => field_value >= min && field_value <= max,
                            (Some(min), None) => field_value >= min,
                            (None, Some(max)) => field_value <= max,
                            (None, None) => true,
                        };

                        if in_range {
                            record_keys.push(record_key.to_string());
                        }
                    }
                }
            }
        }

        Ok(record_keys)
    }

    /// Check if a unique constraint would be violated
    pub async fn check_unique_constraint(
        &self,
        namespace: &str,
        index_name: &str,
        field_value: &str,
        exclude_key: Option<&str>,
    ) -> UnifiedStorageResult<bool> {
        let existing = self
            .query_by_index(namespace, index_name, field_value)
            .await?;

        if existing.is_empty() {
            return Ok(true); // No conflict
        }

        // If we're updating a record, exclude it from the check
        if let Some(key) = exclude_key {
            let conflicts: Vec<_> = existing.iter().filter(|k| k.as_str() != key).collect();
            return Ok(conflicts.is_empty());
        }

        Ok(false) // Conflict exists
    }

    /// Rebuild all indexes for a namespace
    pub async fn rebuild_indexes(&self, namespace: &str) -> UnifiedStorageResult<usize> {
        let indexes = self.get_indexes(namespace).await;
        if indexes.is_empty() {
            return Ok(0);
        }

        // First, delete all existing index entries
        let idx_namespace = format!("__idx__{}", namespace);
        let result = self.storage.scan_keys(&idx_namespace, None, None).await?;

        if let super::types::UniversalResult::Values(keys) = result {
            for key_val in keys {
                if let UniversalValue::String(key) = key_val {
                    self.storage.delete(&idx_namespace, &key).await?;
                }
            }
        }

        // Now scan all records and rebuild indexes
        let result = self
            .storage
            .scan(namespace, None, None, None, None, vec![])
            .await?;

        let mut indexed_count = 0;

        if let super::types::UniversalResult::Records(records) = result {
            for record in records {
                indexed_count += self.index_record(&record).await?;
            }
        }

        Ok(indexed_count)
    }

    /// Get statistics for an index
    pub async fn get_index_stats(
        &self,
        namespace: &str,
        index_name: &str,
    ) -> UnifiedStorageResult<IndexStats> {
        let indexes = self.get_indexes(namespace).await;
        let index = indexes.iter().find(|i| i.name == index_name);

        let (fields, unique) = index
            .map(|i| (i.fields.clone(), i.unique))
            .unwrap_or_default();

        let idx_namespace = format!("__idx__{}", namespace);
        let prefix = format!("{}:", index_name);

        let result = self
            .storage
            .scan_keys(&idx_namespace, Some(format!("{}*", prefix)), None)
            .await?;

        let mut entry_count = 0u64;
        let mut unique_values = std::collections::HashSet::new();

        if let super::types::UniversalResult::Values(keys) = result {
            for key_val in keys {
                if let UniversalValue::String(idx_key) = key_val {
                    entry_count += 1;
                    // Extract field_value: {index_name}:{field_value}:{record_key}
                    let parts: Vec<&str> = idx_key.splitn(3, ':').collect();
                    if parts.len() >= 2 {
                        unique_values.insert(parts[1].to_string());
                    }
                }
            }
        }

        Ok(IndexStats {
            entry_count,
            unique_values: unique_values.len() as u64,
            name: index_name.to_string(),
            fields,
            unique,
        })
    }

    /// Convert a UniversalValue to a string for indexing
    fn value_to_string(&self, value: &UniversalValue) -> String {
        match value {
            UniversalValue::Null => "null".to_string(),
            UniversalValue::Bool(b) => b.to_string(),
            UniversalValue::Int(i) => i.to_string(),
            UniversalValue::Float(f) => f.to_string(),
            UniversalValue::String(s) => s.clone(),
            UniversalValue::Bytes(b) => b.iter().map(|byte| format!("{:02x}", byte)).collect(),
            UniversalValue::Timestamp(t) => t.to_string(),
            UniversalValue::Uuid(u) => u.iter().map(|byte| format!("{:02x}", byte)).collect(),
            // For complex types, use JSON representation
            _ => serde_json::to_string(value).unwrap_or_else(|_| "unknown".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified::operations::IndexType;
    use crate::unified::types::RecordId;
    use std::collections::BTreeMap;

    async fn create_test_storage() -> Arc<UnifiedStorage> {
        let storage = UnifiedStorage::with_memory_backend();
        storage.initialize().await.unwrap();
        Arc::new(storage)
    }

    fn create_user_record(
        namespace: &str,
        key: &str,
        name: &str,
        email: &str,
        city: &str,
    ) -> UniversalRecord {
        let mut fields = BTreeMap::new();
        fields.insert("name".to_string(), UniversalValue::String(name.to_string()));
        fields.insert(
            "email".to_string(),
            UniversalValue::String(email.to_string()),
        );
        fields.insert("city".to_string(), UniversalValue::String(city.to_string()));

        UniversalRecord {
            id: RecordId::new(namespace, key),
            value: UniversalValue::Map(fields),
            metadata: crate::unified::types::RecordMetadata::new("test"),
        }
    }

    #[tokio::test]
    async fn test_index_registration() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage);

        let indexes = vec![
            IndexDefinition {
                name: "idx_email".to_string(),
                fields: vec!["email".to_string()],
                unique: true,
                index_type: IndexType::BTree,
            },
            IndexDefinition {
                name: "idx_city".to_string(),
                fields: vec!["city".to_string()],
                unique: false,
                index_type: IndexType::BTree,
            },
        ];

        index_manager.register_indexes("users", indexes).await;

        let registered = index_manager.get_indexes("users").await;
        assert_eq!(registered.len(), 2);
        assert_eq!(registered[0].name, "idx_email");
        assert_eq!(registered[1].name, "idx_city");
    }

    #[tokio::test]
    async fn test_index_record() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage.clone());

        // Register indexes
        let indexes = vec![IndexDefinition {
            name: "idx_email".to_string(),
            fields: vec!["email".to_string()],
            unique: true,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create and store a record
        let record = create_user_record("users", "alice", "Alice", "alice@example.com", "NYC");
        storage
            .put(
                &record.id.namespace,
                &record.id.key,
                record.value.clone(),
                None,
                false,
                None,
            )
            .await
            .unwrap();

        // Index the record
        let indexed = index_manager.index_record(&record).await.unwrap();
        assert_eq!(indexed, 1);

        // Query by index
        let keys = index_manager
            .query_by_index("users", "idx_email", "alice@example.com")
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "alice");
    }

    #[tokio::test]
    async fn test_query_multiple_records() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage.clone());

        // Register city index
        let indexes = vec![IndexDefinition {
            name: "idx_city".to_string(),
            fields: vec!["city".to_string()],
            unique: false,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create multiple users in the same city
        let users = [
            ("alice", "Alice", "alice@example.com", "NYC"),
            ("bob", "Bob", "bob@example.com", "NYC"),
            ("charlie", "Charlie", "charlie@example.com", "LA"),
        ];

        for (key, name, email, city) in users {
            let record = create_user_record("users", key, name, email, city);
            storage
                .put(
                    &record.id.namespace,
                    &record.id.key,
                    record.value.clone(),
                    None,
                    false,
                    None,
                )
                .await
                .unwrap();
            index_manager.index_record(&record).await.unwrap();
        }

        // Query NYC users
        let nyc_users = index_manager
            .query_by_index("users", "idx_city", "NYC")
            .await
            .unwrap();
        assert_eq!(nyc_users.len(), 2);
        assert!(nyc_users.contains(&"alice".to_string()));
        assert!(nyc_users.contains(&"bob".to_string()));

        // Query LA users
        let la_users = index_manager
            .query_by_index("users", "idx_city", "LA")
            .await
            .unwrap();
        assert_eq!(la_users.len(), 1);
        assert_eq!(la_users[0], "charlie");
    }

    #[tokio::test]
    async fn test_unique_constraint() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage.clone());

        // Register unique email index
        let indexes = vec![IndexDefinition {
            name: "idx_email".to_string(),
            fields: vec!["email".to_string()],
            unique: true,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Index first user
        let record1 = create_user_record("users", "alice", "Alice", "alice@example.com", "NYC");
        storage
            .put(
                &record1.id.namespace,
                &record1.id.key,
                record1.value.clone(),
                None,
                false,
                None,
            )
            .await
            .unwrap();
        index_manager.index_record(&record1).await.unwrap();

        // Check unique constraint - should fail for same email
        let can_insert = index_manager
            .check_unique_constraint("users", "idx_email", "alice@example.com", None)
            .await
            .unwrap();
        assert!(!can_insert);

        // Check unique constraint - should pass for different email
        let can_insert = index_manager
            .check_unique_constraint("users", "idx_email", "bob@example.com", None)
            .await
            .unwrap();
        assert!(can_insert);

        // Check unique constraint - should pass when excluding the same record
        let can_insert = index_manager
            .check_unique_constraint("users", "idx_email", "alice@example.com", Some("alice"))
            .await
            .unwrap();
        assert!(can_insert);
    }

    #[tokio::test]
    async fn test_unindex_record() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage.clone());

        // Register index
        let indexes = vec![IndexDefinition {
            name: "idx_email".to_string(),
            fields: vec!["email".to_string()],
            unique: true,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create and index a record
        let record = create_user_record("users", "alice", "Alice", "alice@example.com", "NYC");
        storage
            .put(
                &record.id.namespace,
                &record.id.key,
                record.value.clone(),
                None,
                false,
                None,
            )
            .await
            .unwrap();
        index_manager.index_record(&record).await.unwrap();

        // Verify index entry exists
        let keys = index_manager
            .query_by_index("users", "idx_email", "alice@example.com")
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);

        // Unindex the record
        index_manager
            .unindex_record("users", "alice", Some(&record))
            .await
            .unwrap();

        // Verify index entry is removed
        let keys = index_manager
            .query_by_index("users", "idx_email", "alice@example.com")
            .await
            .unwrap();
        assert_eq!(keys.len(), 0);
    }

    #[tokio::test]
    async fn test_index_stats() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage.clone());

        // Register index
        let indexes = vec![IndexDefinition {
            name: "idx_city".to_string(),
            fields: vec!["city".to_string()],
            unique: false,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create multiple records
        let users = [
            ("alice", "Alice", "alice@example.com", "NYC"),
            ("bob", "Bob", "bob@example.com", "NYC"),
            ("charlie", "Charlie", "charlie@example.com", "LA"),
        ];

        for (key, name, email, city) in users {
            let record = create_user_record("users", key, name, email, city);
            storage
                .put(
                    &record.id.namespace,
                    &record.id.key,
                    record.value.clone(),
                    None,
                    false,
                    None,
                )
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
        assert_eq!(stats.unique_values, 2); // NYC and LA
        assert_eq!(stats.name, "idx_city");
    }

    #[tokio::test]
    async fn test_composite_index() {
        let storage = create_test_storage().await;
        let index_manager = SecondaryIndexManager::new(storage.clone());

        // Register composite index
        let indexes = vec![IndexDefinition {
            name: "idx_city_name".to_string(),
            fields: vec!["city".to_string(), "name".to_string()],
            unique: false,
            index_type: IndexType::BTree,
        }];
        index_manager.register_indexes("users", indexes).await;

        // Create record
        let record = create_user_record("users", "alice", "Alice", "alice@example.com", "NYC");
        storage
            .put(
                &record.id.namespace,
                &record.id.key,
                record.value.clone(),
                None,
                false,
                None,
            )
            .await
            .unwrap();
        index_manager.index_record(&record).await.unwrap();

        // Query by composite value
        let keys = index_manager
            .query_by_index("users", "idx_city_name", "NYC:Alice")
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "alice");
    }
}
