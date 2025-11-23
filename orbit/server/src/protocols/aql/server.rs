//! AQL/ArangoDB server with RocksDB persistence

use crate::protocols::aql::storage::AqlStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

/// AQL/ArangoDB protocol server
pub struct AqlServer {
    bind_addr: String,
    #[allow(dead_code)]
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
        let listener = TcpListener::bind(&self.bind_addr).await.map_err(|e| {
            error!("Failed to bind AQL server to {}: {}", self.bind_addr, e);
            crate::protocols::error::ProtocolError::Other(format!(
                "Failed to bind AQL server: {}",
                e
            ))
        })?;

        info!("AQL/ArangoDB server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New AQL connection from {}", addr);
                    // TODO: Handle ArangoDB HTTP/WebSocket protocol
                    // For now, just accept and close
                    drop(stream);
                }
                Err(e) => {
                    error!("Error accepting AQL connection: {}", e);
                }
            }
        }
    }
}

