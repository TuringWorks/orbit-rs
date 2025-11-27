//! Cypher/Bolt server with RocksDB persistence

#![cfg(feature = "storage-rocksdb")]

use crate::protocols::cypher::bolt_protocol::BoltProtocolHandler;
use crate::protocols::cypher::storage::CypherGraphStorage;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Cypher/Bolt protocol server
pub struct CypherServer {
    bind_addr: String,
    storage: Arc<CypherGraphStorage>,
}

impl CypherServer {
    /// Create a new Cypher server with storage
    pub fn new_with_storage(
        bind_addr: impl Into<String>,
        storage: Arc<CypherGraphStorage>,
    ) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            storage,
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await.map_err(|e| {
            error!("Failed to bind Cypher server to {}: {}", self.bind_addr, e);
            crate::protocols::error::ProtocolError::Other(format!(
                "Failed to bind Cypher server: {}",
                e
            ))
        })?;

        info!("Cypher/Bolt server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New Cypher/Bolt connection from {}", addr);

                    let storage = self.storage.clone();

                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let mut handler = BoltProtocolHandler::new(storage);
                        if let Err(e) = handler.handle_connection(stream).await {
                            error!("Error handling Bolt connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting Cypher connection: {}", e);
                }
            }
        }
    }
}
