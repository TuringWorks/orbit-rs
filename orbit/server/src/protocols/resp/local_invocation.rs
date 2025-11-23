//! Local invocation system for RESP protocol actors
//!
//! This module provides a local invocation system that can execute RESP actor methods
//! without requiring a full orbit-server cluster. It's designed specifically for the
//! RESP server example to provide Redis-compatible functionality.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;
use tracing::debug;

use orbit_shared::*;
use crate::protocols::resp::actors::*;

/// Local actor registry for RESP actors
pub struct LocalActorRegistry {
    /// Registry of active KeyValueActor instances
    keyvalue_actors: Arc<RwLock<HashMap<String, KeyValueActor>>>,
    /// Registry of active HashActor instances
    hash_actors: Arc<RwLock<HashMap<String, HashActor>>>,
    /// Registry of active ListActor instances
    list_actors: Arc<RwLock<HashMap<String, ListActor>>>,
    /// Registry of active SetActor instances
    set_actors: Arc<RwLock<HashMap<String, SetActor>>>,
    /// Registry of active SortedSetActor instances
    sorted_set_actors: Arc<RwLock<HashMap<String, SortedSetActor>>>,
    /// Registry of active PubSubActor instances
    pubsub_actors: Arc<RwLock<HashMap<String, PubSubActor>>>,
}

impl LocalActorRegistry {
    pub fn new() -> Self {
        Self {
            keyvalue_actors: Arc::new(RwLock::new(HashMap::new())),
            hash_actors: Arc::new(RwLock::new(HashMap::new())),
            list_actors: Arc::new(RwLock::new(HashMap::new())),
            set_actors: Arc::new(RwLock::new(HashMap::new())),
            sorted_set_actors: Arc::new(RwLock::new(HashMap::new())),
            pubsub_actors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a method on a KeyValueActor
    pub async fn invoke_keyvalue_actor(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.keyvalue_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(KeyValueActor::new);

        debug!("Invoking KeyValueActor method '{}' on key '{}'", method, key);

        match method {
            "get_value" => {
                let result = actor.get_value();
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
                let result = actor.set_expiration(seconds).await?;
                Ok(serde_json::to_value(result)?)
            }
            "append_value" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let value: String = serde_json::from_value(args[0].clone())?;
                let result = actor.append_value(value).await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_range" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let start: i64 = serde_json::from_value(args[0].clone())?;
                let end: i64 = serde_json::from_value(args[1].clone())?;
                let result = actor.get_range(start, end).await?;
                Ok(serde_json::to_value(result)?)
            }
            "set_range" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let offset: usize = serde_json::from_value(args[0].clone())?;
                let value: String = serde_json::from_value(args[1].clone())?;
                let result = actor.set_range(offset, value).await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_and_set" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let new_value: String = serde_json::from_value(args[0].clone())?;
                let result = actor.get_and_set(new_value).await?;
                Ok(serde_json::to_value(result)?)
            }
            "strlen" => {
                let result = actor.strlen().await?;
                Ok(serde_json::to_value(result)?)
            }
            "persist" => {
                let result = actor.persist().await?;
                Ok(serde_json::to_value(result)?)
            }
            "set_pexpiration" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "KeyValueActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let milliseconds: u64 = serde_json::from_value(args[0].clone())?;
                let result = actor.set_pexpiration(milliseconds).await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_pttl" => {
                let result = actor.get_pttl().await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "KeyValueActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
            }),
        }
    }

    /// Execute a method on a HashActor
    pub async fn invoke_hash_actor(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.hash_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(HashActor::new);

        debug!("Invoking HashActor method '{}' on key '{}'", method, key);

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
                let result = actor.hget(&field).await?;
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
                let result = actor.hset(field, value).await?;
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
                let result = actor.hdel(&field).await?;
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
                let result = actor.hexists(&field).await?;
                Ok(serde_json::to_value(result)?)
            }
            "hkeys" => {
                let result = actor.hkeys().await?;
                Ok(serde_json::to_value(result)?)
            }
            "hvals" => {
                let result = actor.hvals().await?;
                Ok(serde_json::to_value(result)?)
            }
            "hgetall" => {
                let result = actor.hgetall().await?;
                Ok(serde_json::to_value(result)?)
            }
            "hlen" => {
                let result = actor.hlen().await?;
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
                let result = actor.hincrby(field, increment).await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "HashActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
            }),
        }
    }

    /// Execute a method on a ListActor
    pub async fn invoke_list_actor(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.list_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(ListActor::new);

        debug!("Invoking ListActor method '{}' on key '{}'", method, key);

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
                let result = actor.lpush(values).await?;
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
                let result = actor.rpush(values).await?;
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
                let result = actor.lpop(count).await?;
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
                let result = actor.rpop(count).await?;
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
                let result = actor.lrange(start, stop).await?;
                Ok(serde_json::to_value(result)?)
            }
            "llen" => {
                let result = actor.llen().await?;
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
                let result = actor.lindex(index).await?;
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
                let result = actor.lset(index, value).await?;
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
                let result = actor.lrem(count, &value).await?;
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
                let result = actor.ltrim(start, stop).await?;
                Ok(serde_json::to_value(result)?)
            }
            "linsert" => {
                if args.len() != 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "ListActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 3 arguments".to_string(),
                    });
                }
                let before_after: String = serde_json::from_value(args[0].clone())?;
                let pivot: String = serde_json::from_value(args[1].clone())?;
                let element: String = serde_json::from_value(args[2].clone())?;
                let result = actor.linsert(&before_after, &pivot, element).await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "ListActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
            }),
        }
    }

    /// Execute a method on a SetActor
    pub async fn invoke_set_actor(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.set_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(SetActor::new);

        debug!("Invoking SetActor method '{}' on key '{}'", method, key);

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
                let result = actor.sadd(members).await?;
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
                let result = actor.srem(members).await?;
                Ok(serde_json::to_value(result)?)
            }
            "smembers" => {
                let result = actor.smembers().await?;
                Ok(serde_json::to_value(result)?)
            }
            "scard" => {
                let result = actor.scard().await?;
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
                let result = actor.sismember(&member).await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "SetActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
            }),
        }
    }

    /// Execute a method on a SortedSetActor
    pub async fn invoke_sorted_set_actor(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.sorted_set_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(SortedSetActor::new);

        debug!("Invoking SortedSetActor method '{}' on key '{}'", method, key);

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
                let result = actor.zadd(member, score).await?;
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
                let result = actor.zrem(members).await?;
                Ok(serde_json::to_value(result)?)
            }
            "zcard" => {
                let result = actor.zcard().await?;
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
                let result = actor.zscore(&member).await?;
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
                let result = actor.zincrby(member, increment).await?;
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
                let result = actor.zrange(start, stop, with_scores).await?;
                Ok(serde_json::to_value(result)?)
            }
            "zrangebyscore" => {
                if args.len() != 3 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 3 arguments".to_string(),
                    });
                }
                let min_score: f64 = serde_json::from_value(args[0].clone())?;
                let max_score: f64 = serde_json::from_value(args[1].clone())?;
                let with_scores: bool = serde_json::from_value(args[2].clone())?;
                let result = actor.zrangebyscore(min_score, max_score, with_scores).await?;
                Ok(serde_json::to_value(result)?)
            }
            "zcount" => {
                if args.len() != 2 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "SortedSetActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 2 arguments".to_string(),
                    });
                }
                let min_score: f64 = serde_json::from_value(args[0].clone())?;
                let max_score: f64 = serde_json::from_value(args[1].clone())?;
                let result = actor.zcount(min_score, max_score).await?;
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
                let result = actor.zrank(&member).await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "SortedSetActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
            }),
        }
    }

    /// Execute a method on a PubSubActor
    pub async fn invoke_pubsub_actor(&self, key: &str, method: &str, args: &[Value]) -> OrbitResult<Value> {
        let mut actors = self.pubsub_actors.write().await;
        let actor = actors.entry(key.to_string()).or_insert_with(PubSubActor::new);

        debug!("Invoking PubSubActor method '{}' on key '{}'", method, key);

        match method {
            "subscribe" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "PubSubActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let subscriber_id: String = serde_json::from_value(args[0].clone())?;
                let result = actor.subscribe(subscriber_id).await?;
                Ok(serde_json::to_value(result)?)
            }
            "unsubscribe" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "PubSubActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let subscriber_id: String = serde_json::from_value(args[0].clone())?;
                let result = actor.unsubscribe(&subscriber_id).await?;
                Ok(serde_json::to_value(result)?)
            }
            "publish" => {
                if args.len() != 1 {
                    return Err(OrbitError::InvocationFailed {
                        addressable_type: "PubSubActor".to_string(),
                        method: method.to_string(),
                        reason: "Expected 1 argument".to_string(),
                    });
                }
                let message: String = serde_json::from_value(args[0].clone())?;
                let result = actor.publish(message).await?;
                Ok(serde_json::to_value(result)?)
            }
            "subscriber_count" => {
                let result = actor.subscriber_count().await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "PubSubActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
            }),
        }
    }

    /// Route and execute an invocation
    pub async fn execute_invocation(&self, invocation: &AddressableInvocation) -> OrbitResult<Value> {
        let key = match &invocation.reference.key {
            Key::StringKey { key } => key.clone(),
            Key::Int32Key { key } => key.to_string(),
            Key::Int64Key { key } => key.to_string(),
            Key::UuidKey { key } => key.to_string(),
        };

        let args: Vec<Value> = invocation.args.iter().map(|arg| arg.value.clone()).collect();

        match invocation.reference.addressable_type.as_str() {
            "KeyValueActor" => {
                self.invoke_keyvalue_actor(&key, &invocation.method, &args).await
            }
            "HashActor" => {
                self.invoke_hash_actor(&key, &invocation.method, &args).await
            }
            "ListActor" => {
                self.invoke_list_actor(&key, &invocation.method, &args).await
            }
            "SetActor" => {
                self.invoke_set_actor(&key, &invocation.method, &args).await
            }
            "SortedSetActor" => {
                self.invoke_sorted_set_actor(&key, &invocation.method, &args).await
            }
            "PubSubActor" => {
                self.invoke_pubsub_actor(&key, &invocation.method, &args).await
            }
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: invocation.reference.addressable_type.clone(),
                method: invocation.method.clone(),
                reason: format!("Unknown actor type: {}", invocation.reference.addressable_type),
            }),
        }
    }
}

impl Default for LocalActorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// RESP-specific InvocationSystem that uses local actor registry
pub struct RespInvocationSystem {
    registry: Arc<LocalActorRegistry>,
    pending_invocations: Arc<RwLock<HashMap<u64, tokio::sync::oneshot::Sender<orbit_client::invocation::InvocationResult>>>>,
    invocation_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl RespInvocationSystem {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(LocalActorRegistry::new()),
            pending_invocations: Arc::new(RwLock::new(HashMap::new())),
            invocation_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Send an invocation to an actor and wait for the result
    pub async fn send_invocation(
        &self,
        invocation: AddressableInvocation,
    ) -> OrbitResult<orbit_client::invocation::InvocationResult> {
        let invocation_id = self
            .invocation_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (tx, rx) = tokio::sync::oneshot::channel();

        // Store the pending invocation
        {
            let mut pending = self.pending_invocations.write().await;
            pending.insert(invocation_id, tx);
        }

        // Execute the invocation using local registry
        let result = self.route_invocation(invocation_id, invocation).await;

        // Wait for the result with timeout
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(OrbitError::internal("Invocation sender dropped")),
            Err(_) => {
                // Cleanup timed out invocation
                let mut pending = self.pending_invocations.write().await;
                pending.remove(&invocation_id);
                Err(OrbitError::timeout("Actor invocation"))
            }
        }?;

        result
    }

    /// Route an invocation to the local registry
    async fn route_invocation(
        &self,
        invocation_id: u64,
        invocation: AddressableInvocation,
    ) -> OrbitResult<orbit_client::invocation::InvocationResult> {
        // Execute using local registry
        let result_value = self.registry.execute_invocation(&invocation).await;

        let result = orbit_client::invocation::InvocationResult {
            invocation_id,
            reference: invocation.reference,
            method: invocation.method,
            result: result_value.map_err(|e| e.to_string()),
        };

        // Complete the pending invocation
        self.complete_invocation(invocation_id, result.clone()).await;

        Ok(result)
    }

    /// Complete a pending invocation with a result
    async fn complete_invocation(&self, invocation_id: u64, result: orbit_client::invocation::InvocationResult) {
        let mut pending = self.pending_invocations.write().await;
        if let Some(sender) = pending.remove(&invocation_id) {
            let _ = sender.send(result);
        }
    }

    /// Get the registry for direct access
    pub fn registry(&self) -> Arc<LocalActorRegistry> {
        self.registry.clone()
    }
}

impl Default for RespInvocationSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Create an OrbitClient that uses the local RESP actor registry
pub async fn create_resp_orbit_client(namespace: &str) -> OrbitResult<orbit_client::OrbitClient> {
    // Create a specialized invocation system for RESP
    let resp_invocation_system = Arc::new(RespInvocationSystem::new());
    
    // Create an OrbitClient builder
    let client = orbit_client::OrbitClient::builder()
        .with_namespace(namespace)
        .with_offline_mode(true)
        .build()
        .await?;

    // TODO: Replace the client's invocation system with our RESP-aware one
    // This would require modifying OrbitClient to allow custom invocation systems
    // For now, we'll return the client as-is, but the invocation system needs to be patched
    
    Ok(client)
}
