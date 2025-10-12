//! Simplified local invocation system for RESP protocol actors

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::resp::actors::*;
use orbit_shared::*;

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
                reason: format!("Unknown method: {}", method),
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
                reason: format!("Unknown method: {}", method),
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
                reason: format!("Unknown method: {}", method),
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
                reason: format!("Unknown method: {}", method),
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
            _ => Err(OrbitError::InvocationFailed {
                addressable_type: "SortedSetActor".to_string(),
                method: method.to_string(),
                reason: format!("Unknown method: {}", method),
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
