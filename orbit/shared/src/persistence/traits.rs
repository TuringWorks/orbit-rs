//! Trait-based abstractions for persistence providers
//!
//! This module provides common traits with default implementations to reduce
//! redundancy across persistence provider implementations.

use crate::error::{OrbitError, OrbitResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ===== Metrics Collection Trait =====

/// Trait for providers that collect operational metrics
#[async_trait]
pub trait MetricsCollector {
    /// Get metrics type
    type Metrics: Clone + Default + Send + Sync;

    /// Get reference to metrics storage
    fn metrics_storage(&self) -> &Arc<RwLock<Self::Metrics>>;

    /// Get current metrics snapshot
    async fn get_metrics(&self) -> Self::Metrics {
        self.metrics_storage().read().await.clone()
    }
}

/// Standard persistence metrics that can be extended
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistenceMetrics {
    pub read_operations: u64,
    pub write_operations: u64,
    pub delete_operations: u64,
    pub error_count: u64,
    pub read_latency_avg: f64,
    pub write_latency_avg: f64,
    pub delete_latency_avg: f64,
}

impl PersistenceMetrics {
    /// Update metrics for a read operation
    pub fn record_read(&mut self, duration: Duration, success: bool) {
        self.read_operations += 1;
        self.read_latency_avg = (self.read_latency_avg * (self.read_operations - 1) as f64
            + duration.as_secs_f64())
            / self.read_operations as f64;
        if !success {
            self.error_count += 1;
        }
    }

    /// Update metrics for a write operation
    pub fn record_write(&mut self, duration: Duration, success: bool) {
        self.write_operations += 1;
        self.write_latency_avg = (self.write_latency_avg * (self.write_operations - 1) as f64
            + duration.as_secs_f64())
            / self.write_operations as f64;
        if !success {
            self.error_count += 1;
        }
    }

    /// Update metrics for a delete operation
    pub fn record_delete(&mut self, duration: Duration, success: bool) {
        self.delete_operations += 1;
        self.delete_latency_avg = (self.delete_latency_avg * (self.delete_operations - 1) as f64
            + duration.as_secs_f64())
            / self.delete_operations as f64;
        if !success {
            self.error_count += 1;
        }
    }

    /// Generic operation recorder
    pub fn record_operation(&mut self, operation: &str, duration: Duration, success: bool) {
        match operation {
            "read" => self.record_read(duration, success),
            "write" => self.record_write(duration, success),
            "delete" => self.record_delete(duration, success),
            _ => {
                if !success {
                    self.error_count += 1;
                }
            }
        }
    }
}

/// Extension trait for automatic metrics recording
#[async_trait]
pub trait AutoMetrics: MetricsCollector
where
    Self::Metrics: From<PersistenceMetrics> + Into<PersistenceMetrics>,
{
    /// Execute operation with automatic metrics collection
    async fn with_metrics<F, T>(&self, operation: &str, f: F) -> OrbitResult<T>
    where
        F: std::future::Future<Output = OrbitResult<T>> + Send,
        T: Send,
    {
        let start = Instant::now();
        let result = f.await;
        let duration = start.elapsed();
        let success = result.is_ok();

        let mut metrics = self.metrics_storage().write().await;
        let mut base_metrics: PersistenceMetrics = (*metrics).clone().into();
        base_metrics.record_operation(operation, duration, success);
        *metrics = base_metrics.into();

        result
    }
}

// Blanket implementation for all types that implement MetricsCollector
impl<T> AutoMetrics for T
where
    T: MetricsCollector,
    T::Metrics: From<PersistenceMetrics> + Into<PersistenceMetrics>,
{
}

// ===== Transaction Management Trait =====

/// Transaction context identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub String);

impl TransactionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_context(ctx: &TransactionContext) -> Self {
        Self(ctx.transaction_id.clone())
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Transaction context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionContext {
    pub transaction_id: String,
    pub isolation_level: IsolationLevel,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum IsolationLevel {
    ReadUncommitted,
    #[default]
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

/// Trait for transaction management with default implementations
#[async_trait]
pub trait TransactionManager {
    /// Storage for active transactions
    type TransactionStorage: Send + Sync;

    /// Get reference to transaction storage
    fn transaction_storage(&self) -> &Self::TransactionStorage;

    /// Begin a new transaction (default implementation)
    async fn begin_transaction(&self, context: TransactionContext) -> OrbitResult<String> {
        // Default: just return the transaction ID
        // Override in providers that need actual transaction tracking
        tracing::debug!(
            tx_id = %context.transaction_id,
            "Beginning transaction (default implementation)"
        );
        Ok(context.transaction_id)
    }

    /// Commit a transaction (default implementation)
    async fn commit_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        // Default: no-op with logging
        // Override in providers that need actual commit logic
        tracing::debug!(
            tx_id = %transaction_id,
            "Committing transaction (default implementation)"
        );
        Ok(())
    }

    /// Rollback a transaction (default implementation)
    async fn rollback_transaction(&self, transaction_id: &str) -> OrbitResult<()> {
        // Default: no-op with warning
        // Override in providers that need actual rollback logic
        tracing::warn!(
            tx_id = %transaction_id,
            "Rolling back transaction (default implementation)"
        );
        Ok(())
    }
}

// ===== Key Encoding Utilities =====

use crate::addressable::Key;

/// Trait for encoding keys to strings
pub trait KeyEncoder {
    /// Encode a key to a string representation
    fn encode_key(&self, key: &Key) -> String {
        match key {
            Key::StringKey { key } => key.clone(),
            Key::Int32Key { key } => key.to_string(),
            Key::Int64Key { key } => key.to_string(),
            Key::NoKey => "no-key".to_string(),
        }
    }

    /// Encode key for URL/path usage
    fn encode_key_url_safe(&self, key: &Key) -> String {
        match key {
            Key::StringKey { key } => {
                // Simple URL encoding without external dependency
                key.chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                            c.to_string()
                        } else {
                            format!("%{:02X}", c as u8)
                        }
                    })
                    .collect()
            }
            Key::Int32Key { key } => key.to_string(),
            Key::Int64Key { key } => key.to_string(),
            Key::NoKey => "no-key".to_string(),
        }
    }

    /// Create a composite key from addressable reference
    fn composite_key(&self, addressable_type: &str, key: &Key) -> String {
        format!("{}:{}", addressable_type, self.encode_key(key))
    }

    /// Create a prefixed composite key
    fn prefixed_composite_key(&self, prefix: &str, addressable_type: &str, key: &Key) -> String {
        format!("{}:{}:{}", prefix, addressable_type, self.encode_key(key))
    }
}

// ===== Lifecycle Management =====

/// Provider health status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Trait for provider lifecycle management with default implementations
#[async_trait]
pub trait ProviderLifecycle {
    /// Initialize the provider
    async fn initialize(&self) -> OrbitResult<()> {
        tracing::info!("Initializing provider (default implementation)");
        Ok(())
    }

    /// Shutdown the provider
    async fn shutdown(&self) -> OrbitResult<()> {
        tracing::info!("Shutting down provider (default implementation)");
        Ok(())
    }

    /// Check provider health
    async fn health_check(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }
}

// ===== Bulk Operations Trait =====

/// Trait for bulk operations with default sequential implementations
#[async_trait]
pub trait BulkOperations<T>
where
    T: Send + Sync,
{
    /// Store a single item
    async fn store_item(&self, item: &T) -> OrbitResult<()>;

    /// Delete a single item
    async fn delete_item(&self, id: &str) -> OrbitResult<()>;

    /// Store multiple items (default: sequential)
    /// Override in providers that support batch operations
    async fn store_items_bulk(&self, items: &[T]) -> OrbitResult<()> {
        for item in items {
            self.store_item(item).await?;
        }
        Ok(())
    }

    /// Delete multiple items (default: sequential)
    /// Override in providers that support batch operations
    async fn delete_items_bulk(&self, ids: &[String]) -> OrbitResult<()> {
        for id in ids {
            self.delete_item(id).await?;
        }
        Ok(())
    }
}

// ===== Data Directory Management =====

use std::path::PathBuf;

/// Trait for providers that use data directories
#[async_trait]
pub trait DataDirectoryProvider {
    /// Get the data directory path
    fn data_dir(&self) -> &PathBuf;

    /// Ensure data directory exists
    async fn ensure_data_dir(&self) -> OrbitResult<()> {
        let data_dir = self.data_dir();
        tokio::fs::create_dir_all(data_dir).await.map_err(|e| {
            OrbitError::io_with_source(
                format!("Failed to create data directory: {}", data_dir.display()),
                e.to_string(),
            )
        })?;
        tracing::info!("Data directory ensured at: {}", data_dir.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistence_metrics() {
        let mut metrics = PersistenceMetrics::default();

        metrics.record_write(Duration::from_millis(10), true);
        assert_eq!(metrics.write_operations, 1);
        assert_eq!(metrics.error_count, 0);

        metrics.record_write(Duration::from_millis(20), false);
        assert_eq!(metrics.write_operations, 2);
        assert_eq!(metrics.error_count, 1);
    }

    #[test]
    fn test_transaction_id() {
        let id1 = TransactionId::new();
        let id2 = TransactionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_key_encoding() {
        struct TestEncoder;
        impl KeyEncoder for TestEncoder {}

        let encoder = TestEncoder;

        let string_key = Key::StringKey {
            key: "test".to_string(),
        };
        assert_eq!(encoder.encode_key(&string_key), "test");

        let int_key = Key::Int32Key { key: 42 };
        assert_eq!(encoder.encode_key(&int_key), "42");

        assert_eq!(encoder.composite_key("Actor", &string_key), "Actor:test");
    }
}
