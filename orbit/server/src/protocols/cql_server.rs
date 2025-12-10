//! CQL protocol server wrapper

use super::cql::{adapter::CqlAdapter, CqlConfig};
use crate::protocols::common::storage::TableStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;

use crate::config::TlsConfig;
use crate::protocols::tls::OrbitTlsAcceptor;

/// CQL protocol server
pub struct CqlServer {
    adapter: CqlAdapter,
    tls_acceptor: Option<OrbitTlsAcceptor>,
}

impl CqlServer {
    /// Create a new CQL server with shared storage
    pub async fn new_with_storage(
        config: CqlConfig,
        storage: Arc<dyn TableStorage>,
    ) -> ProtocolResult<Self> {
        let adapter = CqlAdapter::new_with_storage(config, storage).await?;
        Ok(Self {
            adapter,
            tls_acceptor: None,
        })
    }

    /// Create a new CQL server (creates its own isolated storage)
    pub async fn new(config: CqlConfig) -> ProtocolResult<Self> {
        let adapter = CqlAdapter::new(config).await?;
        Ok(Self {
            adapter,
            tls_acceptor: None,
        })
    }

    /// Enable TLS with the provided configuration
    pub fn with_tls_config(mut self, tls_config: Option<TlsConfig>) -> Self {
        if tls_config.is_some() {
            self.tls_acceptor =
                Some(OrbitTlsAcceptor::new(&tls_config).expect("Invalid TLS configuration"));
        }
        self
    }

    /// Start the CQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        self.adapter.start_with_tls(self.tls_acceptor.clone()).await
    }
}
