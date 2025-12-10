//! Simplified local invocation system for RESP protocol actors

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::protocols::persistence::redis_data::{RedisDataProvider, RedisValue};
use crate::protocols::resp::actors::{
    HashActor, KeyValueActor, ListActor, SetActor, SortedSetActor, StreamActor,
};
use orbit_shared::{AddressableInvocation, Key, OrbitError, OrbitResult};

/// Simplified local actor registry for core RESP commands
pub struct SimpleLocalRegistry {
    /// KeyValue actors (in-memory cache)
    keyvalue_actors: Arc<RwLock<HashMap<String, KeyValueActor>>>,
    /// Hash actors
    hash_actors: Arc<RwLock<HashMap<String, HashActor>>>,
    /// List actors
    list_actors: Arc<RwLock<HashMap<String, ListActor>>>,
    /// Set actors
    set_actors: Arc<RwLock<HashMap<String, SetActor>>>,
    /// Sorted set actors
    sorted_set_actors: Arc<RwLock<HashMap<String, SortedSetActor>>>,
    /// Stream actors
    stream_actors: Arc<RwLock<HashMap<String, StreamActor>>>,
    /// Optional persistent storage provider
    persistent_storage: Option<Arc<dyn RedisDataProvider>>,
}

impl SimpleLocalRegistry {
    pub fn new() -> Self {
        Self {
            keyvalue_actors: Arc::new(RwLock::new(HashMap::new())),
            hash_actors: Arc::new(RwLock::new(HashMap::new())),
            list_actors: Arc::new(RwLock::new(HashMap::new())),
            set_actors: Arc::new(RwLock::new(HashMap::new())),
            sorted_set_actors: Arc::new(RwLock::new(HashMap::new())),
            stream_actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: None,
        }
    }

    /// Create a new registry with persistent storage
    pub fn with_persistence(provider: Arc<dyn RedisDataProvider>) -> Self {
        Self {
            keyvalue_actors: Arc::new(RwLock::new(HashMap::new())),
            hash_actors: Arc::new(RwLock::new(HashMap::new())),
            list_actors: Arc::new(RwLock::new(HashMap::new())),
            set_actors: Arc::new(RwLock::new(HashMap::new())),
            sorted_set_actors: Arc::new(RwLock::new(HashMap::new())),
            stream_actors: Arc::new(RwLock::new(HashMap::new())),
            persistent_storage: Some(provider),
        }
    }

    /// Load all keys from persistent storage on startup
    pub async fn load_from_persistence(&self) -> OrbitResult<()> {
        if let Some(provider) = &self.persistent_storage {
            debug!("Loading keys from persistent storage");
            let keys = provider.keys("*").await?;
            let mut actors = self.keyvalue_actors.write().await;

            for key in keys {
                if let Some(value) = provider.get(&key).await? {
                    let mut actor = KeyValueActor::new();
                    actor.set_value(value.data);
                    if let Some(expiration) = value.expiration {
                        // Convert expiration timestamp to TTL seconds
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        if expiration > now {
                            actor.set_expiration(expiration - now);
                        }
                    }
                    actors.insert(key, actor);
                }
            }
            debug!("Loaded {} keys from persistent storage", actors.len());
        }
        Ok(())
    }

    /// Execute keyvalue actor methods
    pub async fn execute_keyvalue(
        &self,
        key: &str,
        method: &str,
        args: &[Value],
    ) -> OrbitResult<Value> {
        // For get_value, check persistent storage first before creating actor
        if method == "get_value" {
            if let Some(provider) = &self.persistent_storage {
                if let Ok(Some(redis_value)) = provider.get(key).await {
                    // Update in-memory cache
                    let mut actors = self.keyvalue_actors.write().await;
                    let actor = actors
                        .entry(key.to_string())
                        .or_insert_with(KeyValueActor::new);
                    actor.set_value(redis_value.data.clone());
                    if let Some(expiration) = redis_value.expiration {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        if expiration > now {
                            actor.set_expiration(expiration - now);
                        }
                    }
                    return Ok(serde_json::to_value(Some(redis_value.data))?);
                }
            }
        }

        let mut actors = self.keyvalue_actors.write().await;
        let actor = actors
            .entry(key.to_string())
            .or_insert_with(KeyValueActor::new);

        debug!("Executing KeyValue method '{}' on key '{}'", method, key);

        match method {
            "get_value" => {
                // Fall back to in-memory value (persistent storage already checked above)
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
                actor.set_value(value.clone());

                // Persist to storage if available
                if let Some(provider) = &self.persistent_storage {
                    let redis_value = RedisValue::new(value);
                    if let Err(e) = provider.set(key, redis_value).await {
                        debug!("Failed to persist key {}: {}", key, e);
                    }
                }

                Ok(serde_json::to_value(())?)
            }
            "delete_value" => {
                let existed = actor.value.is_some();
                actor.value = None;
                actor.expiration = None;

                // Delete from persistent storage if available
                if let Some(provider) = &self.persistent_storage {
                    let _ = provider.delete(key).await;
                }

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

                // Update expiration in persistent storage if available
                if let Some(provider) = &self.persistent_storage {
                    if let Some(value_str) = actor.get_value() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let expiration = now + seconds;
                        let mut redis_value = RedisValue::new(value_str.clone());
                        redis_value.expiration = Some(expiration);
                        if let Err(e) = provider.set(key, redis_value).await {
                            debug!("Failed to persist expiration for key {}: {}", key, e);
                        }
                    }
                }

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
            "lset" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let index: i64 = serde_json::from_value(args[0].clone())?;
                let value: String = serde_json::from_value(args[1].clone())?;
                let result = actor.lset(index, value);
                Ok(serde_json::to_value(result)?)
            }
            "lrem" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let count: i64 = serde_json::from_value(args[0].clone())?;
                let value: String = serde_json::from_value(args[1].clone())?;
                let result = actor.lrem(count, &value);
                Ok(serde_json::to_value(result)?)
            }
            "ltrim" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let start: i64 = serde_json::from_value(args[0].clone())?;
                let stop: i64 = serde_json::from_value(args[1].clone())?;
                actor.ltrim(start, stop);
                Ok(serde_json::to_value(())?)
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
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let member: String = serde_json::from_value(args[0].clone())?;
                let score: f64 = serde_json::from_value(args[1].clone())?;
                let result = actor.zadd(member, score);
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
            "zrange" => {
                if args.len() != 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 3 arguments".to_string(),
                    });
                }
                let start: i64 = serde_json::from_value(args[0].clone())?;
                let stop: i64 = serde_json::from_value(args[1].clone())?;
                let with_scores: bool = serde_json::from_value(args[2].clone())?;
                let result = actor.zrange(start, stop, with_scores);
                Ok(serde_json::to_value(result)?)
            }
            "zincrby" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let member: String = serde_json::from_value(args[0].clone())?;
                let increment: f64 = serde_json::from_value(args[1].clone())?;
                let result = actor.zincrby(member, increment);
                Ok(serde_json::to_value(result)?)
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
            "zcount" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (min, max)".to_string(),
                    });
                }
                let min_score: f64 = serde_json::from_value(args[0].clone())?;
                let max_score: f64 = serde_json::from_value(args[1].clone())?;
                let result = actor.zcount(min_score, max_score);
                Ok(serde_json::to_value(result)?)
            }
            "zrank" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let member: String = serde_json::from_value(args[0].clone())?;
                let result = actor.zrank(&member);
                Ok(serde_json::to_value(result)?)
            }
            "zrevrank" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let member: String = serde_json::from_value(args[0].clone())?;
                let result = actor.zrevrank(&member);
                Ok(serde_json::to_value(result)?)
            }
            "zrevrange" => {
                if args.len() != 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 3 arguments (start, stop, with_scores)".to_string(),
                    });
                }
                let start: i64 = serde_json::from_value(args[0].clone())?;
                let stop: i64 = serde_json::from_value(args[1].clone())?;
                let with_scores: bool = serde_json::from_value(args[2].clone())?;
                let result = actor.zrevrange(start, stop, with_scores);
                Ok(serde_json::to_value(result)?)
            }
            "zrangebyscore" => {
                if args.len() != 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 3 arguments (min, max, with_scores)".to_string(),
                    });
                }
                let min_score: f64 = serde_json::from_value(args[0].clone())?;
                let max_score: f64 = serde_json::from_value(args[1].clone())?;
                let with_scores: bool = serde_json::from_value(args[2].clone())?;
                let result = actor.zrangebyscore(min_score, max_score, with_scores);
                Ok(serde_json::to_value(result)?)
            }
            "zrevrangebyscore" => {
                if args.len() != 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 3 arguments (max, min, with_scores)".to_string(),
                    });
                }
                let max_score: f64 = serde_json::from_value(args[0].clone())?;
                let min_score: f64 = serde_json::from_value(args[1].clone())?;
                let with_scores: bool = serde_json::from_value(args[2].clone())?;
                let result = actor.zrevrangebyscore(max_score, min_score, with_scores);
                Ok(serde_json::to_value(result)?)
            }
            "zremrangebyrank" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (start, stop)".to_string(),
                    });
                }
                let start: i64 = serde_json::from_value(args[0].clone())?;
                let stop: i64 = serde_json::from_value(args[1].clone())?;
                let result = actor.zremrangebyrank(start, stop);
                Ok(serde_json::to_value(result)?)
            }
            "zremrangebyscore" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (min, max)".to_string(),
                    });
                }
                let min_score: f64 = serde_json::from_value(args[0].clone())?;
                let max_score: f64 = serde_json::from_value(args[1].clone())?;
                let result = actor.zremrangebyscore(min_score, max_score);
                Ok(serde_json::to_value(result)?)
            }
            "zpopmin" => {
                let count: usize = if args.is_empty() {
                    1
                } else {
                    serde_json::from_value(args[0].clone())?
                };
                let result = actor.zpopmin(count);
                Ok(serde_json::to_value(result)?)
            }
            "zpopmax" => {
                let count: usize = if args.is_empty() {
                    1
                } else {
                    serde_json::from_value(args[0].clone())?
                };
                let result = actor.zpopmax(count);
                Ok(serde_json::to_value(result)?)
            }
            "zlexcount" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (min, max)".to_string(),
                    });
                }
                let min: String = serde_json::from_value(args[0].clone())?;
                let max: String = serde_json::from_value(args[1].clone())?;
                let result = actor.zlexcount(&min, &max);
                Ok(serde_json::to_value(result)?)
            }
            "zscan" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (cursor, count)".to_string(),
                    });
                }
                let cursor: usize = serde_json::from_value(args[0].clone())?;
                let count: usize = serde_json::from_value(args[1].clone())?;
                let result = actor.zscan(cursor, count);
                Ok(serde_json::to_value(result)?)
            }
            "zmscore" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (members array)".to_string(),
                    });
                }
                let members: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.zmscore(&members);
                Ok(serde_json::to_value(result)?)
            }
            "zrangebylex" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 2 arguments (min, max)".to_string(),
                    });
                }
                let min: String = serde_json::from_value(args[0].clone())?;
                let max: String = serde_json::from_value(args[1].clone())?;
                let offset: Option<usize> = if args.len() > 2 {
                    serde_json::from_value(args[2].clone()).ok()
                } else {
                    None
                };
                let count: Option<usize> = if args.len() > 3 {
                    serde_json::from_value(args[3].clone()).ok()
                } else {
                    None
                };
                let result = actor.zrangebylex(&min, &max, offset, count);
                Ok(serde_json::to_value(result)?)
            }
            "zrevrangebylex" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 2 arguments (max, min)".to_string(),
                    });
                }
                let max: String = serde_json::from_value(args[0].clone())?;
                let min: String = serde_json::from_value(args[1].clone())?;
                let offset: Option<usize> = if args.len() > 2 {
                    serde_json::from_value(args[2].clone()).ok()
                } else {
                    None
                };
                let count: Option<usize> = if args.len() > 3 {
                    serde_json::from_value(args[3].clone()).ok()
                } else {
                    None
                };
                let result = actor.zrevrangebylex(&max, &min, offset, count);
                Ok(serde_json::to_value(result)?)
            }
            "zremrangebylex" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (min, max)".to_string(),
                    });
                }
                let min: String = serde_json::from_value(args[0].clone())?;
                let max: String = serde_json::from_value(args[1].clone())?;
                let result = actor.zremrangebylex(&min, &max);
                Ok(serde_json::to_value(result)?)
            }
            "zrandmember" => {
                let count: i64 = if !args.is_empty() {
                    serde_json::from_value(args[0].clone())?
                } else {
                    1
                };
                let with_scores: bool = if args.len() > 1 {
                    serde_json::from_value(args[1].clone())?
                } else {
                    false
                };
                let result = actor.zrandmember(count, with_scores);
                Ok(serde_json::to_value(result)?)
            }
            "get_all_members" => {
                let result = actor.get_all_members();
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "SortedSetActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {method}"),
            }),
        }
    }

    /// Execute stream actor methods
    pub async fn execute_stream(
        &self,
        key: &str,
        method: &str,
        args: &[Value],
    ) -> OrbitResult<Value> {
        let mut actors = self.stream_actors.write().await;
        let actor = actors
            .entry(key.to_string())
            .or_insert_with(StreamActor::new);

        debug!("Executing Stream method '{}' on key '{}'", method, key);

        match method {
            "xadd" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 2 arguments (id, fields)".to_string(),
                    });
                }
                let id: Option<String> = serde_json::from_value(args[0].clone())?;
                let fields: Vec<(String, String)> = serde_json::from_value(args[1].clone())?;
                let result = actor.xadd(id.as_deref(), fields);
                match result {
                    Ok(entry_id) => Ok(serde_json::to_value(entry_id)?),
                    Err(e) => Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }),
                }
            }
            "xlen" => {
                let result = actor.xlen();
                Ok(serde_json::to_value(result)?)
            }
            "xrange" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 2 arguments (start, end)".to_string(),
                    });
                }
                let start: String = serde_json::from_value(args[0].clone())?;
                let end: String = serde_json::from_value(args[1].clone())?;
                let count: Option<usize> = if args.len() > 2 {
                    serde_json::from_value(args[2].clone()).ok()
                } else {
                    None
                };
                let result = actor.xrange(&start, &end, count);
                Ok(serde_json::to_value(result)?)
            }
            "xrevrange" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 2 arguments (end, start)".to_string(),
                    });
                }
                let end: String = serde_json::from_value(args[0].clone())?;
                let start: String = serde_json::from_value(args[1].clone())?;
                let count: Option<usize> = if args.len() > 2 {
                    serde_json::from_value(args[2].clone()).ok()
                } else {
                    None
                };
                let result = actor.xrevrange(&end, &start, count);
                Ok(serde_json::to_value(result)?)
            }
            "xread" => {
                if args.is_empty() {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 1 argument (id)".to_string(),
                    });
                }
                let id: String = serde_json::from_value(args[0].clone())?;
                let count: Option<usize> = if args.len() > 1 {
                    serde_json::from_value(args[1].clone()).ok()
                } else {
                    None
                };
                let result = actor.xread(&id, count);
                Ok(serde_json::to_value(result)?)
            }
            "xtrim" => {
                if args.is_empty() {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 1 argument (maxlen)".to_string(),
                    });
                }
                let max_len: usize = serde_json::from_value(args[0].clone())?;
                let approximate: bool = if args.len() > 1 {
                    serde_json::from_value(args[1].clone()).unwrap_or(false)
                } else {
                    false
                };
                let result = actor.xtrim(max_len, approximate);
                Ok(serde_json::to_value(result)?)
            }
            "xdel" => {
                if args.is_empty() {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (ids)".to_string(),
                    });
                }
                let ids: Vec<String> = serde_json::from_value(args[0].clone())?;
                let result = actor.xdel(ids);
                Ok(serde_json::to_value(result)?)
            }
            "xinfo_stream" => {
                let result = actor.xinfo_stream();
                Ok(serde_json::to_value(result)?)
            }
            "xgroup_create" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (group_name, start_id)".to_string(),
                    });
                }
                let group_name: String = serde_json::from_value(args[0].clone())?;
                let start_id: String = serde_json::from_value(args[1].clone())?;
                match actor.xgroup_create(&group_name, &start_id) {
                    Ok(()) => Ok(serde_json::to_value("OK")?),
                    Err(e) => Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }),
                }
            }
            "xgroup_destroy" => {
                if args.is_empty() {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (group_name)".to_string(),
                    });
                }
                let group_name: String = serde_json::from_value(args[0].clone())?;
                let result = actor.xgroup_destroy(&group_name);
                Ok(serde_json::to_value(if result { 1 } else { 0 })?)
            }
            "xreadgroup" => {
                if args.len() < 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected at least 3 arguments (group, consumer, id)".to_string(),
                    });
                }
                let group_name: String = serde_json::from_value(args[0].clone())?;
                let consumer_name: String = serde_json::from_value(args[1].clone())?;
                let id: String = serde_json::from_value(args[2].clone())?;
                let count: Option<usize> = if args.len() > 3 {
                    serde_json::from_value(args[3].clone()).ok()
                } else {
                    None
                };
                match actor.xreadgroup(&group_name, &consumer_name, &id, count) {
                    Ok(entries) => Ok(serde_json::to_value(entries)?),
                    Err(e) => Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }),
                }
            }
            "xack" => {
                if args.len() < 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments (group_name, ids)".to_string(),
                    });
                }
                let group_name: String = serde_json::from_value(args[0].clone())?;
                let ids: Vec<String> = serde_json::from_value(args[1].clone())?;
                match actor.xack(&group_name, ids) {
                    Ok(count) => Ok(serde_json::to_value(count)?),
                    Err(e) => Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }),
                }
            }
            "xpending" => {
                if args.is_empty() {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (group_name)".to_string(),
                    });
                }
                let group_name: String = serde_json::from_value(args[0].clone())?;
                match actor.xpending(&group_name) {
                    Ok((count, min_id, max_id, consumers)) => {
                        Ok(serde_json::to_value((count, min_id, max_id, consumers))?)
                    }
                    Err(e) => Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }),
                }
            }
            "xsetid" => {
                if args.is_empty() {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument (id)".to_string(),
                    });
                }
                let id: String = serde_json::from_value(args[0].clone())?;
                match actor.xsetid(&id) {
                    Ok(()) => Ok(serde_json::to_value("OK")?),
                    Err(e) => Err(OrbitError::InvocationFailed {
                        addressable_type: "StreamActor".to_string(),
                        method: method.to_string(),
                        reason: e,
                    }),
                }
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "StreamActor".to_string(),
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
            "StreamActor" => self.execute_stream(&key, &invocation.method, &args).await,
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
