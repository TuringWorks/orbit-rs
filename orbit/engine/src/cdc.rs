//! Change Data Capture (CDC) module for tracking all data modifications
//!
//! This module provides comprehensive CDC capabilities for capturing and
//! streaming all DML operations (INSERT, UPDATE, DELETE) with support for
//! multiple consumers, filtering, and event sourcing patterns.

use crate::error::{EngineError, EngineResult};
use crate::transaction_log::PersistentTransactionLogger;
use crate::transactions::TransactionLogEntry;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Types of DML operations captured by CDC
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum DmlOperation {
    /// INSERT operation - new data added
    Insert,
    /// UPDATE operation - existing data modified
    Update,
    /// DELETE operation - data removed
    Delete,
    /// TRUNCATE operation - table cleared
    Truncate,
}

/// CDC change event representing a single data modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcEvent {
    /// Unique event ID
    pub event_id: Uuid,
    /// Timestamp when the change occurred
    pub timestamp: i64,
    /// Type of DML operation
    pub operation: DmlOperation,
    /// Table/collection name
    pub table: String,
    /// Schema name (if applicable)
    pub schema: Option<String>,
    /// Primary key or row identifier
    pub row_id: String,
    /// New data values (for INSERT and UPDATE)
    pub new_values: Option<HashMap<String, serde_json::Value>>,
    /// Old data values (for UPDATE and DELETE)
    pub old_values: Option<HashMap<String, serde_json::Value>>,
    /// Transaction ID that caused this change
    pub transaction_id: Option<String>,
    /// Log sequence number for ordering
    pub lsn: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl CdcEvent {
    /// Create a new INSERT event
    pub fn insert(
        table: String,
        row_id: String,
        new_values: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            operation: DmlOperation::Insert,
            table,
            schema: None,
            row_id,
            new_values: Some(new_values),
            old_values: None,
            transaction_id: None,
            lsn: 0, // Will be assigned by CDC system
            metadata: HashMap::new(),
        }
    }

    /// Create a new UPDATE event
    pub fn update(
        table: String,
        row_id: String,
        old_values: HashMap<String, serde_json::Value>,
        new_values: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            operation: DmlOperation::Update,
            table,
            schema: None,
            row_id,
            new_values: Some(new_values),
            old_values: Some(old_values),
            transaction_id: None,
            lsn: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a new DELETE event
    pub fn delete(
        table: String,
        row_id: String,
        old_values: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            operation: DmlOperation::Delete,
            table,
            schema: None,
            row_id,
            new_values: None,
            old_values: Some(old_values),
            transaction_id: None,
            lsn: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set transaction ID
    pub fn with_transaction_id(mut self, transaction_id: String) -> Self {
        self.transaction_id = Some(transaction_id);
        self
    }

    /// Set schema
    pub fn with_schema(mut self, schema: String) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// CDC event filter for selective consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcFilter {
    /// Filter by tables
    pub tables: Option<Vec<String>>,
    /// Filter by schemas
    pub schemas: Option<Vec<String>>,
    /// Filter by DML operations
    pub operations: Option<Vec<DmlOperation>>,
    /// Filter by transaction IDs
    pub transaction_ids: Option<Vec<String>>,
}

impl CdcFilter {
    /// Create a filter that matches all events
    pub fn all() -> Self {
        Self {
            tables: None,
            schemas: None,
            operations: None,
            transaction_ids: None,
        }
    }

    /// Create a filter for specific tables
    pub fn tables(tables: Vec<String>) -> Self {
        Self {
            tables: Some(tables),
            schemas: None,
            operations: None,
            transaction_ids: None,
        }
    }

    /// Check if an event matches this filter
    pub fn matches(&self, event: &CdcEvent) -> bool {
        // Check table filter
        if let Some(ref tables) = self.tables {
            if !tables.contains(&event.table) {
                return false;
            }
        }

        // Check schema filter
        if let Some(ref schemas) = self.schemas {
            if let Some(ref event_schema) = event.schema {
                if !schemas.contains(event_schema) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check operation filter
        if let Some(ref operations) = self.operations {
            if !operations.contains(&event.operation) {
                return false;
            }
        }

        // Check transaction ID filter
        if let Some(ref tx_ids) = self.transaction_ids {
            if let Some(ref event_tx_id) = event.transaction_id {
                if !tx_ids.contains(event_tx_id) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

/// CDC consumer that receives change events
#[async_trait]
pub trait CdcConsumer: Send + Sync {
    /// Process a CDC event
    async fn process_event(&self, event: &CdcEvent) -> EngineResult<()>;

    /// Handle consumer errors
    async fn on_error(&self, error: EngineError) {
        error!("CDC consumer error: {}", error);
    }
}

/// CDC subscription configuration
#[derive(Debug, Clone)]
pub struct CdcSubscription {
    /// Unique subscription ID
    pub subscription_id: Uuid,
    /// Event filter
    pub filter: CdcFilter,
    /// Buffer size for the event channel
    pub buffer_size: usize,
    /// Whether to include old values in events
    pub include_old_values: bool,
    /// Whether to include new values in events
    pub include_new_values: bool,
}

impl Default for CdcSubscription {
    fn default() -> Self {
        Self {
            subscription_id: Uuid::new_v4(),
            filter: CdcFilter::all(),
            buffer_size: 1000,
            include_old_values: true,
            include_new_values: true,
        }
    }
}

/// CDC stream for consuming events
pub struct CdcStream {
    receiver: tokio::sync::mpsc::Receiver<CdcEvent>,
    subscription_id: Uuid,
}

impl CdcStream {
    /// Receive the next CDC event
    pub async fn next(&mut self) -> Option<CdcEvent> {
        self.receiver.recv().await
    }

    /// Get the subscription ID
    pub fn subscription_id(&self) -> Uuid {
        self.subscription_id
    }
}

/// Main CDC coordinator that manages change capture and distribution
pub struct CdcCoordinator {
    /// Broadcast channel for CDC events
    event_broadcast: broadcast::Sender<CdcEvent>,
    /// Transaction logger for persisting CDC events
    transaction_logger: Option<Arc<dyn PersistentTransactionLogger>>,
    /// Active subscriptions
    subscriptions: Arc<RwLock<HashMap<Uuid, CdcSubscription>>>,
    /// Current LSN counter
    lsn_counter: Arc<RwLock<u64>>,
    /// CDC statistics
    stats: Arc<RwLock<CdcStats>>,
}

impl CdcCoordinator {
    /// Create a new CDC coordinator
    pub fn new(transaction_logger: Option<Arc<dyn PersistentTransactionLogger>>) -> Self {
        let (event_broadcast, _) = broadcast::channel(10000);

        Self {
            event_broadcast,
            transaction_logger,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            lsn_counter: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(CdcStats::default())),
        }
    }

    /// Capture a CDC event
    pub async fn capture_event(&self, mut event: CdcEvent) -> EngineResult<()> {
        // Assign LSN
        {
            let mut lsn = self.lsn_counter.write().await;
            event.lsn = *lsn;
            *lsn += 1;
        }

        // Persist to transaction log if available
        if let Some(ref logger) = self.transaction_logger {
            let log_entry = TransactionLogEntry {
                timestamp: event.timestamp,
                transaction_id: crate::transactions::TransactionId::new(
                    "cdc-node".to_string(), // NodeId is just a String
                ),
                event: crate::transactions::TransactionEvent::Started,
                details: Some(serde_json::to_value(&event).map_err(|e| {
                    EngineError::internal(format!("Failed to serialize CDC event: {e}"))
                })?),
            };

            logger.write_entry(&log_entry).await?;
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events += 1;
            *stats
                .events_by_operation
                .entry(event.operation.clone())
                .or_insert(0) += 1;
            *stats
                .events_by_table
                .entry(event.table.clone())
                .or_insert(0) += 1;
        }

        // Broadcast to all subscribers
        match self.event_broadcast.send(event.clone()) {
            Ok(receiver_count) => {
                debug!(
                    "CDC event {} broadcast to {} subscribers",
                    event.event_id, receiver_count
                );
                Ok(())
            }
            Err(_) => {
                // No active subscribers
                debug!(
                    "CDC event {} captured but no active subscribers",
                    event.event_id
                );
                Ok(())
            }
        }
    }

    /// Subscribe to CDC events
    pub async fn subscribe(&self, subscription: CdcSubscription) -> EngineResult<CdcStream> {
        let subscription_id = subscription.subscription_id;
        let filter = subscription.filter.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(subscription.buffer_size);

        // Store subscription
        self.subscriptions
            .write()
            .await
            .insert(subscription_id, subscription);

        // Spawn task to filter and forward events
        let mut event_receiver = self.event_broadcast.subscribe();
        tokio::spawn(async move {
            loop {
                match event_receiver.recv().await {
                    Ok(event) => {
                        if filter.matches(&event) && tx.send(event).await.is_err() {
                            debug!("CDC subscription {} dropped", subscription_id);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        error!(
                            "CDC subscription {} lagged, skipped {} events",
                            subscription_id, skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("CDC broadcast closed for subscription {}", subscription_id);
                        break;
                    }
                }
            }
        });

        info!("Created CDC subscription {}", subscription_id);

        Ok(CdcStream {
            receiver: rx,
            subscription_id,
        })
    }

    /// Unsubscribe from CDC events
    pub async fn unsubscribe(&self, subscription_id: Uuid) -> EngineResult<()> {
        if self
            .subscriptions
            .write()
            .await
            .remove(&subscription_id)
            .is_some()
        {
            info!("Removed CDC subscription {}", subscription_id);
            Ok(())
        } else {
            Err(EngineError::AddressableNotFound(format!(
                "CDC subscription {subscription_id}"
            )))
        }
    }

    /// Get CDC statistics
    pub async fn get_stats(&self) -> CdcStats {
        self.stats.read().await.clone()
    }

    /// Get active subscriptions count
    pub async fn subscription_count(&self) -> usize {
        self.subscriptions.read().await.len()
    }
}

impl Clone for CdcCoordinator {
    fn clone(&self) -> Self {
        Self {
            event_broadcast: self.event_broadcast.clone(),
            transaction_logger: self.transaction_logger.clone(),
            subscriptions: Arc::clone(&self.subscriptions),
            lsn_counter: Arc::clone(&self.lsn_counter),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// CDC statistics
#[derive(Debug, Clone, Default)]
pub struct CdcStats {
    /// Total events captured
    pub total_events: u64,
    /// Events by operation type
    pub events_by_operation: HashMap<DmlOperation, u64>,
    /// Events by table
    pub events_by_table: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdc_event_creation() {
        let mut values = HashMap::new();
        values.insert("id".to_string(), serde_json::json!(1));
        values.insert("name".to_string(), serde_json::json!("test"));

        let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

        assert_eq!(event.operation, DmlOperation::Insert);
        assert_eq!(event.table, "users");
        assert_eq!(event.row_id, "1");
        assert!(event.new_values.is_some());
        assert!(event.old_values.is_none());
    }

    #[test]
    fn test_cdc_filter_matching() {
        let filter = CdcFilter::tables(vec!["users".to_string(), "orders".to_string()]);

        let mut values = HashMap::new();
        values.insert("id".to_string(), serde_json::json!(1));

        let event1 = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone());
        let event2 = CdcEvent::insert("products".to_string(), "1".to_string(), values);

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2));
    }

    #[tokio::test]
    async fn test_cdc_coordinator_creation() {
        let coordinator = CdcCoordinator::new(None);
        let stats = coordinator.get_stats().await;

        assert_eq!(stats.total_events, 0);
        assert_eq!(coordinator.subscription_count().await, 0);
    }

    #[tokio::test]
    async fn test_cdc_subscription() {
        let coordinator = CdcCoordinator::new(None);

        let subscription = CdcSubscription {
            filter: CdcFilter::tables(vec!["users".to_string()]),
            ..Default::default()
        };

        let _stream = coordinator.subscribe(subscription).await.unwrap();

        assert_eq!(coordinator.subscription_count().await, 1);
    }

    #[tokio::test]
    async fn test_cdc_event_capture_and_delivery() {
        let coordinator = CdcCoordinator::new(None);

        let subscription = CdcSubscription {
            filter: CdcFilter::tables(vec!["users".to_string()]),
            ..Default::default()
        };

        let mut stream = coordinator.subscribe(subscription).await.unwrap();

        // Capture an event
        let mut values = HashMap::new();
        values.insert("id".to_string(), serde_json::json!(1));
        let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

        coordinator.capture_event(event.clone()).await.unwrap();

        // Receive the event
        let received = tokio::time::timeout(tokio::time::Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received.table, "users");
        assert_eq!(received.operation, DmlOperation::Insert);
    }
}
