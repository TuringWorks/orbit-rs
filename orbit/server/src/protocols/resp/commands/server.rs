//! Server-related RESP commands
//!
//! Handles Redis server commands like INFO, DBSIZE, FLUSHDB, FLUSHALL, COMMAND

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::{error::ProtocolResult, resp::RespValue};
use async_trait::async_trait;
use orbit_client::OrbitClient;
use std::sync::Arc;

/// Handler for server commands
pub struct ServerCommands {
    #[allow(dead_code)] // Used by trait methods but compiler doesn't detect it
    base: BaseCommandHandler,
}

impl ServerCommands {
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        let local_registry = Arc::new(crate::protocols::resp::simple_local::SimpleLocalRegistry::new());
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    async fn cmd_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // INFO command can have 0 or 1 argument (section name)
        // For now, we'll return basic server information
        let section = if args.is_empty() {
            None
        } else {
            Some(self.get_string_arg(args, 0, "INFO")?)
        };

        // Build basic INFO response
        let mut info_lines = Vec::new();
        
        match section.as_deref() {
            Some("server") | None => {
                info_lines.push("# Server");
                info_lines.push("redis_version:0.1.0-orbit");
                info_lines.push("redis_mode:standalone");
                info_lines.push("os:unknown");
                info_lines.push("arch_bits:64");
            }
            Some("clients") => {
                info_lines.push("# Clients");
                info_lines.push("connected_clients:0");
            }
            Some("memory") => {
                info_lines.push("# Memory");
                info_lines.push("used_memory:0");
                info_lines.push("used_memory_human:0B");
            }
            Some("stats") => {
                info_lines.push("# Stats");
                info_lines.push("total_commands_processed:0");
            }
            Some("replication") => {
                info_lines.push("# Replication");
                info_lines.push("role:master");
            }
            Some(_) => {
                // Unknown section, return empty
            }
        }

        Ok(RespValue::bulk_string(info_lines.join("\r\n")))
    }

    async fn cmd_dbsize(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // DBSIZE returns the number of keys in the current database
        // For now, we'll return 0 as we don't have a way to count all keys efficiently
        // TODO: Implement actual key counting if needed
        Ok(RespValue::Integer(0))
    }

    async fn cmd_flushdb(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // FLUSHDB removes all keys from the current database
        // For now, we'll just return OK
        // TODO: Implement actual database flushing if needed
        Ok(RespValue::ok())
    }

    async fn cmd_flushall(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // FLUSHALL removes all keys from all databases
        // For now, we'll just return OK
        // TODO: Implement actual database flushing if needed
        Ok(RespValue::ok())
    }

    async fn cmd_command(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // COMMAND returns information about available commands
        // For now, return an empty array
        // TODO: Return actual command information if needed
        Ok(RespValue::Array(vec![]))
    }
}

#[async_trait]
impl CommandHandler for ServerCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name {
            "INFO" => self.cmd_info(args).await,
            "DBSIZE" => self.cmd_dbsize(args).await,
            "FLUSHDB" => self.cmd_flushdb(args).await,
            "FLUSHALL" => self.cmd_flushall(args).await,
            "COMMAND" => self.cmd_command(args).await,
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown server command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["INFO", "DBSIZE", "FLUSHDB", "FLUSHALL", "COMMAND"]
    }
}
