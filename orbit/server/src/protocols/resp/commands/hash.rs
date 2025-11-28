//! Hash command handlers for Redis RESP protocol
//!
//! This module implements Redis hash commands (HGET, HSET, HGETALL, etc.)
//! that operate on hash-like data structures.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tracing::debug;

pub struct HashCommands {
    base: BaseCommandHandler,
}

impl HashCommands {
    pub fn new(
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        // Use provided local_registry
        Self {
            base: BaseCommandHandler::new(local_registry),
        }
    }

    async fn cmd_hget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HGET", args, 2)?;

        let key = self.get_string_arg(args, 0, "HGET")?;
        let field = self.get_string_arg(args, 1, "HGET")?;

        let result = self
            .base
            .local_registry
            .execute_hash(&key, "hget", &[serde_json::Value::String(field.clone())])
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let value: Option<String> = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        debug!("HGET {} {} -> {:?}", key, field, value);

        Ok(value
            .map(|v| RespValue::BulkString(Bytes::from(v.into_bytes())))
            .unwrap_or(RespValue::null()))
    }

    async fn cmd_hset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // HSET needs at least 3 args and odd number (key + field-value pairs)
        if args.len() < 3 || args.len().is_multiple_of(2) {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'hset' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "HSET")?;

        let mut fields_set = 0i64;
        for i in (1..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }

            let field = self.get_string_arg(args, i, "HSET")?;
            let value = self.get_string_arg(args, i + 1, "HSET")?;

            let result = self
                .base
                .local_registry
                .execute_hash(
                    &key,
                    "hset",
                    &[
                        serde_json::Value::String(field.clone()),
                        serde_json::Value::String(value.clone()),
                    ],
                )
                .await
                .map_err(|e| {
                    crate::protocols::error::ProtocolError::RespError(format!(
                        "ERR actor invocation failed: {}",
                        e
                    ))
                })?;

            let was_new: bool = serde_json::from_value(result).map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR serialization error: {}",
                    e
                ))
            })?;

            debug!("HSET {} {} {} -> new: {}", key, field, value, was_new);
            if was_new {
                fields_set += 1;
            }
        }

        Ok(RespValue::Integer(fields_set))
    }

    async fn cmd_hgetall(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HGETALL", args, 1)?;

        let key = self.get_string_arg(args, 0, "HGETALL")?;

        let result = self
            .base
            .local_registry
            .execute_hash(&key, "hgetall", &[])
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let pairs: Vec<(String, String)> = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        let mut resp_result = Vec::new();
        for (field, value) in pairs {
            resp_result.push(RespValue::BulkString(Bytes::from(field.into_bytes())));
            resp_result.push(RespValue::BulkString(Bytes::from(value.into_bytes())));
        }

        debug!(
            "HGETALL {} -> {} field-value pairs",
            key,
            resp_result.len() / 2
        );
        Ok(RespValue::Array(resp_result))
    }

    async fn cmd_hmget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'hmget' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "HMGET")?;

        let mut result = Vec::new();
        for i in 1..args.len() {
            let field = self.get_string_arg(args, i, "HMGET")?;

            match self
                .base
                .local_registry
                .execute_hash(&key, "hget", &[serde_json::Value::String(field.clone())])
                .await
            {
                Ok(value_json) => {
                    let value: Option<String> =
                        serde_json::from_value(value_json).map_err(|e| {
                            crate::protocols::error::ProtocolError::RespError(format!(
                                "ERR serialization error: {}",
                                e
                            ))
                        })?;

                    result.push(
                        value
                            .map(|v| RespValue::BulkString(Bytes::from(v.into_bytes())))
                            .unwrap_or(RespValue::null()),
                    );
                }
                Err(_) => result.push(RespValue::null()),
            }
        }

        debug!("HMGET {} -> {} values", key, result.len());
        Ok(RespValue::Array(result))
    }

    async fn cmd_hmset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // HMSET needs at least 3 args and odd number (key + field-value pairs)
        if args.len() < 3 || args.len().is_multiple_of(2) {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'hmset' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "HMSET")?;

        for i in (1..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }

            let field = self.get_string_arg(args, i, "HMSET")?;
            let value = self.get_string_arg(args, i + 1, "HMSET")?;

            self.base
                .local_registry
                .execute_hash(
                    &key,
                    "hset",
                    &[
                        serde_json::Value::String(field.clone()),
                        serde_json::Value::String(value.clone()),
                    ],
                )
                .await
                .map_err(|e| {
                    crate::protocols::error::ProtocolError::RespError(format!(
                        "ERR actor invocation failed: {}",
                        e
                    ))
                })?;

            debug!("HMSET {} {} {}", key, field, value);
        }

        Ok(RespValue::SimpleString("OK".to_string()))
    }

    async fn cmd_hdel(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'hdel' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "HDEL")?;

        let mut deleted_count = 0i64;
        for i in 1..args.len() {
            let field = self.get_string_arg(args, i, "HDEL")?;

            let result = self
                .base
                .local_registry
                .execute_hash(&key, "hdel", &[serde_json::Value::String(field.clone())])
                .await
                .map_err(|e| {
                    crate::protocols::error::ProtocolError::RespError(format!(
                        "ERR actor invocation failed: {}",
                        e
                    ))
                })?;

            let was_deleted: bool = serde_json::from_value(result).map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR serialization error: {}",
                    e
                ))
            })?;

            if was_deleted {
                deleted_count += 1;
            }
            debug!("HDEL {} {} -> deleted: {}", key, field, was_deleted);
        }

        Ok(RespValue::Integer(deleted_count))
    }

    async fn cmd_hexists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HEXISTS", args, 2)?;

        let key = self.get_string_arg(args, 0, "HEXISTS")?;
        let field = self.get_string_arg(args, 1, "HEXISTS")?;

        let result = self
            .base
            .local_registry
            .execute_hash(&key, "hexists", &[serde_json::Value::String(field.clone())])
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let exists: bool = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        debug!("HEXISTS {} {} -> {}", key, field, exists);
        Ok(RespValue::Integer(if exists { 1 } else { 0 }))
    }

    async fn cmd_hkeys(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HKEYS", args, 1)?;

        let key = self.get_string_arg(args, 0, "HKEYS")?;

        let result = self
            .base
            .local_registry
            .execute_hash(&key, "hkeys", &[])
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let keys: Vec<String> = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        let resp_keys: Vec<RespValue> = keys
            .into_iter()
            .map(|k| RespValue::BulkString(Bytes::from(k.into_bytes())))
            .collect();

        debug!("HKEYS {} -> {} keys", key, resp_keys.len());
        Ok(RespValue::Array(resp_keys))
    }

    async fn cmd_hvals(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HVALS", args, 1)?;

        let key = self.get_string_arg(args, 0, "HVALS")?;

        let result = self
            .base
            .local_registry
            .execute_hash(&key, "hvals", &[])
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let values: Vec<String> = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        let resp_values: Vec<RespValue> = values
            .into_iter()
            .map(|v| RespValue::BulkString(Bytes::from(v.into_bytes())))
            .collect();

        debug!("HVALS {} -> {} values", key, resp_values.len());
        Ok(RespValue::Array(resp_values))
    }

    async fn cmd_hlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HLEN", args, 1)?;

        let key = self.get_string_arg(args, 0, "HLEN")?;

        let result = self
            .base
            .local_registry
            .execute_hash(&key, "hlen", &[])
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let len: i64 = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        debug!("HLEN {} -> {}", key, len);
        Ok(RespValue::Integer(len))
    }

    async fn cmd_hincrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("HINCRBY", args, 3)?;

        let key = self.get_string_arg(args, 0, "HINCRBY")?;
        let field = self.get_string_arg(args, 1, "HINCRBY")?;
        let increment = self.get_int_arg(args, 2, "HINCRBY")?;

        let result = self
            .base
            .local_registry
            .execute_hash(
                &key,
                "hincrby",
                &[
                    serde_json::Value::String(field.clone()),
                    serde_json::Value::Number(serde_json::Number::from(increment)),
                ],
            )
            .await
            .map_err(|e| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                ))
            })?;

        let new_value: i64 = serde_json::from_value(result).map_err(|e| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR serialization error: {}",
                e
            ))
        })?;

        debug!("HINCRBY {} {} {} -> {}", key, field, increment, new_value);
        Ok(RespValue::Integer(new_value))
    }
}

#[async_trait]
impl CommandHandler for HashCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "HGET" => self.cmd_hget(args).await,
            "HSET" => self.cmd_hset(args).await,
            "HGETALL" => self.cmd_hgetall(args).await,
            "HMGET" => self.cmd_hmget(args).await,
            "HMSET" => self.cmd_hmset(args).await,
            "HDEL" => self.cmd_hdel(args).await,
            "HEXISTS" => self.cmd_hexists(args).await,
            "HKEYS" => self.cmd_hkeys(args).await,
            "HVALS" => self.cmd_hvals(args).await,
            "HLEN" => self.cmd_hlen(args).await,
            "HINCRBY" => self.cmd_hincrby(args).await,
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown hash command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "HGET", "HSET", "HGETALL", "HMGET", "HMSET", "HDEL", "HEXISTS", "HKEYS", "HVALS",
            "HLEN", "HINCRBY",
        ]
    }
}
