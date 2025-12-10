//! PostgreSQL TCP server

use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::{protocol::PostgresWireProtocol, query_engine::QueryEngine};
use crate::protocols::ProtocolError;

use crate::config::TlsConfig;
use crate::protocols::tls::OrbitTlsAcceptor;

/// PostgreSQL wire protocol server
pub struct PostgresServer {
    bind_addr: String,
    query_engine: Option<Arc<QueryEngine>>,
    tls_config: Option<TlsConfig>,
}

impl PostgresServer {
    /// Create a new PostgreSQL server
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            query_engine: None,
            tls_config: None,
        }
    }

    /// Create a new PostgreSQL server with custom query engine
    pub fn new_with_query_engine(bind_addr: impl Into<String>, query_engine: QueryEngine) -> Self {
        println!("DEBUG: PostgresServer initialized with custom query engine");
        Self {
            bind_addr: bind_addr.into(),
            query_engine: Some(Arc::new(query_engine)),
            tls_config: None,
        }
    }

    /// Set TLS configuration
    pub fn with_tls_config(mut self, tls_config: Option<TlsConfig>) -> Self {
        self.tls_config = tls_config;
        self
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        let tls_acceptor = OrbitTlsAcceptor::new(&self.tls_config).map_err(|e| {
             ProtocolError::IoError(e.to_string())
        })?;

        info!("PostgreSQL server listening on {}", self.bind_addr);
        if tls_acceptor.is_enabled() {
             info!("PostgreSQL TLS enabled");
        }

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);
                    let query_engine = self.query_engine.clone();
                    let tls_acceptor = tls_acceptor.clone();

                    tokio::spawn(async move {
                        let mut protocol = if let Some(engine) = query_engine {
                            PostgresWireProtocol::new_with_query_engine(engine)
                        } else {
                            PostgresWireProtocol::new()
                        };
                        
                        // Wrap stream with TLS if enabled
                        match tls_acceptor.accept(stream).await {
                             Ok(stream) => {
                                 if let Err(e) = protocol.handle_connection(stream).await {
                                     error!("Connection error: {}", e);
                                 }
                             }
                             Err(e) => {
                                 error!("TLS handshake error: {}", e);
                             }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

impl Default for PostgresServer {
    fn default() -> Self {
        Self::new("127.0.0.1:5432")
    }
}
