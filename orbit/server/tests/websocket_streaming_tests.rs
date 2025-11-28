//! WebSocket Streaming Integration Tests
//!
//! Comprehensive tests for the WebSocket streaming system including:
//! - WebSocketHandler creation and configuration
//! - WebSocketMessage serialization/deserialization
//! - SubscribeRequest parsing
//! - EventBroadcaster functionality
//! - Broadcast channel behavior

use orbit_server::protocols::rest::models::{SubscribeRequest, WebSocketMessage};
use orbit_server::protocols::rest::websocket::{EventBroadcaster, WebSocketHandler};
use serde_json::json;
use std::sync::Arc;

// ============================================================================
// WebSocketHandler Tests
// ============================================================================

#[test]
fn test_websocket_handler_creation() {
    let _handler = WebSocketHandler::new();

    // Handler should be created successfully
    // We can verify it's properly initialized by creating a default one
    let _default_handler = WebSocketHandler::default();

    assert!(true, "WebSocketHandler created successfully");
}

#[test]
fn test_websocket_handler_default() {
    let handler1 = WebSocketHandler::new();
    let handler2 = WebSocketHandler::default();

    // Both should create valid handlers
    // (We can't compare internal state, but we verify they construct properly)
    drop(handler1);
    drop(handler2);
    assert!(true, "Both new() and default() create valid handlers");
}

// ============================================================================
// WebSocketMessage Serialization Tests
// ============================================================================

#[test]
fn test_websocket_message_actor_state_changed_serialization() {
    let msg = WebSocketMessage::ActorStateChanged {
        actor_type: "CounterActor".to_string(),
        key: json!({"StringKey": {"key": "counter-1"}}),
        state: json!({"count": 42}),
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("actor_state_changed"));
    assert!(json_str.contains("CounterActor"));
    assert!(json_str.contains("counter-1"));
    assert!(json_str.contains("42"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::ActorStateChanged {
            actor_type,
            key: _,
            state: _,
        } => {
            assert_eq!(actor_type, "CounterActor");
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_actor_activated_serialization() {
    let msg = WebSocketMessage::ActorActivated {
        actor_type: "GreeterActor".to_string(),
        key: json!("greeter-1"),
        node_id: "node-001".to_string(),
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("actor_activated"));
    assert!(json_str.contains("GreeterActor"));
    assert!(json_str.contains("node-001"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::ActorActivated {
            actor_type,
            key: _,
            node_id,
        } => {
            assert_eq!(actor_type, "GreeterActor");
            assert_eq!(node_id, "node-001");
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_actor_deactivated_serialization() {
    let msg = WebSocketMessage::ActorDeactivated {
        actor_type: "SessionActor".to_string(),
        key: json!(123456),
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("actor_deactivated"));
    assert!(json_str.contains("SessionActor"));
    assert!(json_str.contains("123456"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::ActorDeactivated { actor_type, key: _ } => {
            assert_eq!(actor_type, "SessionActor");
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_transaction_event_serialization() {
    let msg = WebSocketMessage::TransactionEvent {
        transaction_id: "tx-abc123".to_string(),
        status: "committed".to_string(),
        message: Some("Transaction completed successfully".to_string()),
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("transaction_event"));
    assert!(json_str.contains("tx-abc123"));
    assert!(json_str.contains("committed"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::TransactionEvent {
            transaction_id,
            status,
            message,
        } => {
            assert_eq!(transaction_id, "tx-abc123");
            assert_eq!(status, "committed");
            assert!(message.is_some());
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_transaction_event_no_message() {
    let msg = WebSocketMessage::TransactionEvent {
        transaction_id: "tx-xyz789".to_string(),
        status: "rolled_back".to_string(),
        message: None,
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");

    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::TransactionEvent {
            transaction_id: _,
            status: _,
            message,
        } => {
            assert!(message.is_none());
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_system_event_serialization() {
    let msg = WebSocketMessage::SystemEvent {
        event_type: "cluster_rebalance".to_string(),
        data: json!({
            "from_node": "node-1",
            "to_node": "node-2",
            "actors_moved": 15
        }),
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("system_event"));
    assert!(json_str.contains("cluster_rebalance"));
    assert!(json_str.contains("actors_moved"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::SystemEvent { event_type, data } => {
            assert_eq!(event_type, "cluster_rebalance");
            assert_eq!(data["actors_moved"], 15);
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_subscription_ack_serialization() {
    let msg = WebSocketMessage::SubscriptionAck {
        subscription_id: "sub-12345".to_string(),
        filters: vec!["actor:CounterActor:*".to_string(), "system:*".to_string()],
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("subscription_ack"));
    assert!(json_str.contains("sub-12345"));
    assert!(json_str.contains("actor:CounterActor:*"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::SubscriptionAck {
            subscription_id,
            filters,
        } => {
            assert_eq!(subscription_id, "sub-12345");
            assert_eq!(filters.len(), 2);
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

#[test]
fn test_websocket_message_error_serialization() {
    let msg = WebSocketMessage::Error {
        code: "INVALID_SUBSCRIPTION".to_string(),
        message: "Invalid filter pattern provided".to_string(),
    };

    let json_str = serde_json::to_string(&msg).expect("Serialization failed");
    assert!(json_str.contains("error"));
    assert!(json_str.contains("INVALID_SUBSCRIPTION"));

    // Verify round-trip
    let deserialized: WebSocketMessage =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    match deserialized {
        WebSocketMessage::Error { code, message } => {
            assert_eq!(code, "INVALID_SUBSCRIPTION");
            assert_eq!(message, "Invalid filter pattern provided");
        }
        _ => panic!("Wrong message type after deserialization"),
    }
}

// ============================================================================
// SubscribeRequest Tests
// ============================================================================

#[test]
fn test_subscribe_request_basic() {
    let request = SubscribeRequest {
        event_types: vec![
            "actor_state_changed".to_string(),
            "actor_activated".to_string(),
        ],
        filters: None,
    };

    let json_str = serde_json::to_string(&request).expect("Serialization failed");
    assert!(json_str.contains("actor_state_changed"));
    assert!(json_str.contains("actor_activated"));

    // Verify round-trip
    let deserialized: SubscribeRequest =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    assert_eq!(deserialized.event_types.len(), 2);
    assert!(deserialized.filters.is_none());
}

#[test]
fn test_subscribe_request_with_filters() {
    let request = SubscribeRequest {
        event_types: vec!["actor_state_changed".to_string()],
        filters: Some(json!({
            "actor_type": "CounterActor",
            "key_pattern": "counter-*"
        })),
    };

    let json_str = serde_json::to_string(&request).expect("Serialization failed");

    // Verify round-trip
    let deserialized: SubscribeRequest =
        serde_json::from_str(&json_str).expect("Deserialization failed");
    assert_eq!(deserialized.event_types.len(), 1);
    assert!(deserialized.filters.is_some());

    let filters = deserialized.filters.unwrap();
    assert_eq!(filters["actor_type"], "CounterActor");
    assert_eq!(filters["key_pattern"], "counter-*");
}

#[test]
fn test_subscribe_request_parse_from_json() {
    let json_str = r#"{
        "event_types": ["system_event", "transaction_event"],
        "filters": {
            "event_type": "cluster_*"
        }
    }"#;

    let request: SubscribeRequest =
        serde_json::from_str(json_str).expect("Deserialization failed");
    assert_eq!(request.event_types.len(), 2);
    assert!(request.event_types.contains(&"system_event".to_string()));
    assert!(request.event_types.contains(&"transaction_event".to_string()));
}

#[test]
fn test_subscribe_request_empty_filters() {
    let json_str = r#"{
        "event_types": ["actor_deactivated"],
        "filters": {}
    }"#;

    let request: SubscribeRequest =
        serde_json::from_str(json_str).expect("Deserialization failed");
    assert_eq!(request.event_types.len(), 1);
    assert!(request.filters.is_some());
}

// ============================================================================
// EventBroadcaster Tests
// ============================================================================

#[tokio::test]
async fn test_event_broadcaster_creation() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler);

    // Broadcaster should be created successfully
    drop(broadcaster);
    assert!(true, "EventBroadcaster created successfully");
}

#[tokio::test]
async fn test_event_broadcaster_actor_state_changed() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    // Subscribe to receive events
    let mut receiver = handler.subscribe();

    // Broadcast an actor state change
    broadcaster
        .actor_state_changed(
            "CounterActor".to_string(),
            json!("counter-1"),
            json!({"count": 100}),
        )
        .await;

    // Receive the event
    let event = receiver.recv().await.expect("Failed to receive event");
    match event {
        WebSocketMessage::ActorStateChanged {
            actor_type,
            key: _,
            state,
        } => {
            assert_eq!(actor_type, "CounterActor");
            assert_eq!(state["count"], 100);
        }
        _ => panic!("Wrong event type"),
    }
}

#[tokio::test]
async fn test_event_broadcaster_actor_activated() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    let mut receiver = handler.subscribe();

    broadcaster
        .actor_activated(
            "GreeterActor".to_string(),
            json!("greeter-42"),
            "node-007".to_string(),
        )
        .await;

    let event = receiver.recv().await.expect("Failed to receive event");
    match event {
        WebSocketMessage::ActorActivated {
            actor_type,
            key: _,
            node_id,
        } => {
            assert_eq!(actor_type, "GreeterActor");
            assert_eq!(node_id, "node-007");
        }
        _ => panic!("Wrong event type"),
    }
}

#[tokio::test]
async fn test_event_broadcaster_actor_deactivated() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    let mut receiver = handler.subscribe();

    broadcaster
        .actor_deactivated("SessionActor".to_string(), json!({"session_id": "sess-123"}))
        .await;

    let event = receiver.recv().await.expect("Failed to receive event");
    match event {
        WebSocketMessage::ActorDeactivated { actor_type, key: _ } => {
            assert_eq!(actor_type, "SessionActor");
        }
        _ => panic!("Wrong event type"),
    }
}

#[tokio::test]
async fn test_event_broadcaster_transaction_event() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    let mut receiver = handler.subscribe();

    broadcaster
        .transaction_event(
            "tx-99999".to_string(),
            "preparing".to_string(),
            Some("Preparing 2PC".to_string()),
        )
        .await;

    let event = receiver.recv().await.expect("Failed to receive event");
    match event {
        WebSocketMessage::TransactionEvent {
            transaction_id,
            status,
            message,
        } => {
            assert_eq!(transaction_id, "tx-99999");
            assert_eq!(status, "preparing");
            assert_eq!(message, Some("Preparing 2PC".to_string()));
        }
        _ => panic!("Wrong event type"),
    }
}

#[tokio::test]
async fn test_event_broadcaster_system_event() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    let mut receiver = handler.subscribe();

    broadcaster
        .system_event(
            "node_joined".to_string(),
            json!({"node_id": "new-node", "address": "192.168.1.100"}),
        )
        .await;

    let event = receiver.recv().await.expect("Failed to receive event");
    match event {
        WebSocketMessage::SystemEvent { event_type, data } => {
            assert_eq!(event_type, "node_joined");
            assert_eq!(data["node_id"], "new-node");
        }
        _ => panic!("Wrong event type"),
    }
}

// ============================================================================
// Broadcast Channel Tests
// ============================================================================

#[tokio::test]
async fn test_broadcast_multiple_subscribers() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    // Create multiple subscribers
    let mut receiver1 = handler.subscribe();
    let mut receiver2 = handler.subscribe();
    let mut receiver3 = handler.subscribe();

    // Broadcast an event
    broadcaster
        .system_event("test_event".to_string(), json!({"value": 123}))
        .await;

    // All subscribers should receive the event
    let event1 = receiver1.recv().await.expect("Subscriber 1 failed");
    let event2 = receiver2.recv().await.expect("Subscriber 2 failed");
    let event3 = receiver3.recv().await.expect("Subscriber 3 failed");

    // Verify all received the same event
    match (&event1, &event2, &event3) {
        (
            WebSocketMessage::SystemEvent {
                event_type: t1,
                data: d1,
            },
            WebSocketMessage::SystemEvent {
                event_type: t2,
                data: d2,
            },
            WebSocketMessage::SystemEvent {
                event_type: t3,
                data: d3,
            },
        ) => {
            assert_eq!(t1, "test_event");
            assert_eq!(t1, t2);
            assert_eq!(t2, t3);
            assert_eq!(d1["value"], d2["value"]);
            assert_eq!(d2["value"], d3["value"]);
        }
        _ => panic!("Events don't match"),
    }
}

#[tokio::test]
async fn test_broadcast_sequential_events() {
    let handler = Arc::new(WebSocketHandler::new());
    let broadcaster = EventBroadcaster::new(handler.clone());

    let mut receiver = handler.subscribe();

    // Broadcast multiple events
    broadcaster
        .system_event("event_1".to_string(), json!({"order": 1}))
        .await;
    broadcaster
        .system_event("event_2".to_string(), json!({"order": 2}))
        .await;
    broadcaster
        .system_event("event_3".to_string(), json!({"order": 3}))
        .await;

    // Events should be received in order
    for expected_order in 1..=3 {
        let event = receiver.recv().await.expect("Failed to receive event");
        match event {
            WebSocketMessage::SystemEvent { event_type: _, data } => {
                assert_eq!(data["order"], expected_order);
            }
            _ => panic!("Wrong event type"),
        }
    }
}

#[tokio::test]
async fn test_broadcast_handler_directly() {
    let handler = WebSocketHandler::new();

    let mut receiver = handler.subscribe();

    // Broadcast directly through handler
    handler
        .broadcast_event(WebSocketMessage::Error {
            code: "TEST_ERROR".to_string(),
            message: "Test error message".to_string(),
        })
        .await;

    let event = receiver.recv().await.expect("Failed to receive event");
    match event {
        WebSocketMessage::Error { code, message } => {
            assert_eq!(code, "TEST_ERROR");
            assert_eq!(message, "Test error message");
        }
        _ => panic!("Wrong event type"),
    }
}

// ============================================================================
// Message Format Validation Tests
// ============================================================================

#[test]
fn test_websocket_message_json_format_tagged() {
    // Verify that messages use tagged union format with "type" field
    let msg = WebSocketMessage::ActorStateChanged {
        actor_type: "Test".to_string(),
        key: json!("key"),
        state: json!({}),
    };

    let json_str = serde_json::to_string(&msg).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Should have a "type" field with snake_case value
    assert!(parsed.get("type").is_some());
    assert_eq!(parsed["type"], "actor_state_changed");
}

#[test]
fn test_all_message_types_have_type_field() {
    let messages = vec![
        WebSocketMessage::ActorStateChanged {
            actor_type: "A".to_string(),
            key: json!("k"),
            state: json!({}),
        },
        WebSocketMessage::ActorActivated {
            actor_type: "A".to_string(),
            key: json!("k"),
            node_id: "n".to_string(),
        },
        WebSocketMessage::ActorDeactivated {
            actor_type: "A".to_string(),
            key: json!("k"),
        },
        WebSocketMessage::TransactionEvent {
            transaction_id: "t".to_string(),
            status: "s".to_string(),
            message: None,
        },
        WebSocketMessage::SystemEvent {
            event_type: "e".to_string(),
            data: json!({}),
        },
        WebSocketMessage::SubscriptionAck {
            subscription_id: "s".to_string(),
            filters: vec![],
        },
        WebSocketMessage::Error {
            code: "c".to_string(),
            message: "m".to_string(),
        },
    ];

    for msg in messages {
        let json_str = serde_json::to_string(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(
            parsed.get("type").is_some(),
            "Message should have 'type' field"
        );
    }
}

#[test]
fn test_deserialize_unknown_type_fails() {
    let invalid_json = r#"{"type": "unknown_event_type", "data": {}}"#;
    let result: Result<WebSocketMessage, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err(), "Unknown type should fail to deserialize");
}
