//! In-memory storage backend for PostgreSQL wire protocol
//!
//! This implementation provides a simple in-memory storage backend that's fast
//! but non-durable. Useful for testing and development scenarios.

use super::{StorageMetrics, StorageTransaction, TableStorage};
use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::{
    executor::{ExtensionDefinition, IndexSchema, SchemaDefinition, TableSchema, ViewSchema},
    types::SqlValue,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// Type alias to reduce complexity
type TableDataMap = Arc<RwLock<HashMap<String, Vec<HashMap<String, SqlValue>>>>>;

/// In-memory storage implementation
pub struct MemoryTableStorage {
    // Schema storage
    tables: Arc<RwLock<HashMap<String, TableSchema>>>,
    indexes: Arc<RwLock<HashMap<String, IndexSchema>>>,
    views: Arc<RwLock<HashMap<String, ViewSchema>>>,
    schemas: Arc<RwLock<HashMap<String, SchemaDefinition>>>,
    extensions: Arc<RwLock<HashMap<String, ExtensionDefinition>>>,

    // Data storage
    table_data: TableDataMap,

    // Configuration storage
    settings: Arc<RwLock<HashMap<String, String>>>,

    // Metrics
    metrics: Arc<RwLock<StorageMetrics>>,

    // Active transactions
    transactions: Arc<RwLock<HashMap<String, StorageTransaction>>>,
}

impl MemoryTableStorage {
    /// Create a new memory storage backend
    pub fn new() -> Self {
        Self {
            tables: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            views: Arc::new(RwLock::new(HashMap::new())),
            schemas: Arc::new(RwLock::new(HashMap::new())),
            extensions: Arc::new(RwLock::new(HashMap::new())),
            table_data: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(StorageMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update metrics for an operation
    async fn update_metrics(&self, operation: &str, duration: std::time::Duration, success: bool) {
        let mut metrics = self.metrics.write().await;

        match operation {
            "read" => {
                metrics.read_operations += 1;
                if metrics.read_operations > 1 {
                    metrics.read_latency_avg =
                        (metrics.read_latency_avg + duration.as_secs_f64()) / 2.0;
                } else {
                    metrics.read_latency_avg = duration.as_secs_f64();
                }
            }
            "write" => {
                metrics.write_operations += 1;
                if metrics.write_operations > 1 {
                    metrics.write_latency_avg =
                        (metrics.write_latency_avg + duration.as_secs_f64()) / 2.0;
                } else {
                    metrics.write_latency_avg = duration.as_secs_f64();
                }
            }
            "delete" => {
                metrics.delete_operations += 1;
                if metrics.delete_operations > 1 {
                    metrics.delete_latency_avg =
                        (metrics.delete_latency_avg + duration.as_secs_f64()) / 2.0;
                } else {
                    metrics.delete_latency_avg = duration.as_secs_f64();
                }
            }
            _ => {}
        }

        if !success {
            metrics.error_count += 1;
        }

        // Update cache hit rate (always 100% for memory storage)
        metrics.cache_hit_rate = 1.0;
    }

    /// Estimate memory usage
    async fn estimate_memory_usage(&self) -> u64 {
        // This is a rough estimate - in a real implementation you'd want more precise measurement
        let tables = self.tables.read().await;
        let table_data = self.table_data.read().await;

        let mut size = 0u64;

        // Estimate schema size
        size += tables.len() as u64 * 1024; // Rough estimate per table schema

        // Estimate data size
        for (_, rows) in table_data.iter() {
            size += rows.len() as u64 * 256; // Rough estimate per row
        }

        size
    }
}

impl Default for MemoryTableStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TableStorage for MemoryTableStorage {
    async fn initialize(&self) -> ProtocolResult<()> {
        // Initialize default settings
        let mut settings = self.settings.write().await;
        settings.insert(
            "server_version".to_string(),
            "14.0 (Orbit-RS Memory)".to_string(),
        );
        settings.insert("server_encoding".to_string(), "UTF8".to_string());
        settings.insert("client_encoding".to_string(), "UTF8".to_string());
        settings.insert("DateStyle".to_string(), "ISO, MDY".to_string());
        settings.insert("TimeZone".to_string(), "UTC".to_string());

        tracing::info!("Memory storage backend initialized");
        Ok(())
    }

    async fn shutdown(&self) -> ProtocolResult<()> {
        // Clear all data
        self.tables.write().await.clear();
        self.table_data.write().await.clear();
        self.transactions.write().await.clear();

        tracing::info!("Memory storage backend shutdown");
        Ok(())
    }

    async fn metrics(&self) -> StorageMetrics {
        let mut metrics = self.metrics.read().await.clone();
        metrics.memory_usage_bytes = self.estimate_memory_usage().await;
        metrics.disk_usage_bytes = 0; // No disk usage for memory backend
        metrics
    }

    async fn begin_transaction(&self) -> ProtocolResult<StorageTransaction> {
        let tx = StorageTransaction {
            id: Uuid::new_v4().to_string(),
            isolation_level: "READ COMMITTED".to_string(),
            read_timestamp: std::time::Instant::now(),
            write_buffer: HashMap::new(),
        };

        let mut transactions = self.transactions.write().await;
        transactions.insert(tx.id.clone(), tx.clone());

        Ok(tx)
    }

    async fn commit_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.remove(&tx.id);

        // In memory storage, all changes are immediately visible, so no additional work needed
        Ok(())
    }

    async fn rollback_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.remove(&tx.id);

        // In a full implementation, this would revert any changes made during the transaction
        // For now, we'll just remove the transaction record
        Ok(())
    }

    // Schema Operations

    async fn store_table_schema(
        &self,
        schema: &TableSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();
        let table_name = schema.name.clone();

        let mut tables = self.tables.write().await;
        tables.insert(table_name.clone(), schema.clone());

        // Initialize empty data storage for the table
        let mut table_data = self.table_data.write().await;
        table_data.insert(table_name, Vec::new());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>> {
        let start = std::time::Instant::now();
        let tables = self.tables.read().await;
        let result = tables.get(table_name).cloned();
        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn list_table_schemas(&self) -> ProtocolResult<Vec<TableSchema>> {
        let start = std::time::Instant::now();
        let tables = self.tables.read().await;
        let result = tables.values().cloned().collect();
        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn remove_table_schema(
        &self,
        table_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        let start = std::time::Instant::now();

        let mut tables = self.tables.write().await;
        let removed = tables.remove(table_name).is_some();

        if removed {
            // Also remove table data
            let mut table_data = self.table_data.write().await;
            table_data.remove(table_name);
        }

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    // Data Operations

    async fn insert_row(
        &self,
        table_name: &str,
        row: &HashMap<String, SqlValue>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut table_data = self.table_data.write().await;
        let table_rows = table_data
            .entry(table_name.to_string())
            .or_insert_with(Vec::new);
        table_rows.push(row.clone());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn insert_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, SqlValue>],
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut table_data = self.table_data.write().await;
        let table_rows = table_data
            .entry(table_name.to_string())
            .or_insert_with(Vec::new);
        table_rows.extend_from_slice(rows);

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_table_data(
        &self,
        table_name: &str,
    ) -> ProtocolResult<Vec<HashMap<String, SqlValue>>> {
        let start = std::time::Instant::now();

        let table_data = self.table_data.read().await;
        let result = table_data.get(table_name).cloned().unwrap_or_default();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn update_rows(
        &self,
        table_name: &str,
        updates: &HashMap<String, SqlValue>,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        let start = std::time::Instant::now();
        let mut count = 0;

        let mut table_data = self.table_data.write().await;

        if let Some(table_rows) = table_data.get_mut(table_name) {
            // For now, update all rows (condition support to be added later)
            for row in table_rows.iter_mut() {
                for (key, value) in updates.iter() {
                    row.insert(key.clone(), value.clone());
                }
                count += 1;
            }
        }

        self.update_metrics("write", start.elapsed(), count > 0)
            .await;
        Ok(count)
    }

    async fn delete_rows(
        &self,
        table_name: &str,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        let start = std::time::Instant::now();
        let mut count = 0;

        let mut table_data = self.table_data.write().await;

        if let Some(table_rows) = table_data.get_mut(table_name) {
            let original_len = table_rows.len();
            // For now, delete all rows (condition support to be added later)
            table_rows.clear();
            count = original_len;
        }

        self.update_metrics("delete", start.elapsed(), count > 0)
            .await;
        Ok(count)
    }

    async fn truncate_table(
        &self,
        table_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut table_data = self.table_data.write().await;
        if let Some(table_rows) = table_data.get_mut(table_name) {
            table_rows.clear();
        }

        self.update_metrics("delete", start.elapsed(), true).await;
        Ok(())
    }

    // Index Operations

    async fn store_index(
        &self,
        index: &IndexSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut indexes = self.indexes.write().await;
        indexes.insert(index.name.clone(), index.clone());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_index(&self, index_name: &str) -> ProtocolResult<Option<IndexSchema>> {
        let start = std::time::Instant::now();

        let indexes = self.indexes.read().await;
        let result = indexes.get(index_name).cloned();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn list_table_indexes(&self, table_name: &str) -> ProtocolResult<Vec<IndexSchema>> {
        let start = std::time::Instant::now();

        let indexes = self.indexes.read().await;
        let result = indexes
            .values()
            .filter(|idx| idx.table == table_name)
            .cloned()
            .collect();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn remove_index(
        &self,
        index_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        let start = std::time::Instant::now();

        let mut indexes = self.indexes.write().await;
        let removed = indexes.remove(index_name).is_some();

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    // View Operations

    async fn store_view(
        &self,
        view: &ViewSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut views = self.views.write().await;
        views.insert(view.name.clone(), view.clone());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_view(&self, view_name: &str) -> ProtocolResult<Option<ViewSchema>> {
        let start = std::time::Instant::now();

        let views = self.views.read().await;
        let result = views.get(view_name).cloned();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn list_views(&self) -> ProtocolResult<Vec<ViewSchema>> {
        let start = std::time::Instant::now();

        let views = self.views.read().await;
        let result = views.values().cloned().collect();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn remove_view(
        &self,
        view_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        let start = std::time::Instant::now();

        let mut views = self.views.write().await;
        let removed = views.remove(view_name).is_some();

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    // Schema Operations

    async fn store_schema(
        &self,
        schema: &SchemaDefinition,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut schemas = self.schemas.write().await;
        schemas.insert(schema.name.clone(), schema.clone());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_schema(&self, schema_name: &str) -> ProtocolResult<Option<SchemaDefinition>> {
        let start = std::time::Instant::now();

        let schemas = self.schemas.read().await;
        let result = schemas.get(schema_name).cloned();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn list_schemas(&self) -> ProtocolResult<Vec<SchemaDefinition>> {
        let start = std::time::Instant::now();

        let schemas = self.schemas.read().await;
        let result = schemas.values().cloned().collect();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn remove_schema(
        &self,
        schema_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        let start = std::time::Instant::now();

        let mut schemas = self.schemas.write().await;
        let removed = schemas.remove(schema_name).is_some();

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    // Extension Operations

    async fn store_extension(
        &self,
        extension: &ExtensionDefinition,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut extensions = self.extensions.write().await;
        extensions.insert(extension.name.clone(), extension.clone());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_extension(
        &self,
        extension_name: &str,
    ) -> ProtocolResult<Option<ExtensionDefinition>> {
        let start = std::time::Instant::now();

        let extensions = self.extensions.read().await;
        let result = extensions.get(extension_name).cloned();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn list_extensions(&self) -> ProtocolResult<Vec<ExtensionDefinition>> {
        let start = std::time::Instant::now();

        let extensions = self.extensions.read().await;
        let result = extensions.values().cloned().collect();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn remove_extension(
        &self,
        extension_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        let start = std::time::Instant::now();

        let mut extensions = self.extensions.write().await;
        let removed = extensions.remove(extension_name).is_some();

        self.update_metrics("delete", start.elapsed(), removed)
            .await;
        Ok(removed)
    }

    // Configuration Operations

    async fn store_setting(
        &self,
        key: &str,
        value: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let start = std::time::Instant::now();

        let mut settings = self.settings.write().await;
        settings.insert(key.to_string(), value.to_string());

        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> ProtocolResult<Option<String>> {
        let start = std::time::Instant::now();

        let settings = self.settings.read().await;
        let result = settings.get(key).cloned();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn list_settings(&self) -> ProtocolResult<HashMap<String, String>> {
        let start = std::time::Instant::now();

        let settings = self.settings.read().await;
        let result = settings.clone();

        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    // Maintenance Operations

    async fn checkpoint(&self) -> ProtocolResult<()> {
        // No-op for memory storage
        Ok(())
    }

    async fn compact(&self) -> ProtocolResult<()> {
        // No-op for memory storage
        Ok(())
    }

    async fn storage_size(&self) -> ProtocolResult<u64> {
        Ok(self.estimate_memory_usage().await)
    }
}
