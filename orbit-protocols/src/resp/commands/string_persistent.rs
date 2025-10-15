//! Persistent String command handlers for Redis RESP protocol
//!
//! This module implements Redis string commands using the RedisDataProvider
//! for persistent storage with TTL support.

use super::traits::CommandHandler;
use crate::error::{ProtocolError, ProtocolResult};
use crate::persistence::redis_data::{RedisDataProvider, RedisValue};
use crate::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use orbit_client::OrbitClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

pub struct PersistentStringCommands {
    #[allow(dead_code)] // Reserved for future actor integration
    orbit_client: Arc<OrbitClient>,
    redis_provider: Arc<dyn RedisDataProvider>,
}

impl PersistentStringCommands {
    pub fn new(orbit_client: Arc<OrbitClient>, redis_provider: Arc<dyn RedisDataProvider>) -> Self {
        Self {
            orbit_client,
            redis_provider,
        }
    }

    // Helper methods for argument validation and extraction
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

    fn validate_min_arg_count(
        &self,
        command_name: &str,
        args: &[RespValue],
        min_expected: usize,
    ) -> ProtocolResult<()> {
        if args.len() < min_expected {
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

    // Parse SET command options (EX, PX, NX, XX)
    fn parse_set_options(
        &self,
        args: &[RespValue],
        start_index: usize,
    ) -> ProtocolResult<SetOptions> {
        let mut options = SetOptions::default();
        let mut i = start_index;

        while i < args.len() {
            let option = self.get_string_arg(args, i, "SET")?.to_uppercase();
            match option.as_str() {
                "EX" => {
                    if i + 1 >= args.len() {
                        return Err(ProtocolError::RespError("ERR syntax error".to_string()));
                    }
                    let seconds = self.get_int_arg(args, i + 1, "SET")?;
                    if seconds <= 0 {
                        return Err(ProtocolError::RespError(
                            "ERR invalid expire time in set".to_string(),
                        ));
                    }
                    options.ttl_seconds = Some(seconds as u64);
                    i += 2;
                }
                "PX" => {
                    if i + 1 >= args.len() {
                        return Err(ProtocolError::RespError("ERR syntax error".to_string()));
                    }
                    let milliseconds = self.get_int_arg(args, i + 1, "SET")?;
                    if milliseconds <= 0 {
                        return Err(ProtocolError::RespError(
                            "ERR invalid expire time in set".to_string(),
                        ));
                    }
                    options.ttl_seconds = Some((milliseconds as u64 + 999) / 1000); // Round up to seconds
                    i += 2;
                }
                "NX" => {
                    options.nx = true;
                    i += 1;
                }
                "XX" => {
                    options.xx = true;
                    i += 1;
                }
                _ => {
                    return Err(ProtocolError::RespError("ERR syntax error".to_string()));
                }
            }
        }

        if options.nx && options.xx {
            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
        }

        Ok(options)
    }

    // Redis command implementations
    async fn cmd_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("GET", args, 1)?;

        let key = self.get_string_arg(args, 0, "GET")?;

        match self.redis_provider.get(&key).await {
            Ok(Some(redis_value)) => {
                debug!("GET {} -> {}", key, redis_value.data);
                Ok(RespValue::BulkString(Bytes::from(
                    redis_value.data.into_bytes(),
                )))
            }
            Ok(None) => {
                debug!("GET {} -> (nil)", key);
                Ok(RespValue::null())
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_set(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_min_arg_count("SET", args, 2)?;

        let key = self.get_string_arg(args, 0, "SET")?;
        let value_str = self.get_string_arg(args, 1, "SET")?;

        let options = if args.len() > 2 {
            self.parse_set_options(args, 2)?
        } else {
            SetOptions::default()
        };

        let redis_value = if let Some(ttl) = options.ttl_seconds {
            RedisValue::with_ttl(value_str.clone(), ttl)
        } else {
            RedisValue::new(value_str.clone())
        };

        // Handle NX/XX options
        if options.nx {
            match self.redis_provider.setnx(&key, redis_value).await {
                Ok(true) => {
                    debug!("SET {} {} (NX) -> OK", key, value_str);
                    Ok(RespValue::ok())
                }
                Ok(false) => {
                    debug!("SET {} {} (NX) -> (nil)", key, value_str);
                    Ok(RespValue::null())
                }
                Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
            }
        } else if options.xx {
            let exists = match self.redis_provider.exists(&key).await {
                Ok(exists) => exists,
                Err(e) => return Err(ProtocolError::RespError(format!("ERR {}", e))),
            };

            if exists {
                match self.redis_provider.set(&key, redis_value).await {
                    Ok(()) => {
                        debug!("SET {} {} (XX) -> OK", key, value_str);
                        Ok(RespValue::ok())
                    }
                    Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
                }
            } else {
                debug!("SET {} {} (XX) -> (nil)", key, value_str);
                Ok(RespValue::null())
            }
        } else {
            // Normal SET
            match self.redis_provider.set(&key, redis_value).await {
                Ok(()) => {
                    debug!("SET {} {} -> OK", key, value_str);
                    Ok(RespValue::ok())
                }
                Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
            }
        }
    }

    async fn cmd_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'del' command".to_string(),
            ));
        }

        let mut deleted_count = 0i64;

        for arg in args {
            let key = self.get_string_arg(&[arg.clone()], 0, "DEL")?;

            match self.redis_provider.delete(&key).await {
                Ok(true) => deleted_count += 1,
                Ok(false) => {} // Key didn't exist
                Err(e) => return Err(ProtocolError::RespError(format!("ERR {}", e))),
            }
        }

        debug!("DEL -> {} keys deleted", deleted_count);
        Ok(RespValue::Integer(deleted_count))
    }

    async fn cmd_exists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'exists' command".to_string(),
            ));
        }

        let mut exists_count = 0i64;

        for arg in args {
            let key = self.get_string_arg(&[arg.clone()], 0, "EXISTS")?;

            match self.redis_provider.exists(&key).await {
                Ok(true) => exists_count += 1,
                Ok(false) => {} // Key doesn't exist
                Err(e) => return Err(ProtocolError::RespError(format!("ERR {}", e))),
            }
        }

        debug!("EXISTS -> {} keys exist", exists_count);
        Ok(RespValue::Integer(exists_count))
    }

    async fn cmd_strlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("STRLEN", args, 1)?;

        let key = self.get_string_arg(args, 0, "STRLEN")?;

        match self.redis_provider.strlen(&key).await {
            Ok(length) => {
                debug!("STRLEN {} -> {}", key, length);
                Ok(RespValue::Integer(length as i64))
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_mget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'mget' command".to_string(),
            ));
        }

        let keys: Result<Vec<String>, _> = args
            .iter()
            .enumerate()
            .map(|(_i, arg)| self.get_string_arg(&[arg.clone()], 0, "MGET"))
            .collect();
        let keys = keys?;

        match self.redis_provider.mget(&keys).await {
            Ok(values) => {
                let results: Vec<RespValue> = values
                    .into_iter()
                    .map(|opt_value| {
                        opt_value
                            .map(|v| RespValue::BulkString(Bytes::from(v.data.into_bytes())))
                            .unwrap_or(RespValue::null())
                    })
                    .collect();

                debug!("MGET -> {} values", results.len());
                Ok(RespValue::Array(results))
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_mset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() % 2 != 0 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'mset' command".to_string(),
            ));
        }

        let mut values = HashMap::new();

        for chunk in args.chunks(2) {
            let key = self.get_string_arg(&chunk, 0, "MSET")?;
            let value_str = self.get_string_arg(&chunk, 1, "MSET")?;
            let redis_value = RedisValue::new(value_str.clone());
            values.insert(key.clone(), redis_value);
            debug!("MSET {} {}", key, value_str);
        }

        match self.redis_provider.mset(values).await {
            Ok(()) => Ok(RespValue::ok()),
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_incr(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_incrby_impl(args, 1).await
    }

    async fn cmd_decr(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.cmd_incrby_impl(args, -1).await
    }

    async fn cmd_incrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("INCRBY", args, 2)?;
        let increment = self.get_int_arg(args, 1, "INCRBY")?;
        self.cmd_incrby_impl(&args[..1], increment).await
    }

    async fn cmd_decrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("DECRBY", args, 2)?;
        let decrement = self.get_int_arg(args, 1, "DECRBY")?;
        self.cmd_incrby_impl(&args[..1], -decrement).await
    }

    async fn cmd_incrby_impl(&self, args: &[RespValue], delta: i64) -> ProtocolResult<RespValue> {
        self.validate_arg_count("INCR", args, 1)?;
        let key = self.get_string_arg(args, 0, "INCR")?;

        match self.redis_provider.incr(&key, delta).await {
            Ok(new_value) => {
                debug!("INCR/DECR {} by {} -> {}", key, delta, new_value);
                Ok(RespValue::Integer(new_value))
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_append(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("APPEND", args, 2)?;

        let key = self.get_string_arg(args, 0, "APPEND")?;
        let append_value = self.get_string_arg(args, 1, "APPEND")?;

        match self.redis_provider.append(&key, &append_value).await {
            Ok(new_length) => {
                debug!("APPEND {} -> new length {}", key, new_length);
                Ok(RespValue::Integer(new_length as i64))
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_getset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("GETSET", args, 2)?;

        let key = self.get_string_arg(args, 0, "GETSET")?;
        let new_value_str = self.get_string_arg(args, 1, "GETSET")?;
        let redis_value = RedisValue::new(new_value_str.clone());

        match self.redis_provider.getset(&key, redis_value).await {
            Ok(Some(old_value)) => {
                debug!("GETSET {} -> old value returned", key);
                Ok(RespValue::BulkString(Bytes::from(old_value.into_bytes())))
            }
            Ok(None) => {
                debug!("GETSET {} -> (nil)", key);
                Ok(RespValue::null())
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_setex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SETEX", args, 3)?;

        let key = self.get_string_arg(args, 0, "SETEX")?;
        let ttl_seconds = self.get_int_arg(args, 1, "SETEX")?;
        let value_str = self.get_string_arg(args, 2, "SETEX")?;

        if ttl_seconds <= 0 {
            return Err(ProtocolError::RespError(
                "ERR invalid expire time in setex".to_string(),
            ));
        }

        let redis_value = RedisValue::with_ttl(value_str.clone(), ttl_seconds as u64);

        match self.redis_provider.set(&key, redis_value).await {
            Ok(()) => {
                debug!("SETEX {} {} {} -> OK", key, ttl_seconds, value_str);
                Ok(RespValue::ok())
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }

    async fn cmd_setnx(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("SETNX", args, 2)?;

        let key = self.get_string_arg(args, 0, "SETNX")?;
        let value_str = self.get_string_arg(args, 1, "SETNX")?;
        let redis_value = RedisValue::new(value_str.clone());

        match self.redis_provider.setnx(&key, redis_value).await {
            Ok(true) => {
                debug!("SETNX {} {} -> 1", key, value_str);
                Ok(RespValue::Integer(1))
            }
            Ok(false) => {
                debug!("SETNX {} {} -> 0", key, value_str);
                Ok(RespValue::Integer(0))
            }
            Err(e) => Err(ProtocolError::RespError(format!("ERR {}", e))),
        }
    }
}

#[async_trait]
impl CommandHandler for PersistentStringCommands {
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

// Helper structures
#[derive(Default)]
struct SetOptions {
    ttl_seconds: Option<u64>,
    nx: bool, // Only set if key doesn't exist
    xx: bool, // Only set if key exists
}
