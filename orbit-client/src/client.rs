//! Orbit client implementation for connecting to and managing actors in a cluster

use crate::*;
use orbit_shared::*;
use orbit_proto::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tonic::transport::{Channel, Endpoint};

/// Configuration for the Orbit client
#[derive(Debug, Clone)]
pub struct OrbitClientConfig {
    pub namespace: String,
    pub server_urls: Vec<String>,
    pub connection_timeout: Duration,
    pub retry_attempts: u32,
    pub actor_timeout: Duration,
}

impl Default for OrbitClientConfig {
    fn default() -> Self {
        Self {
            namespace: "default".to_string(),
            server_urls: vec!["http://localhost:50051".to_string()],
            connection_timeout: Duration::from_secs(10),
            retry_attempts: 3,
            actor_timeout: Duration::from_secs(300),
        }
    }
}

/// Builder for configuring and creating an Orbit client
pub struct OrbitClientBuilder {
    config: OrbitClientConfig,
}

impl OrbitClientBuilder {
    pub fn new() -> Self {
        Self {
            config: OrbitClientConfig::default(),
        }
    }

    pub fn with_namespace<S: Into<String>>(mut self, namespace: S) -> Self {
        self.config.namespace = namespace.into();
        self
    }

    pub fn with_server_urls(mut self, urls: Vec<String>) -> Self {
        self.config.server_urls = urls;
        self
    }

    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.config.connection_timeout = timeout;
        self
    }

    pub fn with_retry_attempts(mut self, attempts: u32) -> Self {
        self.config.retry_attempts = attempts;
        self
    }

    pub fn with_actor_timeout(mut self, timeout: Duration) -> Self {
        self.config.actor_timeout = timeout;
        self
    }

    pub async fn build(self) -> OrbitResult<OrbitClient> {
        OrbitClient::new(self.config).await
    }
}

impl Default for OrbitClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Main Orbit client for interacting with the actor system
pub struct OrbitClient {
    config: OrbitClientConfig,
    node_id: Option<NodeId>,
    registry: ActorRegistry,
    connections: Arc<RwLock<HashMap<String, Channel>>>,
    invocation_system: Arc<InvocationSystem>,
}

impl OrbitClient {
    /// Create a new Orbit client with the given configuration
    pub async fn new(config: OrbitClientConfig) -> OrbitResult<Self> {
        let registry = ActorRegistry::new();
        let connections = Arc::new(RwLock::new(HashMap::new()));
        let invocation_system = Arc::new(InvocationSystem::new());

        let mut client = Self {
            config,
            node_id: None,
            registry,
            connections,
            invocation_system,
        };

        // Initialize connections to servers
        client.initialize_connections().await?;

        // Start background tasks
        client.start_background_tasks().await;

        Ok(client)
    }

    /// Get a builder for configuring a new client
    pub fn builder() -> OrbitClientBuilder {
        OrbitClientBuilder::new()
    }

    /// Get the client's node ID
    pub fn node_id(&self) -> Option<&NodeId> {
        self.node_id.as_ref()
    }

    /// Register an actor constructor for a specific actor type
    pub async fn register_actor<T, F>(&self, factory: F) -> OrbitResult<()>
    where
        T: Addressable + ActorImplementation + 'static,
        F: Fn() -> T + Send + Sync + 'static,
    {
        let constructor = Arc::new(DefaultActorConstructor::new(factory));
        self.registry.register_constructor::<T>(constructor).await;
        Ok(())
    }

    /// Get a reference to an actor
    pub async fn actor_reference<T>(&self, key: Key) -> OrbitResult<ActorReference<T>>
    where
        T: Addressable,
    {
        let reference = AddressableReference {
            addressable_type: T::addressable_type().to_string(),
            key,
        };

        Ok(ActorReference::new(reference, self.invocation_system.clone()))
    }

    /// Deactivate a specific actor instance
    pub async fn deactivate_actor(&self, reference: &AddressableReference) -> OrbitResult<()> {
        self.registry
            .deactivate_instance(reference, DeactivationReason::Explicit)
            .await
    }

    /// Get statistics about the client
    pub async fn stats(&self) -> OrbitResult<ClientStats> {
        let connections = self.connections.read().await;
        Ok(ClientStats {
            namespace: self.config.namespace.clone(),
            server_connections: connections.len(),
            node_id: self.node_id.clone(),
        })
    }

    /// Shutdown the client gracefully
    pub async fn shutdown(&self) -> OrbitResult<()> {
        // Deactivate all actor instances
        self.registry
            .cleanup_idle_instances(Duration::from_secs(0))
            .await?;

        tracing::info!("Orbit client shutdown completed");
        Ok(())
    }

    async fn initialize_connections(&mut self) -> OrbitResult<()> {
        let mut connections = self.connections.write().await;

        for url in &self.config.server_urls {
            match self.connect_to_server(url).await {
                Ok(channel) => {
                    connections.insert(url.clone(), channel);
                    tracing::info!("Connected to Orbit server at {}", url);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to server at {}: {}", url, e);
                }
            }
        }

        if connections.is_empty() {
            return Err(OrbitError::network("No server connections available"));
        }

        // Get node ID from one of the servers
        if let Some(channel) = connections.values().next() {
            self.node_id = Some(self.get_node_id_from_server(channel).await?);
        }

        Ok(())
    }

    async fn connect_to_server(&self, url: &str) -> OrbitResult<Channel> {
        let endpoint = Endpoint::from_shared(url.to_string())
            .map_err(|e| OrbitError::network(format!("Invalid endpoint: {}", e)))?
            .timeout(self.config.connection_timeout);

        endpoint
            .connect()
            .await
            .map_err(|e| OrbitError::network(format!("Connection failed: {}", e)))
    }

    async fn get_node_id_from_server(&self, channel: &Channel) -> OrbitResult<NodeId> {
        let mut client = connection_service_client::ConnectionServiceClient::new(channel.clone());
        
        let request = tonic::Request::new(ConnectionInfoRequestProto {});
        let response = client
            .get_connection_info(request)
            .await
            .map_err(|e| OrbitError::network(format!("Failed to get connection info: {}", e)))?;

        let node_id_proto = response
            .into_inner()
            .node_id
            .ok_or_else(|| OrbitError::internal("Missing node ID in response"))?;

        Ok(NodeIdConverter::from_proto(&node_id_proto))
    }

    async fn start_background_tasks(&self) {
        let registry = self.registry.clone();
        let actor_timeout = self.config.actor_timeout;

        // Actor cleanup task
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));
            loop {
                cleanup_interval.tick().await;
                if let Err(e) = registry.cleanup_idle_instances(actor_timeout).await {
                    tracing::warn!("Failed to cleanup idle actors: {}", e);
                }
            }
        });
    }
}

/// Statistics about the Orbit client
#[derive(Debug, Clone)]
pub struct ClientStats {
    pub namespace: String,
    pub server_connections: usize,
    pub node_id: Option<NodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_builder() {
        let builder = OrbitClient::builder()
            .with_namespace("test")
            .with_connection_timeout(Duration::from_secs(5));

        assert_eq!(builder.config.namespace, "test");
        assert_eq!(builder.config.connection_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_client_config_default() {
        let config = OrbitClientConfig::default();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.server_urls.len(), 1);
        assert_eq!(config.retry_attempts, 3);
    }
}