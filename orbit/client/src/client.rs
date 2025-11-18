//! Orbit client implementation for connecting to and managing actors in a cluster

use crate::{
    ActorImplementation, ActorReference, ActorRegistry, DeactivationReason,
    DefaultActorConstructor, InvocationSystem,
};
use orbit_proto::{
    connection_service_client::ConnectionServiceClient, ConnectionInfoRequestProto, NodeIdConverter,
};
use orbit_shared::{Addressable, AddressableReference, Key, NodeId, OrbitError, OrbitResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
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
#[derive(Default)]
pub struct OrbitClientBuilder {
    config: OrbitClientConfig,
    offline_mode: bool,
}

impl OrbitClientBuilder {
    pub fn new() -> Self {
        Self {
            config: OrbitClientConfig::default(),
            offline_mode: false,
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

    pub fn with_offline_mode(mut self, offline: bool) -> Self {
        self.offline_mode = offline;
        self
    }

    pub async fn build(self) -> OrbitResult<OrbitClient> {
        if self.offline_mode {
            OrbitClient::new_offline(self.config).await
        } else {
            OrbitClient::new(self.config).await
        }
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

    /// Create a new Orbit client in offline mode (no server connections)
    pub async fn new_offline(config: OrbitClientConfig) -> OrbitResult<Self> {
        let registry = ActorRegistry::new();
        let connections = Arc::new(RwLock::new(HashMap::new()));
        let invocation_system = Arc::new(InvocationSystem::new());

        let client = Self {
            config,
            node_id: Some(NodeId {
                key: "offline-node".to_string(),
                namespace: "offline".to_string(),
            }),
            registry,
            connections,
            invocation_system,
        };

        // Start background tasks (but skip connection initialization)
        client.start_background_tasks().await;

        tracing::info!("OrbitClient initialized in offline mode");
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

        Ok(ActorReference::new(
            reference,
            self.invocation_system.clone(),
        ))
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
        let mut client = ConnectionServiceClient::new(channel.clone());

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
    use mockall::predicate::*;
    use tokio::time::Duration;

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
        assert_eq!(config.actor_timeout, Duration::from_secs(300));
        assert_eq!(config.connection_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_client_builder_all_options() {
        let urls = vec![
            "http://server1:50051".to_string(),
            "http://server2:50051".to_string(),
        ];

        let builder = OrbitClient::builder()
            .with_namespace("production")
            .with_server_urls(urls.clone())
            .with_connection_timeout(Duration::from_secs(15))
            .with_retry_attempts(5)
            .with_actor_timeout(Duration::from_secs(600));

        assert_eq!(builder.config.namespace, "production");
        assert_eq!(builder.config.server_urls, urls);
        assert_eq!(builder.config.connection_timeout, Duration::from_secs(15));
        assert_eq!(builder.config.retry_attempts, 5);
        assert_eq!(builder.config.actor_timeout, Duration::from_secs(600));
    }

    #[test]
    fn test_client_builder_default() {
        let builder = OrbitClientBuilder::default();
        assert_eq!(builder.config.namespace, "default");
    }

    #[test]
    fn test_client_config_clone_and_debug() {
        let config = OrbitClientConfig {
            namespace: "test".to_string(),
            server_urls: vec!["http://test:50051".to_string()],
            connection_timeout: Duration::from_secs(5),
            retry_attempts: 2,
            actor_timeout: Duration::from_secs(120),
        };

        let cloned = config.clone();
        assert_eq!(config.namespace, cloned.namespace);
        assert_eq!(config.server_urls, cloned.server_urls);
        assert_eq!(config.connection_timeout, cloned.connection_timeout);
        assert_eq!(config.retry_attempts, cloned.retry_attempts);
        assert_eq!(config.actor_timeout, cloned.actor_timeout);

        // Test Debug implementation
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("OrbitClientConfig"));
    }

    #[test]
    fn test_client_stats_debug() {
        let stats = ClientStats {
            namespace: "test".to_string(),
            server_connections: 3,
            node_id: Some(NodeId {
                key: "node-123".to_string(),
                namespace: "default".to_string(),
            }),
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("3"));
        assert!(debug_str.contains("node-123"));

        let cloned = stats.clone();
        assert_eq!(stats.namespace, cloned.namespace);
        assert_eq!(stats.server_connections, cloned.server_connections);
        assert_eq!(stats.node_id, cloned.node_id);
    }

    #[test]
    fn test_client_stats_without_node_id() {
        let stats = ClientStats {
            namespace: "test".to_string(),
            server_connections: 0,
            node_id: None,
        };

        assert_eq!(stats.namespace, "test");
        assert_eq!(stats.server_connections, 0);
        assert!(stats.node_id.is_none());
    }

    #[test]
    fn test_builder_fluent_interface() {
        // Test that builder methods can be chained
        let config = OrbitClient::builder()
            .with_namespace("chain-test")
            .with_retry_attempts(10)
            .with_connection_timeout(Duration::from_secs(30))
            .with_actor_timeout(Duration::from_secs(900))
            .config;

        assert_eq!(config.namespace, "chain-test");
        assert_eq!(config.retry_attempts, 10);
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
        assert_eq!(config.actor_timeout, Duration::from_secs(900));
    }

    #[test]
    fn test_config_edge_values() {
        let config = OrbitClientConfig {
            namespace: String::new(),                   // Empty namespace
            server_urls: vec![],                        // Empty server list
            connection_timeout: Duration::from_secs(0), // Zero timeout
            retry_attempts: 0,                          // No retries
            actor_timeout: Duration::from_millis(1),    // Very short timeout
        };

        assert_eq!(config.namespace, "");
        assert_eq!(config.server_urls.len(), 0);
        assert_eq!(config.connection_timeout, Duration::from_secs(0));
        assert_eq!(config.retry_attempts, 0);
        assert_eq!(config.actor_timeout, Duration::from_millis(1));
    }

    #[test]
    fn test_config_large_values() {
        let large_urls: Vec<String> = (0..1000)
            .map(|i| format!("http://server{}:50051", i))
            .collect();

        let config = OrbitClientConfig {
            namespace: "x".repeat(1000), // Very long namespace
            server_urls: large_urls.clone(),
            connection_timeout: Duration::from_secs(3600), // 1 hour
            retry_attempts: u32::MAX,                      // Maximum retries
            actor_timeout: Duration::from_secs(86400),     // 24 hours
        };

        assert_eq!(config.namespace.len(), 1000);
        assert_eq!(config.server_urls.len(), 1000);
        assert_eq!(config.server_urls, large_urls);
        assert_eq!(config.connection_timeout, Duration::from_secs(3600));
        assert_eq!(config.retry_attempts, u32::MAX);
        assert_eq!(config.actor_timeout, Duration::from_secs(86400));
    }

    #[test]
    fn test_string_conversions() {
        // Test with different string types
        let builder1 = OrbitClient::builder().with_namespace("str_slice");
        assert_eq!(builder1.config.namespace, "str_slice");

        let builder2 = OrbitClient::builder().with_namespace(String::from("owned_string"));
        assert_eq!(builder2.config.namespace, "owned_string");

        let namespace: &str = "borrowed";
        let builder3 = OrbitClient::builder().with_namespace(namespace);
        assert_eq!(builder3.config.namespace, "borrowed");
    }

    #[test]
    fn test_server_url_handling() {
        // Test various URL formats
        let urls = vec![
            "http://localhost:50051".to_string(),
            "https://secure.example.com:443".to_string(),
            "http://192.168.1.100:8080".to_string(),
            "grpc://service.internal:9090".to_string(),
        ];

        let config = OrbitClient::builder().with_server_urls(urls.clone()).config;

        assert_eq!(config.server_urls, urls);
    }

    #[test]
    fn test_duration_edge_cases() {
        // Test minimum duration
        let builder1 = OrbitClient::builder().with_connection_timeout(Duration::from_nanos(1));
        assert_eq!(builder1.config.connection_timeout, Duration::from_nanos(1));

        // Test maximum reasonable duration
        let builder2 =
            OrbitClient::builder().with_actor_timeout(Duration::from_secs(u64::MAX / 1000));
        // Should not panic, duration should be set
        assert!(builder2.config.actor_timeout > Duration::from_secs(0));
    }

    #[tokio::test]
    async fn test_builder_creates_different_instances() {
        // Verify that builder creates fresh instances each time
        let builder1 = OrbitClient::builder().with_namespace("first");
        let builder2 = OrbitClient::builder().with_namespace("second");

        assert_eq!(builder1.config.namespace, "first");
        assert_eq!(builder2.config.namespace, "second");
        assert_ne!(builder1.config.namespace, builder2.config.namespace);
    }

    // Note: Integration tests that require actual server connections
    // would be placed in a separate integration test module or
    // use mock servers. These are unit tests for the configuration
    // and builder pattern aspects that can run without external dependencies.

    #[test]
    fn test_memory_usage() {
        // Ensure config structures don't consume excessive memory
        let config_size = std::mem::size_of::<OrbitClientConfig>();
        let stats_size = std::mem::size_of::<ClientStats>();
        let builder_size = std::mem::size_of::<OrbitClientBuilder>();

        // These are reasonable limits - adjust if structures grow legitimately
        assert!(
            config_size < 1024,
            "OrbitClientConfig is too large: {} bytes",
            config_size
        );
        assert!(
            stats_size < 512,
            "ClientStats is too large: {} bytes",
            stats_size
        );
        assert!(
            builder_size < 1024,
            "OrbitClientBuilder is too large: {} bytes",
            builder_size
        );
    }
}
