//! Cypher/Bolt server with pluggable storage

#![cfg(feature = "storage-rocksdb")]

use crate::protocols::cypher::bolt_protocol::BoltProtocolHandler;
use crate::protocols::cypher::storage::CypherStorageProvider;
use crate::protocols::error::ProtocolResult;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::protocols::tls::OrbitTlsAcceptor;

/// Cypher/Bolt protocol server
pub struct CypherServer {
    bind_addr: String,
    storage: Arc<dyn CypherStorageProvider>,
    tls_acceptor: Option<OrbitTlsAcceptor>,
}

impl CypherServer {
    /// Create a new Cypher server with storage provider
    pub fn new_with_storage(
        bind_addr: impl Into<String>,
        storage: Arc<dyn CypherStorageProvider>,
    ) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            storage,
            tls_acceptor: None,
        }
    }

    pub fn with_tls_config(mut self, tls_config: Option<crate::config::TlsConfig>) -> Self {
        if tls_config.is_some() {
            self.tls_acceptor = Some(
                crate::protocols::tls::OrbitTlsAcceptor::new(&tls_config)
                    .expect("Invalid TLS configuration"),
            );
        }
        self
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
                    let tls_acceptor = self.tls_acceptor.clone();

                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let mut handler = BoltProtocolHandler::new(Some(storage));

                        if let Some(acceptor) = tls_acceptor {
                            match acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    if let Err(e) = handler.handle_connection(tls_stream).await {
                                        error!("Error handling Bolt TLS connection: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Cypher TLS handshake failed: {}", e);
                                }
                            }
                        } else if let Err(e) = handler.handle_connection(stream).await {
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
