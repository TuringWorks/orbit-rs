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
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        // Use provided local_registry
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
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
                v.as_integer()
                    .or_else(|| v.as_string().and_then(|s| s.parse::<i64>().ok()))
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
        if args.len() < 3 || !(args.len() - 1).is_multiple_of(2) {
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
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;

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
            .execute_sorted_set(
                &key,
                "zscore",
                &[serde_json::to_value(member.clone()).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let score: Option<f64> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .ok()
            .flatten();

        match score {
            Some(s) => {
                debug!("ZSCORE {} {} -> {}", key, member, s);
                Ok(RespValue::BulkString(Bytes::from(
                    s.to_string().into_bytes(),
                )))
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
                    result_values.push(RespValue::BulkString(Bytes::from(
                        score.to_string().into_bytes(),
                    )));
                }
            }
        }

        debug!(
            "ZRANGE {} {} {} -> {} members",
            key,
            start,
            stop,
            result_values.len()
        );
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
        Ok(RespValue::BulkString(Bytes::from(
            new_score.to_string().into_bytes(),
        )))
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

    /// ZCOUNT key min max - Count members with scores in range
    async fn cmd_zcount(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZCOUNT", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZCOUNT")?;
        let min = self.get_float_arg(args, 1, "ZCOUNT")?;
        let max = self.get_float_arg(args, 2, "ZCOUNT")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zcount",
                &[
                    serde_json::to_value(min).unwrap(),
                    serde_json::to_value(max).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let count: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!("ZCOUNT {} {} {} -> {}", key, min, max, count);
        Ok(RespValue::Integer(count))
    }

    /// ZRANK key member - Get rank of member
    async fn cmd_zrank(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZRANK", args, 2)?;

        let key = self.get_string_arg(args, 0, "ZRANK")?;
        let member = self.get_string_arg(args, 1, "ZRANK")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zrank",
                &[serde_json::to_value(member.clone()).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let rank: Option<i64> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .ok()
            .flatten();

        match rank {
            Some(r) => {
                debug!("ZRANK {} {} -> {}", key, member, r);
                Ok(RespValue::Integer(r))
            }
            None => {
                debug!("ZRANK {} {} -> null", key, member);
                Ok(RespValue::null())
            }
        }
    }

    /// ZREVRANK key member - Get reverse rank of member
    async fn cmd_zrevrank(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZREVRANK", args, 2)?;

        let key = self.get_string_arg(args, 0, "ZREVRANK")?;
        let member = self.get_string_arg(args, 1, "ZREVRANK")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zrevrank",
                &[serde_json::to_value(member.clone()).unwrap()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let rank: Option<i64> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .ok()
            .flatten();

        match rank {
            Some(r) => {
                debug!("ZREVRANK {} {} -> {}", key, member, r);
                Ok(RespValue::Integer(r))
            }
            None => {
                debug!("ZREVRANK {} {} -> null", key, member);
                Ok(RespValue::null())
            }
        }
    }

    /// ZREVRANGE key start stop [WITHSCORES] - Get range in reverse order
    async fn cmd_zrevrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() > 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrevrange' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZREVRANGE")?;
        let start = self.get_int_arg(args, 1, "ZREVRANGE")?;
        let stop = self.get_int_arg(args, 2, "ZREVRANGE")?;
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
                "zrevrange",
                &[
                    serde_json::to_value(start).unwrap(),
                    serde_json::to_value(stop).unwrap(),
                    serde_json::to_value(with_scores).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members_with_scores: Vec<(String, Option<f64>)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score_opt) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                if let Some(score) = score_opt {
                    result_values.push(RespValue::BulkString(Bytes::from(
                        score.to_string().into_bytes(),
                    )));
                }
            }
        }

        debug!(
            "ZREVRANGE {} {} {} -> {} members",
            key,
            start,
            stop,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZRANGEBYSCORE key min max [WITHSCORES] [LIMIT offset count] - Get range by score
    async fn cmd_zrangebyscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrangebyscore' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANGEBYSCORE")?;
        let min = self.get_float_arg(args, 1, "ZRANGEBYSCORE")?;
        let max = self.get_float_arg(args, 2, "ZRANGEBYSCORE")?;

        let mut with_scores = false;
        for i in 3..args.len() {
            if let Some(s) = args[i].as_string() {
                if s.to_uppercase() == "WITHSCORES" {
                    with_scores = true;
                }
            }
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zrangebyscore",
                &[
                    serde_json::to_value(min).unwrap(),
                    serde_json::to_value(max).unwrap(),
                    serde_json::to_value(with_scores).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members_with_scores: Vec<(String, Option<f64>)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score_opt) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                if let Some(score) = score_opt {
                    result_values.push(RespValue::BulkString(Bytes::from(
                        score.to_string().into_bytes(),
                    )));
                }
            }
        }

        debug!(
            "ZRANGEBYSCORE {} {} {} -> {} members",
            key,
            min,
            max,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZREVRANGEBYSCORE key max min [WITHSCORES] [LIMIT offset count] - Get range by score in reverse
    async fn cmd_zrevrangebyscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrevrangebyscore' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZREVRANGEBYSCORE")?;
        let max = self.get_float_arg(args, 1, "ZREVRANGEBYSCORE")?;
        let min = self.get_float_arg(args, 2, "ZREVRANGEBYSCORE")?;

        let mut with_scores = false;
        for i in 3..args.len() {
            if let Some(s) = args[i].as_string() {
                if s.to_uppercase() == "WITHSCORES" {
                    with_scores = true;
                }
            }
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zrevrangebyscore",
                &[
                    serde_json::to_value(max).unwrap(),
                    serde_json::to_value(min).unwrap(),
                    serde_json::to_value(with_scores).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members_with_scores: Vec<(String, Option<f64>)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score_opt) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                if let Some(score) = score_opt {
                    result_values.push(RespValue::BulkString(Bytes::from(
                        score.to_string().into_bytes(),
                    )));
                }
            }
        }

        debug!(
            "ZREVRANGEBYSCORE {} {} {} -> {} members",
            key,
            max,
            min,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZREMRANGEBYRANK key start stop - Remove members by rank range
    async fn cmd_zremrangebyrank(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZREMRANGEBYRANK", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZREMRANGEBYRANK")?;
        let start = self.get_int_arg(args, 1, "ZREMRANGEBYRANK")?;
        let stop = self.get_int_arg(args, 2, "ZREMRANGEBYRANK")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zremrangebyrank",
                &[
                    serde_json::to_value(start).unwrap(),
                    serde_json::to_value(stop).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let removed: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!(
            "ZREMRANGEBYRANK {} {} {} -> {} removed",
            key, start, stop, removed
        );
        Ok(RespValue::Integer(removed))
    }

    /// ZREMRANGEBYSCORE key min max - Remove members by score range
    async fn cmd_zremrangebyscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZREMRANGEBYSCORE", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZREMRANGEBYSCORE")?;
        let min = self.get_float_arg(args, 1, "ZREMRANGEBYSCORE")?;
        let max = self.get_float_arg(args, 2, "ZREMRANGEBYSCORE")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zremrangebyscore",
                &[
                    serde_json::to_value(min).unwrap(),
                    serde_json::to_value(max).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let removed: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!(
            "ZREMRANGEBYSCORE {} {} {} -> {} removed",
            key, min, max, removed
        );
        Ok(RespValue::Integer(removed))
    }

    /// ZPOPMIN key [count] - Remove and return members with lowest scores
    async fn cmd_zpopmin(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zpopmin' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZPOPMIN")?;
        let count: usize = if args.len() == 2 {
            self.get_int_arg(args, 1, "ZPOPMIN")? as usize
        } else {
            1
        };

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zpopmin", &[serde_json::to_value(count).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members_with_scores: Vec<(String, f64)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            result_values.push(RespValue::BulkString(Bytes::from(
                score.to_string().into_bytes(),
            )));
        }

        debug!(
            "ZPOPMIN {} {} -> {} elements",
            key,
            count,
            result_values.len() / 2
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZPOPMAX key [count] - Remove and return members with highest scores
    async fn cmd_zpopmax(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zpopmax' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZPOPMAX")?;
        let count: usize = if args.len() == 2 {
            self.get_int_arg(args, 1, "ZPOPMAX")? as usize
        } else {
            1
        };

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zpopmax", &[serde_json::to_value(count).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members_with_scores: Vec<(String, f64)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            result_values.push(RespValue::BulkString(Bytes::from(
                score.to_string().into_bytes(),
            )));
        }

        debug!(
            "ZPOPMAX {} {} -> {} elements",
            key,
            count,
            result_values.len() / 2
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZLEXCOUNT key min max - Count members in lexicographical range
    async fn cmd_zlexcount(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZLEXCOUNT", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZLEXCOUNT")?;
        let min = self.get_string_arg(args, 1, "ZLEXCOUNT")?;
        let max = self.get_string_arg(args, 2, "ZLEXCOUNT")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zlexcount",
                &[
                    serde_json::to_value(min.clone()).unwrap(),
                    serde_json::to_value(max.clone()).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let count: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!("ZLEXCOUNT {} {} {} -> {}", key, min, max, count);
        Ok(RespValue::Integer(count))
    }

    /// ZSCAN key cursor [MATCH pattern] [COUNT count] - Iterate sorted set
    async fn cmd_zscan(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zscan' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZSCAN")?;
        let cursor: usize = self.get_int_arg(args, 1, "ZSCAN")? as usize;
        let count: usize = 10; // Default count

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zscan",
                &[
                    serde_json::to_value(cursor).unwrap(),
                    serde_json::to_value(count).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let (next_cursor, members): (usize, Vec<(String, f64)>) = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or((0, Vec::new()));

        let mut member_array: Vec<RespValue> = Vec::new();
        for (member, score) in members {
            member_array.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            member_array.push(RespValue::BulkString(Bytes::from(
                score.to_string().into_bytes(),
            )));
        }

        debug!(
            "ZSCAN {} {} -> cursor {}, {} elements",
            key,
            cursor,
            next_cursor,
            member_array.len() / 2
        );
        Ok(RespValue::Array(vec![
            RespValue::BulkString(Bytes::from(next_cursor.to_string().into_bytes())),
            RespValue::Array(member_array),
        ]))
    }

    /// ZMSCORE key member [member ...] - Get scores of multiple members
    async fn cmd_zmscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zmscore' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZMSCORE")?;
        let mut members = Vec::new();
        for i in 1..args.len() {
            members.push(self.get_string_arg(args, i, "ZMSCORE")?);
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zmscore", &[serde_json::to_value(&members).unwrap()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let scores: Vec<Option<f64>> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let result_values: Vec<RespValue> = scores
            .into_iter()
            .map(|score_opt| match score_opt {
                Some(score) => RespValue::BulkString(Bytes::from(score.to_string().into_bytes())),
                None => RespValue::null(),
            })
            .collect();

        debug!(
            "ZMSCORE {} {:?} -> {} scores",
            key,
            members,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZRANGEBYLEX key min max [LIMIT offset count] - Get members in lexicographical range
    async fn cmd_zrangebylex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrangebylex' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANGEBYLEX")?;
        let min = self.get_string_arg(args, 1, "ZRANGEBYLEX")?;
        let max = self.get_string_arg(args, 2, "ZRANGEBYLEX")?;

        // Parse optional LIMIT
        let mut offset: Option<usize> = None;
        let mut count: Option<usize> = None;
        let mut i = 3;
        while i < args.len() {
            if let Some(s) = args[i].as_string() {
                if s.to_uppercase() == "LIMIT" && i + 2 < args.len() {
                    offset = Some(self.get_int_arg(args, i + 1, "ZRANGEBYLEX")? as usize);
                    count = Some(self.get_int_arg(args, i + 2, "ZRANGEBYLEX")? as usize);
                    i += 3;
                    continue;
                }
            }
            i += 1;
        }

        let mut invoke_args = vec![
            serde_json::to_value(min.clone()).unwrap(),
            serde_json::to_value(max.clone()).unwrap(),
        ];
        if let Some(off) = offset {
            invoke_args.push(serde_json::to_value(off).unwrap());
        } else {
            invoke_args.push(serde_json::Value::Null);
        }
        if let Some(cnt) = count {
            invoke_args.push(serde_json::to_value(cnt).unwrap());
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zrangebylex", &invoke_args)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members: Vec<String> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let result_values: Vec<RespValue> = members
            .into_iter()
            .map(|m| RespValue::BulkString(Bytes::from(m.into_bytes())))
            .collect();

        debug!(
            "ZRANGEBYLEX {} {} {} -> {} members",
            key,
            min,
            max,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZREVRANGEBYLEX key max min [LIMIT offset count] - Get members in reverse lexicographical range
    async fn cmd_zrevrangebylex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrevrangebylex' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZREVRANGEBYLEX")?;
        let max = self.get_string_arg(args, 1, "ZREVRANGEBYLEX")?;
        let min = self.get_string_arg(args, 2, "ZREVRANGEBYLEX")?;

        // Parse optional LIMIT
        let mut offset: Option<usize> = None;
        let mut count: Option<usize> = None;
        let mut i = 3;
        while i < args.len() {
            if let Some(s) = args[i].as_string() {
                if s.to_uppercase() == "LIMIT" && i + 2 < args.len() {
                    offset = Some(self.get_int_arg(args, i + 1, "ZREVRANGEBYLEX")? as usize);
                    count = Some(self.get_int_arg(args, i + 2, "ZREVRANGEBYLEX")? as usize);
                    i += 3;
                    continue;
                }
            }
            i += 1;
        }

        let mut invoke_args = vec![
            serde_json::to_value(max.clone()).unwrap(),
            serde_json::to_value(min.clone()).unwrap(),
        ];
        if let Some(off) = offset {
            invoke_args.push(serde_json::to_value(off).unwrap());
        } else {
            invoke_args.push(serde_json::Value::Null);
        }
        if let Some(cnt) = count {
            invoke_args.push(serde_json::to_value(cnt).unwrap());
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zrevrangebylex", &invoke_args)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members: Vec<String> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        let result_values: Vec<RespValue> = members
            .into_iter()
            .map(|m| RespValue::BulkString(Bytes::from(m.into_bytes())))
            .collect();

        debug!(
            "ZREVRANGEBYLEX {} {} {} -> {} members",
            key,
            max,
            min,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZREMRANGEBYLEX key min max - Remove members in lexicographical range
    async fn cmd_zremrangebylex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZREMRANGEBYLEX", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZREMRANGEBYLEX")?;
        let min = self.get_string_arg(args, 1, "ZREMRANGEBYLEX")?;
        let max = self.get_string_arg(args, 2, "ZREMRANGEBYLEX")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zremrangebylex",
                &[
                    serde_json::to_value(min.clone()).unwrap(),
                    serde_json::to_value(max.clone()).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let removed: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or(0);

        debug!(
            "ZREMRANGEBYLEX {} {} {} -> {} removed",
            key, min, max, removed
        );
        Ok(RespValue::Integer(removed))
    }

    /// ZRANDMEMBER key [count [WITHSCORES]] - Get random member(s) from sorted set
    async fn cmd_zrandmember(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrandmember' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANDMEMBER")?;
        let count: i64 = if args.len() > 1 {
            self.get_int_arg(args, 1, "ZRANDMEMBER")?
        } else {
            1
        };

        let mut with_scores = false;
        for i in 2..args.len() {
            if let Some(s) = args[i].as_string() {
                if s.to_uppercase() == "WITHSCORES" {
                    with_scores = true;
                }
            }
        }

        let result = self
            .base
            .local_registry
            .execute_sorted_set(
                &key,
                "zrandmember",
                &[
                    serde_json::to_value(count).unwrap(),
                    serde_json::to_value(with_scores).unwrap(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let members_with_scores: Vec<(String, Option<f64>)> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))
            .unwrap_or_default();

        // If count == 1 and no WITHSCORES, return single bulk string
        if count.abs() == 1 && !with_scores && args.len() == 1 {
            if let Some((member, _)) = members_with_scores.first() {
                return Ok(RespValue::BulkString(Bytes::from(
                    member.clone().into_bytes(),
                )));
            }
            return Ok(RespValue::null());
        }

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score_opt) in members_with_scores {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                if let Some(score) = score_opt {
                    result_values.push(RespValue::BulkString(Bytes::from(
                        score.to_string().into_bytes(),
                    )));
                }
            }
        }

        debug!(
            "ZRANDMEMBER {} {} -> {} elements",
            key,
            count,
            result_values.len()
        );
        Ok(RespValue::Array(result_values))
    }

    /// ZUNION numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX] [WITHSCORES] - Union of multiple sorted sets
    async fn cmd_zunion(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zunion' command".to_string(),
            ));
        }

        let numkeys = self.get_int_arg(args, 0, "ZUNION")? as usize;
        if args.len() < 1 + numkeys {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zunion' command".to_string(),
            ));
        }

        let mut keys = Vec::new();
        for i in 1..=numkeys {
            keys.push(self.get_string_arg(args, i, "ZUNION")?);
        }

        // Parse WEIGHTS, AGGREGATE, WITHSCORES
        let mut weights: Vec<f64> = vec![1.0; numkeys];
        let mut aggregate = "SUM".to_string();
        let mut with_scores = false;

        let mut i = 1 + numkeys;
        while i < args.len() {
            if let Some(s) = args[i].as_string() {
                match s.to_uppercase().as_str() {
                    "WEIGHTS" => {
                        for j in 0..numkeys {
                            if i + 1 + j < args.len() {
                                weights[j] = self.get_float_arg(args, i + 1 + j, "ZUNION")?;
                            }
                        }
                        i += 1 + numkeys;
                        continue;
                    }
                    "AGGREGATE" => {
                        if i + 1 < args.len() {
                            aggregate = self.get_string_arg(args, i + 1, "ZUNION")?.to_uppercase();
                        }
                        i += 2;
                        continue;
                    }
                    "WITHSCORES" => {
                        with_scores = true;
                        i += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        // Collect all members from all keys
        let mut combined: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        for (idx, key) in keys.iter().enumerate() {
            let weight = weights[idx];
            let result = self
                .base
                .local_registry
                .execute_sorted_set(key, "get_all_members", &[])
                .await
                .unwrap_or(serde_json::Value::Array(vec![]));

            let members: Vec<(String, f64)> = serde_json::from_value(result).unwrap_or_default();

            for (member, score) in members {
                let weighted_score = score * weight;
                combined
                    .entry(member)
                    .and_modify(|existing| {
                        *existing = match aggregate.as_str() {
                            "MIN" => existing.min(weighted_score),
                            "MAX" => existing.max(weighted_score),
                            _ => *existing + weighted_score, // SUM
                        };
                    })
                    .or_insert(weighted_score);
            }
        }

        // Sort by score, then lexicographically
        let mut result_list: Vec<(String, f64)> = combined.into_iter().collect();
        result_list.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score) in result_list {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                result_values.push(RespValue::BulkString(Bytes::from(
                    score.to_string().into_bytes(),
                )));
            }
        }

        debug!("ZUNION {:?} -> {} elements", keys, result_values.len());
        Ok(RespValue::Array(result_values))
    }

    /// ZINTER numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX] [WITHSCORES] - Intersection of multiple sorted sets
    async fn cmd_zinter(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zinter' command".to_string(),
            ));
        }

        let numkeys = self.get_int_arg(args, 0, "ZINTER")? as usize;
        if args.len() < 1 + numkeys {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zinter' command".to_string(),
            ));
        }

        let mut keys = Vec::new();
        for i in 1..=numkeys {
            keys.push(self.get_string_arg(args, i, "ZINTER")?);
        }

        // Parse WEIGHTS, AGGREGATE, WITHSCORES
        let mut weights: Vec<f64> = vec![1.0; numkeys];
        let mut aggregate = "SUM".to_string();
        let mut with_scores = false;

        let mut i = 1 + numkeys;
        while i < args.len() {
            if let Some(s) = args[i].as_string() {
                match s.to_uppercase().as_str() {
                    "WEIGHTS" => {
                        for j in 0..numkeys {
                            if i + 1 + j < args.len() {
                                weights[j] = self.get_float_arg(args, i + 1 + j, "ZINTER")?;
                            }
                        }
                        i += 1 + numkeys;
                        continue;
                    }
                    "AGGREGATE" => {
                        if i + 1 < args.len() {
                            aggregate = self.get_string_arg(args, i + 1, "ZINTER")?.to_uppercase();
                        }
                        i += 2;
                        continue;
                    }
                    "WITHSCORES" => {
                        with_scores = true;
                        i += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        // Get members from first key
        let first_result = self
            .base
            .local_registry
            .execute_sorted_set(&keys[0], "get_all_members", &[])
            .await
            .unwrap_or(serde_json::Value::Array(vec![]));

        let first_members: Vec<(String, f64)> =
            serde_json::from_value(first_result).unwrap_or_default();

        let mut combined: std::collections::HashMap<String, f64> = first_members
            .into_iter()
            .map(|(m, s)| (m, s * weights[0]))
            .collect();

        // Intersect with remaining keys
        for (idx, key) in keys.iter().enumerate().skip(1) {
            let weight = weights[idx];
            let result = self
                .base
                .local_registry
                .execute_sorted_set(key, "get_all_members", &[])
                .await
                .unwrap_or(serde_json::Value::Array(vec![]));

            let members: Vec<(String, f64)> = serde_json::from_value(result).unwrap_or_default();

            let key_members: std::collections::HashMap<String, f64> = members.into_iter().collect();

            // Keep only members that exist in both
            combined.retain(|member, existing_score| {
                if let Some(&score) = key_members.get(member) {
                    let weighted_score = score * weight;
                    *existing_score = match aggregate.as_str() {
                        "MIN" => existing_score.min(weighted_score),
                        "MAX" => existing_score.max(weighted_score),
                        _ => *existing_score + weighted_score, // SUM
                    };
                    true
                } else {
                    false
                }
            });
        }

        // Sort by score, then lexicographically
        let mut result_list: Vec<(String, f64)> = combined.into_iter().collect();
        result_list.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score) in result_list {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                result_values.push(RespValue::BulkString(Bytes::from(
                    score.to_string().into_bytes(),
                )));
            }
        }

        debug!("ZINTER {:?} -> {} elements", keys, result_values.len());
        Ok(RespValue::Array(result_values))
    }

    /// ZDIFF numkeys key [key ...] [WITHSCORES] - Difference of multiple sorted sets
    async fn cmd_zdiff(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zdiff' command".to_string(),
            ));
        }

        let numkeys = self.get_int_arg(args, 0, "ZDIFF")? as usize;
        if args.len() < 1 + numkeys {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zdiff' command".to_string(),
            ));
        }

        let mut keys = Vec::new();
        for i in 1..=numkeys {
            keys.push(self.get_string_arg(args, i, "ZDIFF")?);
        }

        let mut with_scores = false;
        for i in (1 + numkeys)..args.len() {
            if let Some(s) = args[i].as_string() {
                if s.to_uppercase() == "WITHSCORES" {
                    with_scores = true;
                }
            }
        }

        // Get members from first key
        let first_result = self
            .base
            .local_registry
            .execute_sorted_set(&keys[0], "get_all_members", &[])
            .await
            .unwrap_or(serde_json::Value::Array(vec![]));

        let first_members: Vec<(String, f64)> =
            serde_json::from_value(first_result).unwrap_or_default();

        let mut combined: std::collections::HashMap<String, f64> =
            first_members.into_iter().collect();

        // Remove members from subsequent keys
        for key in keys.iter().skip(1) {
            let result = self
                .base
                .local_registry
                .execute_sorted_set(key, "get_all_members", &[])
                .await
                .unwrap_or(serde_json::Value::Array(vec![]));

            let members: Vec<(String, f64)> = serde_json::from_value(result).unwrap_or_default();

            for (member, _) in members {
                combined.remove(&member);
            }
        }

        // Sort by score, then lexicographically
        let mut result_list: Vec<(String, f64)> = combined.into_iter().collect();
        result_list.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        let mut result_values: Vec<RespValue> = Vec::new();
        for (member, score) in result_list {
            result_values.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
            if with_scores {
                result_values.push(RespValue::BulkString(Bytes::from(
                    score.to_string().into_bytes(),
                )));
            }
        }

        debug!("ZDIFF {:?} -> {} elements", keys, result_values.len());
        Ok(RespValue::Array(result_values))
    }

    /// ZUNIONSTORE destination numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX] - Store union in destination
    async fn cmd_zunionstore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zunionstore' command".to_string(),
            ));
        }

        let destination = self.get_string_arg(args, 0, "ZUNIONSTORE")?;
        let numkeys = self.get_int_arg(args, 1, "ZUNIONSTORE")? as usize;
        if args.len() < 2 + numkeys {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zunionstore' command".to_string(),
            ));
        }

        let mut keys = Vec::new();
        for i in 2..(2 + numkeys) {
            keys.push(self.get_string_arg(args, i, "ZUNIONSTORE")?);
        }

        // Parse WEIGHTS, AGGREGATE
        let mut weights: Vec<f64> = vec![1.0; numkeys];
        let mut aggregate = "SUM".to_string();

        let mut i = 2 + numkeys;
        while i < args.len() {
            if let Some(s) = args[i].as_string() {
                match s.to_uppercase().as_str() {
                    "WEIGHTS" => {
                        for j in 0..numkeys {
                            if i + 1 + j < args.len() {
                                weights[j] = self.get_float_arg(args, i + 1 + j, "ZUNIONSTORE")?;
                            }
                        }
                        i += 1 + numkeys;
                        continue;
                    }
                    "AGGREGATE" => {
                        if i + 1 < args.len() {
                            aggregate = self
                                .get_string_arg(args, i + 1, "ZUNIONSTORE")?
                                .to_uppercase();
                        }
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        // Collect all members from all keys
        let mut combined: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        for (idx, key) in keys.iter().enumerate() {
            let weight = weights[idx];
            let result = self
                .base
                .local_registry
                .execute_sorted_set(key, "get_all_members", &[])
                .await
                .unwrap_or(serde_json::Value::Array(vec![]));

            let members: Vec<(String, f64)> = serde_json::from_value(result).unwrap_or_default();

            for (member, score) in members {
                let weighted_score = score * weight;
                combined
                    .entry(member)
                    .and_modify(|existing| {
                        *existing = match aggregate.as_str() {
                            "MIN" => existing.min(weighted_score),
                            "MAX" => existing.max(weighted_score),
                            _ => *existing + weighted_score, // SUM
                        };
                    })
                    .or_insert(weighted_score);
            }
        }

        // Store in destination
        let mut added = 0i64;
        for (member, score) in combined {
            self.base
                .local_registry
                .execute_sorted_set(
                    &destination,
                    "zadd",
                    &[
                        serde_json::to_value(member).unwrap(),
                        serde_json::to_value(score).unwrap(),
                    ],
                )
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;
            added += 1;
        }

        debug!("ZUNIONSTORE {} {:?} -> {} added", destination, keys, added);
        Ok(RespValue::Integer(added))
    }

    /// ZINTERSTORE destination numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX] - Store intersection in destination
    async fn cmd_zinterstore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zinterstore' command".to_string(),
            ));
        }

        let destination = self.get_string_arg(args, 0, "ZINTERSTORE")?;
        let numkeys = self.get_int_arg(args, 1, "ZINTERSTORE")? as usize;
        if args.len() < 2 + numkeys {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zinterstore' command".to_string(),
            ));
        }

        let mut keys = Vec::new();
        for i in 2..(2 + numkeys) {
            keys.push(self.get_string_arg(args, i, "ZINTERSTORE")?);
        }

        // Parse WEIGHTS, AGGREGATE
        let mut weights: Vec<f64> = vec![1.0; numkeys];
        let mut aggregate = "SUM".to_string();

        let mut i = 2 + numkeys;
        while i < args.len() {
            if let Some(s) = args[i].as_string() {
                match s.to_uppercase().as_str() {
                    "WEIGHTS" => {
                        for j in 0..numkeys {
                            if i + 1 + j < args.len() {
                                weights[j] = self.get_float_arg(args, i + 1 + j, "ZINTERSTORE")?;
                            }
                        }
                        i += 1 + numkeys;
                        continue;
                    }
                    "AGGREGATE" => {
                        if i + 1 < args.len() {
                            aggregate = self
                                .get_string_arg(args, i + 1, "ZINTERSTORE")?
                                .to_uppercase();
                        }
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        // Get members from first key
        let first_result = self
            .base
            .local_registry
            .execute_sorted_set(&keys[0], "get_all_members", &[])
            .await
            .unwrap_or(serde_json::Value::Array(vec![]));

        let first_members: Vec<(String, f64)> =
            serde_json::from_value(first_result).unwrap_or_default();

        let mut combined: std::collections::HashMap<String, f64> = first_members
            .into_iter()
            .map(|(m, s)| (m, s * weights[0]))
            .collect();

        // Intersect with remaining keys
        for (idx, key) in keys.iter().enumerate().skip(1) {
            let weight = weights[idx];
            let result = self
                .base
                .local_registry
                .execute_sorted_set(key, "get_all_members", &[])
                .await
                .unwrap_or(serde_json::Value::Array(vec![]));

            let members: Vec<(String, f64)> = serde_json::from_value(result).unwrap_or_default();

            let key_members: std::collections::HashMap<String, f64> = members.into_iter().collect();

            combined.retain(|member, existing_score| {
                if let Some(&score) = key_members.get(member) {
                    let weighted_score = score * weight;
                    *existing_score = match aggregate.as_str() {
                        "MIN" => existing_score.min(weighted_score),
                        "MAX" => existing_score.max(weighted_score),
                        _ => *existing_score + weighted_score,
                    };
                    true
                } else {
                    false
                }
            });
        }

        // Store in destination
        let mut added = 0i64;
        for (member, score) in combined {
            self.base
                .local_registry
                .execute_sorted_set(
                    &destination,
                    "zadd",
                    &[
                        serde_json::to_value(member).unwrap(),
                        serde_json::to_value(score).unwrap(),
                    ],
                )
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;
            added += 1;
        }

        debug!("ZINTERSTORE {} {:?} -> {} added", destination, keys, added);
        Ok(RespValue::Integer(added))
    }

    /// ZDIFFSTORE destination numkeys key [key ...] - Store difference in destination
    async fn cmd_zdiffstore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zdiffstore' command".to_string(),
            ));
        }

        let destination = self.get_string_arg(args, 0, "ZDIFFSTORE")?;
        let numkeys = self.get_int_arg(args, 1, "ZDIFFSTORE")? as usize;
        if args.len() < 2 + numkeys {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zdiffstore' command".to_string(),
            ));
        }

        let mut keys = Vec::new();
        for i in 2..(2 + numkeys) {
            keys.push(self.get_string_arg(args, i, "ZDIFFSTORE")?);
        }

        // Get members from first key
        let first_result = self
            .base
            .local_registry
            .execute_sorted_set(&keys[0], "get_all_members", &[])
            .await
            .unwrap_or(serde_json::Value::Array(vec![]));

        let first_members: Vec<(String, f64)> =
            serde_json::from_value(first_result).unwrap_or_default();

        let mut combined: std::collections::HashMap<String, f64> =
            first_members.into_iter().collect();

        // Remove members from subsequent keys
        for key in keys.iter().skip(1) {
            let result = self
                .base
                .local_registry
                .execute_sorted_set(key, "get_all_members", &[])
                .await
                .unwrap_or(serde_json::Value::Array(vec![]));

            let members: Vec<(String, f64)> = serde_json::from_value(result).unwrap_or_default();

            for (member, _) in members {
                combined.remove(&member);
            }
        }

        // Store in destination
        let mut added = 0i64;
        for (member, score) in combined {
            self.base
                .local_registry
                .execute_sorted_set(
                    &destination,
                    "zadd",
                    &[
                        serde_json::to_value(member).unwrap(),
                        serde_json::to_value(score).unwrap(),
                    ],
                )
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;
            added += 1;
        }

        debug!("ZDIFFSTORE {} {:?} -> {} added", destination, keys, added);
        Ok(RespValue::Integer(added))
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
            "ZCOUNT" => self.cmd_zcount(args).await,
            "ZRANK" => self.cmd_zrank(args).await,
            "ZREVRANK" => self.cmd_zrevrank(args).await,
            "ZREVRANGE" => self.cmd_zrevrange(args).await,
            "ZRANGEBYSCORE" => self.cmd_zrangebyscore(args).await,
            "ZREVRANGEBYSCORE" => self.cmd_zrevrangebyscore(args).await,
            "ZREMRANGEBYRANK" => self.cmd_zremrangebyrank(args).await,
            "ZREMRANGEBYSCORE" => self.cmd_zremrangebyscore(args).await,
            "ZPOPMIN" => self.cmd_zpopmin(args).await,
            "ZPOPMAX" => self.cmd_zpopmax(args).await,
            "ZLEXCOUNT" => self.cmd_zlexcount(args).await,
            "ZSCAN" => self.cmd_zscan(args).await,
            "ZMSCORE" => self.cmd_zmscore(args).await,
            "ZRANGEBYLEX" => self.cmd_zrangebylex(args).await,
            "ZREVRANGEBYLEX" => self.cmd_zrevrangebylex(args).await,
            "ZREMRANGEBYLEX" => self.cmd_zremrangebylex(args).await,
            "ZRANDMEMBER" => self.cmd_zrandmember(args).await,
            "ZUNION" => self.cmd_zunion(args).await,
            "ZINTER" => self.cmd_zinter(args).await,
            "ZDIFF" => self.cmd_zdiff(args).await,
            "ZUNIONSTORE" => self.cmd_zunionstore(args).await,
            "ZINTERSTORE" => self.cmd_zinterstore(args).await,
            "ZDIFFSTORE" => self.cmd_zdiffstore(args).await,
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
            "ZREVRANGE",
            "ZRANGEBYSCORE",
            "ZREVRANGEBYSCORE",
            "ZCOUNT",
            "ZRANK",
            "ZREVRANK",
            "ZREMRANGEBYRANK",
            "ZREMRANGEBYSCORE",
            "ZPOPMIN",
            "ZPOPMAX",
            "ZLEXCOUNT",
            "ZSCAN",
            "ZMSCORE",
            "ZRANGEBYLEX",
            "ZREVRANGEBYLEX",
            "ZREMRANGEBYLEX",
            "ZRANDMEMBER",
            "ZUNION",
            "ZINTER",
            "ZDIFF",
            "ZUNIONSTORE",
            "ZINTERSTORE",
            "ZDIFFSTORE",
        ]
    }
}
