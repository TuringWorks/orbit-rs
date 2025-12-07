//! WebSocket handler for real-time actor events

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info};

use super::handlers::ApiState;
use super::models::*;

/// WebSocket connection manager
pub struct WebSocketHandler {
    /// Broadcast channel for actor events
    actor_events: broadcast::Sender<WebSocketMessage>,

    /// Active subscriptions
    #[allow(dead_code)]
    subscriptions: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl WebSocketHandler {
    pub fn new() -> Self {
        let (actor_events, _) = broadcast::channel(1000);

        Self {
            actor_events,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Broadcast an event to all subscribers
    pub async fn broadcast_event(&self, event: WebSocketMessage) {
        if let Err(e) = self.actor_events.send(event) {
            error!("Failed to broadcast event: {}", e);
        }
    }

    /// Subscribe to receive broadcast events
    /// Returns a receiver that will receive all events broadcast through this handler
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.actor_events.subscribe()
    }

    /// Handle new WebSocket connection for actor events
    pub async fn handle_actor_socket(
        ws: WebSocketUpgrade,
        Path((actor_type, key)): Path<(String, String)>,
        State(state): State<ApiState>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| Self::actor_websocket(socket, actor_type, key, state))
    }

    /// Handle new WebSocket connection for system events
    pub async fn handle_events_socket(
        ws: WebSocketUpgrade,
        State(state): State<ApiState>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| Self::events_websocket(socket, state))
    }

    /// WebSocket handler for specific actor
    async fn actor_websocket(socket: WebSocket, actor_type: String, key: String, _state: ApiState) {
        let (mut sender, mut receiver) = socket.split();
        let subscription_id = uuid::Uuid::new_v4().to_string();

        info!(
            "WebSocket connected for actor {}/{} (subscription: {})",
            actor_type, key, subscription_id
        );

        // Send subscription acknowledgment
        let ack = WebSocketMessage::SubscriptionAck {
            subscription_id: subscription_id.clone(),
            filters: vec![format!("actor:{}:{}", actor_type, key)],
        };

        if let Ok(json) = serde_json::to_string(&ack) {
            let _ = sender.send(Message::Text(json.into())).await;
        }

        // TODO: Subscribe to actor state changes via OrbitClient
        // For now, just handle incoming messages and keep connection alive

        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle client messages (e.g., subscription updates)
                    if let Ok(request) = serde_json::from_str::<SubscribeRequest>(&text) {
                        info!("Subscription update: {:?}", request);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket closed for actor {}/{}", actor_type, key);
                    break;
                }
                Ok(Message::Ping(data)) => {
                    let _ = sender.send(Message::Pong(data)).await;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    }

    /// WebSocket handler for system events
    async fn events_websocket(socket: WebSocket, state: ApiState) {
        let (mut sender, mut receiver) = socket.split();
        let subscription_id = uuid::Uuid::new_v4().to_string();

        info!(
            "WebSocket connected for system events (subscription: {})",
            subscription_id
        );

        // Send subscription acknowledgment
        let ack = WebSocketMessage::SubscriptionAck {
            subscription_id: subscription_id.clone(),
            filters: vec!["system:*".to_string()],
        };

        if let Ok(json) = serde_json::to_string(&ack) {
            let _ = sender.send(Message::Text(json.into())).await;
        }

        // Spawn task to forward events
        let _event_rx = state.orbit_client.clone(); // TODO: Get event stream from orbit_client
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // TODO: Receive events from orbit_client event stream
                    // For now, just handle incoming client messages
                    msg = receiver.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(request) = serde_json::from_str::<SubscribeRequest>(&text) {
                                    info!("Subscription update: {:?}", request);
                                    // Update subscription filters
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("WebSocket closed");
                                break;
                            }
                            Some(Ok(Message::Ping(data))) => {
                                let _ = sender.send(Message::Pong(data)).await;
                            }
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            None => break,
                            _ => {}
                        }
                    }
                }
            }
        });
    }
}

impl Default for WebSocketHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Example event broadcaster (would be integrated with OrbitClient)
pub struct EventBroadcaster {
    ws_handler: Arc<WebSocketHandler>,
}

impl EventBroadcaster {
    pub fn new(ws_handler: Arc<WebSocketHandler>) -> Self {
        Self { ws_handler }
    }

    /// Broadcast actor state change
    pub async fn actor_state_changed(
        &self,
        actor_type: String,
        key: serde_json::Value,
        state: serde_json::Value,
    ) {
        let event = WebSocketMessage::ActorStateChanged {
            actor_type,
            key,
            state,
        };
        self.ws_handler.broadcast_event(event).await;
    }

    /// Broadcast actor activation
    pub async fn actor_activated(
        &self,
        actor_type: String,
        key: serde_json::Value,
        node_id: String,
    ) {
        let event = WebSocketMessage::ActorActivated {
            actor_type,
            key,
            node_id,
        };
        self.ws_handler.broadcast_event(event).await;
    }

    /// Broadcast actor deactivation
    pub async fn actor_deactivated(&self, actor_type: String, key: serde_json::Value) {
        let event = WebSocketMessage::ActorDeactivated { actor_type, key };
        self.ws_handler.broadcast_event(event).await;
    }

    /// Broadcast transaction event
    pub async fn transaction_event(
        &self,
        transaction_id: String,
        status: String,
        message: Option<String>,
    ) {
        let event = WebSocketMessage::TransactionEvent {
            transaction_id,
            status,
            message,
        };
        self.ws_handler.broadcast_event(event).await;
    }

    /// Broadcast system event
    pub async fn system_event(&self, event_type: String, data: serde_json::Value) {
        let event = WebSocketMessage::SystemEvent { event_type, data };
        self.ws_handler.broadcast_event(event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_handler_new() {
        let handler = WebSocketHandler::new();
        // Just verify it can be created without panicking
        let _rx = handler.subscribe();
    }

    #[test]
    fn test_websocket_handler_default() {
        let handler = WebSocketHandler::default();
        let _rx = handler.subscribe();
    }

    #[tokio::test]
    async fn test_broadcast_event() {
        let handler = WebSocketHandler::new();
        let mut rx = handler.subscribe();

        let event = WebSocketMessage::SystemEvent {
            event_type: "test".to_string(),
            data: serde_json::json!({"message": "hello"}),
        };

        handler.broadcast_event(event.clone()).await;

        // Receive the event
        let received = rx.recv().await.unwrap();
        match received {
            WebSocketMessage::SystemEvent { event_type, data } => {
                assert_eq!(event_type, "test");
                assert_eq!(data["message"], "hello");
            }
            _ => panic!("Expected SystemEvent"),
        }
    }

    #[test]
    fn test_websocket_message_serialization() {
        // Test ActorStateChanged
        let msg = WebSocketMessage::ActorStateChanged {
            actor_type: "TestActor".to_string(),
            key: serde_json::json!("key-1"),
            state: serde_json::json!({"count": 42}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("actor_state_changed"));
        assert!(json.contains("TestActor"));

        // Test ActorActivated
        let msg = WebSocketMessage::ActorActivated {
            actor_type: "TestActor".to_string(),
            key: serde_json::json!("key-1"),
            node_id: "node-1".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("actor_activated"));
        assert!(json.contains("node-1"));

        // Test ActorDeactivated
        let msg = WebSocketMessage::ActorDeactivated {
            actor_type: "TestActor".to_string(),
            key: serde_json::json!("key-1"),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("actor_deactivated"));

        // Test TransactionEvent
        let msg = WebSocketMessage::TransactionEvent {
            transaction_id: "tx-123".to_string(),
            status: "committed".to_string(),
            message: Some("Success".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("transaction_event"));
        assert!(json.contains("tx-123"));

        // Test Error
        let msg = WebSocketMessage::Error {
            code: "ACTOR_NOT_FOUND".to_string(),
            message: "Actor not found".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("ACTOR_NOT_FOUND"));
    }

    #[test]
    fn test_subscribe_request_deserialization() {
        let json = r#"{"event_types": ["actor_state_changed", "actor_activated"], "filters": {"actor_type": "TestActor"}}"#;
        let request: SubscribeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.event_types.len(), 2);
        assert!(request.filters.is_some());
    }

    #[test]
    fn test_event_broadcaster() {
        let handler = Arc::new(WebSocketHandler::new());
        let broadcaster = EventBroadcaster::new(handler);
        // Just verify it can be created
        let _ = broadcaster;
    }

    #[tokio::test]
    async fn test_event_broadcaster_actor_state_changed() {
        let handler = Arc::new(WebSocketHandler::new());
        let mut rx = handler.subscribe();
        let broadcaster = EventBroadcaster::new(handler);

        broadcaster
            .actor_state_changed(
                "TestActor".to_string(),
                serde_json::json!("key-1"),
                serde_json::json!({"value": 100}),
            )
            .await;

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WebSocketMessage::ActorStateChanged { .. }));
    }

    #[tokio::test]
    async fn test_event_broadcaster_actor_lifecycle() {
        let handler = Arc::new(WebSocketHandler::new());
        let mut rx = handler.subscribe();
        let broadcaster = EventBroadcaster::new(handler);

        // Test activation
        broadcaster
            .actor_activated(
                "TestActor".to_string(),
                serde_json::json!("key-1"),
                "node-1".to_string(),
            )
            .await;

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WebSocketMessage::ActorActivated { .. }));

        // Test deactivation
        broadcaster
            .actor_deactivated("TestActor".to_string(), serde_json::json!("key-1"))
            .await;

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WebSocketMessage::ActorDeactivated { .. }));
    }

    #[tokio::test]
    async fn test_event_broadcaster_transaction_event() {
        let handler = Arc::new(WebSocketHandler::new());
        let mut rx = handler.subscribe();
        let broadcaster = EventBroadcaster::new(handler);

        broadcaster
            .transaction_event(
                "tx-123".to_string(),
                "committed".to_string(),
                Some("Success".to_string()),
            )
            .await;

        let received = rx.recv().await.unwrap();
        assert!(matches!(
            received,
            WebSocketMessage::TransactionEvent { .. }
        ));
    }

    #[tokio::test]
    async fn test_event_broadcaster_system_event() {
        let handler = Arc::new(WebSocketHandler::new());
        let mut rx = handler.subscribe();
        let broadcaster = EventBroadcaster::new(handler);

        broadcaster
            .system_event(
                "cluster_update".to_string(),
                serde_json::json!({"nodes": 3}),
            )
            .await;

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WebSocketMessage::SystemEvent { .. }));
    }
}
