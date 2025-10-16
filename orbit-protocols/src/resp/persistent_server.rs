//! Persistent RESP protocol server using RedisDataProvider
//!
//! This server implementation uses the RedisDataProvider for persistent storage
//! instead of the ephemeral actor system, enabling true data persistence across restarts.

use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

use super::{types::RespValue, RespCodec};
use crate::error::{ProtocolError, ProtocolResult};
use crate::persistence::redis_data::RedisDataProvider;
use crate::resp::commands::string_persistent::PersistentStringCommands;
use crate::resp::commands::traits::CommandHandler as CommandHandlerTrait;
use crate::resp::commands::CommandHandler;
use orbit_client::{OrbitClient, OrbitClientConfig};

/// Persistent RESP protocol server using RedisDataProvider for storage
pub struct PersistentRespServer {
    bind_addr: String,
    command_handler: Arc<CommandHandler>,
    // Keep persistent string commands for backward compatibility with persistent operations
    string_commands: Arc<PersistentStringCommands>,
}

impl PersistentRespServer {
    /// Create a new persistent RESP server
    pub fn new(
        bind_addr: impl Into<String>,
        orbit_client: Arc<OrbitClient>,
        redis_provider: Arc<dyn RedisDataProvider>,
    ) -> Self {
        // Create a new OrbitClient for the command handler with default config
        // Since we can't clone OrbitClient, we'll create a new one
        let command_handler_client = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                OrbitClient::new(OrbitClientConfig::default())
                    .await
                    .expect("Failed to create OrbitClient for command handler")
            })
        });

        Self {
            bind_addr: bind_addr.into(),
            command_handler: Arc::new(CommandHandler::new(command_handler_client)),
            string_commands: Arc::new(PersistentStringCommands::new(orbit_client, redis_provider)),
        }
    }

    /// Start the server
    pub async fn run(&self) -> ProtocolResult<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!("Persistent RESP server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("New persistent RESP connection from {}", addr);
                    let command_handler = Arc::clone(&self.command_handler);
                    let string_commands = Arc::clone(&self.string_commands);
                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_connection(socket, (command_handler, string_commands))
                                .await
                        {
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
        commands: (Arc<CommandHandler>, Arc<PersistentStringCommands>),
    ) -> ProtocolResult<()> {
        let mut framed = Framed::new(socket, RespCodec::new());

        while let Some(result) = framed.next().await {
            if !Self::process_message_result(&mut framed, &commands, result).await? {
                break;
            }
        }

        debug!("Persistent connection closed");
        Ok(())
    }

    /// Process a single message result, returns false if connection should be closed
    async fn process_message_result(
        framed: &mut Framed<TcpStream, RespCodec>,
        commands: &(Arc<CommandHandler>, Arc<PersistentStringCommands>),
        result: Result<RespValue, crate::error::ProtocolError>,
    ) -> ProtocolResult<bool> {
        match result {
            Ok(command) => {
                debug!("Received command: {}", command);
                let response = Self::handle_command_with_error_handling(commands, command).await;
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
        commands: &(Arc<CommandHandler>, Arc<PersistentStringCommands>),
        command: RespValue,
    ) -> RespValue {
        let (command_handler, string_commands) = commands;
        match Self::parse_command(command) {
            Ok((command_name, args)) => {
                // List of commands that should use persistent string commands for true persistence
                let persistent_string_commands = [
                    "GET", "SET", "DEL", "EXISTS", "TTL", "EXPIRE", "KEYS", "SETEX", "PERSIST",
                    "PEXPIRE", "PTTL", "TYPE",
                ];

                if persistent_string_commands.contains(&command_name.as_str()) {
                    // Use persistent string commands for true persistence with RocksDB
                    string_commands
                        .handle(&command_name, &args)
                        .await
                        .unwrap_or_else(|e| {
                            error!("Persistent string command error: {}", e);
                            RespValue::error(format!("ERR {e}"))
                        })
                } else {
                    // Use full command handler for all other commands (hash, list, set, etc.)
                    command_handler
                        .handle_command(RespValue::Array({
                            let mut cmd_args = vec![RespValue::BulkString(
                                command_name.as_bytes().to_vec().into(),
                            )];
                            cmd_args.extend(args);
                            cmd_args
                        }))
                        .await
                        .unwrap_or_else(|e| {
                            error!("Command handler error: {}", e);
                            RespValue::error(format!("ERR {e}"))
                        })
                }
            }
            Err(e) => {
                error!("Command parse error: {}", e);
                RespValue::error(format!("ERR {e}"))
            }
        }
    }

    /// Parse command and extract name and arguments
    fn parse_command(command: RespValue) -> ProtocolResult<(String, Vec<RespValue>)> {
        let args = match command {
            RespValue::Array(args) => args,
            _ => {
                return Err(ProtocolError::RespError(
                    "Command must be an array".to_string(),
                ))
            }
        };

        if args.is_empty() {
            return Err(ProtocolError::RespError("Empty command".to_string()));
        }

        let command_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("Command name must be a string".to_string()))?
            .to_uppercase();

        Ok((command_name, args[1..].to_vec()))
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
