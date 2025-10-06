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
            let _ = sender.send(Message::Text(json)).await;
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
            let _ = sender.send(Message::Text(json)).await;
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
