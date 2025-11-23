//! Sorted set command handlers for Redis RESP protocol

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use tracing::debug;

pub struct SortedSetCommands {
    base: BaseCommandHandler,
}

impl SortedSetCommands {
    pub fn new(local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>) -> Self {
        // Use provided local_registry
        Self {
            base: BaseCommandHandler::new(local_registry),
        }
    }

    // Helper methods
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
                ProtocolError::RespError(format!(
                    "ERR invalid float argument for '{}' command",
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
                v.as_integer().or_else(|| {
                    v.as_string()
                        .and_then(|s| s.parse::<i64>().ok())
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

    /// ZADD key score member [score member ...] - Add members with scores
    async fn cmd_zadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || (args.len() - 1) % 2 != 0 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zadd' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZADD")?;
        let mut added = 0i64;

        // Process score-member pairs
        for i in (1..args.len()).step_by(2) {
            let score = self.get_float_arg(args, i, "ZADD")?;
            let member = self.get_string_arg(args, i + 1, "ZADD")?;

            let result = self
                .base
                .local_registry
                .execute_sorted_set(
                    &key,
                    "zadd",
                    &[
                        serde_json::to_value(member.clone()).unwrap(),
                        serde_json::to_value(score).unwrap(),
                    ],
                )
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

            let was_added: bool = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
                .unwrap_or(false);

            if was_added {
                added += 1;
            }
        }

        debug!("ZADD {} -> {} added", key, added);
        Ok(RespValue::Integer(added))
    }

    /// ZCARD key - Get cardinality of sorted set
    async fn cmd_zcard(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZCARD", args, 1)?;

        let key = self.get_string_arg(args, 0, "ZCARD")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zcard", &[])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let size: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!("ZCARD {} -> {}", key, size);
        Ok(RespValue::Integer(size))
    }

    /// ZSCORE key member - Get score of member
    async fn cmd_zscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZSCORE", args, 2)?;

        let key = self.get_string_arg(args, 0, "ZSCORE")?;
        let member = self.get_string_arg(args, 1, "ZSCORE")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zscore", &[serde_json::to_value(member.clone()).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let score: Option<f64> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .ok()
            .flatten();

        match score {
            Some(s) => {
                debug!("ZSCORE {} {} -> {}", key, member, s);
                Ok(RespValue::BulkString(Bytes::from(s.to_string().into_bytes())))
            }
            None => {
                debug!("ZSCORE {} {} -> null", key, member);
                Ok(RespValue::null())
            }
        }
    }

    /// ZRANGE key start stop [WITHSCORES] - Get range of members
    async fn cmd_zrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() > 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrange' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANGE")?;
        let start = self.get_int_arg(args, 1, "ZRANGE")?;
        let stop = self.get_int_arg(args, 2, "ZRANGE")?;
        let with_scores = args.len() == 4
            && args[3]
                .as_string()
                .map(|s| s.to_uppercase() == "WITHSCORES")
                .unwrap_or(false);

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zrange",
                &[
                    serde_json::to_value(start).unwrap(),
                    serde_json::to_value(stop).unwrap(),
                    serde_json::to_value(with_scores).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        // zrange returns Vec<(String, Option<f64>)>
        let members_with_scores: Vec<(String, Option<f64>)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score_opt) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                if let Some(score) = score_opt {
                    result_values.push(RespValue::BulkString(Bytes::from(score.to_string().into_bytes())));
                }
            }
        }

        debug!("ZRANGE {} {} {} -> {} members", key, start, stop, result_values.len());
        Ok(RespValue::Array(result_values))
    }

    /// ZINCRBY key increment member - Increment score
    async fn cmd_zincrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZINCRBY", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZINCRBY")?;
        let increment = self.get_float_arg(args, 1, "ZINCRBY")?;
        let member = self.get_string_arg(args, 2, "ZINCRBY")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zincrby",
                &[
                    serde_json::to_value(member.clone()).unwrap(),
                    serde_json::to_value(increment).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let new_score: f64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0.0);

        debug!("ZINCRBY {} {} {} -> {}", key, increment, member, new_score);
        Ok(RespValue::BulkString(Bytes::from(new_score.to_string().into_bytes())))
    }

    /// ZREM key member [member ...] - Remove members
    async fn cmd_zrem(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrem' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZREM")?;
        let mut members = Vec::new();
        for i in 1..args.len() {
            members.push(self.get_string_arg(args, i, "ZREM")?);
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zrem", &[serde_json::to_value(&members).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let removed: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!("ZREM {} {:?} -> {} removed", key, members, removed);
        Ok(RespValue::Integer(removed))
    }
}

#[async_trait]
impl CommandHandler for SortedSetCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "ZADD" => self.cmd_zadd(args).await,
            "ZCARD" => self.cmd_zcard(args).await,
            "ZSCORE" => self.cmd_zscore(args).await,
            "ZRANGE" => self.cmd_zrange(args).await,
            "ZINCRBY" => self.cmd_zincrby(args).await,
            "ZREM" => self.cmd_zrem(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR sorted set command '{}' not yet implemented",
                command_name
            ))),
        }
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
