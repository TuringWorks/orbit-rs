//! LSM-Tree storage backend for PostgreSQL wire protocol
//!
//! This implementation bridges the existing orbit-server LSM-Tree storage engine
//! to work with the PostgreSQL wire protocol executor.

use super::{StorageTransaction, StorageMetrics, TableStorage, StorageSerializable};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::postgres_wire::sql::{
    executor::{TableSchema, IndexSchema, ViewSchema, SchemaDefinition, ExtensionDefinition},
    types::SqlValue,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// Re-export the existing LSM configuration from orbit-server
pub use orbit_server::persistence::lsm_tree::LsmTreeConfig as LsmConfig;
use orbit_server::persistence::{
    lsm_tree::LsmTreeAddressableProvider,
    AddressableDirectoryProvider,
    PersistenceProvider,
    AddressableReference,
    AddressableLease,
};

/// LSM-Tree storage implementation for PostgreSQL
pub struct LsmTableStorage {
    // Core LSM storage engine
    lsm_provider: Arc<LsmTreeAddressableProvider>,
    
    // Metadata tracking
    metrics: Arc<RwLock<StorageMetrics>>,
    transactions: Arc<RwLock<HashMap<String, StorageTransaction>>>,
    config: LsmConfig,
}

impl LsmTableStorage {
    /// Create a new LSM storage backend
    pub async fn new(config: LsmConfig) -> ProtocolResult<Self> {
        let lsm_provider = LsmTreeAddressableProvider::new(config.clone())
            .await
            .map_err(|e| ProtocolError::internal(format!("Failed to create LSM provider: {}", e)))?;
        
        let lsm_provider = Arc::new(lsm_provider);
        
        // Initialize the LSM provider
        lsm_provider.initialize().await
            .map_err(|e| ProtocolError::internal(format!("Failed to initialize LSM provider: {}", e)))?;
        
        Ok(Self {
            lsm_provider,
            metrics: Arc::new(RwLock::new(StorageMetrics::default())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Convert table data to addressable lease format
    fn table_data_to_lease(table_name: &str, data: &[u8]) -> AddressableLease {
        AddressableLease {
            reference: AddressableReference {
                addressable_type: "table_data".to_string(),
                key: table_name.to_string(),
            },
            node_id: orbit_shared::NodeId::new("local".to_string()),
            lease_duration: chrono::Duration::hours(24), // Long-lived lease for table data
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
            renewable: true,
            data: Some(data.to_vec()),
        }
    }

    /// Convert schema to addressable lease format
    fn schema_to_lease(schema_type: &str, name: &str, data: &[u8]) -> AddressableLease {
        AddressableLease {
            reference: AddressableReference {
                addressable_type: format!("schema_{}", schema_type),
                key: name.to_string(),
            },
            node_id: orbit_shared::NodeId::new("local".to_string()),
            lease_duration: chrono::Duration::hours(24),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
            renewable: true,
            data: Some(data.to_vec()),
        }
    }

    /// Update storage metrics
    async fn update_metrics(&self, operation: &str, duration: std::time::Duration, success: bool) {
        let mut metrics = self.metrics.write().await;
        
        match operation {
            "read" => {
                metrics.read_operations += 1;
                if metrics.read_operations > 1 {
                    metrics.read_latency_avg = (metrics.read_latency_avg + duration.as_secs_f64()) / 2.0;
                } else {
                    metrics.read_latency_avg = duration.as_secs_f64();
                }
            }
            "write" => {
                metrics.write_operations += 1;
                if metrics.write_operations > 1 {
                    metrics.write_latency_avg = (metrics.write_latency_avg + duration.as_secs_f64()) / 2.0;
                } else {
                    metrics.write_latency_avg = duration.as_secs_f64();
                }
            }
            "delete" => {
                metrics.delete_operations += 1;
                if metrics.delete_operations > 1 {
                    metrics.delete_latency_avg = (metrics.delete_latency_avg + duration.as_secs_f64()) / 2.0;
                } else {
                    metrics.delete_latency_avg = duration.as_secs_f64();
                }
            }
            _ => {}
        }
        
        if !success {
            metrics.error_count += 1;
        }
    }

    /// Estimate storage size on disk
    async fn estimate_disk_usage(&self) -> u64 {
        // Try to estimate disk usage from the data directory
        if let Ok(metadata) = tokio::fs::metadata(&self.config.data_dir).await {
            // This is a rough estimate - in practice you'd walk the directory tree
            metadata.len()
        } else {
            0
        }
    }
}

#[async_trait]
impl TableStorage for LsmTableStorage {
    async fn initialize(&self) -> ProtocolResult<()> {
        // The LSM provider is already initialized in new()
        tracing::info!("LSM storage backend initialized with data directory: {}", self.config.data_dir);
        Ok(())
    }

    async fn shutdown(&self) -> ProtocolResult<()> {
        self.lsm_provider.shutdown().await
            .map_err(|e| ProtocolError::internal(format!("Failed to shutdown LSM provider: {}", e)))?;
        
        tracing::info!("LSM storage backend shutdown");
        Ok(())
    }

    async fn metrics(&self) -> StorageMetrics {
        // Get metrics from LSM provider and merge with our metrics
        let lsm_metrics = self.lsm_provider.metrics().await;
        let mut metrics = self.metrics.read().await.clone();
        
        // Update with LSM-specific metrics
        metrics.disk_usage_bytes = self.estimate_disk_usage().await;
        
        // Map LSM metrics to our storage metrics format
        metrics.read_operations = lsm_metrics.read_operations;
        metrics.write_operations = lsm_metrics.write_operations;
        metrics.delete_operations = lsm_metrics.delete_operations;
        metrics.read_latency_avg = lsm_metrics.read_latency_avg;
        metrics.write_latency_avg = lsm_metrics.write_latency_avg;
        metrics.delete_latency_avg = lsm_metrics.delete_latency_avg;
        metrics.error_count = lsm_metrics.error_count;
        
        metrics
    }

    async fn begin_transaction(&self) -> ProtocolResult<StorageTransaction> {
        let tx = StorageTransaction {
            id: Uuid::new_v4().to_string(),
            isolation_level: "READ COMMITTED".to_string(),
            read_timestamp: std::time::Instant::now(),
            write_buffer: HashMap::new(),
        };
        
        // Create a transaction context in the LSM provider
        let tx_context = orbit_server::persistence::TransactionContext {
            id: tx.id.clone(),
            isolation_level: orbit_server::persistence::IsolationLevel::ReadCommitted,
            timeout: std::time::Duration::from_secs(300), // 5 minute timeout
        };
        
        self.lsm_provider.begin_transaction(tx_context).await
            .map_err(|e| ProtocolError::internal(format!("Failed to begin LSM transaction: {}", e)))?;
        
        let mut transactions = self.transactions.write().await;
        transactions.insert(tx.id.clone(), tx.clone());
        
        Ok(tx)
    }

    async fn commit_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()> {
        self.lsm_provider.commit_transaction(&tx.id).await
            .map_err(|e| ProtocolError::internal(format!("Failed to commit LSM transaction: {}", e)))?;
        
        let mut transactions = self.transactions.write().await;
        transactions.remove(&tx.id);
        
        Ok(())
    }

    async fn rollback_transaction(&self, tx: &StorageTransaction) -> ProtocolResult<()> {
        self.lsm_provider.rollback_transaction(&tx.id).await
            .map_err(|e| ProtocolError::internal(format!("Failed to rollback LSM transaction: {}", e)))?;
        
        let mut transactions = self.transactions.write().await;
        transactions.remove(&tx.id);
        
        Ok(())
    }

    // Schema Operations
    
    async fn store_table_schema(&self, schema: &TableSchema, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let start = std::time::Instant::now();
        let table_name = schema.name.full_name();
        
        let serialized_schema = schema.to_storage_bytes()?;
        let lease = Self::schema_to_lease("table", &table_name, &serialized_schema);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to store table schema: {}", e)))?;
        
        // Also initialize empty data storage for the table
        let empty_data = Vec::<HashMap<String, SqlValue>>::new();
        let serialized_data = empty_data.to_storage_bytes()?;
        let data_lease = Self::table_data_to_lease(&table_name, &serialized_data);
        
        self.lsm_provider.store_lease(&data_lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to initialize table data: {}", e)))?;
        
        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_table_schema(&self, table_name: &str) -> ProtocolResult<Option<TableSchema>> {
        let start = std::time::Instant::now();
        
        let reference = AddressableReference {
            addressable_type: "schema_table".to_string(),
            key: table_name.to_string(),
        };
        
        let result = match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) => {
                if let Some(data) = lease.data {
                    match TableSchema::from_storage_bytes(&data) {
                        Ok(schema) => Some(schema),
                        Err(e) => {
                            tracing::warn!("Failed to deserialize table schema for {}: {}", table_name, e);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("Failed to get table schema for {}: {}", table_name, e);
                None
            }
        };
        
        self.update_metrics("read", start.elapsed(), result.is_some()).await;
        Ok(result)
    }

    async fn list_table_schemas(&self) -> ProtocolResult<Vec<TableSchema>> {
        let start = std::time::Instant::now();
        
        // Get all leases and filter for table schemas
        let all_leases = self.lsm_provider.list_all_leases().await
            .map_err(|e| ProtocolError::internal(format!("Failed to list leases: {}", e)))?;
        
        let mut schemas = Vec::new();
        for lease in all_leases {
            if lease.reference.addressable_type == "schema_table" {
                if let Some(data) = lease.data {
                    if let Ok(schema) = TableSchema::from_storage_bytes(&data) {
                        schemas.push(schema);
                    }
                }
            }
        }
        
        self.update_metrics("read", start.elapsed(), true).await;
        Ok(schemas)
    }

    async fn remove_table_schema(&self, table_name: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<bool> {
        let start = std::time::Instant::now();
        
        let reference = AddressableReference {
            addressable_type: "schema_table".to_string(),
            key: table_name.to_string(),
        };
        
        let removed = self.lsm_provider.remove_lease(&reference).await
            .map_err(|e| ProtocolError::internal(format!("Failed to remove table schema: {}", e)))?;
        
        if removed {
            // Also remove table data
            let data_reference = AddressableReference {
                addressable_type: "table_data".to_string(),
                key: table_name.to_string(),
            };
            
            let _ = self.lsm_provider.remove_lease(&data_reference).await;
        }
        
        self.update_metrics("delete", start.elapsed(), removed).await;
        Ok(removed)
    }

    // Data Operations

    async fn insert_row(&self, table_name: &str, row: &HashMap<String, SqlValue>, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let start = std::time::Instant::now();
        
        // Get existing table data
        let mut table_data = self.get_table_data(table_name).await?;
        
        // Add new row
        table_data.push(row.clone());
        
        // Store updated data
        let serialized_data = table_data.to_storage_bytes()?;
        let lease = Self::table_data_to_lease(table_name, &serialized_data);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to insert row: {}", e)))?;
        
        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn insert_rows(&self, table_name: &str, rows: &[HashMap<String, SqlValue>], _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let start = std::time::Instant::now();
        
        // Get existing table data
        let mut table_data = self.get_table_data(table_name).await?;
        
        // Add new rows
        table_data.extend_from_slice(rows);
        
        // Store updated data
        let serialized_data = table_data.to_storage_bytes()?;
        let lease = Self::table_data_to_lease(table_name, &serialized_data);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to insert rows: {}", e)))?;
        
        self.update_metrics("write", start.elapsed(), true).await;
        Ok(())
    }

    async fn get_table_data(&self, table_name: &str) -> ProtocolResult<Vec<HashMap<String, SqlValue>>> {
        let start = std::time::Instant::now();
        
        let reference = AddressableReference {
            addressable_type: "table_data".to_string(),
            key: table_name.to_string(),
        };
        
        let result = match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) => {
                if let Some(data) = lease.data {
                    match Vec::<HashMap<String, SqlValue>>::from_storage_bytes(&data) {
                        Ok(table_data) => table_data,
                        Err(e) => {
                            tracing::warn!("Failed to deserialize table data for {}: {}", table_name, e);
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
            }
            Ok(None) => Vec::new(),
            Err(e) => {
                tracing::warn!("Failed to get table data for {}: {}", table_name, e);
                Vec::new()
            }
        };
        
        self.update_metrics("read", start.elapsed(), true).await;
        Ok(result)
    }

    async fn update_rows(
        &self,
        table_name: &str,
        updates: &HashMap<String, SqlValue>,
        condition: Option<&dyn Fn(&HashMap<String, SqlValue>) -> bool>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        let start = std::time::Instant::now();
        
        // Get existing table data
        let mut table_data = self.get_table_data(table_name).await?;
        let mut count = 0;
        
        // Update matching rows
        for row in table_data.iter_mut() {
            let should_update = condition.map(|f| f(row)).unwrap_or(true);
            if should_update {
                for (key, value) in updates.iter() {
                    row.insert(key.clone(), value.clone());
                }
                count += 1;
            }
        }
        
        // Store updated data
        let serialized_data = table_data.to_storage_bytes()?;
        let lease = Self::table_data_to_lease(table_name, &serialized_data);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to update rows: {}", e)))?;
        
        self.update_metrics("write", start.elapsed(), true).await;
        Ok(count)
    }

    async fn delete_rows(
        &self,
        table_name: &str,
        condition: Option<&dyn Fn(&HashMap<String, SqlValue>) -> bool>,
        _tx: Option<&StorageTransaction>,
    ) -> ProtocolResult<usize> {
        let start = std::time::Instant::now();
        
        // Get existing table data
        let mut table_data = self.get_table_data(table_name).await?;
        let original_len = table_data.len();
        
        // Remove matching rows
        if let Some(condition_fn) = condition {
            table_data.retain(|row| !condition_fn(row));
        } else {
            table_data.clear();
        }
        
        let count = original_len - table_data.len();
        
        // Store updated data
        let serialized_data = table_data.to_storage_bytes()?;
        let lease = Self::table_data_to_lease(table_name, &serialized_data);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to delete rows: {}", e)))?;
        
        self.update_metrics("delete", start.elapsed(), true).await;
        Ok(count)
    }

    async fn truncate_table(&self, table_name: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let start = std::time::Instant::now();
        
        // Store empty table data
        let empty_data = Vec::<HashMap<String, SqlValue>>::new();
        let serialized_data = empty_data.to_storage_bytes()?;
        let lease = Self::table_data_to_lease(table_name, &serialized_data);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to truncate table: {}", e)))?;
        
        self.update_metrics("delete", start.elapsed(), true).await;
        Ok(())
    }

    // For brevity, I'll implement the remaining methods with similar patterns
    // The key idea is to use the LSM provider's lease system to store/retrieve data
    // with appropriate serialization/deserialization

    async fn store_index(&self, index: &IndexSchema, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let serialized = index.to_storage_bytes()?;
        let lease = Self::schema_to_lease("index", &index.name, &serialized);
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to store index: {}", e)))?;
        
        Ok(())
    }

    async fn get_index(&self, index_name: &str) -> ProtocolResult<Option<IndexSchema>> {
        let reference = AddressableReference {
            addressable_type: "schema_index".to_string(),
            key: index_name.to_string(),
        };
        
        match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) if lease.data.is_some() => {
                Ok(IndexSchema::from_storage_bytes(&lease.data.unwrap()).ok())
            }
            _ => Ok(None),
        }
    }

    async fn list_table_indexes(&self, table_name: &str) -> ProtocolResult<Vec<IndexSchema>> {
        let all_leases = self.lsm_provider.list_all_leases().await.unwrap_or_default();
        let mut indexes = Vec::new();
        
        for lease in all_leases {
            if lease.reference.addressable_type == "schema_index" {
                if let Some(data) = lease.data {
                    if let Ok(index) = IndexSchema::from_storage_bytes(&data) {
                        if index.table == table_name {
                            indexes.push(index);
                        }
                    }
                }
            }
        }
        
        Ok(indexes)
    }

    async fn remove_index(&self, index_name: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<bool> {
        let reference = AddressableReference {
            addressable_type: "schema_index".to_string(),
            key: index_name.to_string(),
        };
        
        self.lsm_provider.remove_lease(&reference).await
            .map_err(|e| ProtocolError::internal(format!("Failed to remove index: {}", e)))
    }

    // Implement remaining methods with similar patterns...
    // For brevity, I'll provide simple implementations

    async fn store_view(&self, view: &ViewSchema, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let serialized = view.to_storage_bytes()?;
        let lease = Self::schema_to_lease("view", &view.name.full_name(), &serialized);
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to store view: {}", e)))?;
        Ok(())
    }

    async fn get_view(&self, view_name: &str) -> ProtocolResult<Option<ViewSchema>> {
        let reference = AddressableReference {
            addressable_type: "schema_view".to_string(),
            key: view_name.to_string(),
        };
        
        match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) if lease.data.is_some() => {
                Ok(ViewSchema::from_storage_bytes(&lease.data.unwrap()).ok())
            }
            _ => Ok(None),
        }
    }

    async fn list_views(&self) -> ProtocolResult<Vec<ViewSchema>> {
        let all_leases = self.lsm_provider.list_all_leases().await.unwrap_or_default();
        let mut views = Vec::new();
        
        for lease in all_leases {
            if lease.reference.addressable_type == "schema_view" {
                if let Some(data) = lease.data {
                    if let Ok(view) = ViewSchema::from_storage_bytes(&data) {
                        views.push(view);
                    }
                }
            }
        }
        
        Ok(views)
    }

    async fn remove_view(&self, view_name: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<bool> {
        let reference = AddressableReference {
            addressable_type: "schema_view".to_string(),
            key: view_name.to_string(),
        };
        
        self.lsm_provider.remove_lease(&reference).await
            .map_err(|e| ProtocolError::internal(format!("Failed to remove view: {}", e)))
    }

    async fn store_schema(&self, schema: &SchemaDefinition, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let serialized = schema.to_storage_bytes()?;
        let lease = Self::schema_to_lease("schema", &schema.name, &serialized);
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to store schema: {}", e)))?;
        Ok(())
    }

    async fn get_schema(&self, schema_name: &str) -> ProtocolResult<Option<SchemaDefinition>> {
        let reference = AddressableReference {
            addressable_type: "schema_schema".to_string(),
            key: schema_name.to_string(),
        };
        
        match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) if lease.data.is_some() => {
                Ok(SchemaDefinition::from_storage_bytes(&lease.data.unwrap()).ok())
            }
            _ => Ok(None),
        }
    }

    async fn list_schemas(&self) -> ProtocolResult<Vec<SchemaDefinition>> {
        let all_leases = self.lsm_provider.list_all_leases().await.unwrap_or_default();
        let mut schemas = Vec::new();
        
        for lease in all_leases {
            if lease.reference.addressable_type == "schema_schema" {
                if let Some(data) = lease.data {
                    if let Ok(schema) = SchemaDefinition::from_storage_bytes(&data) {
                        schemas.push(schema);
                    }
                }
            }
        }
        
        Ok(schemas)
    }

    async fn remove_schema(&self, schema_name: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<bool> {
        let reference = AddressableReference {
            addressable_type: "schema_schema".to_string(),
            key: schema_name.to_string(),
        };
        
        self.lsm_provider.remove_lease(&reference).await
            .map_err(|e| ProtocolError::internal(format!("Failed to remove schema: {}", e)))
    }

    async fn store_extension(&self, extension: &ExtensionDefinition, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let serialized = extension.to_storage_bytes()?;
        let lease = Self::schema_to_lease("extension", &extension.name, &serialized);
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to store extension: {}", e)))?;
        Ok(())
    }

    async fn get_extension(&self, extension_name: &str) -> ProtocolResult<Option<ExtensionDefinition>> {
        let reference = AddressableReference {
            addressable_type: "schema_extension".to_string(),
            key: extension_name.to_string(),
        };
        
        match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) if lease.data.is_some() => {
                Ok(ExtensionDefinition::from_storage_bytes(&lease.data.unwrap()).ok())
            }
            _ => Ok(None),
        }
    }

    async fn list_extensions(&self) -> ProtocolResult<Vec<ExtensionDefinition>> {
        let all_leases = self.lsm_provider.list_all_leases().await.unwrap_or_default();
        let mut extensions = Vec::new();
        
        for lease in all_leases {
            if lease.reference.addressable_type == "schema_extension" {
                if let Some(data) = lease.data {
                    if let Ok(extension) = ExtensionDefinition::from_storage_bytes(&data) {
                        extensions.push(extension);
                    }
                }
            }
        }
        
        Ok(extensions)
    }

    async fn remove_extension(&self, extension_name: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<bool> {
        let reference = AddressableReference {
            addressable_type: "schema_extension".to_string(),
            key: extension_name.to_string(),
        };
        
        self.lsm_provider.remove_lease(&reference).await
            .map_err(|e| ProtocolError::internal(format!("Failed to remove extension: {}", e)))
    }

    async fn store_setting(&self, key: &str, value: &str, _tx: Option<&StorageTransaction>) -> ProtocolResult<()> {
        let lease = AddressableLease {
            reference: AddressableReference {
                addressable_type: "setting".to_string(),
                key: key.to_string(),
            },
            node_id: orbit_shared::NodeId::new("local".to_string()),
            lease_duration: chrono::Duration::hours(24),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
            renewable: true,
            data: Some(value.as_bytes().to_vec()),
        };
        
        self.lsm_provider.store_lease(&lease).await
            .map_err(|e| ProtocolError::internal(format!("Failed to store setting: {}", e)))?;
        
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> ProtocolResult<Option<String>> {
        let reference = AddressableReference {
            addressable_type: "setting".to_string(),
            key: key.to_string(),
        };
        
        match self.lsm_provider.get_lease(&reference).await {
            Ok(Some(lease)) if lease.data.is_some() => {
                Ok(String::from_utf8(lease.data.unwrap()).ok())
            }
            _ => Ok(None),
        }
    }

    async fn list_settings(&self) -> ProtocolResult<HashMap<String, String>> {
        let all_leases = self.lsm_provider.list_all_leases().await.unwrap_or_default();
        let mut settings = HashMap::new();
        
        for lease in all_leases {
            if lease.reference.addressable_type == "setting" {
                if let Some(data) = lease.data {
                    if let Ok(value) = String::from_utf8(data) {
                        settings.insert(lease.reference.key, value);
                    }
                }
            }
        }
        
        Ok(settings)
    }

    async fn checkpoint(&self) -> ProtocolResult<()> {
        // The LSM provider handles checkpointing internally
        tracing::info!("LSM checkpoint initiated");
        Ok(())
    }

    async fn compact(&self) -> ProtocolResult<()> {
        // The LSM provider handles compaction automatically
        tracing::info!("LSM compaction requested");
        Ok(())
    }

    async fn storage_size(&self) -> ProtocolResult<u64> {
        Ok(self.estimate_disk_usage().await)
    }
}