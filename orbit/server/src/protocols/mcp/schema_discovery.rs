//! Real-time Schema Discovery for MCP Server
//!
//! This module provides real-time schema discovery and updates
//! by connecting to Orbit-RS metadata system.

#![cfg(feature = "storage-rocksdb")]

use crate::protocols::mcp::integration::OrbitMcpIntegration;
use crate::protocols::mcp::schema::{SchemaAnalyzer, TableSchema};
use crate::protocols::mcp::types::McpError;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Schema Discovery Manager for real-time updates
pub struct SchemaDiscoveryManager {
    /// Schema analyzer
    schema_analyzer: Arc<SchemaAnalyzer>,
    /// Orbit integration for schema queries
    integration: Arc<OrbitMcpIntegration>,
    /// Discovery interval in seconds
    discovery_interval: Duration,
    /// Whether discovery is running
    is_running: Arc<RwLock<bool>>,
}

impl SchemaDiscoveryManager {
    /// Create a new schema discovery manager
    pub fn new(
        schema_analyzer: Arc<SchemaAnalyzer>,
        integration: Arc<OrbitMcpIntegration>,
        discovery_interval_secs: u64,
    ) -> Self {
        Self {
            schema_analyzer,
            integration,
            discovery_interval: Duration::from_secs(discovery_interval_secs),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start background schema discovery
    pub async fn start_discovery(&self) -> Result<(), McpError> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(McpError::InternalError(
                "Schema discovery is already running".to_string(),
            ));
        }
        *is_running = true;
        drop(is_running);

        let schema_analyzer = self.schema_analyzer.clone();
        let integration = self.integration.clone();
        let interval_duration = self.discovery_interval;
        let is_running_flag = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                // Check if we should stop
                {
                    let running = is_running_flag.read().await;
                    if !*running {
                        break;
                    }
                }

                // Discover and update schemas
                if let Err(e) =
                    Self::discover_and_update_schemas(&schema_analyzer, &integration).await
                {
                    tracing::warn!("Schema discovery error: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Stop background schema discovery
    pub async fn stop_discovery(&self) {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
    }

    /// Discover and update all table schemas
    async fn discover_and_update_schemas(
        schema_analyzer: &SchemaAnalyzer,
        integration: &OrbitMcpIntegration,
    ) -> Result<(), McpError> {
        // Get list of tables
        let tables = integration.list_tables().await?;

        tracing::debug!("Discovering schemas for {} tables", tables.len());

        // Discover schema for each table
        for table_name in tables {
            match integration.get_table_schema(&table_name).await {
                Ok(Some(schema)) => {
                    schema_analyzer.update_schema(schema);
                    tracing::debug!("Updated schema for table: {}", table_name);
                }
                Ok(None) => {
                    tracing::debug!("Table {} has no schema", table_name);
                }
                Err(e) => {
                    tracing::warn!("Failed to get schema for table {}: {}", table_name, e);
                }
            }
        }

        Ok(())
    }

    /// Trigger immediate schema refresh
    pub async fn refresh_now(&self) -> Result<(), McpError> {
        Self::discover_and_update_schemas(&self.schema_analyzer, &self.integration).await
    }

    /// Get current schema cache statistics
    pub async fn get_cache_stats(&self) -> SchemaCacheStats {
        // Access the internal cache to get statistics
        // This would require exposing cache stats from SchemaAnalyzer
        SchemaCacheStats {
            table_count: 0, // Would be populated from actual cache
            last_update: std::time::SystemTime::now(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }
}

/// Schema cache statistics
#[derive(Debug, Clone)]
pub struct SchemaCacheStats {
    /// Number of tables in cache
    pub table_count: usize,
    /// Last update time
    pub last_update: std::time::SystemTime,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
}

/// Schema change notification
#[derive(Debug, Clone)]
pub enum SchemaChange {
    /// Table was created
    TableCreated {
        table_name: String,
        schema: TableSchema,
    },
    /// Table was dropped
    TableDropped { table_name: String },
    /// Table schema was modified
    TableModified {
        table_name: String,
        schema: TableSchema,
    },
    /// Column was added
    ColumnAdded {
        table_name: String,
        column: crate::protocols::mcp::schema::ColumnInfo,
    },
    /// Column was removed
    ColumnRemoved {
        table_name: String,
        column_name: String,
    },
}

/// Schema change listener trait
pub trait SchemaChangeListener: Send + Sync {
    /// Called when a schema change is detected
    fn on_schema_change(&self, change: SchemaChange);
}

/// Schema change notifier
pub struct SchemaChangeNotifier {
    listeners: Arc<RwLock<Vec<Arc<dyn SchemaChangeListener>>>>,
}

impl SchemaChangeNotifier {
    /// Create a new schema change notifier
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a schema change listener
    pub async fn register_listener(&self, listener: Arc<dyn SchemaChangeListener>) {
        let mut listeners = self.listeners.write().await;
        listeners.push(listener);
    }

    /// Notify all listeners of a schema change
    pub async fn notify_change(&self, change: SchemaChange) {
        let listeners = self.listeners.read().await;
        for listener in listeners.iter() {
            listener.on_schema_change(change.clone());
        }
    }
}

impl Default for SchemaChangeNotifier {
    fn default() -> Self {
        Self::new()
    }
}
