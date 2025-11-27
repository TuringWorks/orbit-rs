//! RESP protocol server

use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

use super::resp::{types::RespValue, CommandHandler, RespCodec};
use crate::protocols::error::ProtocolResult;

/// RESP protocol server
pub struct RespServer {
    bind_addr: String,
    command_handler: Arc<CommandHandler>,
}

impl RespServer {
    /// Create a new RESP server
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self::new_with_persistence(bind_addr, None)
    }

    /// Create a new RESP server with optional persistent storage
    pub fn new_with_persistence(
        bind_addr: impl Into<String>,
        persistent_storage: Option<
            Arc<dyn crate::protocols::persistence::redis_data::RedisDataProvider>,
        >,
    ) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            command_handler: Arc::new(CommandHandler::new_with_persistence(persistent_storage)),
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        // Load data from persistent storage on startup
        self.command_handler.load_from_persistence().await;

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
            if !Self::process_message_result(&mut framed, &handler, result).await? {
                break;
            }
        }

        debug!("Connection closed");
        Ok(())
    }

    /// Process a single message result, returns false if connection should be closed
    async fn process_message_result(
        framed: &mut Framed<TcpStream, RespCodec>,
        handler: &Arc<CommandHandler>,
        result: Result<RespValue, crate::protocols::error::ProtocolError>,
    ) -> ProtocolResult<bool> {
        match result {
            Ok(command) => {
                debug!("Received command: {}", command);
                let response = Self::handle_command_with_error_handling(handler, command).await;
                Self::send_response(framed, response).await
            }
            Err(e) => {
                error!("Protocol error: {}", e);
                Ok(false) // Close connection on protocol error
            }
        }
    }

    /// Handle command with proper error handling
    async fn handle_command_with_error_handling(
        handler: &Arc<CommandHandler>,
        command: RespValue,
    ) -> RespValue {
        handler.handle_command(command).await.unwrap_or_else(|e| {
            error!("Command error: {}", e);
            RespValue::error(format!("ERR {e}"))
        })
    }

    /// Send response and handle send errors
    async fn send_response(
        framed: &mut Framed<TcpStream, RespCodec>,
        response: RespValue,
    ) -> ProtocolResult<bool> {
        if let Err(e) = framed.send(response).await {
            error!("Send error: {}", e);
            Ok(false) // Close connection on send error
        } else {
            Ok(true) // Continue processing
        }
    }
}
