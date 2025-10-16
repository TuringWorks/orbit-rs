//! Simplified local invocation system for RESP protocol actors

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::resp::actors::{HashActor, KeyValueActor, ListActor, SetActor, SortedSetActor};
use orbit_shared::{AddressableInvocation, Key, OrbitError, OrbitResult};

/// Simplified local actor registry for core RESP commands
pub struct SimpleLocalRegistry {
    /// KeyValue actors
    keyvalue_actors: Arc<RwLock<HashMap<String, KeyValueActor>>>,
    /// Hash actors
    hash_actors: Arc<RwLock<HashMap<String, HashActor>>>,
    /// List actors
    list_actors: Arc<RwLock<HashMap<String, ListActor>>>,
    /// Set actors
    set_actors: Arc<RwLock<HashMap<String, SetActor>>>,
    /// Sorted set actors
    sorted_set_actors: Arc<RwLock<HashMap<String, SortedSetActor>>>,
}

impl SimpleLocalRegistry {
    pub fn new() -> Self {
        Self {
            keyvalue_actors: Arc::new(RwLock::new(HashMap::new())),
            hash_actors: Arc::new(RwLock::new(HashMap::new())),
            list_actors: Arc::new(RwLock::new(HashMap::new())),
            set_actors: Arc::new(RwLock::new(HashMap::new())),
            sorted_set_actors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute keyvalue actor methods
    pub async fn execute_keyvalue(
        &self,
        key: &str,
        method: &str,
        args: &[Value],
    ) -> OrbitResult<Value> {
        let mut actors = self.keyvalue_actors.write().await;
        let actor = actors
            .entry(key.to_string())
            .or_insert_with(KeyValueActor::new);

        debug!("Executing KeyValue method '{}' on key '{}'", method, key);

        match method {
            "get_value" => {
                let result = actor.get_value().cloned();
                Ok(serde_json::to_value(result)?)
            }
            "set_value" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let value: String = serde_json::from_value(args[0].clone())?;
                actor.set_value(value);
                Ok(serde_json::to_value(())?)
            }
            "delete_value" => {
                let existed = actor.value.is_some();
                actor.value = None;
                actor.expiration = None;
                Ok(serde_json::to_value(existed)?)
            }
            "exists" => {
                let exists = actor.value.is_some() && !actor.is_expired();
                Ok(serde_json::to_value(exists)?)
            }
            "get_ttl" => {
                let result = actor.get_ttl();
                Ok(serde_json::to_value(result)?)
            }
            "set_expiration" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let seconds: u64 = serde_json::from_value(args[0].clone())?;
                actor.set_expiration(seconds);
                Ok(serde_json::to_value(())?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "KeyValueActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {method}"),
            }),
        }
    }

    /// Execute hash actor methods
    pub async fn execute_hash(
        &self,
        key: &str,
        method: &str,
        args: &[Value],
    ) -> OrbitResult<Value> {
        let mut actors = self.hash_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(HashActor::new);

        debug!("Executing Hash method '{}' on key '{}'", method, key);

        match method {
            "hget" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "HashActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let field: String = serde_json::from_value(args[0].clone())?;
                let result = actor.hget(&field).cloned();
                Ok(serde_json::to_value(result)?)
            }
            "hset" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "HashActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let field: String = serde_json::from_value(args[0].clone())?;
                let value: String = serde_json::from_value(args[1].clone())?;
                let result = actor.hset(field, value);
                Ok(serde_json::to_value(result)?)
            }
            "hgetall" => {
                let result = actor.hgetall();
                Ok(serde_json::to_value(result)?)
            }
            "hlen" => {
                let result = actor.hlen();
                Ok(serde_json::to_value(result)?)
            }
            "hexists" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "HashActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let field: String = serde_json::from_value(args[0].clone())?;
                let result = actor.hexists(&field);
                Ok(serde_json::to_value(result)?)
            }
            "hdel" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "HashActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let field: String = serde_json::from_value(args[0].clone())?;
                let result = actor.hdel(&field);
                Ok(serde_json::to_value(result)?)
            }
            "hkeys" => {
                let result = actor.hkeys();
                Ok(serde_json::to_value(result)?)
            }
            "hvals" => {
                let result = actor.hvals();
                Ok(serde_json::to_value(result)?)
            }
            "hincrby" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "HashActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let field: String = serde_json::from_value(args[0].clone())?;
                let increment: i64 = serde_json::from_value(args[1].clone())?;
                let result = actor.hincrby(field, increment);
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "HashActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {method}"),
            }),
        }
    }

    /// Execute list actor methods
    pub async fn execute_list(
        &self,
        key: &str,
        method: &str,
        args: &[Value],
    ) -> OrbitResult<Value> {
        let mut actors = self.list_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(ListActor::new);

        debug!("Executing List method '{}' on key '{}'", method, key);

        match method {
            "lpush" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let values: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.lpush(values);
                Ok(serde_json::to_value(result)?)
            }
            "rpush" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let values: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.rpush(values);
                Ok(serde_json::to_value(result)?)
            }
            "llen" => {
                let result = actor.llen();
                Ok(serde_json::to_value(result)?)
            }
            "lrange" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let start: i64 = serde_json::from_value(args[0].clone())?;
                let stop: i64 = serde_json::from_value(args[1].clone())?;
                let result = actor.lrange(start, stop);
                Ok(serde_json::to_value(result)?)
            }
            "lindex" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let index: i64 = serde_json::from_value(args[0].clone())?;
                let result = actor.lindex(index).cloned();
                Ok(serde_json::to_value(result)?)
            }
            "lpop" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let count: usize = serde_json::from_value(args[0].clone())?;
                let result = actor.lpop(count);
                Ok(serde_json::to_value(result)?)
            }
            "rpop" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let count: usize = serde_json::from_value(args[0].clone())?;
                let result = actor.rpop(count);
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "ListActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {method}"),
            }),
        }
    }

    /// Execute set actor methods
    pub async fn execute_set(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.set_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(SetActor::new);

        debug!("Executing Set method '{}' on key '{}'", method, key);

        match method {
            "sadd" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let members: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.sadd(members);
                Ok(serde_json::to_value(result)?)
            }
            "scard" => {
                let result = actor.scard();
                Ok(serde_json::to_value(result)?)
            }
            "sismember" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let member: String = serde_json::from_value(args[0].clone())?;
                let result = actor.sismember(&member);
                Ok(serde_json::to_value(result)?)
            }
            "smembers" => {
                let result = actor.smembers();
                Ok(serde_json::to_value(result)?)
            }
            "srem" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let members: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.srem(members);
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "SetActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {method}"),
            }),
        }
    }

    /// Execute sorted set actor methods
    pub async fn execute_sorted_set(
        &self,
        key: &str,
        method: &str,
        args: &[Value],
    ) -> OrbitResult<Value> {
        let mut actors = self.sorted_set_actors.write().await;
        let actor = actors
            .entry(key.to_string())
            .or_insert_with(SortedSetActor::new);

        debug!("Executing SortedSet method '{}' on key '{}'", method, key);

        match method {
            "zadd" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (JSON params)".to_string(),
                    });
                }

                // Extract complex zadd parameters
                let params = &args[0];
                let score_members: Vec<(f64, String)> = serde_json::from_value(
                    params
                        .get("score_members")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let nx: bool = params.get("nx").and_then(|v| v.as_bool()).unwrap_or(false);
                let xx: bool = params.get("xx").and_then(|v| v.as_bool()).unwrap_or(false);
                let ch: bool = params.get("ch").and_then(|v| v.as_bool()).unwrap_or(false);
                let incr: bool = params
                    .get("incr")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let result = if nx || xx || ch || incr {
                    actor
                        .zadd_with_options(score_members, nx, xx, ch, incr)
                        .map_err(|e| OrbitError::InvocationFailed {
                            addressable_type: "SortedSetActor".to_string(),
                            method: method.to_string(),
                            reason: e,
                        })?
                } else {
                    // Simple zadd - count added elements
                    let mut added = 0i64;
                    for (score, member) in score_members {
                        if actor.zadd(member, score) {
                            added += 1;
                        }
                    }
                    serde_json::to_value(added).unwrap()
                };

                Ok(result)
            }
            "zrem" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let members: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.zrem(members);
                Ok(serde_json::to_value(result)?)
            }
            "zcard" => {
                let result = actor.zcard();
                Ok(serde_json::to_value(result)?)
            }
            "zscore" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let member: String = serde_json::from_value(args[0].clone())?;
                let result = actor.zscore(&member);
                Ok(serde_json::to_value(result)?)
            }
            "zincrby" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (JSON params)".to_string(),
                    });
                }

                let params = &args[0];
                let increment: f64 = serde_json::from_value(
                    params
                        .get("increment")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let member: String = serde_json::from_value(
                    params
                        .get("member")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;

                let result = actor.zincrby(member, increment);
                Ok(serde_json::to_value(result)?)
            }
            "zrange" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (JSON params)".to_string(),
                    });
                }

                let params = &args[0];
                let start: i64 = serde_json::from_value(
                    params
                        .get("start")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let stop: i64 = serde_json::from_value(
                    params
                        .get("stop")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let withscores: bool = params
                    .get("withscores")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let rev: bool = params.get("rev").and_then(|v| v.as_bool()).unwrap_or(false);

                let result = if rev || withscores {
                    actor.zrange_with_options(start, stop, withscores, rev)
                } else {
                    actor.zrange(start, stop, withscores)
                };

                // Convert result based on withscores
                if withscores {
                    // Return Vec<(String, f64)> for withscores
                    let with_scores: Vec<(String, f64)> = result
                        .into_iter()
                        .map(|(member, score)| (member, score.unwrap_or(0.0)))
                        .collect();
                    Ok(serde_json::to_value(with_scores)?)
                } else {
                    // Return Vec<String> for members only
                    let members: Vec<String> =
                        result.into_iter().map(|(member, _)| member).collect();
                    Ok(serde_json::to_value(members)?)
                }
            }
            "zrangebyscore" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (JSON params)".to_string(),
                    });
                }

                let params = &args[0];
                let min: String = serde_json::from_value(
                    params
                        .get("min")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let max: String = serde_json::from_value(
                    params
                        .get("max")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let withscores: bool = params
                    .get("withscores")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let limit: Option<(i64, i64)> = params
                    .get("limit")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());

                let result = actor
                    .zrangebyscore_with_options(&min, &max, withscores, limit)
                    .map_err(|e| OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    })?;

                // Convert result based on withscores
                if withscores {
                    // Return Vec<(String, f64)> for withscores
                    let with_scores: Vec<(String, f64)> = result
                        .into_iter()
                        .map(|(member, score)| (member, score.unwrap_or(0.0)))
                        .collect();
                    Ok(serde_json::to_value(with_scores)?)
                } else {
                    // Return Vec<String> for members only
                    let members: Vec<String> =
                        result.into_iter().map(|(member, _)| member).collect();
                    Ok(serde_json::to_value(members)?)
                }
            }
            "zcount" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (JSON params)".to_string(),
                    });
                }

                let params = &args[0];
                let min: String = serde_json::from_value(
                    params
                        .get("min")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let max: String = serde_json::from_value(
                    params
                        .get("max")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;

                let result = actor.zcount_with_bounds(&min, &max).map_err(|e| {
                    OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }
                })?;

                Ok(serde_json::to_value(result)?)
            }
            "zrank" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (JSON params)".to_string(),
                    });
                }

                let params = &args[0];
                let member: String = serde_json::from_value(
                    params
                        .get("member")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )?;
                let withscore: bool = params
                    .get("withscore")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if withscore {
                    let result = match actor.zrank_with_score(&member, true) {
                        Some((rank, score)) => Some((rank as i64, score)),
                        None => None,
                    };
                    Ok(serde_json::to_value(result)?)
                } else {
                    let result = actor.zrank(&member).map(|rank| rank as i64);
                    Ok(serde_json::to_value(result)?)
                }
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "SortedSetActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {method}"),
            }),
        }
    }

    /// Execute an invocation
    pub async fn execute_invocation(
        &self,
        invocation: &AddressableInvocation,
    ) -> OrbitResult<Value> {
        let key = match &invocation.reference.key {
            Key::StringKey { key } => key.clone(),
            Key::Int32Key { key } => key.to_string(),
            Key::Int64Key { key } => key.to_string(),
            Key::NoKey => "no-key".to_string(),
        };

        let args: Vec<Value> = invocation
            .args
            .iter()
            .map(|arg| arg.value.clone())
            .collect();

        match invocation.reference.addressable_type.as_str() {
            "KeyValueActor" => self.execute_keyvalue(&key, &invocation.method, &args).await,
            "HashActor" => self.execute_hash(&key, &invocation.method, &args).await,
            "ListActor" => self.execute_list(&key, &invocation.method, &args).await,
            "SetActor" => self.execute_set(&key, &invocation.method, &args).await,
            "SortedSetActor" => {
                self.execute_sorted_set(&key, &invocation.method, &args)
                    .await
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: invocation.reference.addressable_type.clone(),
                method: invocation.method.clone(),
                reason: format!(
                    "Unknown actor type: {}",
                    invocation.reference.addressable_type
                ),
            }),
        }
    }
}

impl Default for SimpleLocalRegistry {
    fn default() -> Self {
        Self::new()
    }
}
