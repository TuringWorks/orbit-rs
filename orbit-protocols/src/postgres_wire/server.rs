//! PostgreSQL TCP server

use tokio::net::TcpListener;
use tracing::{error, info};

use super::protocol::PostgresWireProtocol;
use crate::error::ProtocolResult;

/// PostgreSQL wire protocol server
pub struct PostgresServer {
    bind_addr: String,
}

impl PostgresServer {
    /// Create a new PostgreSQL server
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
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
                    
                    tokio::spawn(async move {
                        let mut protocol = PostgresWireProtocol::new();
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
