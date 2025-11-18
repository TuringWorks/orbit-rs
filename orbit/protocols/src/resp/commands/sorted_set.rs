//! Sorted set command handlers for Redis RESP protocol

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::error::ProtocolResult;
use crate::resp::RespValue;
use async_trait::async_trait;
use orbit_client::OrbitClient;
use std::sync::Arc;

pub struct SortedSetCommands {
    #[allow(dead_code)] // Reserved for future implementation
    base: BaseCommandHandler,
}

impl SortedSetCommands {
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        let local_registry = Arc::new(crate::resp::simple_local::SimpleLocalRegistry::new());
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }
}

#[async_trait]
impl CommandHandler for SortedSetCommands {
    async fn handle(&self, command_name: &str, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        Err(crate::error::ProtocolError::RespError(format!(
            "ERR sorted set command '{}' not yet implemented",
            command_name
        )))
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "ZADD",
            "ZREM",
            "ZCARD",
            "ZSCORE",
            "ZINCRBY",
            "ZRANGE",
            "ZRANGEBYSCORE",
            "ZCOUNT",
            "ZRANK",
        ]
    }
}
