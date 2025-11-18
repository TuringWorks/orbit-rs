//! Event Sourcing implementation for Orbit
//!
//! This module provides event sourcing patterns where state is derived from a
//! sequence of immutable events rather than stored directly. Supports event
//! store, snapshots, and event replay.

use crate::cdc::CdcEvent;
use crate::exception::OrbitResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Domain event in event sourcing system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Unique event ID
    pub event_id: Uuid,
    /// Aggregate ID this event belongs to
    pub aggregate_id: String,
    /// Aggregate type
    pub aggregate_type: String,
    /// Event type
    pub event_type: String,
    /// Event version (for schema evolution)
    pub version: u32,
    /// Event timestamp
    pub timestamp: i64,
    /// Event data/payload
    pub data: serde_json::Value,
    /// Event metadata
    pub metadata: HashMap<String, String>,
    /// Sequence number within the aggregate
    pub sequence: u64,
}

impl DomainEvent {
    /// Create a new domain event
    pub fn new(
        aggregate_id: String,
        aggregate_type: String,
        event_type: String,
        data: serde_json::Value,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            aggregate_id,
            aggregate_type,
            event_type,
            version: 1,
            timestamp: chrono::Utc::now().timestamp_millis(),
            data,
            metadata: HashMap::new(),
            sequence: 0, // Will be set by event store
        }
    }

    /// Add metadata to event
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set event version
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }
}

/// Snapshot of aggregate state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Aggregate ID
    pub aggregate_id: String,
    /// Aggregate type
    pub aggregate_type: String,
    /// Snapshot timestamp
    pub timestamp: i64,
    /// Last event sequence number included in snapshot
    pub last_sequence: u64,
    /// Aggregate state
    pub state: serde_json::Value,
}

/// Event store configuration
#[derive(Debug, Clone)]
pub struct EventStoreConfig {
    /// Enable snapshotting
    pub enable_snapshots: bool,
    /// Snapshot interval (create snapshot every N events)
    pub snapshot_interval: u64,
    /// Maximum events to keep in memory per aggregate
    pub max_events_in_memory: usize,
    /// Enable event archiving
    pub enable_archiving: bool,
}

impl Default for EventStoreConfig {
    fn default() -> Self {
        Self {
            enable_snapshots: true,
            snapshot_interval: 100,
            max_events_in_memory: 1000,
            enable_archiving: true,
        }
    }
}

/// Event store for persisting and retrieving domain events
pub struct EventStore {
    /// Configuration
    config: EventStoreConfig,
    /// Events by aggregate ID
    events: Arc<RwLock<HashMap<String, Vec<DomainEvent>>>>,
    /// Snapshots by aggregate ID
    snapshots: Arc<RwLock<HashMap<String, Snapshot>>>,
    /// Event sequence counters by aggregate ID
    sequences: Arc<RwLock<HashMap<String, u64>>>,
    /// Statistics
    stats: Arc<RwLock<EventStoreStats>>,
}

impl EventStore {
    /// Create a new event store
    pub fn new(config: EventStoreConfig) -> Self {
        Self {
            config,
            events: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            sequences: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(EventStoreStats::default())),
        }
    }

    /// Append event to store
    pub async fn append_event(&self, mut event: DomainEvent) -> OrbitResult<u64> {
        let aggregate_id = event.aggregate_id.clone();

        // Assign sequence number
        let sequence = {
            let mut sequences = self.sequences.write().await;
            let seq = sequences.entry(aggregate_id.clone()).or_insert(0);
            *seq += 1;
            *seq
        };
        event.sequence = sequence;

        // Store event
        {
            let mut events = self.events.write().await;
            events
                .entry(aggregate_id.clone())
                .or_insert_with(Vec::new)
                .push(event.clone());
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_events += 1;
            *stats
                .events_by_type
                .entry(event.event_type.clone())
                .or_insert(0) += 1;
        }

        info!(
            "Event {} appended to aggregate {} at sequence {}",
            event.event_id, aggregate_id, sequence
        );

        // Check if we should create a snapshot
        if self.config.enable_snapshots && sequence % self.config.snapshot_interval == 0 {
            debug!("Snapshot interval reached for aggregate {}", aggregate_id);
            // In a real implementation, we would trigger snapshot creation here
        }

        Ok(sequence)
    }

    /// Get events for an aggregate
    pub async fn get_events(
        &self,
        aggregate_id: &str,
        from_sequence: Option<u64>,
    ) -> OrbitResult<Vec<DomainEvent>> {
        let events = self.events.read().await;

        if let Some(aggregate_events) = events.get(aggregate_id) {
            let filtered = if let Some(from_seq) = from_sequence {
                aggregate_events
                    .iter()
                    .filter(|e| e.sequence >= from_seq)
                    .cloned()
                    .collect()
            } else {
                aggregate_events.clone()
            };
            Ok(filtered)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get events for an aggregate after a specific timestamp
    pub async fn get_events_after(
        &self,
        aggregate_id: &str,
        after_timestamp: i64,
    ) -> OrbitResult<Vec<DomainEvent>> {
        let events = self.events.read().await;

        if let Some(aggregate_events) = events.get(aggregate_id) {
            let filtered = aggregate_events
                .iter()
                .filter(|e| e.timestamp > after_timestamp)
                .cloned()
                .collect();
            Ok(filtered)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get all events for an aggregate type
    pub async fn get_events_by_type(&self, aggregate_type: &str) -> OrbitResult<Vec<DomainEvent>> {
        let events = self.events.read().await;

        let mut result = Vec::new();
        for aggregate_events in events.values() {
            result.extend(
                aggregate_events
                    .iter()
                    .filter(|e| e.aggregate_type == aggregate_type)
                    .cloned(),
            );
        }

        // Sort by timestamp
        result.sort_by_key(|e| e.timestamp);
        Ok(result)
    }

    /// Save a snapshot
    pub async fn save_snapshot(&self, snapshot: Snapshot) -> OrbitResult<()> {
        let aggregate_id = snapshot.aggregate_id.clone();

        self.snapshots
            .write()
            .await
            .insert(aggregate_id.clone(), snapshot);

        info!("Snapshot saved for aggregate {}", aggregate_id);

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_snapshots += 1;
        }

        Ok(())
    }

    /// Get latest snapshot for an aggregate
    pub async fn get_snapshot(&self, aggregate_id: &str) -> Option<Snapshot> {
        self.snapshots.read().await.get(aggregate_id).cloned()
    }

    /// Rebuild aggregate state from events
    pub async fn rebuild_state<T, F>(
        &self,
        aggregate_id: &str,
        initial_state: T,
        apply_event: F,
    ) -> OrbitResult<T>
    where
        T: Clone,
        F: Fn(T, &DomainEvent) -> OrbitResult<T>,
    {
        let mut state = initial_state;

        // Check for snapshot first
        if let Some(snapshot) = self.get_snapshot(aggregate_id).await {
            // In a real implementation, we would deserialize state from snapshot
            // For now, we'll start from initial state
            debug!("Using snapshot at sequence {}", snapshot.last_sequence);

            // Get events after snapshot
            let events = self
                .get_events(aggregate_id, Some(snapshot.last_sequence + 1))
                .await?;

            for event in events {
                state = apply_event(state, &event)?;
            }
        } else {
            // No snapshot, replay all events
            let events = self.get_events(aggregate_id, None).await?;

            for event in events {
                state = apply_event(state, &event)?;
            }
        }

        Ok(state)
    }

    /// Get event store statistics
    pub async fn get_stats(&self) -> EventStoreStats {
        self.stats.read().await.clone()
    }

    /// Convert CDC event to domain event
    pub fn cdc_to_domain_event(&self, cdc_event: &CdcEvent) -> DomainEvent {
        DomainEvent {
            event_id: cdc_event.event_id,
            aggregate_id: format!("{}:{}", cdc_event.table, cdc_event.row_id),
            aggregate_type: cdc_event.table.clone(),
            event_type: format!("{:?}", cdc_event.operation),
            version: 1,
            timestamp: cdc_event.timestamp,
            data: serde_json::json!({
                "new_values": cdc_event.new_values,
                "old_values": cdc_event.old_values,
                "transaction_id": cdc_event.transaction_id,
            }),
            metadata: cdc_event.metadata.clone(),
            sequence: cdc_event.lsn,
        }
    }
}

/// Event store statistics
#[derive(Debug, Clone, Default)]
pub struct EventStoreStats {
    pub total_events: u64,
    pub total_snapshots: u64,
    pub events_by_type: HashMap<String, u64>,
}

/// Event sourced aggregate trait
pub trait EventSourcedAggregate: Sized {
    /// Aggregate ID
    fn aggregate_id(&self) -> String;

    /// Apply an event to this aggregate
    fn apply_event(&mut self, event: &DomainEvent) -> OrbitResult<()>;

    /// Get uncommitted events
    fn uncommitted_events(&self) -> &[DomainEvent];

    /// Clear uncommitted events
    fn clear_uncommitted_events(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_store_creation() {
        let config = EventStoreConfig::default();
        let store = EventStore::new(config);

        let stats = store.get_stats().await;
        assert_eq!(stats.total_events, 0);
        assert_eq!(stats.total_snapshots, 0);
    }

    #[tokio::test]
    async fn test_append_and_get_events() {
        let store = EventStore::new(EventStoreConfig::default());

        let event1 = DomainEvent::new(
            "user-1".to_string(),
            "User".to_string(),
            "UserCreated".to_string(),
            serde_json::json!({"name": "John"}),
        );

        let event2 = DomainEvent::new(
            "user-1".to_string(),
            "User".to_string(),
            "UserUpdated".to_string(),
            serde_json::json!({"name": "John Doe"}),
        );

        let seq1 = store.append_event(event1).await.unwrap();
        let seq2 = store.append_event(event2).await.unwrap();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);

        let events = store.get_events("user-1", None).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[1].sequence, 2);
    }

    #[tokio::test]
    async fn test_get_events_from_sequence() {
        let store = EventStore::new(EventStoreConfig::default());

        for i in 1..=5 {
            let event = DomainEvent::new(
                "user-1".to_string(),
                "User".to_string(),
                format!("Event{}", i),
                serde_json::json!({"value": i}),
            );
            store.append_event(event).await.unwrap();
        }

        // Get events from sequence 3
        let events = store.get_events("user-1", Some(3)).await.unwrap();
        assert_eq!(events.len(), 3); // sequences 3, 4, 5
        assert_eq!(events[0].sequence, 3);
    }

    #[tokio::test]
    async fn test_snapshot_operations() {
        let store = EventStore::new(EventStoreConfig::default());

        let snapshot = Snapshot {
            aggregate_id: "user-1".to_string(),
            aggregate_type: "User".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            last_sequence: 10,
            state: serde_json::json!({"name": "John", "version": 10}),
        };

        store.save_snapshot(snapshot.clone()).await.unwrap();

        let retrieved = store.get_snapshot("user-1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().last_sequence, 10);

        let stats = store.get_stats().await;
        assert_eq!(stats.total_snapshots, 1);
    }

    #[tokio::test]
    async fn test_rebuild_state() {
        let store = EventStore::new(EventStoreConfig::default());

        // Create events that modify a counter
        for i in 1..=5 {
            let event = DomainEvent::new(
                "counter-1".to_string(),
                "Counter".to_string(),
                "Incremented".to_string(),
                serde_json::json!({"amount": i}),
            );
            store.append_event(event).await.unwrap();
        }

        // Rebuild state by summing increments
        let final_state = store
            .rebuild_state("counter-1", 0i64, |state, event| {
                let amount = event.data["amount"].as_i64().unwrap_or(0);
                Ok(state + amount)
            })
            .await
            .unwrap();

        assert_eq!(final_state, 15); // 1+2+3+4+5
    }

    #[test]
    fn test_domain_event_creation() {
        let event = DomainEvent::new(
            "user-1".to_string(),
            "User".to_string(),
            "UserCreated".to_string(),
            serde_json::json!({"name": "John"}),
        );

        assert_eq!(event.aggregate_id, "user-1");
        assert_eq!(event.aggregate_type, "User");
        assert_eq!(event.event_type, "UserCreated");
        assert_eq!(event.version, 1);
    }

    #[tokio::test]
    async fn test_get_events_by_type() {
        let store = EventStore::new(EventStoreConfig::default());

        // Add events for different aggregate types
        for i in 1..=3 {
            let user_event = DomainEvent::new(
                format!("user-{}", i),
                "User".to_string(),
                "UserCreated".to_string(),
                serde_json::json!({"id": i}),
            );
            store.append_event(user_event).await.unwrap();

            let order_event = DomainEvent::new(
                format!("order-{}", i),
                "Order".to_string(),
                "OrderCreated".to_string(),
                serde_json::json!({"id": i}),
            );
            store.append_event(order_event).await.unwrap();
        }

        let user_events = store.get_events_by_type("User").await.unwrap();
        assert_eq!(user_events.len(), 3);

        let order_events = store.get_events_by_type("Order").await.unwrap();
        assert_eq!(order_events.len(), 3);
    }
}
