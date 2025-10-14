//! Main Orbit server implementation for hosting actors and managing the cluster

use crate::mesh::{AddressableDirectory, ClusterManager, ClusterStats};
use crate::persistence::config::PersistenceProviderConfig;
use crate::persistence::PersistenceProviderRegistry;
use crate::LoadBalancer;
use orbit_proto::{
    connection_service_server, health_check_response, health_service_server,
    OrbitConnectionService, OrbitHealthService,
};
use orbit_shared::{NodeCapabilities, NodeId, NodeInfo, NodeStatus, OrbitError, OrbitResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tonic::transport::Server;

/// Configuration for the Orbit server
#[derive(Debug, Clone)]
pub struct OrbitServerConfig {
    pub namespace: String,
    pub bind_address: String,
    pub port: u16,
    pub lease_duration: Duration,
    pub cleanup_interval: Duration,
    pub max_addressables: Option<u32>,
    pub tags: HashMap<String, String>,
    pub persistence: PersistenceProviderConfig,
}

impl Default for OrbitServerConfig {
    fn default() -> Self {
        Self {
            namespace: "default".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 50051,
            lease_duration: Duration::from_secs(300), // 5 minutes
            cleanup_interval: Duration::from_secs(60), // 1 minute
            max_addressables: None,
            tags: HashMap::new(),
            persistence: PersistenceProviderConfig::default_memory(),
        }
    }
}

/// Builder for configuring and creating an Orbit server
pub struct OrbitServerBuilder {
    config: OrbitServerConfig,
}

impl OrbitServerBuilder {
    pub fn new() -> Self {
        Self {
            config: OrbitServerConfig::default(),
        }
    }

    pub fn with_namespace<S: Into<String>>(mut self, namespace: S) -> Self {
        self.config.namespace = namespace.into();
        self
    }

    pub fn with_bind_address<S: Into<String>>(mut self, address: S) -> Self {
        self.config.bind_address = address.into();
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    pub fn with_lease_duration(mut self, duration: Duration) -> Self {
        self.config.lease_duration = duration;
        self
    }

    pub fn with_max_addressables(mut self, max: u32) -> Self {
        self.config.max_addressables = Some(max);
        self
    }

    pub fn with_tag<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.config.tags.insert(key.into(), value.into());
        self
    }

    pub fn with_persistence(mut self, persistence: PersistenceProviderConfig) -> Self {
        self.config.persistence = persistence;
        self
    }

    pub async fn build(self) -> OrbitResult<OrbitServer> {
        OrbitServer::new(self.config).await
    }
}

impl Default for OrbitServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Main Orbit server for hosting actors and managing cluster membership
pub struct OrbitServer {
    config: OrbitServerConfig,
    node_info: NodeInfo,
    cluster_manager: ClusterManager,
    addressable_directory: AddressableDirectory,
    #[allow(dead_code)]
    load_balancer: LoadBalancer,
    connection_service: OrbitConnectionService,
    health_service: OrbitHealthService,
    #[allow(dead_code)]
    persistence_registry: Arc<PersistenceProviderRegistry>,
}

impl OrbitServer {
    /// Create a new Orbit server with the given configuration
    pub async fn new(config: OrbitServerConfig) -> OrbitResult<Self> {
        let node_id = NodeId::generate(config.namespace.clone());

        let capabilities = NodeCapabilities {
            addressable_types: vec![], // Will be populated as actors are registered
            max_addressables: config.max_addressables,
            tags: config.tags.clone(),
        };

        let node_info = NodeInfo {
            id: node_id,
            url: config.bind_address.clone(),
            port: config.port,
            capabilities,
            status: NodeStatus::Active,
            lease: None,
        };

        // Initialize persistence provider registry
        let persistence_registry = Arc::new(config.persistence.create_registry().await?);

        let cluster_manager = ClusterManager::new(config.lease_duration);
        let addressable_directory = AddressableDirectory::new();
        let load_balancer = LoadBalancer::new();
        let connection_service = OrbitConnectionService::new();
        let health_service = OrbitHealthService::new();

        let server = Self {
            config,
            node_info,
            cluster_manager,
            addressable_directory,
            load_balancer,
            connection_service,
            health_service,
            persistence_registry,
        };

        Ok(server)
    }

    /// Get a builder for configuring a new server
    pub fn builder() -> OrbitServerBuilder {
        OrbitServerBuilder::new()
    }

    /// Start the server and begin accepting connections
    pub async fn start(&mut self) -> OrbitResult<()> {
        // Register this node in the cluster
        self.cluster_manager
            .register_node(self.node_info.clone())
            .await?;

        // Start background tasks
        self.start_background_tasks().await;

        // Start gRPC server
        let addr = format!("{}:{}", self.config.bind_address, self.config.port)
            .parse()
            .map_err(|e| OrbitError::configuration(format!("Invalid bind address: {}", e)))?;

        tracing::info!("Starting Orbit server on {}", addr);

        Server::builder()
            .add_service(connection_service_server::ConnectionServiceServer::new(
                self.connection_service.clone(),
            ))
            .add_service(health_service_server::HealthServiceServer::new(
                self.health_service.clone(),
            ))
            .serve(addr)
            .await
            .map_err(|e| OrbitError::network(format!("Server failed: {}", e)))?;

        Ok(())
    }

    /// Get the node information for this server
    pub fn node_info(&self) -> &NodeInfo {
        &self.node_info
    }

    /// Register an addressable type that this server can host
    pub async fn register_addressable_type(&mut self, addressable_type: String) -> OrbitResult<()> {
        if !self
            .node_info
            .capabilities
            .addressable_types
            .contains(&addressable_type)
        {
            self.node_info
                .capabilities
                .addressable_types
                .push(addressable_type);

            // Update cluster with new capabilities
            self.cluster_manager
                .update_node(self.node_info.clone())
                .await?;
        }
        Ok(())
    }

    /// Get server statistics
    pub async fn stats(&self) -> OrbitResult<ServerStats> {
        let cluster_stats = self.cluster_manager.stats().await;
        let directory_stats = self.addressable_directory.stats().await;

        Ok(ServerStats {
            node_id: self.node_info.id.clone(),
            cluster_stats,
            directory_stats,
            supported_types: self.node_info.capabilities.addressable_types.clone(),
        })
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&self) -> OrbitResult<()> {
        // Set health status to not serving
        self.health_service
            .set_serving_status(health_check_response::ServingStatus::NotServing)
            .await;

        // Remove from cluster
        self.cluster_manager.remove_node(&self.node_info.id).await?;

        tracing::info!("Orbit server shutdown completed");
        Ok(())
    }

    async fn start_background_tasks(&self) {
        let cluster_manager = self.cluster_manager.clone();
        let addressable_directory = self.addressable_directory.clone();
        let cleanup_interval = self.config.cleanup_interval;

        // Cluster and directory cleanup task
        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);
            loop {
                interval.tick().await;

                if let Err(e) = cluster_manager.cleanup_expired_nodes().await {
                    tracing::warn!("Failed to cleanup expired nodes: {}", e);
                }

                if let Err(e) = addressable_directory.cleanup_expired_leases().await {
                    tracing::warn!("Failed to cleanup expired leases: {}", e);
                }
            }
        });

        // Start cluster manager background tasks
        self.cluster_manager.start_background_tasks().await;
    }
}

/// Statistics about the Orbit server
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub node_id: NodeId,
    pub cluster_stats: ClusterStats,
    pub directory_stats: DirectoryStats,
    pub supported_types: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = OrbitServerConfig::default();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.port, 50051);
        assert_eq!(config.lease_duration, Duration::from_secs(300));
    }

    #[test]
    fn test_server_builder() {
        let builder = OrbitServer::builder()
            .with_namespace("test")
            .with_port(8080)
            .with_tag("environment", "test");

        assert_eq!(builder.config.namespace, "test");
        assert_eq!(builder.config.port, 8080);
        assert_eq!(
            builder.config.tags.get("environment"),
            Some(&"test".to_string())
        );
    }

    #[tokio::test]
    async fn test_server_creation() {
        let server = OrbitServer::new(OrbitServerConfig::default())
            .await
            .unwrap();
        assert_eq!(server.node_info.port, 50051);
        assert_eq!(server.node_info.status, NodeStatus::Active);
    }
}
