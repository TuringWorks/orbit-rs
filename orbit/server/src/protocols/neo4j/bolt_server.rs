//! Bolt Server Implementation
//!
//! Handles incoming Bolt connections and executes Cypher queries.

// This module depends on types from orbit-protocols which needs to be added as a dependency
#![cfg(feature = "protocol-neo4j")]

use crate::protocols::cypher::cypher_parser::CypherParser;
use orbit_protocols::neo4j::bolt::{MessageType, PackStreamValue};
use orbit_protocols::neo4j::BoltProtocol;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

/// Bolt Server
pub struct BoltServer {
    bind_address: String,
    parser: Arc<CypherParser>,
}

impl BoltServer {
    /// Create a new Bolt server
    pub fn new(bind_address: String) -> Self {
        Self {
            bind_address,
            parser: Arc::new(CypherParser::new()),
        }
    }

    /// Start the server
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(&self.bind_address).await?;
        info!("⚡ Bolt server listening on {}", self.bind_address);

        loop {
            match listener.accept().await {
                Ok((mut socket, addr)) => {
                    info!("⚡ New Bolt connection from {}", addr);
                    let parser = self.parser.clone();

                    tokio::spawn(async move {
                        let mut protocol = BoltProtocol::new();

                        // 1. Handshake
                        if let Err(e) = protocol.handshake(&mut socket).await {
                            error!("Bolt handshake failed: {}", e);
                            return;
                        }

                        info!("⚡ Bolt handshake successful with {}", addr);

                        // Track pending query for PULL
                        let mut pending_query: Option<String> = None;

                        // 2. Message Loop
                        loop {
                            match protocol.read_message(&mut socket).await {
                                Ok((msg_type, data)) => {
                                    match msg_type {
                                        MessageType::Hello => {
                                            // Send SUCCESS with server info
                                            if let Err(e) =
                                                protocol.send_success(&mut socket, &[]).await
                                            {
                                                error!("Failed to send SUCCESS: {}", e);
                                                break;
                                            }
                                        }
                                        MessageType::Run => {
                                            // Parse RUN message to extract query
                                            match protocol.parse_run_message(&data) {
                                                Ok(run_msg) => {
                                                    debug!(
                                                        "⚡ Received Cypher query: {}",
                                                        run_msg.query
                                                    );
                                                    pending_query = Some(run_msg.query.clone());

                                                    // Parse and execute query
                                                    match parser.parse(&run_msg.query) {
                                                        Ok(_ast) => {
                                                            // Query parsed successfully
                                                            if let Err(e) = protocol
                                                                .send_success(&mut socket, &[])
                                                                .await
                                                            {
                                                                error!(
                                                                    "Failed to send SUCCESS: {}",
                                                                    e
                                                                );
                                                                break;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            warn!(
                                                                "Failed to parse Cypher query: {}",
                                                                e
                                                            );
                                                            if let Err(e) = protocol.send_failure(&mut socket, "Neo.ClientError.Statement.SyntaxError", &e.to_string()).await {
                                                                error!("Failed to send FAILURE: {}", e);
                                                                break;
                                                            }
                                                            pending_query = None;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("Failed to parse RUN message: {}", e);
                                                    if let Err(e) = protocol
                                                        .send_failure(
                                                            &mut socket,
                                                            "Neo.ClientError.Request.Invalid",
                                                            "Invalid RUN message",
                                                        )
                                                        .await
                                                    {
                                                        error!("Failed to send FAILURE: {}", e);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        MessageType::Pull => {
                                            // Return results for pending query
                                            if let Some(ref _query) = pending_query {
                                                // For this MVP, send sample data
                                                // In a real implementation, we'd execute the query
                                                // and stream back actual results

                                                // Example: Send a sample record
                                                let sample_record = vec![
                                                    PackStreamValue::String(
                                                        "sample_node".to_string(),
                                                    ),
                                                    PackStreamValue::Integer(42),
                                                ];

                                                if let Err(e) = protocol
                                                    .send_record(&mut socket, &sample_record)
                                                    .await
                                                {
                                                    error!("Failed to send RECORD: {}", e);
                                                    break;
                                                }

                                                // Send SUCCESS to indicate end of stream
                                                if let Err(e) =
                                                    protocol.send_success(&mut socket, &[]).await
                                                {
                                                    error!("Failed to send SUCCESS: {}", e);
                                                    break;
                                                }

                                                pending_query = None;
                                            } else {
                                                // No pending query
                                                if let Err(e) =
                                                    protocol.send_success(&mut socket, &[]).await
                                                {
                                                    error!("Failed to send SUCCESS: {}", e);
                                                    break;
                                                }
                                            }
                                        }
                                        MessageType::Discard => {
                                            // Discard pending results
                                            pending_query = None;
                                            if let Err(e) =
                                                protocol.send_success(&mut socket, &[]).await
                                            {
                                                error!("Failed to send SUCCESS: {}", e);
                                                break;
                                            }
                                        }
                                        MessageType::Reset => {
                                            // Reset connection state
                                            pending_query = None;
                                            if let Err(e) =
                                                protocol.send_success(&mut socket, &[]).await
                                            {
                                                error!("Failed to send SUCCESS: {}", e);
                                                break;
                                            }
                                        }
                                        MessageType::Begin => {
                                            // Begin transaction
                                            if let Err(e) =
                                                protocol.send_success(&mut socket, &[]).await
                                            {
                                                error!("Failed to send SUCCESS: {}", e);
                                                break;
                                            }
                                        }
                                        MessageType::Commit => {
                                            // Commit transaction
                                            if let Err(e) =
                                                protocol.send_success(&mut socket, &[]).await
                                            {
                                                error!("Failed to send SUCCESS: {}", e);
                                                break;
                                            }
                                        }
                                        MessageType::Rollback => {
                                            // Rollback transaction
                                            if let Err(e) =
                                                protocol.send_success(&mut socket, &[]).await
                                            {
                                                error!("Failed to send SUCCESS: {}", e);
                                                break;
                                            }
                                        }
                                        MessageType::Goodbye => {
                                            info!("⚡ Client {} said goodbye", addr);
                                            break;
                                        }
                                        _ => {
                                            warn!("Unhandled Bolt message type: {:?}", msg_type);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Bolt protocol error: {}", e);
                                    break;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept Bolt connection: {}", e);
                }
            }
        }
    }
}
