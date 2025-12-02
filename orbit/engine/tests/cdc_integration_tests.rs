//! Change Data Capture (CDC) Integration Tests
//!
//! Comprehensive tests for the CDC system including:
//! - CdcEvent creation (INSERT, UPDATE, DELETE, TRUNCATE)
//! - CdcFilter for selective event consumption
//! - CdcSubscription and CdcStream
//! - CdcCoordinator for event capture and broadcast
//! - CdcConsumer trait implementation

// CDC is always available in orbit-engine (part of core functionality)

use async_trait::async_trait;
use orbit_shared::{
    CdcConsumer, CdcCoordinator, CdcEvent, CdcFilter, CdcStats, CdcSubscription, DmlOperation,
    OrbitResult,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

// ============================================================================
// Mock Implementations for Testing
// ============================================================================

/// Mock CDC consumer that tracks processed events
struct MockCdcConsumer {
    processed_events: Arc<Mutex<Vec<CdcEvent>>>,
    process_count: AtomicUsize,
}

impl MockCdcConsumer {
    fn new() -> Self {
        Self {
            processed_events: Arc::new(Mutex::new(Vec::new())),
            process_count: AtomicUsize::new(0),
        }
    }

    async fn get_processed_events(&self) -> Vec<CdcEvent> {
        self.processed_events.lock().await.clone()
    }

    fn get_process_count(&self) -> usize {
        self.process_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CdcConsumer for MockCdcConsumer {
    async fn process_event(&self, event: &CdcEvent) -> OrbitResult<()> {
        self.processed_events.lock().await.push(event.clone());
        self.process_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Helper function to create test values
fn test_values(data: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    data.iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

// ============================================================================
// DmlOperation Tests
// ============================================================================

#[test]
fn test_dml_operation_insert() {
    let op = DmlOperation::Insert;
    assert_eq!(op, DmlOperation::Insert);
}

#[test]
fn test_dml_operation_update() {
    let op = DmlOperation::Update;
    assert_eq!(op, DmlOperation::Update);
}

#[test]
fn test_dml_operation_delete() {
    let op = DmlOperation::Delete;
    assert_eq!(op, DmlOperation::Delete);
}

#[test]
fn test_dml_operation_truncate() {
    let op = DmlOperation::Truncate;
    assert_eq!(op, DmlOperation::Truncate);
}

#[test]
fn test_dml_operation_serialization() {
    let op = DmlOperation::Update;
    let serialized = serde_json::to_string(&op).unwrap();
    let deserialized: DmlOperation = serde_json::from_str(&serialized).unwrap();
    assert_eq!(op, deserialized);
}

// ============================================================================
// CdcEvent Tests
// ============================================================================

#[test]
fn test_cdc_event_insert_creation() {
    let values = test_values(&[
        ("id", json!(1)),
        ("name", json!("Alice")),
        ("email", json!("alice@example.com")),
    ]);

    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone());

    assert_eq!(event.operation, DmlOperation::Insert);
    assert_eq!(event.table, "users");
    assert_eq!(event.row_id, "1");
    assert!(event.new_values.is_some());
    assert!(event.old_values.is_none());
    assert!(event.timestamp > 0);
}

#[test]
fn test_cdc_event_update_creation() {
    let old_values = test_values(&[("name", json!("Alice"))]);
    let new_values = test_values(&[("name", json!("Alice Smith"))]);

    let event = CdcEvent::update(
        "users".to_string(),
        "1".to_string(),
        old_values.clone(),
        new_values.clone(),
    );

    assert_eq!(event.operation, DmlOperation::Update);
    assert!(event.new_values.is_some());
    assert!(event.old_values.is_some());
    assert_eq!(event.old_values.as_ref().unwrap()["name"], json!("Alice"));
    assert_eq!(
        event.new_values.as_ref().unwrap()["name"],
        json!("Alice Smith")
    );
}

#[test]
fn test_cdc_event_delete_creation() {
    let old_values = test_values(&[("id", json!(1)), ("name", json!("Alice"))]);

    let event = CdcEvent::delete("users".to_string(), "1".to_string(), old_values);

    assert_eq!(event.operation, DmlOperation::Delete);
    assert!(event.new_values.is_none());
    assert!(event.old_values.is_some());
}

#[test]
fn test_cdc_event_with_transaction_id() {
    let values = test_values(&[("id", json!(1))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values)
        .with_transaction_id("tx-12345".to_string());

    assert_eq!(event.transaction_id, Some("tx-12345".to_string()));
}

#[test]
fn test_cdc_event_with_schema() {
    let values = test_values(&[("id", json!(1))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values)
        .with_schema("public".to_string());

    assert_eq!(event.schema, Some("public".to_string()));
}

#[test]
fn test_cdc_event_with_metadata() {
    let values = test_values(&[("id", json!(1))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values)
        .with_metadata("source".to_string(), "api".to_string())
        .with_metadata("user_id".to_string(), "admin".to_string());

    assert_eq!(event.metadata.get("source"), Some(&"api".to_string()));
    assert_eq!(event.metadata.get("user_id"), Some(&"admin".to_string()));
}

#[test]
fn test_cdc_event_serialization() {
    let values = test_values(&[("id", json!(1)), ("name", json!("Test"))]);
    let event = CdcEvent::insert("test_table".to_string(), "1".to_string(), values);

    let serialized = serde_json::to_string(&event).unwrap();
    let deserialized: CdcEvent = serde_json::from_str(&serialized).unwrap();

    assert_eq!(event.event_id, deserialized.event_id);
    assert_eq!(event.table, deserialized.table);
    assert_eq!(event.operation, deserialized.operation);
}

// ============================================================================
// CdcFilter Tests
// ============================================================================

#[test]
fn test_cdc_filter_all() {
    let filter = CdcFilter::all();

    assert!(filter.tables.is_none());
    assert!(filter.schemas.is_none());
    assert!(filter.operations.is_none());
    assert!(filter.transaction_ids.is_none());
}

#[test]
fn test_cdc_filter_tables() {
    let filter = CdcFilter::tables(vec!["users".to_string(), "orders".to_string()]);

    assert!(filter.tables.is_some());
    assert_eq!(filter.tables.as_ref().unwrap().len(), 2);
    assert!(filter
        .tables
        .as_ref()
        .unwrap()
        .contains(&"users".to_string()));
}

#[test]
fn test_cdc_filter_matches_all() {
    let filter = CdcFilter::all();
    let values = test_values(&[("id", json!(1))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

    assert!(filter.matches(&event));
}

#[test]
fn test_cdc_filter_matches_table() {
    let filter = CdcFilter::tables(vec!["users".to_string()]);
    let values = test_values(&[("id", json!(1))]);

    let users_event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone());
    let orders_event = CdcEvent::insert("orders".to_string(), "1".to_string(), values);

    assert!(filter.matches(&users_event));
    assert!(!filter.matches(&orders_event));
}

#[test]
fn test_cdc_filter_matches_schema() {
    let filter = CdcFilter {
        tables: None,
        schemas: Some(vec!["public".to_string()]),
        operations: None,
        transaction_ids: None,
    };

    let values = test_values(&[("id", json!(1))]);

    let public_event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone())
        .with_schema("public".to_string());
    let private_event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone())
        .with_schema("private".to_string());
    let no_schema_event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

    assert!(filter.matches(&public_event));
    assert!(!filter.matches(&private_event));
    assert!(!filter.matches(&no_schema_event));
}

#[test]
fn test_cdc_filter_matches_operation() {
    let filter = CdcFilter {
        tables: None,
        schemas: None,
        operations: Some(vec![DmlOperation::Insert, DmlOperation::Update]),
        transaction_ids: None,
    };

    let values = test_values(&[("id", json!(1))]);

    let insert_event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone());
    let delete_event = CdcEvent::delete("users".to_string(), "1".to_string(), values);

    assert!(filter.matches(&insert_event));
    assert!(!filter.matches(&delete_event));
}

#[test]
fn test_cdc_filter_matches_transaction_id() {
    let filter = CdcFilter {
        tables: None,
        schemas: None,
        operations: None,
        transaction_ids: Some(vec!["tx-123".to_string(), "tx-456".to_string()]),
    };

    let values = test_values(&[("id", json!(1))]);

    let matched_event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone())
        .with_transaction_id("tx-123".to_string());
    let unmatched_event = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone())
        .with_transaction_id("tx-789".to_string());
    let no_tx_event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

    assert!(filter.matches(&matched_event));
    assert!(!filter.matches(&unmatched_event));
    assert!(!filter.matches(&no_tx_event));
}

#[test]
fn test_cdc_filter_combined_criteria() {
    let filter = CdcFilter {
        tables: Some(vec!["users".to_string()]),
        schemas: Some(vec!["public".to_string()]),
        operations: Some(vec![DmlOperation::Insert]),
        transaction_ids: None,
    };

    let values = test_values(&[("id", json!(1))]);

    // All criteria match
    let full_match = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone())
        .with_schema("public".to_string());
    assert!(filter.matches(&full_match));

    // Wrong table
    let wrong_table = CdcEvent::insert("orders".to_string(), "1".to_string(), values.clone())
        .with_schema("public".to_string());
    assert!(!filter.matches(&wrong_table));

    // Wrong schema
    let wrong_schema = CdcEvent::insert("users".to_string(), "1".to_string(), values.clone())
        .with_schema("private".to_string());
    assert!(!filter.matches(&wrong_schema));

    // Wrong operation
    let old_values = test_values(&[("id", json!(1))]);
    let wrong_op = CdcEvent::delete("users".to_string(), "1".to_string(), old_values)
        .with_schema("public".to_string());
    assert!(!filter.matches(&wrong_op));
}

// ============================================================================
// CdcSubscription Tests
// ============================================================================

#[test]
fn test_cdc_subscription_default() {
    let subscription = CdcSubscription::default();

    assert!(!subscription.subscription_id.is_nil());
    assert!(subscription.buffer_size > 0);
    assert!(subscription.include_old_values);
    assert!(subscription.include_new_values);
}

#[test]
fn test_cdc_subscription_custom() {
    let subscription = CdcSubscription {
        subscription_id: Uuid::new_v4(),
        filter: CdcFilter::tables(vec!["users".to_string()]),
        buffer_size: 500,
        include_old_values: false,
        include_new_values: true,
    };

    assert_eq!(subscription.buffer_size, 500);
    assert!(!subscription.include_old_values);
    assert!(subscription.include_new_values);
}

// ============================================================================
// CdcCoordinator Tests
// ============================================================================

#[tokio::test]
async fn test_cdc_coordinator_creation() {
    let coordinator = CdcCoordinator::new(None);
    // Coordinator should be created successfully
    let stats = coordinator.get_stats().await;
    assert_eq!(stats.total_events, 0);
}

#[tokio::test]
async fn test_cdc_coordinator_capture_event() {
    let coordinator = CdcCoordinator::new(None);

    let values = test_values(&[("id", json!(1)), ("name", json!("Test"))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

    let result = coordinator.capture_event(event).await;
    assert!(result.is_ok());

    let stats = coordinator.get_stats().await;
    assert_eq!(stats.total_events, 1);
}

#[tokio::test]
async fn test_cdc_coordinator_capture_multiple_events() {
    let coordinator = CdcCoordinator::new(None);

    for i in 0..10 {
        let values = test_values(&[("id", json!(i)), ("name", json!(format!("User {}", i)))]);
        let event = CdcEvent::insert("users".to_string(), format!("{}", i), values);
        coordinator.capture_event(event).await.unwrap();
    }

    let stats = coordinator.get_stats().await;
    assert_eq!(stats.total_events, 10);
}

#[tokio::test]
async fn test_cdc_coordinator_subscribe_and_receive() {
    let coordinator = CdcCoordinator::new(None);

    // Create subscription
    let subscription = CdcSubscription {
        filter: CdcFilter::tables(vec!["users".to_string()]),
        ..Default::default()
    };

    let mut stream = coordinator.subscribe(subscription).await.unwrap();

    // Capture an event
    let values = test_values(&[("id", json!(1)), ("name", json!("Alice"))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);
    coordinator.capture_event(event.clone()).await.unwrap();

    // Receive event from stream (with timeout)
    let received = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;

    assert!(received.is_ok());
    let received_event = received.unwrap().unwrap();
    assert_eq!(received_event.table, "users");
    assert_eq!(received_event.row_id, "1");
}

#[tokio::test]
async fn test_cdc_coordinator_filter_applied() {
    let coordinator = CdcCoordinator::new(None);

    // Subscribe only to 'orders' table
    let subscription = CdcSubscription {
        filter: CdcFilter::tables(vec!["orders".to_string()]),
        ..Default::default()
    };

    let mut stream = coordinator.subscribe(subscription).await.unwrap();

    // Capture event for 'users' table (should be filtered out)
    let users_values = test_values(&[("id", json!(1))]);
    let users_event = CdcEvent::insert("users".to_string(), "1".to_string(), users_values);
    coordinator.capture_event(users_event).await.unwrap();

    // Capture event for 'orders' table (should be received)
    let orders_values = test_values(&[("id", json!(100))]);
    let orders_event = CdcEvent::insert("orders".to_string(), "100".to_string(), orders_values);
    coordinator.capture_event(orders_event).await.unwrap();

    // Should receive only the orders event
    let received = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;
    assert!(received.is_ok());
    let received_event = received.unwrap().unwrap();
    assert_eq!(received_event.table, "orders");
}

#[tokio::test]
async fn test_cdc_coordinator_multiple_subscribers() {
    let coordinator = CdcCoordinator::new(None);

    // Create two subscriptions
    let subscription1 = CdcSubscription {
        filter: CdcFilter::all(),
        ..Default::default()
    };
    let subscription2 = CdcSubscription {
        filter: CdcFilter::all(),
        ..Default::default()
    };

    let mut stream1 = coordinator.subscribe(subscription1).await.unwrap();
    let mut stream2 = coordinator.subscribe(subscription2).await.unwrap();

    // Capture an event
    let values = test_values(&[("id", json!(1))]);
    let event = CdcEvent::insert("test".to_string(), "1".to_string(), values);
    coordinator.capture_event(event).await.unwrap();

    // Both subscribers should receive the event
    let received1 = tokio::time::timeout(Duration::from_millis(100), stream1.next()).await;
    let received2 = tokio::time::timeout(Duration::from_millis(100), stream2.next()).await;

    assert!(received1.is_ok());
    assert!(received2.is_ok());
}

#[tokio::test]
async fn test_cdc_coordinator_lsn_assignment() {
    let coordinator = CdcCoordinator::new(None);

    // Subscribe to receive events
    let subscription = CdcSubscription::default();
    let mut stream = coordinator.subscribe(subscription).await.unwrap();

    // Capture multiple events
    for i in 0..3 {
        let values = test_values(&[("id", json!(i))]);
        let event = CdcEvent::insert("test".to_string(), format!("{}", i), values);
        coordinator.capture_event(event).await.unwrap();
    }

    // Collect events and verify LSN ordering
    let mut lsns = Vec::new();
    for _ in 0..3 {
        let received = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;
        if let Ok(Some(event)) = received {
            lsns.push(event.lsn);
        }
    }

    // LSNs should be increasing
    assert_eq!(lsns.len(), 3);
    assert!(lsns[0] < lsns[1]);
    assert!(lsns[1] < lsns[2]);
}

#[tokio::test]
async fn test_cdc_coordinator_stats_by_operation() {
    let coordinator = CdcCoordinator::new(None);

    // Capture various operations
    let insert_values = test_values(&[("id", json!(1))]);
    coordinator
        .capture_event(CdcEvent::insert(
            "users".to_string(),
            "1".to_string(),
            insert_values.clone(),
        ))
        .await
        .unwrap();
    coordinator
        .capture_event(CdcEvent::insert(
            "users".to_string(),
            "2".to_string(),
            insert_values,
        ))
        .await
        .unwrap();

    let old_values = test_values(&[("name", json!("Old"))]);
    let new_values = test_values(&[("name", json!("New"))]);
    coordinator
        .capture_event(CdcEvent::update(
            "users".to_string(),
            "1".to_string(),
            old_values.clone(),
            new_values,
        ))
        .await
        .unwrap();

    coordinator
        .capture_event(CdcEvent::delete(
            "users".to_string(),
            "2".to_string(),
            old_values,
        ))
        .await
        .unwrap();

    let stats = coordinator.get_stats().await;
    assert_eq!(stats.total_events, 4);
    assert_eq!(
        *stats
            .events_by_operation
            .get(&DmlOperation::Insert)
            .unwrap_or(&0),
        2
    );
    assert_eq!(
        *stats
            .events_by_operation
            .get(&DmlOperation::Update)
            .unwrap_or(&0),
        1
    );
    assert_eq!(
        *stats
            .events_by_operation
            .get(&DmlOperation::Delete)
            .unwrap_or(&0),
        1
    );
}

#[tokio::test]
async fn test_cdc_coordinator_stats_by_table() {
    let coordinator = CdcCoordinator::new(None);

    // Capture events for different tables
    let values = test_values(&[("id", json!(1))]);

    coordinator
        .capture_event(CdcEvent::insert(
            "users".to_string(),
            "1".to_string(),
            values.clone(),
        ))
        .await
        .unwrap();
    coordinator
        .capture_event(CdcEvent::insert(
            "users".to_string(),
            "2".to_string(),
            values.clone(),
        ))
        .await
        .unwrap();
    coordinator
        .capture_event(CdcEvent::insert(
            "orders".to_string(),
            "1".to_string(),
            values,
        ))
        .await
        .unwrap();

    let stats = coordinator.get_stats().await;
    assert_eq!(*stats.events_by_table.get("users").unwrap_or(&0), 2);
    assert_eq!(*stats.events_by_table.get("orders").unwrap_or(&0), 1);
}

// ============================================================================
// CdcStream Tests
// ============================================================================

#[tokio::test]
async fn test_cdc_stream_subscription_id() {
    let coordinator = CdcCoordinator::new(None);

    let subscription = CdcSubscription::default();
    let expected_id = subscription.subscription_id;

    let stream = coordinator.subscribe(subscription).await.unwrap();

    assert_eq!(stream.subscription_id(), expected_id);
}

#[tokio::test]
async fn test_cdc_stream_receives_in_order() {
    let coordinator = CdcCoordinator::new(None);

    let subscription = CdcSubscription::default();
    let mut stream = coordinator.subscribe(subscription).await.unwrap();

    // Capture events with distinct identifiers
    for i in 0..5 {
        let values = test_values(&[("seq", json!(i))]);
        let event = CdcEvent::insert("test".to_string(), format!("{}", i), values);
        coordinator.capture_event(event).await.unwrap();
    }

    // Verify events are received in order
    for expected_seq in 0..5 {
        let received = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;
        assert!(received.is_ok());
        let event = received.unwrap().unwrap();
        assert_eq!(event.row_id, format!("{}", expected_seq));
    }
}

// ============================================================================
// Integration Scenario Tests
// ============================================================================

#[tokio::test]
async fn test_cdc_full_lifecycle() {
    let coordinator = CdcCoordinator::new(None);

    // Subscribe before any events
    let subscription = CdcSubscription {
        filter: CdcFilter::tables(vec!["users".to_string()]),
        ..Default::default()
    };
    let mut stream = coordinator.subscribe(subscription).await.unwrap();

    // Simulate a complete entity lifecycle
    // 1. Insert
    let insert_values = test_values(&[
        ("id", json!(1)),
        ("name", json!("Alice")),
        ("email", json!("alice@example.com")),
    ]);
    let insert_event = CdcEvent::insert("users".to_string(), "1".to_string(), insert_values)
        .with_transaction_id("tx-001".to_string());
    coordinator.capture_event(insert_event).await.unwrap();

    // 2. Update
    let old_values = test_values(&[("email", json!("alice@example.com"))]);
    let new_values = test_values(&[("email", json!("alice.smith@example.com"))]);
    let update_event =
        CdcEvent::update("users".to_string(), "1".to_string(), old_values, new_values)
            .with_transaction_id("tx-002".to_string());
    coordinator.capture_event(update_event).await.unwrap();

    // 3. Delete
    let delete_values = test_values(&[
        ("id", json!(1)),
        ("name", json!("Alice")),
        ("email", json!("alice.smith@example.com")),
    ]);
    let delete_event = CdcEvent::delete("users".to_string(), "1".to_string(), delete_values)
        .with_transaction_id("tx-003".to_string());
    coordinator.capture_event(delete_event).await.unwrap();

    // Verify all events received in order
    let received1 = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received1.operation, DmlOperation::Insert);

    let received2 = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received2.operation, DmlOperation::Update);

    let received3 = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received3.operation, DmlOperation::Delete);
}

#[tokio::test]
async fn test_cdc_high_throughput() {
    let coordinator = CdcCoordinator::new(None);

    let subscription = CdcSubscription {
        buffer_size: 10000,
        ..Default::default()
    };
    let mut stream = coordinator.subscribe(subscription).await.unwrap();

    // Capture many events rapidly
    let event_count = 1000;
    for i in 0..event_count {
        let values = test_values(&[("id", json!(i))]);
        let event = CdcEvent::insert("bulk_table".to_string(), format!("{}", i), values);
        coordinator.capture_event(event).await.unwrap();
    }

    // Verify we can receive all events
    let mut received_count = 0;
    loop {
        match tokio::time::timeout(Duration::from_millis(50), stream.next()).await {
            Ok(Some(_)) => received_count += 1,
            _ => break,
        }
    }

    assert_eq!(received_count, event_count);
}

#[tokio::test]
async fn test_cdc_selective_consumption() {
    let coordinator = CdcCoordinator::new(None);

    // Subscriber only interested in DELETE operations
    let delete_subscription = CdcSubscription {
        filter: CdcFilter {
            tables: None,
            schemas: None,
            operations: Some(vec![DmlOperation::Delete]),
            transaction_ids: None,
        },
        ..Default::default()
    };
    let mut delete_stream = coordinator.subscribe(delete_subscription).await.unwrap();

    // Capture various operations
    let values = test_values(&[("id", json!(1))]);
    coordinator
        .capture_event(CdcEvent::insert(
            "test".to_string(),
            "1".to_string(),
            values.clone(),
        ))
        .await
        .unwrap();
    coordinator
        .capture_event(CdcEvent::delete(
            "test".to_string(),
            "1".to_string(),
            values,
        ))
        .await
        .unwrap();

    // Should only receive the DELETE event
    let received = tokio::time::timeout(Duration::from_millis(100), delete_stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received.operation, DmlOperation::Delete);

    // No more events should be available
    let no_more = tokio::time::timeout(Duration::from_millis(50), delete_stream.next()).await;
    assert!(no_more.is_err()); // Timeout indicates no more events
}

#[tokio::test]
async fn test_cdc_consumer_trait() {
    let consumer = MockCdcConsumer::new();

    let values = test_values(&[("id", json!(1)), ("name", json!("Test"))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

    consumer.process_event(&event).await.unwrap();

    assert_eq!(consumer.get_process_count(), 1);
    let processed = consumer.get_processed_events().await;
    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0].table, "users");
}

#[tokio::test]
async fn test_cdc_event_chaining() {
    let values = test_values(&[("id", json!(1))]);
    let event = CdcEvent::insert("users".to_string(), "1".to_string(), values)
        .with_transaction_id("tx-123".to_string())
        .with_schema("public".to_string())
        .with_metadata("source".to_string(), "api".to_string())
        .with_metadata("client_ip".to_string(), "192.168.1.1".to_string());

    assert_eq!(event.transaction_id, Some("tx-123".to_string()));
    assert_eq!(event.schema, Some("public".to_string()));
    assert_eq!(event.metadata.len(), 2);
}

#[tokio::test]
async fn test_cdc_stats_default() {
    let stats = CdcStats::default();

    assert_eq!(stats.total_events, 0);
    assert!(stats.events_by_operation.is_empty());
    assert!(stats.events_by_table.is_empty());
}
