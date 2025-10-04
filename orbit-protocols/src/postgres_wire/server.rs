//! PostgreSQL TCP server

use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use super::{protocol::PostgresWireProtocol, query_engine::QueryEngine};
use crate::error::ProtocolResult;

/// PostgreSQL wire protocol server
pub struct PostgresServer {
    bind_addr: String,
    query_engine: Option<Arc<QueryEngine>>,
}

impl PostgresServer {
    /// Create a new PostgreSQL server
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            query_engine: None,
        }
    }

    /// Create a new PostgreSQL server with custom query engine
    pub fn new_with_query_engine(bind_addr: impl Into<String>, query_engine: QueryEngine) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            query_engine: Some(Arc::new(query_engine)),
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;

        info!("PostgreSQL server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);
                    let query_engine = self.query_engine.clone();

                    tokio::spawn(async move {
                        let mut protocol = if let Some(engine) = query_engine {
                            PostgresWireProtocol::new_with_query_engine(engine)
                        } else {
                            PostgresWireProtocol::new()
                        };
                        if let Err(e) = protocol.handle_connection(stream).await {
                            error!("Connection error: {}", e);
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
