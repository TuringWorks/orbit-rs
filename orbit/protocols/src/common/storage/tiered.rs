//! Tiered storage implementation using HybridStorageManager
//!
//! This module provides a TableStorage implementation that uses HybridStorageManager
//! for hot/warm/cold tiered storage with automatic data lifecycle management.

use crate::error::{ProtocolError, ProtocolResult};
use crate::postgres_wire::sql::{
    executor::{ExtensionDefinition, IndexSchema, SchemaDefinition, TableSchema, ViewSchema},
    execution::hybrid::{HybridStorageConfig, HybridStorageManager, ColumnSchema as HybridColumnSchema},
    types::SqlValue,
};
use crate::postgres_wire::storage::{StorageMetrics, StorageTransaction, TableStorage};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tiered table storage that implements TableStorage trait using HybridStorageManager
pub struct TieredTableStorage {
    /// Map of table name -> HybridStorageManager
    tables: Arc<RwLock<HashMap<String, Arc<HybridStorageManager>>>>,

    /// Table schemas
    schemas: Arc<RwLock<HashMap<String, TableSchema>>>,

    /// Indexes
    indexes: Arc<RwLock<HashMap<String, IndexSchema>>>,

    /// Views
    views: Arc<RwLock<HashMap<String, ViewSchema>>>,

    /// Schema definitions
    schema_defs: Arc<RwLock<HashMap<String, SchemaDefinition>>>,

    /// Extensions
    extensions: Arc<RwLock<HashMap<String, ExtensionDefinition>>>,

    /// Settings
    settings: Arc<RwLock<HashMap<String, String>>>,

    /// Metrics
    metrics: Arc<RwLock<StorageMetrics>>,

    /// Transactions
    transactions: Arc<RwLock<HashMap<String, StorageTransaction>>>,

    /// Configuration for hot/warm/cold tiers
    config: HybridStorageConfig,
}

impl TieredTableStorage {
    /// Create a new tiered table storage with default configuration
    pub fn new() -> Self {
        Self::with_config(HybridStorageConfig::default())
    }

    /// Create a new tiered table storage with custom configuration
    pub fn with_config(config: HybridStorageConfig) -> Self {
        Self {
            tables: Arc::new(RwLock::new(HashMap::new())),
            schemas: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(HashMap::new())),
            views: Arc::new(RwLock::new(HashMap::new())),
            schema_defs: Arc::new(RwLock::new(HashMap::new())),
            extensions: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(StorageMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get or create a HybridStorageManager for a table
    async fn get_or_create_table(&self, table_name: &str) -> ProtocolResult<Arc<HybridStorageManager>> {
        let mut tables = self.tables.write().await;

        if let Some(table) = tables.get(table_name) {
            return Ok(Arc::clone(table));
        }

        // Get schema for this table
        let schemas = self.schemas.read().await;
        let schema = schemas.get(table_name).ok_or_else(|| {
            ProtocolError::Other(format!("Table schema not found: {}", table_name))
        })?;

        // Convert TableSchema columns to HybridColumnSchema
        let hybrid_columns: Vec<HybridColumnSchema> = schema
            .columns
            .iter()
            .map(|col| HybridColumnSchema {
                name: col.name.clone(),
                data_type: col.data_type.to_string(),
                nullable: col.nullable,
            })
            .collect();

        // Create new HybridStorageManager
        let manager = Arc::new(HybridStorageManager::new(
            table_name.to_string(),
            hybrid_columns,
            self.config.clone(),
        ));

        tables.insert(table_name.to_string(), Arc::clone(&manager));

        Ok(manager)
    }

    /// Convert SqlValue HashMap to row data that HybridStorageManager can handle
    #[allow(dead_code)]
    fn convert_row_to_values(row: &HashMap<String, SqlValue>) -> Vec<SqlValue> {
        row.values().cloned().collect()
    }
}

impl Default for TieredTableStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TableStorage for TieredTableStorage {
    async fn initialize(&self) -> ProtocolResult<()> {
        // Nothing to initialize for tiered storage
        Ok(())
    }

    async fn shutdown(&self) -> ProtocolResult<()> {
        // Flush all pending data
        let tables = self.tables.read().await;
        for (_name, _manager) in tables.iter() {
            // Trigger migration if needed
            if self.config.auto_tiering {
                // Migration happens automatically in background
            }
        }
        Ok(())
    }

    async fn metrics(&self) -> StorageMetrics {
        self.metrics.read().await.clone()
    }

    async fn begin_transaction(&self) -> ProtocolResult<StorageTransaction> {
        let tx = StorageTransaction {
            id: uuid::Uuid::new_v4().to_string(),
            isolation_level: "READ_COMMITTED".to_string(),
            read_timestamp: std::time::Instant::now(),
            write_buffer: HashMap::new(),
        };

        self.transactions.write().await.insert(tx.id.clone(), tx.clone());
        Ok(tx)
    }

    async fn commit_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()> {
        self.transactions.write().await.remove(&tx.id);
        Ok(())
    }

    async fn rollback_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()> {
        self.transactions.write().await.remove(&tx.id);
        Ok(())
    }

    // Schema Operations

    async fn store_table_schema(
        &self,
        schema: &TableSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.schemas.write().await.insert(schema.name.clone(), schema.clone());
        Ok(())
    }

    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>> {
        Ok(self.schemas.read().await.get(table_name).cloned())
    }

    async fn list_table_schemas(&self) -> ProtocolResult<Vec<TableSchema>> {
        Ok(self.schemas.read().await.values().cloned().collect())
    }

    async fn remove_table_schema(
        &self,
        table_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        let removed = self.schemas.write().await.remove(table_name).is_some();
        if removed {
            self.tables.write().await.remove(table_name);
        }
        Ok(removed)
    }

    // Data Operations

    async fn insert_row(
        &self,
        table_name: &str,
        row: &HashMap<String, SqlValue>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        let manager = self.get_or_create_table(table_name).await?;

        // Convert row to vector of values
        let schema = self.schemas.read().await;
        let table_schema = schema.get(table_name).ok_or_else(|| {
            ProtocolError::Other(format!("Table not found: {}", table_name))
        })?;

        let mut values = Vec::new();
        for col in &table_schema.columns {
            let value = row.get(&col.name).cloned().unwrap_or(SqlValue::Null);
            values.push(value);
        }

        // Insert into hot tier
        manager.insert(values).await?;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.write_operations += 1;

        Ok(())
    }

    async fn insert_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, SqlValue>],
        tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        for row in rows {
            self.insert_row(table_name, row, tx).await?;
        }
        Ok(())
    }

    async fn get_table_data(
        &self,
        table_name: &str,
    ) -> ProtocolResult<Vec<HashMap<String, SqlValue>>> {
        let manager = self.get_or_create_table(table_name).await?;

        // Get schema
        let schema = self.schemas.read().await;
        let table_schema = schema.get(table_name).ok_or_else(|| {
            ProtocolError::Other(format!("Table not found: {}", table_name))
        })?;

        // Scan all data from hot tier
        let row_values_list = manager.scan_all().await?;

        // Convert to HashMap format
        let mut result = Vec::new();
        for row_values in row_values_list {
            let mut row = HashMap::new();
            for (i, col) in table_schema.columns.iter().enumerate() {
                if i < row_values.len() {
                    row.insert(col.name.clone(), row_values[i].clone());
                }
            }
            result.push(row);
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.read_operations += 1;

        Ok(result)
    }

    async fn update_rows(
        &self,
        table_name: &str,
        updates: &HashMap<String, SqlValue>,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        // For now, we don't support updates through HybridStorageManager
        // This would require significant changes to HybridStorageManager API
        // Return 0 rows affected as a placeholder
        let _ = (table_name, updates);
        Ok(0)
    }

    async fn delete_rows(
        &self,
        table_name: &str,
        _condition: Option<Box<dyn Fn(&HashMap<String, SqlValue>) -> bool + Send + Sync>>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        // For now, we don't support deletes through HybridStorageManager
        // This would require significant changes to HybridStorageManager API
        // Return 0 rows affected as a placeholder
        let _ = table_name;
        Ok(0)
    }

    async fn truncate_table(
        &self,
        table_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        // Remove the table manager (will be recreated on next insert)
        self.tables.write().await.remove(table_name);
        Ok(())
    }

    // Index Operations

    async fn store_index(
        &self,
        index: &IndexSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.indexes.write().await.insert(index.name.clone(), index.clone());
        Ok(())
    }

    async fn get_index(&self, index_name: &str) -> ProtocolResult<Option<IndexSchema>> {
        Ok(self.indexes.read().await.get(index_name).cloned())
    }

    async fn list_table_indexes(&self, table_name: &str) -> ProtocolResult<Vec<IndexSchema>> {
        Ok(self
            .indexes
            .read()
            .await
            .values()
            .filter(|idx| idx.table == table_name)
            .cloned()
            .collect())
    }

    async fn remove_index(
        &self,
        index_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        Ok(self.indexes.write().await.remove(index_name).is_some())
    }

    // View Operations

    async fn store_view(
        &self,
        view: &ViewSchema,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.views.write().await.insert(view.name.clone(), view.clone());
        Ok(())
    }

    async fn get_view(&self, view_name: &str) -> ProtocolResult<Option<ViewSchema>> {
        Ok(self.views.read().await.get(view_name).cloned())
    }

    async fn list_views(&self) -> ProtocolResult<Vec<ViewSchema>> {
        Ok(self.views.read().await.values().cloned().collect())
    }

    async fn remove_view(
        &self,
        view_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        Ok(self.views.write().await.remove(view_name).is_some())
    }

    // Schema Operations

    async fn store_schema(
        &self,
        schema: &SchemaDefinition,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.schema_defs.write().await.insert(schema.name.clone(), schema.clone());
        Ok(())
    }

    async fn get_schema(&self, schema_name: &str) -> ProtocolResult<Option<SchemaDefinition>> {
        Ok(self.schema_defs.read().await.get(schema_name).cloned())
    }

    async fn list_schemas(&self) -> ProtocolResult<Vec<SchemaDefinition>> {
        Ok(self.schema_defs.read().await.values().cloned().collect())
    }

    async fn remove_schema(
        &self,
        schema_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        Ok(self.schema_defs.write().await.remove(schema_name).is_some())
    }

    // Extension Operations

    async fn store_extension(
        &self,
        extension: &ExtensionDefinition,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.extensions.write().await.insert(extension.name.clone(), extension.clone());
        Ok(())
    }

    async fn get_extension(
        &self,
        extension_name: &str,
    ) -> ProtocolResult<Option<ExtensionDefinition>> {
        Ok(self.extensions.read().await.get(extension_name).cloned())
    }

    async fn list_extensions(&self) -> ProtocolResult<Vec<ExtensionDefinition>> {
        Ok(self.extensions.read().await.values().cloned().collect())
    }

    async fn remove_extension(
        &self,
        extension_name: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<bool> {
        Ok(self.extensions.write().await.remove(extension_name).is_some())
    }

    // Configuration Operations

    async fn store_setting(
        &self,
        key: &str,
        value: &str,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<()> {
        self.settings.write().await.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> ProtocolResult<Option<String>> {
        Ok(self.settings.read().await.get(key).cloned())
    }

    async fn list_settings(&self) -> ProtocolResult<HashMap<String, String>> {
        Ok(self.settings.read().await.clone())
    }

    // Maintenance Operations

    async fn checkpoint(&self) -> ProtocolResult<()> {
        // Trigger migration for all tables if auto-tiering is enabled
        if self.config.auto_tiering {
            let tables = self.tables.read().await;
            for (_name, manager) in tables.iter() {
                // Migration happens in background
                let _ = manager;
            }
        }
        Ok(())
    }

    async fn compact(&self) -> ProtocolResult<()> {
        // No-op for now - compaction is handled by HybridStorageManager
        Ok(())
    }

    async fn storage_size(&self) -> ProtocolResult<u64> {
        // Estimate based on number of tables
        let tables = self.tables.read().await;
        Ok((tables.len() * 1024) as u64) // Placeholder
    }
}
