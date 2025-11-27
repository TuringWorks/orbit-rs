//! MySQL protocol server wrapper

use super::mysql::{adapter::MySqlAdapter, MySqlConfig};
use crate::protocols::common::storage::TableStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;

/// MySQL protocol server
pub struct MySqlServer {
    adapter: MySqlAdapter,
}

impl MySqlServer {
    /// Create a new MySQL server with shared storage
    pub async fn new_with_storage(
        config: MySqlConfig,
        storage: Arc<dyn TableStorage>,
    ) -> ProtocolResult<Self> {
        let adapter = MySqlAdapter::new_with_storage(config, storage).await?;
        Ok(Self { adapter })
    }

    /// Create a new MySQL server (creates its own isolated storage)
    pub async fn new(config: MySqlConfig) -> ProtocolResult<Self> {
        let adapter = MySqlAdapter::new(config).await?;
        Ok(Self { adapter })
    }

    /// Start the MySQL server
    pub async fn start(&self) -> ProtocolResult<()> {
        self.adapter.start().await
    }
}
