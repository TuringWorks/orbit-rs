//! AQL/ArangoDB server with RocksDB persistence

use crate::protocols::aql::http_server::AqlHttpServer;
use crate::protocols::aql::storage::AqlStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;
use tracing::info;

/// AQL/ArangoDB protocol server
pub struct AqlServer {
    bind_addr: String,
    storage: Arc<AqlStorage>,
}

impl AqlServer {
    /// Create a new AQL server with storage
    pub fn new_with_storage(bind_addr: impl Into<String>, storage: Arc<AqlStorage>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            storage,
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        info!("Starting AQL/ArangoDB HTTP server on {}", self.bind_addr);
        let http_server = AqlHttpServer::new(&self.bind_addr, self.storage.clone());
        http_server.run().await
    }
}

