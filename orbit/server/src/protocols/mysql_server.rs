//! MySQL protocol server wrapper

use super::mysql::{adapter::MySqlAdapter, MySqlConfig};
use crate::protocols::common::storage::TableStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;

/// MySQL protocol server
use crate::config::TlsConfig;
use crate::protocols::tls::OrbitTlsAcceptor;

/// MySQL protocol server
pub struct MySqlServer {
    adapter: MySqlAdapter,
    tls_acceptor: Option<OrbitTlsAcceptor>,
}

impl MySqlServer {
    /// Create a new MySQL server with shared storage
    pub async fn new_with_storage(
        config: MySqlConfig,
        storage: Arc<dyn TableStorage>,
    ) -> ProtocolResult<Self> {
        let adapter = MySqlAdapter::new_with_storage(config, storage).await?;
        Ok(Self { adapter, tls_acceptor: None })
    }

    /// Create a new MySQL server (creates its own isolated storage)
    pub async fn new(config: MySqlConfig) -> ProtocolResult<Self> {
        let adapter = MySqlAdapter::new(config).await?;
        Ok(Self { adapter, tls_acceptor: None })
    }

    /// Enable TLS with the provided configuration
    pub fn with_tls_config(mut self, tls_config: Option<TlsConfig>) -> Self {
        if tls_config.is_some() {
            self.tls_acceptor = Some(OrbitTlsAcceptor::new(&tls_config).expect("Invalid TLS configuration"));
        }
        self
    }

    /// Start the MySQL server
    /// Start the MySQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        // We need to inject the TLS acceptor into the adapter's run loop or wrap the listener.
        // MySqlAdapter likely has its own run loop. Let's check if we can pass the acceptor to it.
        // For now, assuming MySqlAdapter needs modification to accept generic streams or an acceptor.
        // If MySqlAdapter just has a simple start(), we might need to modify it.
        self.adapter.start_with_tls(self.tls_acceptor.clone()).await
    }
}
