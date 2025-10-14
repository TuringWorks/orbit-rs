//! Connection-related RESP commands
//!
//! Handles basic Redis connection commands like PING, ECHO, SELECT, AUTH, QUIT

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::{error::ProtocolResult, resp::RespValue};
use async_trait::async_trait;
use orbit_client::OrbitClient;
use std::sync::Arc;

/// Handler for connection commands
pub struct ConnectionCommands {
    base: BaseCommandHandler,
}

impl ConnectionCommands {
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        // Connection commands don't need local registry
        let local_registry = Arc::new(crate::resp::simple_local::SimpleLocalRegistry::new());
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    async fn cmd_ping(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            Ok(RespValue::simple_string("PONG"))
        } else {
            Ok(args[0].clone())
        }
    }

    async fn cmd_echo(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ECHO", args, 1)?;
        Ok(args[0].clone())
    }

    async fn cmd_select(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SELECT", args, 1)?;
        // Redis database selection - we'll just accept it and return OK
        Ok(RespValue::ok())
    }

    async fn cmd_auth(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // AUTH command can have 1 or 2 arguments:
        // AUTH password
        // AUTH username password (Redis 6.0+)
        self.validate_arg_count_range("AUTH", args, 1, 2)?;

        let (_username, _password) = if args.len() == 1 {
            // Single argument: password only (default user)
            let password = self.get_string_arg(args, 0, "AUTH")?;
            ("default".to_string(), password)
        } else {
            // Two arguments: username and password
            let username = self.get_string_arg(args, 0, "AUTH")?;
            let password = self.get_string_arg(args, 1, "AUTH")?;
            (username, password)
        };

        // TODO: Implement actual authentication logic
        // For now, we'll just accept any authentication
        Ok(RespValue::ok())
    }

    async fn cmd_quit(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if !args.is_empty() {
            return Err(crate::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'quit' command".to_string(),
            ));
        }
        Ok(RespValue::ok())
    }
}

#[async_trait]
impl CommandHandler for ConnectionCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name {
            "PING" => self.cmd_ping(args).await,
            "ECHO" => self.cmd_echo(args).await,
            "SELECT" => self.cmd_select(args).await,
            "AUTH" => self.cmd_auth(args).await,
            "QUIT" => self.cmd_quit(args).await,
            _ => Err(crate::error::ProtocolError::RespError(format!(
                "ERR unknown connection command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["PING", "ECHO", "SELECT", "AUTH", "QUIT"]
    }
}
