//! Common traits and utilities for RESP command handlers

use crate::protocols::resp::simple_local::SimpleLocalRegistry;
use crate::protocols::{error::ProtocolResult, resp::RespValue};
use async_trait::async_trait;
use std::sync::Arc;

/// Common trait for all command handlers
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Handle a command for this category
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue>;

    /// Get supported commands for this handler
    fn supported_commands(&self) -> &[&'static str];

    /// Validate argument count for a command
    fn validate_arg_count(
        &self,
        command_name: &str,
        args: &[RespValue],
        expected: usize,
    ) -> ProtocolResult<()> {
        self.validate_arg_count_range(command_name, args, expected, expected)
    }

    /// Validate argument count within a range
    fn validate_arg_count_range(
        &self,
        command_name: &str,
        args: &[RespValue],
        min: usize,
        max: usize,
    ) -> ProtocolResult<()> {
        if args.len() < min || args.len() > max {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR wrong number of arguments for '{}' command",
                command_name.to_lowercase()
            )));
        }
        Ok(())
    }

    /// Extract string argument at index with error handling
    fn get_string_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<String> {
        args.get(index).and_then(|v| v.as_string()).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR invalid argument for '{}' command",
                command_name.to_lowercase()
            ))
        })
    }

    /// Extract optional string argument at index
    fn get_optional_string_arg(&self, args: &[RespValue], index: usize) -> Option<String> {
        args.get(index).and_then(|v| v.as_string())
    }

    /// Extract integer argument at index with error handling
    /// Supports both RespValue::Integer and RespValue::BulkString (parsed as integer)
    fn get_int_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<i64> {
        args.get(index)
            .and_then(|v| {
                // Try integer first
                v.as_integer().or_else(|| {
                    // Try parsing as string
                    v.as_string()
                        .and_then(|s| s.parse::<i64>().ok())
                })
            })
            .ok_or_else(|| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR invalid integer argument for '{}' command",
                    command_name.to_lowercase()
                ))
            })
    }

    /// Extract float argument at index with error handling  
    fn get_float_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<f64> {
        args.get(index)
            .and_then(|v| {
                v.as_string()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| v.as_integer().map(|i| i as f64))
            })
            .ok_or_else(|| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR invalid float argument for '{}' command",
                    command_name.to_lowercase()
                ))
            })
    }
}

/// Base structure for command handlers that use local registry
pub struct BaseCommandHandler {
    pub local_registry: Arc<SimpleLocalRegistry>,
}

impl BaseCommandHandler {
    /// Create new base handler
    pub fn new(local_registry: Arc<SimpleLocalRegistry>) -> Self {
        Self {
            local_registry,
        }
    }

    /// Get a key for Orbit actor addressing
    pub fn make_key(&self, key: &str) -> orbit_shared::Key {
        orbit_shared::Key::StringKey {
            key: key.to_string(),
        }
    }
}
