//! Cypher/Bolt server (stub)

use crate::error::ProtocolResult;
use orbit_client::OrbitClient;

/// Cypher/Bolt protocol server
pub struct CypherServer {
    bind_addr: String,
    _orbit_client: OrbitClient,
}

impl CypherServer {
    /// Create a new Cypher server
    pub fn new(bind_addr: impl Into<String>, orbit_client: OrbitClient) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            _orbit_client: orbit_client,
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        tracing::info!("Cypher/Bolt server would listen on {}", self.bind_addr);
        // TODO: Implement full Bolt protocol server
        Ok(())
    }
}
