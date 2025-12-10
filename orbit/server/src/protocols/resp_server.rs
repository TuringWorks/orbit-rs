//! RESP protocol server

use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

use super::resp::{types::RespValue, CommandHandler, RespCodec};
use crate::protocols::error::ProtocolResult;

use crate::protocols::tls::OrbitTlsAcceptor;

/// RESP protocol server
pub struct RespServer {
    bind_addr: String,
    command_handler: Arc<CommandHandler>,
    tls_acceptor: Option<OrbitTlsAcceptor>,
}

impl RespServer {
    /// Create a new RESP server
    pub fn new(bind_addr: impl Into<String>, orbit_client: orbit_client::OrbitClient) -> Self {
        Self::new_with_persistence(bind_addr, orbit_client, None)
    }

    /// Create a new RESP server with optional persistent storage
    pub fn new_with_persistence(
        bind_addr: impl Into<String>,
        orbit_client: orbit_client::OrbitClient,
        persistent_storage: Option<
            Arc<dyn crate::protocols::persistence::redis_data::RedisDataProvider>,
        >,
    ) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            command_handler: Arc::new(CommandHandler::new_with_persistence(
                orbit_client,
                persistent_storage,
            )),
            tls_acceptor: None,
        }
    }

    /// Enable TLS with the provided configuration
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
        // Load data from persistent storage on startup
        self.command_handler.load_from_persistence().await;

        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!("RESP server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("New RESP connection from {}", addr);
                    let handler = Arc::clone(&self.command_handler);
                    let tls_acceptor = self.tls_acceptor.clone();

                    tokio::spawn(async move {
                        // Handle TLS handshake if configured
                        if let Some(acceptor) = tls_acceptor {
                            match acceptor.accept(socket).await {
                                Ok(tls_stream) => {
                                    if let Err(e) =
                                        Self::handle_connection(tls_stream, handler).await
                                    {
                                        error!("Connection error: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("TLS handshake failed: {}", e);
                                }
                            }
                        } else {
                            if let Err(e) = Self::handle_connection(socket, handler).await {
                                error!("Connection error: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_connection<S>(socket: S, handler: Arc<CommandHandler>) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
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
    async fn process_message_result<S>(
        framed: &mut Framed<S, RespCodec>,
        handler: &Arc<CommandHandler>,
        result: Result<RespValue, crate::protocols::error::ProtocolError>,
    ) -> ProtocolResult<bool>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
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
    async fn send_response<S>(
        framed: &mut Framed<S, RespCodec>,
        response: RespValue,
    ) -> ProtocolResult<bool>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        if let Err(e) = framed.send(response).await {
            error!("Send error: {}", e);
            Ok(false) // Close connection on send error
        } else {
            Ok(true) // Continue processing
        }
    }
}
