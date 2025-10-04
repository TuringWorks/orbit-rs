//! RESP protocol server

use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

use super::{types::RespValue, CommandHandler, RespCodec};
use crate::error::ProtocolResult;
use orbit_client::OrbitClient;

/// RESP protocol server
pub struct RespServer {
    bind_addr: String,
    command_handler: Arc<CommandHandler>,
}

impl RespServer {
    /// Create a new RESP server
    pub fn new(bind_addr: impl Into<String>, orbit_client: OrbitClient) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            command_handler: Arc::new(CommandHandler::new(orbit_client)),
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!("RESP server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("New RESP connection from {}", addr);
                    let handler = Arc::clone(&self.command_handler);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(socket, handler).await {
                            error!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        socket: TcpStream,
        handler: Arc<CommandHandler>,
    ) -> ProtocolResult<()> {
        let mut framed = Framed::new(socket, RespCodec::new());

        while let Some(result) = framed.next().await {
            match result {
                Ok(command) => {
                    debug!("Received command: {}", command);

                    let response = handler.handle_command(command).await.unwrap_or_else(|e| {
                        error!("Command error: {}", e);
                        RespValue::error(format!("ERR {}", e))
                    });

                    if let Err(e) = framed.send(response).await {
                        error!("Send error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Protocol error: {}", e);
                    break;
                }
            }
        }

        debug!("Connection closed");
        Ok(())
    }
}
