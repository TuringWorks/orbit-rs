//! Sorted set command handlers for Redis RESP protocol
//!
//! This module implements Redis sorted set commands (ZADD, ZREM, ZRANGE, etc.)
//! that operate on sorted set data structures with scores.

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::error::{ProtocolError, ProtocolResult};
use crate::resp::RespValue;
use async_trait::async_trait;
use bytes::Bytes;
use orbit_client::OrbitClient;
use std::sync::Arc;
use tracing::debug;

pub struct SortedSetCommands {
    base: BaseCommandHandler,
}

impl SortedSetCommands {
    pub fn new(orbit_client: Arc<OrbitClient>) -> Self {
        let local_registry = Arc::new(crate::resp::simple_local::SimpleLocalRegistry::new());
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    /// ZADD key [NX|XX] [GT|LT] [CH] [INCR] score member [score member ...]
    async fn cmd_zadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zadd' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZADD")?;

        // Parse options and score-member pairs
        let mut i = 1;
        let mut nx = false;
        let mut xx = false;
        let mut ch = false;
        let mut incr = false;

        // Parse options
        while i < args.len() {
            if let Some(option) = args[i].as_string() {
                match option.to_uppercase().as_str() {
                    "NX" => {
                        nx = true;
                        i += 1;
                        continue;
                    }
                    "XX" => {
                        xx = true;
                        i += 1;
                        continue;
                    }
                    "CH" => {
                        ch = true;
                        i += 1;
                        continue;
                    }
                    "INCR" => {
                        incr = true;
                        i += 1;
                        continue;
                    }
                    _ => break, // Not an option, must be score
                }
            }
            break;
        }

        if nx && xx {
            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
        }

        // Parse score-member pairs
        let mut score_members = Vec::new();
        while i + 1 < args.len() {
            let score = self.get_float_arg(args, i, "ZADD")?;
            let member = self.get_string_arg(args, i + 1, "ZADD")?;
            score_members.push((score, member));
            i += 2;
        }

        if score_members.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zadd' command".to_string(),
            ));
        }

        if incr && score_members.len() > 1 {
            return Err(ProtocolError::RespError(
                "ERR INCR option supports a single increment-element pair".to_string(),
            ));
        }

        let params = serde_json::json!({
            "score_members": score_members,
            "nx": nx,
            "xx": xx,
            "ch": ch,
            "incr": incr
        });

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zadd", &[params])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        if incr {
            let new_score: Option<f64> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;
            match new_score {
                Some(score) => Ok(RespValue::BulkString(Bytes::from(
                    score.to_string().into_bytes(),
                ))),
                None => Ok(RespValue::null()),
            }
        } else {
            let added: i64 = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;
            debug!("ZADD {} -> {} elements added/updated", key, added);
            Ok(RespValue::Integer(added))
        }
    }

    /// ZREM key member [member ...]
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
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        debug!("ZREM {} {:?} -> {} removed", key, members, removed);
        Ok(RespValue::Integer(removed))
    }

    /// ZCARD key
    async fn cmd_zcard(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZCARD", args, 1)?;

        let key = self.get_string_arg(args, 0, "ZCARD")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zcard", &[])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let count: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        debug!("ZCARD {} -> {}", key, count);
        Ok(RespValue::Integer(count))
    }

    /// ZSCORE key member
    async fn cmd_zscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZSCORE", args, 2)?;

        let key = self.get_string_arg(args, 0, "ZSCORE")?;
        let member = self.get_string_arg(args, 1, "ZSCORE")?;

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zscore", &[serde_json::Value::String(member.clone())])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let score: Option<f64> = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        match score {
            Some(s) => {
                debug!("ZSCORE {} {} -> {}", key, member, s);
                Ok(RespValue::BulkString(Bytes::from(
                    s.to_string().into_bytes(),
                )))
            }
            None => {
                debug!("ZSCORE {} {} -> (nil)", key, member);
                Ok(RespValue::null())
            }
        }
    }

    /// ZINCRBY key increment member
    async fn cmd_zincrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZINCRBY", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZINCRBY")?;
        let increment = self.get_float_arg(args, 1, "ZINCRBY")?;
        let member = self.get_string_arg(args, 2, "ZINCRBY")?;

        let params = serde_json::json!({
            "increment": increment,
            "member": member
        });

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zincrby", &[params])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let new_score: f64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        debug!("ZINCRBY {} {} {} -> {}", key, increment, member, new_score);
        Ok(RespValue::BulkString(Bytes::from(
            new_score.to_string().into_bytes(),
        )))
    }

    /// ZRANGE key start stop [BYSCORE|BYLEX] [REV] [LIMIT offset count] [WITHSCORES]
    async fn cmd_zrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrange' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANGE")?;
        let start = self.get_int_arg(args, 1, "ZRANGE")?;
        let stop = self.get_int_arg(args, 2, "ZRANGE")?;

        // Parse options
        let mut withscores = false;
        let mut rev = false;
        let mut i = 3;

        while i < args.len() {
            if let Some(option) = args[i].as_string() {
                match option.to_uppercase().as_str() {
                    "WITHSCORES" => {
                        withscores = true;
                        i += 1;
                    }
                    "REV" => {
                        rev = true;
                        i += 1;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        let params = serde_json::json!({
            "start": start,
            "stop": stop,
            "withscores": withscores,
            "rev": rev
        });

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zrange", &[params])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        if withscores {
            let members_with_scores: Vec<(String, f64)> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

            let mut resp_result = Vec::new();
            for (member, score) in members_with_scores {
                resp_result.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
                resp_result.push(RespValue::BulkString(Bytes::from(
                    score.to_string().into_bytes(),
                )));
            }

            debug!(
                "ZRANGE {} {} {} -> {} members with scores",
                key,
                start,
                stop,
                resp_result.len() / 2
            );
            Ok(RespValue::Array(resp_result))
        } else {
            let members: Vec<String> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

            let resp_result: Vec<RespValue> = members
                .into_iter()
                .map(|member| RespValue::BulkString(Bytes::from(member.into_bytes())))
                .collect();

            debug!(
                "ZRANGE {} {} {} -> {} members",
                key,
                start,
                stop,
                resp_result.len()
            );
            Ok(RespValue::Array(resp_result))
        }
    }

    /// ZRANGEBYSCORE key min max [WITHSCORES] [LIMIT offset count]
    async fn cmd_zrangebyscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrangebyscore' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANGEBYSCORE")?;
        let min = self.get_string_arg(args, 1, "ZRANGEBYSCORE")?; // Can be "-inf", "+inf", or number
        let max = self.get_string_arg(args, 2, "ZRANGEBYSCORE")?;

        // Parse options
        let mut withscores = false;
        let mut limit: Option<(i64, i64)> = None;
        let mut i = 3;

        while i < args.len() {
            if let Some(option) = args[i].as_string() {
                match option.to_uppercase().as_str() {
                    "WITHSCORES" => {
                        withscores = true;
                        i += 1;
                    }
                    "LIMIT" => {
                        if i + 2 >= args.len() {
                            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
                        }
                        let offset = self.get_int_arg(args, i + 1, "ZRANGEBYSCORE")?;
                        let count = self.get_int_arg(args, i + 2, "ZRANGEBYSCORE")?;
                        limit = Some((offset, count));
                        i += 3;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        let params = serde_json::json!({
            "min": min,
            "max": max,
            "withscores": withscores,
            "limit": limit
        });

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zrangebyscore", &[params])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        if withscores {
            let members_with_scores: Vec<(String, f64)> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

            let mut resp_result = Vec::new();
            for (member, score) in members_with_scores {
                resp_result.push(RespValue::BulkString(Bytes::from(member.into_bytes())));
                resp_result.push(RespValue::BulkString(Bytes::from(
                    score.to_string().into_bytes(),
                )));
            }

            debug!(
                "ZRANGEBYSCORE {} {} {} -> {} members with scores",
                key,
                min,
                max,
                resp_result.len() / 2
            );
            Ok(RespValue::Array(resp_result))
        } else {
            let members: Vec<String> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

            let resp_result: Vec<RespValue> = members
                .into_iter()
                .map(|member| RespValue::BulkString(Bytes::from(member.into_bytes())))
                .collect();

            debug!(
                "ZRANGEBYSCORE {} {} {} -> {} members",
                key,
                min,
                max,
                resp_result.len()
            );
            Ok(RespValue::Array(resp_result))
        }
    }

    /// ZCOUNT key min max
    async fn cmd_zcount(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("ZCOUNT", args, 3)?;

        let key = self.get_string_arg(args, 0, "ZCOUNT")?;
        let min = self.get_string_arg(args, 1, "ZCOUNT")?;
        let max = self.get_string_arg(args, 2, "ZCOUNT")?;

        let params = serde_json::json!({
            "min": min,
            "max": max
        });

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zcount", &[params])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        let count: i64 = serde_json::from_value(result)
            .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

        debug!("ZCOUNT {} {} {} -> {}", key, min, max, count);
        Ok(RespValue::Integer(count))
    }

    /// ZRANK key member [WITHSCORE]
    async fn cmd_zrank(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 || args.len() > 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrank' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "ZRANK")?;
        let member = self.get_string_arg(args, 1, "ZRANK")?;
        let withscore = args.len() == 3
            && args[2].as_string().map(|s| s.to_uppercase()) == Some("WITHSCORE".to_string());

        let params = serde_json::json!({
            "member": member,
            "withscore": withscore
        });

        let result = self
            .base
            .local_registry
            .execute_sorted_set(&key, "zrank", &[params])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        if withscore {
            let rank_score: Option<(i64, f64)> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

            match rank_score {
                Some((rank, score)) => {
                    debug!(
                        "ZRANK {} {} -> rank: {}, score: {}",
                        key, member, rank, score
                    );
                    Ok(RespValue::Array(vec![
                        RespValue::Integer(rank),
                        RespValue::BulkString(Bytes::from(score.to_string().into_bytes())),
                    ]))
                }
                None => {
                    debug!("ZRANK {} {} -> (nil)", key, member);
                    Ok(RespValue::null())
                }
            }
        } else {
            let rank: Option<i64> = serde_json::from_value(result)
                .map_err(|e| ProtocolError::RespError(format!("ERR serialization error: {}", e)))?;

            match rank {
                Some(r) => {
                    debug!("ZRANK {} {} -> {}", key, member, r);
                    Ok(RespValue::Integer(r))
                }
                None => {
                    debug!("ZRANK {} {} -> (nil)", key, member);
                    Ok(RespValue::null())
                }
            }
        }
    }
}

#[async_trait]
impl CommandHandler for SortedSetCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name {
            "ZADD" => self.cmd_zadd(args).await,
            "ZREM" => self.cmd_zrem(args).await,
            "ZCARD" => self.cmd_zcard(args).await,
            "ZSCORE" => self.cmd_zscore(args).await,
            "ZINCRBY" => self.cmd_zincrby(args).await,
            "ZRANGE" => self.cmd_zrange(args).await,
            "ZRANGEBYSCORE" => self.cmd_zrangebyscore(args).await,
            "ZCOUNT" => self.cmd_zcount(args).await,
            "ZRANK" => self.cmd_zrank(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown sorted set command '{}'",
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
