use crate::{config::EtcdConfig, EtcdResult};
use etcd_client::Client;
use std::sync::Arc;
use tracing::info;

/// High-level etcd client wrapper for Orbit
pub struct EtcdClient {
    #[allow(dead_code)]
    inner: Arc<Option<Client>>, // Placeholder inner client (not used yet)
    config: EtcdConfig,
}

impl EtcdClient {
    /// Create a new etcd client
    pub async fn new(config: EtcdConfig) -> EtcdResult<Self> {
        // For now, don't actually connect to a real etcd (keeps compile/runtime simple)
        // In a future step, we'll wire up real connections based on config
        info!(
            "Initializing EtcdClient with endpoints: {:?}",
            config.connection.endpoints
        );
        Ok(Self {
            inner: Arc::new(None),
            config,
        })
    }

    /// Register a service instance (placeholder)
    pub async fn register_service(&self, _service: &str, _addr: &str) -> EtcdResult<()> {
        Ok(())
    }

    /// Discover service instances (placeholder)
    pub async fn discover_services(&self, _service: &str) -> EtcdResult<Vec<String>> {
        Ok(vec![])
    }

    /// Get underlying configuration
    pub fn config(&self) -> &EtcdConfig {
        &self.config
    }
}
