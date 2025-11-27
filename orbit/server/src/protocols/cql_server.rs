//! CQL protocol server wrapper

use super::cql::{adapter::CqlAdapter, CqlConfig};
use crate::protocols::common::storage::TableStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;

/// CQL protocol server
pub struct CqlServer {
    adapter: CqlAdapter,
}

impl CqlServer {
    /// Create a new CQL server with shared storage
    pub async fn new_with_storage(
        config: CqlConfig,
        storage: Arc<dyn TableStorage>,
    ) -> ProtocolResult<Self> {
        let adapter = CqlAdapter::new_with_storage(config, storage).await?;
        Ok(Self { adapter })
    }

    /// Create a new CQL server (creates its own isolated storage)
    pub async fn new(config: CqlConfig) -> ProtocolResult<Self> {
        let adapter = CqlAdapter::new(config).await?;
        Ok(Self { adapter })
    }

    /// Start the CQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        self.adapter.start().await
    }
}
