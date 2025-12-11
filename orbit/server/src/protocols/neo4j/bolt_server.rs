//! Bolt Server Implementation
//!
//! TCP server that handles incoming Bolt protocol connections and delegates
//! message handling to BoltMessageHandler.

#![cfg(feature = "protocol-neo4j")]

use crate::protocols::cypher::graph_engine::GraphEngine;
use crate::protocols::cypher::storage::CypherGraphStorage;
use crate::protocols::graph_database::PersistentGraphStorage;
use crate::protocols::neo4j::bolt_messages::{BoltConnectionState, BoltMessageHandler, BoltProtocolWriter};
use crate::protocols::neo4j::bolt_writer::BoltWriter;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Bolt Server
pub struct BoltServer {
    bind_address: String,
    engine: Arc<GraphEngine<PersistentGraphStorage>>,
    storage: Arc<CypherGraphStorage>,
}

impl BoltServer {
    /// Create a new Bolt server
    pub fn new(bind_address: String) -> Self {
        // Use a dedicated directory for Neo4j Bolt server data
        let data_dir = PathBuf::from("./data/neo4j_bolt");
        let storage = Arc::new(CypherGraphStorage::new(data_dir));
        let persistent_storage = Arc::new(PersistentGraphStorage::new(storage.clone()));
        let engine = Arc::new(GraphEngine::new(Some(persistent_storage)));

        Self {
            bind_address,
            engine,
            storage,
        }
    }

    /// Start the server
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        // Initialize storage
        self.storage.initialize().await?;

        let listener = TcpListener::bind(&self.bind_address).await?;
        info!("⚡ Bolt server listening on {}", self.bind_address);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!("⚡ New Bolt connection from {}", addr);
                    let engine = self.engine.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, engine).await {
                            error!("Error handling Bolt connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }
}

/// Handle a single Bolt connection
async fn handle_connection(
    mut socket: TcpStream,
    engine: Arc<GraphEngine<PersistentGraphStorage>>,
) -> Result<(), Box<dyn Error>> {
    // 1. Perform Bolt handshake
    let version = perform_handshake(&mut socket).await?;
    debug!("Bolt handshake completed, version: 0x{:04X}", version);

    // 2. Initialize connection state
    let connection_id = Uuid::new_v4().to_string();
    let mut state = BoltConnectionState::new(connection_id);
    let handler = BoltMessageHandler::new(engine);
    let mut writer = BoltWriter::new();

    // 3. Message loop
    loop {
        // Read message
        let message_data = match read_message(&mut socket).await {
            Ok(data) => data,
            Err(e) => {
                debug!("Connection closed or error reading message: {}", e);
                break;
            }
        };

        if message_data.is_empty() {
            debug!("Empty message, closing connection");
            break;
        }

        // Parse message signature
        let signature = extract_signature(&message_data)?;

        // Handle message based on signature
        let should_continue = match signature {
            0x01 => {
                // HELLO
                let (user_agent, auth_token, routing) = parse_hello(&message_data)?;
                handler
                    .handle_hello(&mut state, user_agent, auth_token, routing, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x02 => {
                // GOODBYE
                handler
                    .handle_goodbye(&mut state, &mut socket, &mut writer)
                    .await?
            }
            0x06 => {
                // LOGON
                let auth_token = parse_logon(&message_data)?;
                handler
                    .handle_logon(&mut state, auth_token, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x07 => {
                // LOGOFF
                handler
                    .handle_logoff(&mut state, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x10 => {
                // RUN
                let (query, parameters, extra) = parse_run(&message_data)?;
                handler
                    .handle_run(&mut state, query, parameters, extra, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x2F => {
                // DISCARD
                let (n, qid) = parse_discard(&message_data)?;
                handler
                    .handle_discard(&mut state, n, qid, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x3F => {
                // PULL
                let (n, qid) = parse_pull(&message_data)?;
                handler
                    .handle_pull(&mut state, n, qid, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x11 => {
                // BEGIN
                let extra = parse_begin(&message_data)?;
                handler
                    .handle_begin(&mut state, extra, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x12 => {
                // COMMIT
                handler
                    .handle_commit(&mut state, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x13 => {
                // ROLLBACK
                handler
                    .handle_rollback(&mut state, &mut socket, &mut writer)
                    .await?;
                true
            }
            0x0F => {
                // RESET
                handler
                    .handle_reset(&mut state, &mut socket, &mut writer)
                    .await?;
                true
            }
            _ => {
                error!("Unknown message signature: 0x{:02X}", signature);
                writer.send_ignored(&mut socket).await?;
                true
            }
        };

        if !should_continue {
            break;
        }
    }

    Ok(())
}

/// Perform Bolt handshake
async fn perform_handshake(socket: &mut TcpStream) -> Result<u32, Box<dyn Error>> {
    // Read handshake (20 bytes: 4 magic + 4 version proposals)
    let mut handshake_buf = [0u8; 20];
    socket.read_exact(&mut handshake_buf).await?;

    // Verify magic bytes
    let magic = u32::from_be_bytes([
        handshake_buf[0],
        handshake_buf[1],
        handshake_buf[2],
        handshake_buf[3],
    ]);

    if magic != 0x6060B017 {
        return Err("Invalid Bolt handshake magic".into());
    }

    // Find highest supported version
    let mut selected_version = 0x0404u32; // Default to Bolt 4.4

    for i in 0..4 {
        let version = u32::from_be_bytes([
            handshake_buf[4 + i * 4],
            handshake_buf[5 + i * 4],
            handshake_buf[6 + i * 4],
            handshake_buf[7 + i * 4],
        ]);

        if version == 0 {
            break;
        }

        // Accept versions 4.0 through 4.4
        if (0x0400..=0x0404).contains(&version) {
            selected_version = selected_version.max(version);
        }
    }

    // Send selected version
    socket.write_all(&selected_version.to_be_bytes()).await?;
    socket.flush().await?;

    Ok(selected_version)
}

/// Read a complete Bolt message (handles chunking)
async fn read_message(socket: &mut TcpStream) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut message_data = Vec::new();

    loop {
        // Read chunk size (2 bytes)
        let mut size_buf = [0u8; 2];
        socket.read_exact(&mut size_buf).await?;
        let chunk_size = u16::from_be_bytes(size_buf) as usize;

        if chunk_size == 0 {
            // End of message
            break;
        }

        // Read chunk data
        let mut chunk = vec![0u8; chunk_size];
        socket.read_exact(&mut chunk).await?;
        message_data.extend_from_slice(&chunk);
    }

    Ok(message_data)
}

/// Extract message signature from message data
fn extract_signature(data: &[u8]) -> Result<u8, Box<dyn Error>> {
    if data.len() < 2 {
        return Err("Message too short".into());
    }

    // First byte should be structure marker (0xB0-0xBF or 0xDC)
    // Second byte is the signature
    Ok(data[1])
}

/// Parse HELLO message
fn parse_hello(
    _data: &[u8],
) -> Result<(String, std::collections::HashMap<String, serde_json::Value>, Option<std::collections::HashMap<String, serde_json::Value>>), Box<dyn Error>> {
    // TODO: Implement proper PackStream parsing
    // For now, return defaults
    Ok((
        "Orbit-Client/1.0".to_string(),
        std::collections::HashMap::new(),
        None,
    ))
}

/// Parse LOGON message
fn parse_logon(_data: &[u8]) -> Result<std::collections::HashMap<String, serde_json::Value>, Box<dyn Error>> {
    // TODO: Implement proper PackStream parsing
    Ok(std::collections::HashMap::new())
}

/// Parse RUN message
fn parse_run(
    _data: &[u8],
) -> Result<(String, std::collections::HashMap<String, serde_json::Value>, std::collections::HashMap<String, serde_json::Value>), Box<dyn Error>> {
    // TODO: Implement proper PackStream parsing
    Ok((
        "RETURN 1".to_string(),
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
    ))
}

/// Parse PULL message
fn parse_pull(_data: &[u8]) -> Result<(Option<i64>, Option<i64>), Box<dyn Error>> {
    // TODO: Implement proper PackStream parsing
    Ok((Some(-1), None))
}

/// Parse DISCARD message
fn parse_discard(_data: &[u8]) -> Result<(Option<i64>, Option<i64>), Box<dyn Error>> {
    // TODO: Implement proper PackStream parsing
    Ok((Some(-1), None))
}

/// Parse BEGIN message
fn parse_begin(_data: &[u8]) -> Result<std::collections::HashMap<String, serde_json::Value>, Box<dyn Error>> {
    // TODO: Implement proper PackStream parsing
    Ok(std::collections::HashMap::new())
}
