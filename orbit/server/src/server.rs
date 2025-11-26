//! Main Orbit server implementation for hosting actors and managing the cluster

use crate::mesh::{AddressableDirectory, ClusterManager, ClusterStats, DirectoryStats};
use crate::persistence::config::PersistenceProviderConfig;
use crate::persistence::PersistenceProviderRegistry;
use crate::protocols::postgres_wire::QueryEngine;
#[cfg(feature = "storage-rocksdb")]
use crate::protocols::postgres_wire::RocksDbTableStorage;
use crate::protocols::{PostgresServer, RespServer};
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
use tonic_reflection::server::Builder as ReflectionBuilder;

/// Configuration for protocol servers
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    pub redis_enabled: bool,
    pub redis_port: u16,
    pub redis_bind_address: String,
    pub postgres_enabled: bool,
    pub postgres_port: u16,
    pub postgres_bind_address: String,
    pub mysql_enabled: bool,
    pub mysql_port: u16,
    pub mysql_bind_address: String,
    pub cql_enabled: bool,
    pub cql_port: u16,
    pub cql_bind_address: String,
    pub cypher_enabled: bool,
    pub cypher_port: u16,
    pub cypher_bind_address: String,
    pub aql_enabled: bool,
    pub aql_port: u16,
    pub aql_bind_address: String,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            redis_enabled: true,
            redis_port: 6379,
            redis_bind_address: "127.0.0.1".to_string(),
            postgres_enabled: true,
            postgres_port: 5432,
            postgres_bind_address: "127.0.0.1".to_string(),
            mysql_enabled: true,
            mysql_port: 3306,
            mysql_bind_address: "127.0.0.1".to_string(),
            cql_enabled: true,
            cql_port: 9042,
            cql_bind_address: "127.0.0.1".to_string(),
            cypher_enabled: true,
            cypher_port: 7687,
            cypher_bind_address: "127.0.0.1".to_string(),
            aql_enabled: true,
            aql_port: 8529,
            aql_bind_address: "127.0.0.1".to_string(),
        }
    }
}

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
    pub protocols: ProtocolConfig,
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
            protocols: ProtocolConfig::default(),
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

    /// Enable or disable Redis protocol server
    pub fn with_redis_enabled(mut self, enabled: bool) -> Self {
        self.config.protocols.redis_enabled = enabled;
        self
    }

    /// Configure Redis server port
    pub fn with_redis_port(mut self, port: u16) -> Self {
        self.config.protocols.redis_port = port;
        self
    }

    /// Configure Redis server bind address
    pub fn with_redis_bind_address<S: Into<String>>(mut self, address: S) -> Self {
        self.config.protocols.redis_bind_address = address.into();
        self
    }

    /// Enable or disable PostgreSQL protocol server
    pub fn with_postgres_enabled(mut self, enabled: bool) -> Self {
        self.config.protocols.postgres_enabled = enabled;
        self
    }

    /// Configure PostgreSQL server port
    pub fn with_postgres_port(mut self, port: u16) -> Self {
        self.config.protocols.postgres_port = port;
        self
    }

    /// Configure PostgreSQL server bind address
    pub fn with_postgres_bind_address<S: Into<String>>(mut self, address: S) -> Self {
        self.config.protocols.postgres_bind_address = address.into();
        self
    }

    /// Configure all protocol settings at once
    pub fn with_protocols(mut self, protocols: ProtocolConfig) -> Self {
        self.config.protocols = protocols;
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
    /// PostgreSQL wire protocol server (initialized at startup)
    postgres_server: Option<PostgresServer>,
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

        // PostgreSQL server can be initialized with persistent storage if available
        let postgres_server = if config.protocols.postgres_enabled {
            let postgres_addr = format!(
                "{}:{}",
                config.protocols.postgres_bind_address, config.protocols.postgres_port
            );

            // Try to create QueryEngine with persistent storage if RocksDB is available
            let query_engine = {
                #[cfg(feature = "storage-rocksdb")]
                {
                    // Check if we have a RocksDB provider configured
                    // The addressable provider is registered as "rocksdb_addressable"
                    match persistence_registry
                        .get_addressable_provider("rocksdb_addressable")
                        .await
                    {
                        Ok(_provider) => {
                            // Create a separate RocksDB instance for PostgreSQL table storage
                            // Use the same data directory structure as the server but with postgresql subdirectory
                            let pg_data_path = "./orbit_integrated_data/postgresql";

                            match RocksDbTableStorage::new(&pg_data_path) {
                                Ok(storage) => {
                                    tracing::info!(
                                        "PostgreSQL using persistent RocksDB storage at: {}",
                                        pg_data_path
                                    );
                                    QueryEngine::new_with_persistent_storage(Arc::new(storage))
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to create PostgreSQL persistent storage, falling back to memory: {}", e);
                                    QueryEngine::new()
                                }
                            }
                        }
                        Err(_) => {
                            tracing::info!("PostgreSQL using in-memory storage (no RocksDB persistence configured)");
                            QueryEngine::new()
                        }
                    }
                }
                #[cfg(not(feature = "storage-rocksdb"))]
                {
                    tracing::info!("PostgreSQL using in-memory storage (RocksDB support disabled)");
                    QueryEngine::new()
                }
            };

            Some(PostgresServer::new_with_query_engine(
                postgres_addr,
                query_engine,
            ))
        } else {
            None
        };

        // RESP server will be created after gRPC server starts

        let server = Self {
            config,
            node_info,
            cluster_manager,
            addressable_directory,
            load_balancer,
            connection_service,
            health_service,
            persistence_registry,
            postgres_server,
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

        // Create futures for all servers
        let mut server_tasks = Vec::new();

        // Start gRPC server (non-blocking)
        let grpc_addr = format!("{}:{}", self.config.bind_address, self.config.port)
            .parse()
            .map_err(|e| OrbitError::configuration(format!("Invalid bind address: {}", e)))?;

        tracing::info!("Starting Orbit gRPC server on {}", grpc_addr);

        // Build reflection service
        let reflection_service = ReflectionBuilder::configure()
            .register_encoded_file_descriptor_set(orbit_proto::FILE_DESCRIPTOR_SET)
            .build_v1()
            .map_err(|e| {
                OrbitError::configuration(format!("Failed to build reflection service: {}", e))
            })?;

        let grpc_server = Server::builder()
            .add_service(connection_service_server::ConnectionServiceServer::new(
                self.connection_service.clone(),
            ))
            .add_service(health_service_server::HealthServiceServer::new(
                self.health_service.clone(),
            ))
            .add_service(reflection_service)
            .serve(grpc_addr);

        server_tasks.push(tokio::spawn(async move {
            if let Err(e) = grpc_server.await {
                tracing::error!("gRPC server failed: {}", e);
            }
        }));

        // Give the gRPC server a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Start RESP server if enabled (after gRPC server is ready)
        if self.config.protocols.redis_enabled {
            let redis_addr = format!(
                "{}:{}",
                self.config.protocols.redis_bind_address, self.config.protocols.redis_port
            );
            tracing::info!("Starting RESP (Redis) server on {}", redis_addr);

            // Create RESP server without OrbitClient
            {
                // Create RocksDB storage for Redis persistence
                // Use consistent path structure: data/redis/rocksdb/ (matching other protocols)
                let redis_data_path = "./data/redis/rocksdb";

                #[cfg(feature = "storage-rocksdb")]
                    let redis_provider: Option<Arc<dyn crate::protocols::persistence::redis_data::RedisDataProvider>> =
                        match crate::protocols::persistence::rocksdb_redis_provider::RocksDbRedisDataProvider::new(
                            redis_data_path,
                            crate::protocols::persistence::redis_data::RedisDataConfig::default(),
                        ) {
                            Ok(provider) => {
                                tracing::info!("Redis using persistent RocksDB storage at: {}", redis_data_path);
                                let provider_arc: Arc<dyn crate::protocols::persistence::redis_data::RedisDataProvider> = Arc::new(provider);
                                // Initialize the provider
                                if let Err(e) = crate::protocols::persistence::redis_data::RedisDataProvider::initialize(&*provider_arc).await {
                                    tracing::warn!("Failed to initialize Redis persistent storage: {}", e);
                                    None
                                } else {
                                    Some(provider_arc)
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to create Redis persistent storage, using in-memory: {}", e);
                                None
                            }
                        };

                #[cfg(not(feature = "storage-rocksdb"))]
                let redis_provider: Option<
                    Arc<dyn crate::protocols::persistence::redis_data::RedisDataProvider>,
                > = {
                    tracing::info!("Redis using in-memory storage (RocksDB support disabled)");
                    None
                };

                let resp_server = RespServer::new_with_persistence(redis_addr, redis_provider);
                server_tasks.push(tokio::spawn(async move {
                    if let Err(e) = resp_server.run().await {
                        tracing::error!("RESP server failed: {}", e);
                    }
                }));
            }
        }

        // Start PostgreSQL server if enabled
        if let Some(postgres_server) = self.postgres_server.take() {
            let postgres_addr = format!(
                "{}:{}",
                self.config.protocols.postgres_bind_address, self.config.protocols.postgres_port
            );
            tracing::info!("Starting PostgreSQL server on {}", postgres_addr);

            server_tasks.push(tokio::spawn(async move {
                if let Err(e) = postgres_server.run().await {
                    tracing::error!("PostgreSQL server failed: {}", e);
                }
            }));
        }

        tracing::info!("All Orbit servers started successfully");
        tracing::info!(
            "  - gRPC: {}:{}",
            self.config.bind_address,
            self.config.port
        );
        if self.config.protocols.redis_enabled {
            tracing::info!(
                "  - Redis: {}:{}",
                self.config.protocols.redis_bind_address,
                self.config.protocols.redis_port
            );
        }
        if self.config.protocols.postgres_enabled {
            tracing::info!(
                "  - PostgreSQL: {}:{}",
                self.config.protocols.postgres_bind_address,
                self.config.protocols.postgres_port
            );
        }
        if self.config.protocols.mysql_enabled {
            tracing::info!(
                "  - MySQL: {}:{}",
                self.config.protocols.mysql_bind_address,
                self.config.protocols.mysql_port
            );
        }
        if self.config.protocols.cql_enabled {
            tracing::info!(
                "  - CQL/Cassandra: {}:{}",
                self.config.protocols.cql_bind_address,
                self.config.protocols.cql_port
            );
        }
        if self.config.protocols.cypher_enabled {
            tracing::info!(
                "  - Cypher/Neo4j: {}:{}",
                self.config.protocols.cypher_bind_address,
                self.config.protocols.cypher_port
            );
        }
        if self.config.protocols.aql_enabled {
            tracing::info!(
                "  - AQL/ArangoDB: {}:{}",
                self.config.protocols.aql_bind_address,
                self.config.protocols.aql_port
            );
        }

        // Wait for any server to finish (which likely means an error occurred)
        if !server_tasks.is_empty() {
            let (result, _index, _remaining) = futures::future::select_all(server_tasks).await;
            if let Err(e) = result {
                tracing::error!("One of the servers failed: {}", e);
                return Err(OrbitError::network(format!("Server task failed: {}", e)));
            }
        }

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

        let protocol_stats = ProtocolStats {
            redis_enabled: self.config.protocols.redis_enabled,
            redis_address: if self.config.protocols.redis_enabled {
                Some(format!(
                    "{}:{}",
                    self.config.protocols.redis_bind_address, self.config.protocols.redis_port
                ))
            } else {
                None
            },
            postgres_enabled: self.config.protocols.postgres_enabled,
            postgres_address: if self.config.protocols.postgres_enabled {
                Some(format!(
                    "{}:{}",
                    self.config.protocols.postgres_bind_address,
                    self.config.protocols.postgres_port
                ))
            } else {
                None
            },
            mysql_enabled: self.config.protocols.mysql_enabled,
            mysql_address: if self.config.protocols.mysql_enabled {
                Some(format!(
                    "{}:{}",
                    self.config.protocols.mysql_bind_address, self.config.protocols.mysql_port
                ))
            } else {
                None
            },
            cql_enabled: self.config.protocols.cql_enabled,
            cql_address: if self.config.protocols.cql_enabled {
                Some(format!(
                    "{}:{}",
                    self.config.protocols.cql_bind_address, self.config.protocols.cql_port
                ))
            } else {
                None
            },
            cypher_enabled: self.config.protocols.cypher_enabled,
            cypher_address: if self.config.protocols.cypher_enabled {
                Some(format!(
                    "{}:{}",
                    self.config.protocols.cypher_bind_address, self.config.protocols.cypher_port
                ))
            } else {
                None
            },
            aql_enabled: self.config.protocols.aql_enabled,
            aql_address: if self.config.protocols.aql_enabled {
                Some(format!(
                    "{}:{}",
                    self.config.protocols.aql_bind_address, self.config.protocols.aql_port
                ))
            } else {
                None
            },
        };

        Ok(ServerStats {
            node_id: self.node_info.id.clone(),
            cluster_stats,
            directory_stats,
            supported_types: self.node_info.capabilities.addressable_types.clone(),
            protocol_stats,
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

/// Information about protocol servers
#[derive(Debug, Clone)]
pub struct ProtocolStats {
    pub redis_enabled: bool,
    pub redis_address: Option<String>,
    pub postgres_enabled: bool,
    pub postgres_address: Option<String>,
    pub mysql_enabled: bool,
    pub mysql_address: Option<String>,
    pub cql_enabled: bool,
    pub cql_address: Option<String>,
    pub cypher_enabled: bool,
    pub cypher_address: Option<String>,
    pub aql_enabled: bool,
    pub aql_address: Option<String>,
}

/// Statistics about the Orbit server
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub node_id: NodeId,
    pub cluster_stats: ClusterStats,
    pub directory_stats: DirectoryStats,
    pub supported_types: Vec<String>,
    pub protocol_stats: ProtocolStats,
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
