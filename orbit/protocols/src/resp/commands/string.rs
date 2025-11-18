//! Simple String command handlers for Redis RESP protocol
//!
//! This module implements basic Redis string commands (GET, SET, DEL, etc.)
//! using the current command handler pattern.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::error::{ProtocolError, ProtocolResult};
use crate::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use orbit_client::OrbitClient;
use std::sync::Arc;
use tracing::debug;

pub struct StringCommands {
    base: BaseCommandHandler,
}

impl StringCommands {
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        let local_registry = Arc::new(crate::resp::simple_local::SimpleLocalRegistry::new());
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    // Delegate trait methods to avoid duplicate code
    fn validate_arg_count(
        &self,
        command_name: &str,
        args: &[RespValue],
        expected: usize,
    ) -> ProtocolResult<()> {
        if args.len() != expected {
            return Err(ProtocolError::RespError(format!(
                "ERR wrong number of arguments for '{}' command",
                command_name.to_lowercase()
            )));
        }
        Ok(())
    }

    fn get_string_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<String> {
        args.get(index).and_then(|v| v.as_string()).ok_or_else(|| {
            ProtocolError::RespError(format!(
                "ERR invalid argument for '{}' command",
                command_name.to_lowercase()
            ))
        })
    }

    async fn cmd_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("GET", args, 1)?;

        let key = self.get_string_arg(args, 0, "GET")?;

        let result = self
            .base
            .local_registry
            .execute_keyvalue(&key, "get_value", &[])
            .await
            .map_err(|e| {
                crate::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let value: Option<String> = serde_json::from_value(result).map_err(|e| {
            crate::error::ProtocolError::RespError(format!("ERR serialization error: {}", e))
        })?;

        debug!("GET {} -> {:?}", key, value);

        Ok(value
            .map(|v| RespValue::BulkString(Bytes::from(v.into_bytes())))
            .unwrap_or(RespValue::null()))
    }

    async fn cmd_set(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'set' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "SET")?;
        let value = self.get_string_arg(args, 1, "SET")?;

        let _result = self
            .base
            .local_registry
            .execute_keyvalue(
                &key,
                "set_value",
                &[serde_json::Value::String(value.clone())],
            )
            .await
            .map_err(|e| {
                crate::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        debug!("SET {} {}", key, value);

        Ok(RespValue::ok())
    }

    async fn cmd_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'del' command".to_string(),
            ));
        }

        let mut deleted_count = 0i64;

        for arg in args {
            let key = self.get_string_arg(&[arg.clone()], 0, "DEL")?;

            // Check if key exists first
            match self
                .base
                .local_registry
                .execute_keyvalue(&key, "get_value", &[])
                .await
            {
                Ok(result) => {
                    let exists: Option<String> = serde_json::from_value(result).unwrap_or(None);
                    if exists.is_some() {
                        // Try to delete
                        let _result = self
                            .base
                            .local_registry
                            .execute_keyvalue(&key, "set_value", &[serde_json::Value::Null])
                            .await
                            .map_err(|e| {
                                crate::error::ProtocolError::RespError(format!(
                                    "ERR actor invocation failed: {}",
                                    e
                                ))
                            })?;
                        deleted_count += 1;
                    }
                }
                Err(_) => {
                    // Key doesn't exist, continue
                }
            }
        }

        debug!("DEL -> {} keys deleted", deleted_count);

        Ok(RespValue::Integer(deleted_count))
    }

    async fn cmd_exists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'exists' command".to_string(),
            ));
        }

        let mut exists_count = 0i64;

        for arg in args {
            let key = self.get_string_arg(&[arg.clone()], 0, "EXISTS")?;

            match self
                .base
                .local_registry
                .execute_keyvalue(&key, "get_value", &[])
                .await
            {
                Ok(result) => {
                    let value: Option<String> = serde_json::from_value(result).unwrap_or(None);
                    if value.is_some() {
                        exists_count += 1;
                    }
                }
                Err(_) => {
                    // Key doesn't exist
                }
            }
        }

        debug!("EXISTS -> {} keys exist", exists_count);

        Ok(RespValue::Integer(exists_count))
    }

    async fn cmd_strlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("STRLEN", args, 1)?;

        let key = self.get_string_arg(args, 0, "STRLEN")?;

        let result = self
            .base
            .local_registry
            .execute_keyvalue(&key, "get_value", &[])
            .await
            .map_err(|e| {
                crate::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let value: Option<String> = serde_json::from_value(result).map_err(|e| {
            crate::error::ProtocolError::RespError(format!("ERR serialization error: {}", e))
        })?;

        let length = value.map(|v| v.len() as i64).unwrap_or(0);
        debug!("STRLEN {} -> {}", key, length);

        Ok(RespValue::Integer(length))
    }

    async fn cmd_mget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'mget' command".to_string(),
            ));
        }

        let mut results = Vec::new();

        for arg in args {
            let key = self.get_string_arg(&[arg.clone()], 0, "MGET")?;

            match self
                .base
                .local_registry
                .execute_keyvalue(&key, "get_value", &[])
                .await
            {
                Ok(result) => {
                    let value: Option<String> = serde_json::from_value(result).unwrap_or(None);
                    if let Some(v) = value {
                        results.push(RespValue::BulkString(Bytes::from(v.into_bytes())));
                    } else {
                        results.push(RespValue::null());
                    }
                }
                Err(_) => {
                    results.push(RespValue::null());
                }
            }
        }

        debug!("MGET -> {} values", results.len());
        Ok(RespValue::Array(results))
    }

    async fn cmd_mset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() % 2 != 0 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'mset' command".to_string(),
            ));
        }

        for chunk in args.chunks(2) {
            let key = self.get_string_arg(&chunk, 0, "MSET")?;
            let value = self.get_string_arg(&chunk, 1, "MSET")?;

            let _result = self
                .base
                .local_registry
                .execute_keyvalue(
                    &key,
                    "set_value",
                    &[serde_json::Value::String(value.clone())],
                )
                .await
                .map_err(|e| {
                    crate::error::ProtocolError::RespError(format!(
                        "ERR actor invocation failed: {}",
                        e
                    ))
                })?;

            debug!("MSET {} {}", key, value);
        }

        Ok(RespValue::ok())
    }

    async fn cmd_incr(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_incrby_impl(args, 1).await
    }

    async fn cmd_decr(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_incrby_impl(args, -1).await
    }

    async fn cmd_incrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'incrby' command".to_string(),
            ));
        }

        let increment = self.get_int_arg(args, 1, "INCRBY")?;
        self.cmd_incrby_impl(&args[..1], increment).await
    }

    async fn cmd_decrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'decrby' command".to_string(),
            ));
        }

        let decrement = self.get_int_arg(args, 1, "DECRBY")?;
        self.cmd_incrby_impl(&args[..1], -decrement).await
    }

    async fn cmd_incrby_impl(&self, args: &[RespValue], delta: i64) -> ProtocolResult<RespValue> {
        let key = self.get_string_arg(args, 0, "INCR")?;

        // Get current value
        let current_value = match self
            .base
            .local_registry
            .execute_keyvalue(&key, "get_value", &[])
            .await
        {
            Ok(result) => {
                let value: Option<String> = serde_json::from_value(result).unwrap_or(None);
                match value {
                    Some(v) => v.parse::<i64>().map_err(|_| {
                        ProtocolError::RespError(
                            "ERR value is not an integer or out of range".to_string(),
                        )
                    })?,
                    None => 0,
                }
            }
            Err(_) => 0, // Key doesn't exist, start from 0
        };

        let new_value = current_value + delta;
        let new_value_str = new_value.to_string();

        // Set the new value
        let _result = self
            .base
            .local_registry
            .execute_keyvalue(
                &key,
                "set_value",
                &[serde_json::Value::String(new_value_str)],
            )
            .await
            .map_err(|e| {
                crate::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        debug!("INCR/DECR {} -> {}", key, new_value);
        Ok(RespValue::Integer(new_value))
    }

    async fn cmd_append(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'append' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "APPEND")?;
        let append_value = self.get_string_arg(args, 1, "APPEND")?;

        // Get current value
        let current_value = match self
            .base
            .local_registry
            .execute_keyvalue(&key, "get_value", &[])
            .await
        {
            Ok(result) => {
                let value: Option<String> = serde_json::from_value(result).unwrap_or(None);
                value.unwrap_or_default()
            }
            Err(_) => String::new(), // Key doesn't exist, start with empty string
        };

        let new_value = format!("{}{}", current_value, append_value);
        let new_length = new_value.len() as i64;

        // Set the new value
        let _result = self
            .base
            .local_registry
            .execute_keyvalue(&key, "set_value", &[serde_json::Value::String(new_value)])
            .await
            .map_err(|e| {
                crate::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        debug!("APPEND {} -> new length {}", key, new_length);
        Ok(RespValue::Integer(new_length))
    }

    async fn cmd_getset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'getset' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "GETSET")?;
        let new_value = self.get_string_arg(args, 1, "GETSET")?;

        // Get the current value first
        let old_value = match self
            .base
            .local_registry
            .execute_keyvalue(&key, "get_value", &[])
            .await
        {
            Ok(result) => {
                let value: Option<String> = serde_json::from_value(result).unwrap_or(None);
                value
                    .map(|v| RespValue::BulkString(Bytes::from(v.into_bytes())))
                    .unwrap_or(RespValue::null())
            }
            Err(_) => RespValue::null(), // Key doesn't exist
        };

        // Set the new value
        let _result = self
            .base
            .local_registry
            .execute_keyvalue(
                &key,
                "set_value",
                &[serde_json::Value::String(new_value.clone())],
            )
            .await
            .map_err(|e| {
                crate::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        debug!("GETSET {} -> new value set", key);
        Ok(old_value)
    }

    // Helper method to get integer argument
    fn get_int_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<i64> {
        args.get(index).and_then(|v| v.as_integer()).ok_or_else(|| {
            ProtocolError::RespError(format!(
                "ERR invalid integer argument for '{}' command",
                command_name.to_lowercase()
            ))
        })
    }
}

#[async_trait]
impl CommandHandler for StringCommands {
    fn supported_commands(&self) -> &[&'static str] {
        &[
            "GET", "SET", "DEL", "EXISTS", "STRLEN", "MGET", "MSET", "INCR", "DECR", "INCRBY",
            "DECRBY", "APPEND", "GETSET", "SETEX", "SETNX",
        ]
    }

    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "GET" => self.cmd_get(args).await,
            "SET" => self.cmd_set(args).await,
            "DEL" => self.cmd_del(args).await,
            "EXISTS" => self.cmd_exists(args).await,
            "STRLEN" => self.cmd_strlen(args).await,
            "MGET" => self.cmd_mget(args).await,
            "MSET" => self.cmd_mset(args).await,
            "INCR" => self.cmd_incr(args).await,
            "DECR" => self.cmd_decr(args).await,
            "INCRBY" => self.cmd_incrby(args).await,
            "DECRBY" => self.cmd_decrby(args).await,
            "APPEND" => self.cmd_append(args).await,
            "GETSET" => self.cmd_getset(args).await,
            "SETEX" => self.cmd_setex(args).await,
            "SETNX" => self.cmd_setnx(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown string command '{}'",
                command_name
            ))),
        }
    }
}

// Individual command implementations
impl StringCommands {
    async fn cmd_setex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SETEX", args, 3)?;

        let key = self.get_string_arg(args, 0, "SETEX")?;
        let ttl_seconds = self.get_int_arg(args, 1, "SETEX")?;
        let value = self.get_string_arg(args, 2, "SETEX")?;

        if ttl_seconds <= 0 {
            return Err(ProtocolError::RespError(
                "ERR invalid expire time in setex".to_string(),
            ));
        }

        // Set the value first
        let _result = self
            .base
            .local_registry
            .execute_keyvalue(
                &key,
                "set_value",
                &[serde_json::Value::String(value.clone())],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        // Set the expiration
        let _result = self
            .base
            .local_registry
            .execute_keyvalue(
                &key,
                "set_expiration",
                &[serde_json::Value::Number(serde_json::Number::from(
                    ttl_seconds as u64,
                ))],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("SETEX {} {} {}", key, ttl_seconds, value);
        Ok(RespValue::ok())
    }

    async fn cmd_setnx(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SETNX", args, 2)?;

        let key = self.get_string_arg(args, 0, "SETNX")?;
        let value = self.get_string_arg(args, 1, "SETNX")?;

        // Check if key exists
        let result = self
            .base
            .local_registry
            .execute_keyvalue(&key, "exists", &[])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let exists: bool = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        if exists {
            Ok(RespValue::Integer(0)) // Key exists, return 0
        } else {
            // Set the value since key doesn't exist
            let _result = self
                .base
                .local_registry
                .execute_keyvalue(
                    &key,
                    "set_value",
                    &[serde_json::Value::String(value.clone())],
                )
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;

            debug!("SETNX {} {}", key, value);
            Ok(RespValue::Integer(1))
        }
    }
}
