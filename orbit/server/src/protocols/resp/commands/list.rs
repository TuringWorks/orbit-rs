//! List command handlers for Redis RESP protocol
//!
//! This module implements Redis list commands (LPUSH, RPUSH, LRANGE, etc.)
//! that operate on list-like data structures.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolError;
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tracing::debug;

pub struct ListCommands {
    base: BaseCommandHandler,
}

impl ListCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        // Use provided local_registry
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    // Helper methods for argument parsing
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
                    v.as_string().and_then(|s| s.parse::<i64>().ok())
                })
            })
            .ok_or_else(|| {
                ProtocolError::RespError(format!(
                    "ERR invalid integer argument for '{}' command",
                    command_name.to_lowercase()
                ))
            })
    }

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

    /// LPUSH key element [element ...] - Insert elements at the head of the list
    async fn cmd_lpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lpush' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "LPUSH")?;
        let mut values = Vec::new();
        for i in 1..args.len() {
            values.push(self.get_string_arg(args, i, "LPUSH")?);
        }

        let result = self
            .base
            .local_registry
            .execute_list(&key, "lpush", &[serde_json::to_value(&values).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let new_length: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<usize>("lpush", vec![serde_json::to_value(&values).unwrap()])
                .await
            {
                tracing::error!("Failed to persist LPUSH to OrbitClient: {}", e);
            }
        }

        debug!("LPUSH {} {:?} -> length: {}", key, values, new_length);
        Ok(RespValue::Integer(new_length))
    }

    /// RPUSH key element [element ...] - Insert elements at the tail of the list
    async fn cmd_rpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'rpush' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "RPUSH")?;
        let mut values = Vec::new();
        for i in 1..args.len() {
            values.push(self.get_string_arg(args, i, "RPUSH")?);
        }

        // Use local registry for consistency
        let result = self
            .base
            .local_registry
            .execute_list(&key, "rpush", &[serde_json::to_value(&values).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let new_length: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<usize>("rpush", vec![serde_json::to_value(&values).unwrap()])
                .await
            {
                tracing::error!("Failed to persist RPUSH to OrbitClient: {}", e);
            }
        }

        debug!("RPUSH {} {:?} -> length: {}", key, values, new_length);
        Ok(RespValue::Integer(new_length))
    }

    /// LPOP key [count] - Remove and return elements from the head of the list
    async fn cmd_lpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lpop' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "LPOP")?;
        let count = if args.len() == 2 {
            self.get_int_arg(args, 1, "LPOP")? as usize
        } else {
            1
        };

        // Use local registry for consistency
        let result = self
            .base
            .local_registry
            .execute_list(&key, "lpop", &[serde_json::to_value(count).unwrap()])
            .await;

        let popped_items: Result<Vec<String>, _> = result
            .map_err(|e| format!("ERR actor invocation failed: {}", e))
            .and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("Serialization error: {}", e))
            });

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<Vec<String>>("lpop", vec![serde_json::to_value(count).unwrap()])
                .await
            {
                tracing::error!("Failed to persist LPOP to OrbitClient: {}", e);
            }
        }

        match popped_items {
            Ok(items) if !items.is_empty() => {
                debug!("LPOP {} {} -> {:?}", key, count, items);
                if count == 1 {
                    Ok(items
                        .first()
                        .map(|s| RespValue::BulkString(Bytes::from(s.as_bytes().to_vec())))
                        .unwrap_or(RespValue::null()))
                } else {
                    let result: Vec<RespValue> = items
                        .into_iter()
                        .map(|item| RespValue::BulkString(Bytes::from(item.as_bytes().to_vec())))
                        .collect();
                    Ok(RespValue::Array(result))
                }
            }
            _ => {
                debug!("LPOP {} -> null (list empty or doesn't exist)", key);
                Ok(RespValue::null())
            }
        }
    }

    /// RPOP key [count] - Remove and return elements from the tail of the list
    async fn cmd_rpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'rpop' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "RPOP")?;
        let count = if args.len() == 2 {
            self.get_int_arg(args, 1, "RPOP")? as usize
        } else {
            1
        };

        let result = self
            .base
            .local_registry
            .execute_list(&key, "rpop", &[serde_json::to_value(count).unwrap()])
            .await;

        let popped_items: Result<Vec<String>, _> = result
            .map_err(|e| format!("ERR actor invocation failed: {}", e))
            .and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("Serialization error: {}", e))
            });

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<Vec<String>>("rpop", vec![serde_json::to_value(count).unwrap()])
                .await
            {
                tracing::error!("Failed to persist RPOP to OrbitClient: {}", e);
            }
        }

        match popped_items {
            Ok(items) => {
                debug!("RPOP {} {} -> {:?}", key, count, items);
                if count == 1 {
                    Ok(items
                        .first()
                        .map(|s| RespValue::BulkString(Bytes::from(s.as_bytes().to_vec())))
                        .unwrap_or(RespValue::null()))
                } else {
                    let result: Vec<RespValue> = items
                        .into_iter()
                        .map(|item| RespValue::BulkString(Bytes::from(item.as_bytes().to_vec())))
                        .collect();
                    Ok(RespValue::Array(result))
                }
            }
            Err(_) => Ok(RespValue::null()),
        }
    }

    /// LRANGE key start stop - Get a range of elements from the list
    async fn cmd_lrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("LRANGE", args, 3)?;

        let key = self.get_string_arg(args, 0, "LRANGE")?;
        // Use get_int_arg which now handles both integer and bulk string formats
        let start = self.get_int_arg(args, 1, "LRANGE")?;
        let stop = self.get_int_arg(args, 2, "LRANGE")?;

        let result = self
            .base
            .local_registry
            .execute_list(
                &key,
                "lrange",
                &[
                    serde_json::to_value(start).unwrap(),
                    serde_json::to_value(stop).unwrap(),
                ],
            )
            .await;

        let range_result: Result<Vec<String>, _> = result
            .map_err(|e| format!("ERR actor invocation failed: {}", e))
            .and_then(|v| {
                serde_json::from_value(v).map_err(|e| format!("Serialization error: {}", e))
            });

        match range_result {
            Ok(items) => {
                if items.is_empty() {
                    // Try OrbitClient
                    let orbit_key = self.base.make_key(&key);
                    if let Ok(actor_ref) = self
                        .base
                        .orbit_client
                        .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
                        .await
                    {
                        if let Ok(remote_items) = actor_ref
                            .invoke::<Vec<String>>(
                                "lrange",
                                vec![
                                    serde_json::to_value(start).unwrap(),
                                    serde_json::to_value(stop).unwrap(),
                                ],
                            )
                            .await
                        {
                            if !remote_items.is_empty() {
                                let result: Vec<RespValue> = remote_items
                                    .into_iter()
                                    .map(|item| {
                                        RespValue::BulkString(Bytes::from(item.as_bytes().to_vec()))
                                    })
                                    .collect();
                                debug!(
                                    "LRANGE {} {} {} -> {} items (OrbitClient)",
                                    key,
                                    start,
                                    stop,
                                    result.len()
                                );
                                return Ok(RespValue::Array(result));
                            }
                        }
                    }
                }

                let result: Vec<RespValue> = items
                    .into_iter()
                    .map(|item| RespValue::BulkString(Bytes::from(item.as_bytes().to_vec())))
                    .collect();
                debug!(
                    "LRANGE {} {} {} -> {} items",
                    key,
                    start,
                    stop,
                    result.len()
                );
                Ok(RespValue::Array(result))
            }
            Err(_) => {
                // Try OrbitClient
                let orbit_key = self.base.make_key(&key);
                if let Ok(actor_ref) = self
                    .base
                    .orbit_client
                    .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
                    .await
                {
                    if let Ok(remote_items) = actor_ref
                        .invoke::<Vec<String>>(
                            "lrange",
                            vec![
                                serde_json::to_value(start).unwrap(),
                                serde_json::to_value(stop).unwrap(),
                            ],
                        )
                        .await
                    {
                        let result: Vec<RespValue> = remote_items
                            .into_iter()
                            .map(|item| {
                                RespValue::BulkString(Bytes::from(item.as_bytes().to_vec()))
                            })
                            .collect();
                        debug!(
                            "LRANGE {} {} {} -> {} items (OrbitClient fallback)",
                            key,
                            start,
                            stop,
                            result.len()
                        );
                        return Ok(RespValue::Array(result));
                    }
                }
                Ok(RespValue::Array(vec![]))
            }
        }
    }

    /// LLEN key - Get the length of the list
    async fn cmd_llen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("LLEN", args, 1)?;

        let key = self.get_string_arg(args, 0, "LLEN")?;

        // Use local registry for consistency
        let result = self
            .base
            .local_registry
            .execute_list(&key, "llen", &[])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let mut length: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        if length == 0 {
            let orbit_key = self.base.make_key(&key);
            if let Ok(actor_ref) = self
                .base
                .orbit_client
                .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
                .await
            {
                if let Ok(remote_len) = actor_ref.invoke::<usize>("llen", vec![]).await {
                    if remote_len > 0 {
                        length = remote_len as i64;
                        debug!("LLEN {} -> {} (OrbitClient)", key, length);
                    }
                }
            }
        }

        debug!("LLEN {} -> {}", key, length);
        Ok(RespValue::Integer(length))
    }

    /// LINDEX key index - Get an element by its index
    async fn cmd_lindex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("LINDEX", args, 2)?;

        let key = self.get_string_arg(args, 0, "LINDEX")?;
        // Use get_int_arg which now handles both integer and bulk string formats
        let index = self.get_int_arg(args, 1, "LINDEX")?;

        // Use local registry for consistency
        let result = self
            .base
            .local_registry
            .execute_list(&key, "lindex", &[serde_json::to_value(index).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let element: Option<String> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        match element {
            Some(value) => {
                debug!("LINDEX {} {} -> {}", key, index, value);
                Ok(RespValue::BulkString(Bytes::from(
                    value.as_bytes().to_vec(),
                )))
            }
            None => {
                // Try OrbitClient
                let orbit_key = self.base.make_key(&key);
                if let Ok(actor_ref) = self
                    .base
                    .orbit_client
                    .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
                    .await
                {
                    if let Ok(Some(remote_value)) = actor_ref
                        .invoke::<Option<String>>(
                            "lindex",
                            vec![serde_json::to_value(index).unwrap()],
                        )
                        .await
                    {
                        debug!("LINDEX {} {} -> {} (OrbitClient)", key, index, remote_value);
                        return Ok(RespValue::BulkString(Bytes::from(
                            remote_value.as_bytes().to_vec(),
                        )));
                    }
                }

                debug!("LINDEX {} {} -> null", key, index);
                Ok(RespValue::null())
            }
        }
    }

    /// LSET key index element - Set the value of an element by its index
    async fn cmd_lset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("LSET", args, 3)?;

        let key = self.get_string_arg(args, 0, "LSET")?;
        let index = self.get_int_arg(args, 1, "LSET")?;
        let value = self.get_string_arg(args, 2, "LSET")?;

        // Use local registry
        let result = self
            .base
            .local_registry
            .execute_list(
                &key,
                "lset",
                &[
                    serde_json::to_value(index)?,
                    serde_json::to_value(value.clone())?,
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let success: bool = serde_json::from_value(result)
            .map_err(|_| ProtocolError::RespError("ERR invalid response".to_string()))?;

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<bool>(
                    "lset",
                    vec![
                        serde_json::to_value(index).unwrap(),
                        serde_json::to_value(value.clone()).unwrap(),
                    ],
                )
                .await
            {
                tracing::error!("Failed to persist LSET to OrbitClient: {}", e);
            }
        }

        if success {
            debug!("LSET {} {} {} -> OK", key, index, value);
            Ok(RespValue::SimpleString("OK".to_string()))
        } else {
            Err(ProtocolError::RespError(
                "ERR index out of range".to_string(),
            ))
        }
    }

    /// LREM key count element - Remove elements from the list
    async fn cmd_lrem(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("LREM", args, 3)?;

        let key = self.get_string_arg(args, 0, "LREM")?;
        let count = self.get_int_arg(args, 1, "LREM")?;
        let element = self.get_string_arg(args, 2, "LREM")?;

        // Use local registry
        let result = self
            .base
            .local_registry
            .execute_list(
                &key,
                "lrem",
                &[
                    serde_json::to_value(count)?,
                    serde_json::to_value(element.clone())?,
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        let num_removed: usize = serde_json::from_value(result).unwrap_or(0);

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<usize>(
                    "lrem",
                    vec![
                        serde_json::to_value(count).unwrap(),
                        serde_json::to_value(element.clone()).unwrap(),
                    ],
                )
                .await
            {
                tracing::error!("Failed to persist LREM to OrbitClient: {}", e);
            }
        }

        debug!("LREM {} {} {} -> {}", key, count, element, num_removed);
        Ok(RespValue::Integer(num_removed as i64))
    }

    /// LTRIM key start stop - Trim the list to the specified range
    async fn cmd_ltrim(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("LTRIM", args, 3)?;

        let key = self.get_string_arg(args, 0, "LTRIM")?;
        let start = self.get_int_arg(args, 1, "LTRIM")?;
        let stop = self.get_int_arg(args, 2, "LTRIM")?;

        // Use local registry
        let _result = self
            .base
            .local_registry
            .execute_list(
                &key,
                "ltrim",
                &[serde_json::to_value(start)?, serde_json::to_value(stop)?],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        // Persist to OrbitClient
        let orbit_key = self.base.make_key(&key);
        if let Ok(actor_ref) = self
            .base
            .orbit_client
            .actor_reference::<crate::protocols::resp::actors::ListActor>(orbit_key)
            .await
        {
            if let Err(e) = actor_ref
                .invoke::<()>(
                    "ltrim",
                    vec![
                        serde_json::to_value(start).unwrap(),
                        serde_json::to_value(stop).unwrap(),
                    ],
                )
                .await
            {
                tracing::error!("Failed to persist LTRIM to OrbitClient: {}", e);
            }
        }

        debug!("LTRIM {} {} {} -> OK", key, start, stop);
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    // Simplified stubs for remaining commands
    async fn cmd_linsert(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR LINSERT not yet implemented".to_string(),
        ))
    }

    async fn cmd_blpop(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR BLPOP not yet implemented".to_string(),
        ))
    }

    async fn cmd_brpop(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        Err(ProtocolError::RespError(
            "ERR BRPOP not yet implemented".to_string(),
        ))
    }
}

#[async_trait]
impl CommandHandler for ListCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "LPUSH" => self.cmd_lpush(args).await,
            "RPUSH" => self.cmd_rpush(args).await,
            "LPOP" => self.cmd_lpop(args).await,
            "RPOP" => self.cmd_rpop(args).await,
            "LRANGE" => self.cmd_lrange(args).await,
            "LLEN" => self.cmd_llen(args).await,
            "LINDEX" => self.cmd_lindex(args).await,
            "LSET" => self.cmd_lset(args).await,
            "LREM" => self.cmd_lrem(args).await,
            "LTRIM" => self.cmd_ltrim(args).await,
            "LINSERT" => self.cmd_linsert(args).await,
            "BLPOP" => self.cmd_blpop(args).await,
            "BRPOP" => self.cmd_brpop(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown list command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "LPUSH", "RPUSH", "LPOP", "RPOP", "LRANGE", "LLEN", "LINDEX", "LSET", "LREM", "LTRIM",
            "LINSERT", "BLPOP", "BRPOP",
        ]
    }
}
