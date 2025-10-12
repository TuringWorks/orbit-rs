//! CDC and Streaming Example
//!
//! This example demonstrates the Change Data Capture (CDC) and real-time
//! streaming capabilities including:
//! - CDC event capture and distribution
//! - Kafka/RabbitMQ integration
//! - Stream processing with windowing
//! - Event sourcing patterns
//! - Replication slots

use orbit_shared::{
    cdc::{CdcCoordinator, CdcEvent, CdcFilter, CdcSubscription},
    event_sourcing::{DomainEvent, EventStore, EventStoreConfig},
    replication::{ReplicationConfig, ReplicationSlotManager},
    stream_processing::{AggregationFunction, StreamEvent, StreamProcessor, WindowType},
    streaming_integrations::{KafkaCdcConsumer, KafkaConfig},
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting CDC and Streaming Example");

    // 1. CDC Demonstration
    demo_cdc().await?;

    // 2. Stream Processing Demonstration
    demo_stream_processing().await?;

    // 3. Event Sourcing Demonstration
    demo_event_sourcing().await?;

    // 4. Replication Slots Demonstration
    demo_replication_slots().await?;

    info!("Example completed successfully!");
    Ok(())
}

async fn demo_cdc() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n=== CDC Demonstration ===");

    // Create CDC coordinator
    let coordinator = CdcCoordinator::new(None);

    // Subscribe to CDC events for specific tables
    let subscription = CdcSubscription {
        filter: CdcFilter::tables(vec!["users".to_string(), "orders".to_string()]),
        ..Default::default()
    };

    let mut stream = coordinator.subscribe(subscription).await?;

    // Spawn task to consume CDC events
    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            info!(
                "CDC Event: {:?} on table {} (row {})",
                event.operation, event.table, event.row_id
            );
        }
    });

    // Simulate some CDC events
    let mut user_data = HashMap::new();
    user_data.insert("id".to_string(), serde_json::json!(1));
    user_data.insert("name".to_string(), serde_json::json!("John Doe"));
    user_data.insert("email".to_string(), serde_json::json!("john@example.com"));

    let insert_event = CdcEvent::insert("users".to_string(), "1".to_string(), user_data.clone());
    coordinator.capture_event(insert_event).await?;

    // Simulate update
    let mut updated_data = user_data.clone();
    updated_data.insert("email".to_string(), serde_json::json!("john.doe@example.com"));

    let update_event = CdcEvent::update(
        "users".to_string(),
        "1".to_string(),
        user_data.clone(),
        updated_data,
    );
    coordinator.capture_event(update_event).await?;

    sleep(Duration::from_millis(100)).await;

    // Show statistics
    let stats = coordinator.get_stats().await;
    info!("CDC Statistics: {:?}", stats);

    // Demonstrate Kafka integration
    info!("\n=== Kafka Integration ===");
    let kafka_config = KafkaConfig::default();
    let kafka_consumer = KafkaCdcConsumer::new(kafka_config).await?;

    // Process an event through Kafka consumer
    let order_data = HashMap::from([
        ("order_id".to_string(), serde_json::json!(123)),
        ("amount".to_string(), serde_json::json!(99.99)),
    ]);
    let order_event = CdcEvent::insert("orders".to_string(), "123".to_string(), order_data);
    kafka_consumer.process_event(&order_event).await?;

    let kafka_stats = kafka_consumer.get_stats().await;
    info!("Kafka Statistics: {:?}", kafka_stats);

    Ok(())
}

async fn demo_stream_processing() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n=== Stream Processing Demonstration ===");

    // Create stream processor with count-based window
    let processor = StreamProcessor::new(
        WindowType::Count { count: 5 },
        AggregationFunction::Avg,
    );

    // Process streaming events
    info!("Processing stream events with count window (size=5, aggregation=Avg)");

    for i in 1..=10 {
        let event = StreamEvent::new(
            "temperature".to_string(),
            serde_json::json!(20.0 + i as f64),
        );

        let results = processor.process_event(event).await?;

        // Window completed - show results
        for result in results {
            info!(
                "Window completed: key={}, count={}, avg_temp={:.2}",
                result.key,
                result.count,
                result.result.as_f64().unwrap_or(0.0)
            );
        }
    }

    let stats = processor.get_stats().await;
    info!("Stream Processing Statistics: {:?}", stats);

    // Demonstrate tumbling window
    info!("\n=== Tumbling Window (1 second) ===");
    let tumbling_processor = StreamProcessor::new(
        WindowType::Tumbling {
            size: Duration::from_secs(1),
        },
        AggregationFunction::Count,
    );

    for i in 1..=3 {
        let event = StreamEvent::new("clicks".to_string(), serde_json::json!(i));
        tumbling_processor.process_event(event).await?;
        sleep(Duration::from_millis(400)).await;
    }

    info!("Tumbling window demonstration completed");

    Ok(())
}

async fn demo_event_sourcing() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n=== Event Sourcing Demonstration ===");

    let config = EventStoreConfig::default();
    let store = EventStore::new(config);

    // Create events for a user aggregate
    let user_id = "user-123".to_string();

    // Event 1: User created
    let created_event = DomainEvent::new(
        user_id.clone(),
        "User".to_string(),
        "UserCreated".to_string(),
        serde_json::json!({
            "name": "Alice",
            "email": "alice@example.com",
            "balance": 0.0
        }),
    );
    store.append_event(created_event).await?;

    // Event 2: Deposit money
    let deposit_event = DomainEvent::new(
        user_id.clone(),
        "User".to_string(),
        "MoneyDeposited".to_string(),
        serde_json::json!({
            "amount": 100.0
        }),
    );
    store.append_event(deposit_event).await?;

    // Event 3: Purchase made
    let purchase_event = DomainEvent::new(
        user_id.clone(),
        "User".to_string(),
        "PurchaseMade".to_string(),
        serde_json::json!({
            "amount": 35.50,
            "item": "Book"
        }),
    );
    store.append_event(purchase_event).await?;

    // Retrieve all events
    let events = store.get_events(&user_id, None).await?;
    info!("Total events for user: {}", events.len());

    for event in &events {
        info!(
            "  Seq {}: {} - {:?}",
            event.sequence, event.event_type, event.data
        );
    }

    // Rebuild state from events
    #[derive(Clone)]
    struct UserState {
        name: String,
        balance: f64,
    }

    let final_state = store
        .rebuild_state(
            &user_id,
            UserState {
                name: String::new(),
                balance: 0.0,
            },
            |mut state, event| {
                match event.event_type.as_str() {
                    "UserCreated" => {
                        state.name = event.data["name"].as_str().unwrap_or("").to_string();
                        state.balance = event.data["balance"].as_f64().unwrap_or(0.0);
                    }
                    "MoneyDeposited" => {
                        state.balance += event.data["amount"].as_f64().unwrap_or(0.0);
                    }
                    "PurchaseMade" => {
                        state.balance -= event.data["amount"].as_f64().unwrap_or(0.0);
                    }
                    _ => {}
                }
                Ok(state)
            },
        )
        .await?;

    info!(
        "Final state - Name: {}, Balance: ${:.2}",
        final_state.name, final_state.balance
    );

    let stats = store.get_stats().await;
    info!("Event Store Statistics: {:?}", stats);

    Ok(())
}

async fn demo_replication_slots() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n=== Replication Slots Demonstration ===");

    let config = ReplicationConfig::default();
    let manager = ReplicationSlotManager::new(config);

    // Create replication slots for different consumers
    let slot1 = manager
        .create_slot("consumer1".to_string(), "kafka".to_string())
        .await?;
    info!("Created replication slot: {}", slot1.name);

    let slot2 = manager
        .create_slot("consumer2".to_string(), "webhook".to_string())
        .await?;
    info!("Created replication slot: {}", slot2.name);

    // Simulate LSN progression
    for _ in 0..10 {
        manager.advance_lsn().await;
    }

    info!("Current LSN: {}", manager.current_lsn().await);

    // Consumer 1 processes some events
    manager.advance_slot("consumer1", 5).await?;
    info!("Consumer1 processed up to LSN 5");

    // Consumer 2 processes more events
    manager.advance_slot("consumer2", 8).await?;
    info!("Consumer2 processed up to LSN 8");

    // Check lag for each consumer
    let lag1 = manager.get_slot_lag("consumer1").await?;
    let lag2 = manager.get_slot_lag("consumer2").await?;

    info!("Consumer1 lag: {} events", lag1);
    info!("Consumer2 lag: {} events", lag2);

    // List all slots
    let slots = manager.list_slots().await;
    info!("Active replication slots:");
    for slot in slots {
        info!(
            "  - {} (plugin: {}, LSN: {}, active: {})",
            slot.name, slot.plugin, slot.confirmed_flush_lsn, slot.active
        );
    }

    let stats = manager.get_stats().await;
    info!("Replication Statistics: {:?}", stats);

    Ok(())
}
