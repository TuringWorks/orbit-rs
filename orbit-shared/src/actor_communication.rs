use crate::addressable::{AddressableReference, AddressableInvocation};
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{Duration, Instant};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Message types for actor-to-actor communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorMessage {
    /// Direct method invocation on another actor
    Invoke {
        target: AddressableReference,
        invocation: AddressableInvocation,
        reply_to: Option<ActorMessageId>,
    },
    /// Response to a previous invocation
    Response {
        request_id: ActorMessageId,
        result: Result<serde_json::Value, String>,
    },
    /// Fire-and-forget message
    Tell {
        target: AddressableReference,
        message: serde_json::Value,
        message_type: String,
    },
    /// Actor lifecycle events
    LifecycleEvent {
        actor: AddressableReference,
        event: ActorLifecycleEvent,
    },
    /// Heartbeat for actor health monitoring
    Heartbeat {
        actor: AddressableReference,
        timestamp: i64,
        metrics: ActorMetrics,
    },
}

/// Unique identifier for actor messages
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorMessageId {
    pub id: String,
    pub sender: AddressableReference,
    pub timestamp: i64,
}

impl ActorMessageId {
    pub fn new(sender: AddressableReference) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Actor lifecycle events for communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorLifecycleEvent {
    Activated { node_id: NodeId },
    Deactivated { reason: String },
    Migrated { from_node: NodeId, to_node: NodeId },
    Failed { error: String },
    Recovered { from_failure: String },
}

/// Actor performance and health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorMetrics {
    pub messages_processed: u64,
    pub average_processing_time_ms: f64,
    pub memory_usage_bytes: u64,
    pub last_activity: i64,
    pub error_count: u32,
    pub active_connections: u32,
}

impl Default for ActorMetrics {
    fn default() -> Self {
        Self {
            messages_processed: 0,
            average_processing_time_ms: 0.0,
            memory_usage_bytes: 0,
            last_activity: chrono::Utc::now().timestamp_millis(),
            error_count: 0,
            active_connections: 0,
        }
    }
}

/// Configuration for actor communication system
#[derive(Debug, Clone)]
pub struct ActorCommunicationConfig {
    pub message_timeout: Duration,
    pub retry_attempts: u32,
    pub batch_size: usize,
    pub heartbeat_interval: Duration,
    pub discovery_interval: Duration,
    pub max_pending_messages: usize,
}

impl Default for ActorCommunicationConfig {
    fn default() -> Self {
        Self {
            message_timeout: Duration::from_secs(30),
            retry_attempts: 3,
            batch_size: 100,
            heartbeat_interval: Duration::from_secs(10),
            discovery_interval: Duration::from_secs(5),
            max_pending_messages: 10000,
        }
    }
}

/// Actor discovery and routing service
#[derive(Debug)]
pub struct ActorDiscoveryService {
    config: ActorCommunicationConfig,
    /// Maps actor references to their current locations
    actor_locations: Arc<RwLock<HashMap<AddressableReference, NodeId>>>,
    /// Maps nodes to the actors they host
    node_actors: Arc<RwLock<HashMap<NodeId, Vec<AddressableReference>>>>,
    /// Pending discovery requests
    pending_discoveries: Arc<RwLock<HashMap<AddressableReference, Vec<oneshot::Sender<Option<NodeId>>>>>>,
    /// Local actor registry
    local_actors: Arc<RwLock<HashMap<AddressableReference, ActorHandle>>>,
}

/// Handle to communicate with a local actor
#[derive(Debug)]
pub struct ActorHandle {
    pub reference: AddressableReference,
    pub sender: mpsc::UnboundedSender<ActorMessage>,
    pub metrics: Arc<RwLock<ActorMetrics>>,
    pub created_at: Instant,
    pub last_accessed: Arc<RwLock<Instant>>,
}

impl ActorDiscoveryService {
    pub fn new(config: ActorCommunicationConfig) -> Self {
        Self {
            config,
            actor_locations: Arc::new(RwLock::new(HashMap::new())),
            node_actors: Arc::new(RwLock::new(HashMap::new())),
            pending_discoveries: Arc::new(RwLock::new(HashMap::new())),
            local_actors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a local actor
    pub async fn register_local_actor(
        &self, 
        reference: AddressableReference,
        sender: mpsc::UnboundedSender<ActorMessage>
    ) -> OrbitResult<()> {
        let handle = ActorHandle {
            reference: reference.clone(),
            sender,
            metrics: Arc::new(RwLock::new(ActorMetrics::default())),
            created_at: Instant::now(),
            last_accessed: Arc::new(RwLock::new(Instant::now())),
        };

        let mut local_actors = self.local_actors.write().await;
        local_actors.insert(reference.clone(), handle);

        info!("Registered local actor: {}", reference);
        Ok(())
    }

    /// Unregister a local actor
    pub async fn unregister_local_actor(&self, reference: &AddressableReference) -> OrbitResult<()> {
        let mut local_actors = self.local_actors.write().await;
        if local_actors.remove(reference).is_some() {
            info!("Unregistered local actor: {}", reference);
        }
        Ok(())
    }

    /// Discover the location of an actor
    pub async fn discover_actor(&self, reference: &AddressableReference) -> OrbitResult<Option<NodeId>> {
        // Check if we know the location
        {
            let locations = self.actor_locations.read().await;
            if let Some(node_id) = locations.get(reference) {
                debug!("Found actor {} on node {}", reference, node_id);
                return Ok(Some(node_id.clone()));
            }
        }

        // Check if it's a local actor
        {
            let local_actors = self.local_actors.read().await;
            if local_actors.contains_key(reference) {
                // Return local node ID (would be provided by the system)
                debug!("Actor {} is local", reference);
                return Ok(None); // None indicates local
            }
        }

        // Actor not found locally or in cache
        debug!("Actor {} not found, initiating discovery", reference);
        self.initiate_actor_discovery(reference).await
    }

    /// Initiate discovery process for an actor
    async fn initiate_actor_discovery(&self, reference: &AddressableReference) -> OrbitResult<Option<NodeId>> {
        let (sender, receiver) = oneshot::channel();
        
        // Add to pending discoveries
        {
            let mut pending = self.pending_discoveries.write().await;
            pending.entry(reference.clone()).or_insert_with(Vec::new).push(sender);
        }

        // In a real implementation, this would send discovery requests to other nodes
        // For now, we'll simulate with a timeout
        match tokio::time::timeout(self.config.message_timeout, receiver).await {
            Ok(result) => result.map_err(|_| OrbitError::internal("Discovery channel closed")),
            Err(_) => {
                // Remove from pending discoveries on timeout
                let mut pending = self.pending_discoveries.write().await;
                pending.remove(reference);
                Ok(None)
            }
        }
    }

    /// Update actor location information
    pub async fn update_actor_location(&self, reference: AddressableReference, node_id: NodeId) -> OrbitResult<()> {
        // Update location mapping
        {
            let mut locations = self.actor_locations.write().await;
            locations.insert(reference.clone(), node_id.clone());
        }

        // Update node actors mapping
        {
            let mut node_actors = self.node_actors.write().await;
            node_actors.entry(node_id.clone()).or_insert_with(Vec::new).push(reference.clone());
        }

        // Notify any pending discoveries
        {
            let mut pending = self.pending_discoveries.write().await;
            if let Some(waiters) = pending.remove(&reference) {
                for waiter in waiters {
                    let _ = waiter.send(Some(node_id.clone()));
                }
            }
        }

        debug!("Updated actor {} location to node {}", reference, node_id);
        Ok(())
    }

    /// Remove actor from location tracking
    pub async fn remove_actor_location(&self, reference: &AddressableReference) -> OrbitResult<()> {
        let node_id = {
            let mut locations = self.actor_locations.write().await;
            locations.remove(reference)
        };

        if let Some(node_id) = node_id {
            let mut node_actors = self.node_actors.write().await;
            if let Some(actors) = node_actors.get_mut(&node_id) {
                actors.retain(|actor| actor != reference);
                if actors.is_empty() {
                    node_actors.remove(&node_id);
                }
            }
        }

        debug!("Removed actor {} from location tracking", reference);
        Ok(())
    }

    /// Get all actors on a specific node
    pub async fn get_node_actors(&self, node_id: &NodeId) -> Vec<AddressableReference> {
        let node_actors = self.node_actors.read().await;
        node_actors.get(node_id).cloned().unwrap_or_default()
    }

    /// Get all known actor locations
    pub async fn get_all_locations(&self) -> HashMap<AddressableReference, NodeId> {
        let locations = self.actor_locations.read().await;
        locations.clone()
    }

    /// Start background services for actor discovery and health monitoring
    pub async fn start_background_services(&self) -> OrbitResult<()> {
        let discovery_service = Arc::new(self.clone());
        
        // Start heartbeat service
        let heartbeat_service = discovery_service.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_service.config.heartbeat_interval);
            loop {
                interval.tick().await;
                if let Err(e) = heartbeat_service.send_heartbeats().await {
                    error!("Heartbeat service error: {}", e);
                }
            }
        });

        // Start cleanup service
        let cleanup_service = discovery_service.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_service.config.discovery_interval);
            loop {
                interval.tick().await;
                if let Err(e) = cleanup_service.cleanup_stale_actors().await {
                    error!("Cleanup service error: {}", e);
                }
            }
        });

        info!("Actor discovery background services started");
        Ok(())
    }

    /// Send heartbeat messages for all local actors
    async fn send_heartbeats(&self) -> OrbitResult<()> {
        let local_actors = self.local_actors.read().await;
        
        for (reference, handle) in local_actors.iter() {
            let metrics = handle.metrics.read().await;
            let _heartbeat = ActorMessage::Heartbeat {
                actor: reference.clone(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                metrics: metrics.clone(),
            };
            
            // In a real implementation, this would be sent to the cluster
            debug!("Sending heartbeat for actor: {}", reference);
        }
        
        Ok(())
    }

    /// Clean up actors that haven't been accessed recently
    async fn cleanup_stale_actors(&self) -> OrbitResult<()> {
        let stale_threshold = Instant::now() - Duration::from_secs(300); // 5 minutes
        let mut to_remove = Vec::new();

        {
            let local_actors = self.local_actors.read().await;
            for (reference, handle) in local_actors.iter() {
                let last_accessed = *handle.last_accessed.read().await;
                if last_accessed < stale_threshold {
                    to_remove.push(reference.clone());
                }
            }
        }

        for reference in to_remove {
            warn!("Removing stale actor: {}", reference);
            self.unregister_local_actor(&reference).await?;
        }

        Ok(())
    }
}

impl Clone for ActorDiscoveryService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            actor_locations: Arc::clone(&self.actor_locations),
            node_actors: Arc::clone(&self.node_actors),
            pending_discoveries: Arc::clone(&self.pending_discoveries),
            local_actors: Arc::clone(&self.local_actors),
        }
    }
}

/// Message router for actor-to-actor communication
#[derive(Debug)]
pub struct ActorMessageRouter {
    discovery_service: Arc<ActorDiscoveryService>,
    config: ActorCommunicationConfig,
    /// Pending responses waiting for replies
    pending_responses: Arc<RwLock<HashMap<ActorMessageId, oneshot::Sender<Result<serde_json::Value, String>>>>>,
}

impl ActorMessageRouter {
    pub fn new(discovery_service: Arc<ActorDiscoveryService>, config: ActorCommunicationConfig) -> Self {
        Self {
            discovery_service,
            config,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Send a message to another actor
    pub async fn send_message(&self, message: ActorMessage) -> OrbitResult<()> {
        match &message {
            ActorMessage::Invoke { target, .. } | 
            ActorMessage::Tell { target, .. } => {
                self.route_to_actor(target, message.clone()).await
            },
            ActorMessage::Response { request_id, .. } => {
                self.handle_response(request_id.clone(), message).await
            },
            ActorMessage::LifecycleEvent { actor, .. } => {
                self.broadcast_lifecycle_event(actor, message.clone()).await
            },
            ActorMessage::Heartbeat { .. } => {
                // Heartbeats are handled by the discovery service
                Ok(())
            },
        }
    }

    /// Send a message and wait for a response
    pub async fn send_and_wait(
        &self, 
        target: AddressableReference,
        invocation: AddressableInvocation,
        timeout: Duration
    ) -> OrbitResult<serde_json::Value> {
        let message_id = ActorMessageId::new(invocation.reference.clone());
        let (sender, receiver) = oneshot::channel();

        // Store pending response
        {
            let mut pending = self.pending_responses.write().await;
            pending.insert(message_id.clone(), sender);
        }

        // Send the message
        let message = ActorMessage::Invoke {
            target: target.clone(),
            invocation,
            reply_to: Some(message_id.clone()),
        };

        self.send_message(message).await?;

        // Wait for response with timeout
        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(result)) => result.map_err(|e| OrbitError::internal(e)),
            Ok(Err(_)) => Err(OrbitError::internal("Response channel closed")),
            Err(_) => {
                // Clean up pending response on timeout
                let mut pending = self.pending_responses.write().await;
                pending.remove(&message_id);
                Err(OrbitError::timeout("send_and_wait"))
            }
        }
    }

    /// Route a message to the appropriate actor
    async fn route_to_actor(&self, target: &AddressableReference, message: ActorMessage) -> OrbitResult<()> {
        // Try to find the actor locally first
        {
            let local_actors = self.discovery_service.local_actors.read().await;
            if let Some(handle) = local_actors.get(target) {
                if let Err(e) = handle.sender.send(message) {
                    error!("Failed to send message to local actor {}: {}", target, e);
                    return Err(OrbitError::internal(format!("Local actor send failed: {}", e)));
                }
                
                // Update last accessed time
                {
                    let mut last_accessed = handle.last_accessed.write().await;
                    *last_accessed = Instant::now();
                }
                
                return Ok(());
            }
        }

        // Actor not local, discover its location
        match self.discovery_service.discover_actor(target).await? {
            Some(node_id) => {
                // Forward message to the remote node
                info!("Forwarding message to actor {} on node {}", target, node_id);
                self.forward_to_node(&node_id, message).await
            },
            None => {
                warn!("Actor {} not found in cluster", target);
                Err(OrbitError::AddressableNotFound { 
                    reference: target.to_string() 
                })
            }
        }
    }

    /// Forward a message to a remote node
    async fn forward_to_node(&self, _node_id: &NodeId, _message: ActorMessage) -> OrbitResult<()> {
        // In a real implementation, this would use the network layer to send the message
        // to the appropriate node via gRPC or other transport
        debug!("Forwarding message to node: {}", _node_id);
        
        // For now, we'll simulate successful forwarding
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(())
    }

    /// Handle a response message
    async fn handle_response(&self, request_id: ActorMessageId, message: ActorMessage) -> OrbitResult<()> {
        if let ActorMessage::Response { result, .. } = message {
            let mut pending = self.pending_responses.write().await;
            if let Some(sender) = pending.remove(&request_id) {
                if let Err(_) = sender.send(result) {
                    warn!("Failed to deliver response for request: {:?}", request_id);
                }
            } else {
                warn!("Received response for unknown request: {:?}", request_id);
            }
        }
        Ok(())
    }

    /// Broadcast a lifecycle event to interested parties
    async fn broadcast_lifecycle_event(&self, _actor: &AddressableReference, _message: ActorMessage) -> OrbitResult<()> {
        // In a real implementation, this would notify subscribers to actor lifecycle events
        debug!("Broadcasting lifecycle event for actor: {}", _actor);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressable::{AddressableReference, Key};

    #[tokio::test]
    async fn test_actor_discovery_service() {
        let config = ActorCommunicationConfig::default();
        let discovery = ActorDiscoveryService::new(config);
        
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey { key: "test-1".to_string() },
        };
        
        let (sender, _) = mpsc::unbounded_channel();
        
        // Register local actor
        discovery.register_local_actor(reference.clone(), sender).await.unwrap();
        
        // Discover should find it locally
        let result = discovery.discover_actor(&reference).await.unwrap();
        assert_eq!(result, None); // None indicates local
    }

    #[tokio::test]
    async fn test_actor_message_id() {
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey { key: "test-1".to_string() },
        };
        
        let id1 = ActorMessageId::new(reference.clone());
        let id2 = ActorMessageId::new(reference);
        
        assert_ne!(id1.id, id2.id); // Should be unique
        assert_eq!(id1.sender, id2.sender); // Same sender
    }

    #[tokio::test]
    async fn test_message_router() {
        let config = ActorCommunicationConfig::default();
        let discovery = Arc::new(ActorDiscoveryService::new(config.clone()));
        let router = ActorMessageRouter::new(discovery.clone(), config);
        
        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey { key: "test-1".to_string() },
        };
        
        let (sender, mut receiver) = mpsc::unbounded_channel();
        discovery.register_local_actor(reference.clone(), sender).await.unwrap();
        
        // Send a tell message
        let message = ActorMessage::Tell {
            target: reference,
            message: serde_json::json!({"test": "value"}),
            message_type: "TestMessage".to_string(),
        };
        
        router.send_message(message.clone()).await.unwrap();
        
        // Should receive the message
        let received = receiver.recv().await.unwrap();
        match received {
            ActorMessage::Tell { message_type, .. } => {
                assert_eq!(message_type, "TestMessage");
            },
            _ => panic!("Unexpected message type"),
        }
    }
}