//! RESP protocol command handler
//!
//! Implements Redis-compatible commands that operate on Orbit actors.
//! Each Redis data type (string, hash, list) maps to a corresponding Orbit actor type.

use std::sync::Arc;
use tracing::{debug, warn};

use super::{
    actors::{HashActor, KeyValueActor, ListActor, PubSubActor, SetActor, SortedSetActor},
    RespValue,
};
use crate::{
    error::{ProtocolError, ProtocolResult},
    graph_database::{ExecutionPlan, GraphActor, QueryProfile, SlowQuery},
    time_series::{
        AggregationType, CompactionRule, DuplicatePolicy, Sample, TimeSeriesActor,
        TimeSeriesConfig, TimeSeriesStats,
    },
    vector_store::{
        SimilarityMetric, Vector, VectorActor, VectorIndexConfig, VectorSearchParams,
        VectorSearchResult, VectorStats,
    },
};
use orbit_client::OrbitClient;
use orbit_shared::Key;
use std::collections::HashMap;

/// Redis command handler that translates Redis commands to Orbit actor operations
pub struct CommandHandler {
    orbit_client: Arc<OrbitClient>,
}

impl CommandHandler {
    /// Create a new command handler
    pub fn new(orbit_client: OrbitClient) -> Self {
        Self {
            orbit_client: Arc::new(orbit_client),
        }
    }

    /// Handle a RESP command
    pub async fn handle_command(&self, command: RespValue) -> ProtocolResult<RespValue> {
        let args = match command {
            RespValue::Array(args) => args,
            _ => {
                return Err(ProtocolError::RespError(
                    "Command must be an array".to_string(),
                ))
            }
        };

        if args.is_empty() {
            return Err(ProtocolError::RespError("Empty command".to_string()));
        }

        let command_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("Command name must be a string".to_string()))?
            .to_uppercase();

        let args = &args[1..];

        debug!(
            "Executing command: {} with {} args",
            command_name,
            args.len()
        );

        match command_name.as_str() {
            // Connection commands
            "PING" => self.cmd_ping(args).await,
            "ECHO" => self.cmd_echo(args).await,
            "SELECT" => self.cmd_select(args).await,
            "AUTH" => self.cmd_auth(args).await,
            "QUIT" => self.cmd_quit(args).await,

            // String/Key commands
            "GET" => self.cmd_get(args).await,
            "SET" => self.cmd_set(args).await,
            "DEL" => self.cmd_del(args).await,
            "EXISTS" => self.cmd_exists(args).await,
            "TTL" => self.cmd_ttl(args).await,
            "EXPIRE" => self.cmd_expire(args).await,
            "KEYS" => self.cmd_keys(args).await,
            "APPEND" => self.cmd_append(args).await,
            "GETRANGE" => self.cmd_getrange(args).await,
            "GETSET" => self.cmd_getset(args).await,
            "MGET" => self.cmd_mget(args).await,
            "MSET" => self.cmd_mset(args).await,
            "SETEX" => self.cmd_setex(args).await,
            "SETRANGE" => self.cmd_setrange(args).await,
            "STRLEN" => self.cmd_strlen(args).await,
            "PERSIST" => self.cmd_persist(args).await,
            "PEXPIRE" => self.cmd_pexpire(args).await,
            "PTTL" => self.cmd_pttl(args).await,
            "RANDOMKEY" => self.cmd_randomkey(args).await,
            "RENAME" => self.cmd_rename(args).await,
            "TYPE" => self.cmd_type(args).await,
            "UNLINK" => self.cmd_unlink(args).await,

            // Hash commands
            "HGET" => self.cmd_hget(args).await,
            "HSET" => self.cmd_hset(args).await,
            "HGETALL" => self.cmd_hgetall(&args[1..]).await,
            "HMGET" => self.cmd_hmget(&args[1..]).await,
            "HMSET" => self.cmd_hmset(&args[1..]).await,
            "HDEL" => self.cmd_hdel(&args[1..]).await,
            "HEXISTS" => self.cmd_hexists(args).await,
            "HKEYS" => self.cmd_hkeys(args).await,
            "HVALS" => self.cmd_hvals(args).await,
            "HLEN" => self.cmd_hlen(args).await,
            "HINCRBY" => self.cmd_hincrby(args).await,

            // List commands
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

            // Pub/Sub commands
            "PUBLISH" => self.cmd_publish(args).await,
            "SUBSCRIBE" => self.cmd_subscribe(args).await,
            "UNSUBSCRIBE" => self.cmd_unsubscribe(args).await,
            "PSUBSCRIBE" => self.cmd_psubscribe(args).await,
            "PUNSUBSCRIBE" => self.cmd_punsubscribe(args).await,
            "PUBSUB" => self.cmd_pubsub(args).await,

            // Set commands
            "SADD" => self.cmd_sadd(args).await,
            "SREM" => self.cmd_srem(args).await,
            "SMEMBERS" => self.cmd_smembers(args).await,
            "SCARD" => self.cmd_scard(args).await,
            "SISMEMBER" => self.cmd_sismember(args).await,
            "SUNION" => self.cmd_sunion(args).await,
            "SINTER" => self.cmd_sinter(args).await,
            "SDIFF" => self.cmd_sdiff(args).await,

            // Sorted Set commands
            "ZADD" => self.cmd_zadd(args).await,
            "ZREM" => self.cmd_zrem(args).await,
            "ZCARD" => self.cmd_zcard(args).await,
            "ZSCORE" => self.cmd_zscore(args).await,
            "ZINCRBY" => self.cmd_zincrby(args).await,
            "ZRANGE" => self.cmd_zrange(args).await,
            "ZRANGEBYSCORE" => self.cmd_zrangebyscore(args).await,
            "ZCOUNT" => self.cmd_zcount(args).await,
            "ZRANK" => self.cmd_zrank(args).await,

            // Vector commands (VECTOR.* namespace)
            "VECTOR.ADD" => self.cmd_vector_add(args).await,
            "VECTOR.GET" => self.cmd_vector_get(args).await,
            "VECTOR.DEL" => self.cmd_vector_del(args).await,
            "VECTOR.STATS" => self.cmd_vector_stats(args).await,
            "VECTOR.LIST" => self.cmd_vector_list(args).await,
            "VECTOR.COUNT" => self.cmd_vector_count(args).await,
            "VECTOR.SEARCH" => self.cmd_vector_search(args).await,
            "VECTOR.KNN" => self.cmd_vector_knn(args).await,

            // RedisSearch-compatible vector commands (FT.* namespace)
            "FT.CREATE" => self.cmd_ft_create(args).await,
            "FT.ADD" => self.cmd_ft_add(args).await,
            "FT.DEL" => self.cmd_ft_del(args).await,
            "FT.SEARCH" => self.cmd_ft_search(args).await,
            "FT.INFO" => self.cmd_ft_info(args).await,

            // Time Series commands (TS.* namespace)
            "TS.CREATE" => self.cmd_ts_create(args).await,
            "TS.ALTER" => self.cmd_ts_alter(args).await,
            "TS.ADD" => self.cmd_ts_add(args).await,
            "TS.MADD" => self.cmd_ts_madd(args).await,
            "TS.INCRBY" => self.cmd_ts_incrby(args).await,
            "TS.DECRBY" => self.cmd_ts_decrby(args).await,
            "TS.DEL" => self.cmd_ts_del(args).await,
            "TS.GET" => self.cmd_ts_get(args).await,
            "TS.MGET" => self.cmd_ts_mget(args).await,
            "TS.INFO" => self.cmd_ts_info(args).await,
            "TS.RANGE" => self.cmd_ts_range(args).await,
            "TS.REVRANGE" => self.cmd_ts_revrange(args).await,
            "TS.MRANGE" => self.cmd_ts_mrange(args).await,
            "TS.MREVRANGE" => self.cmd_ts_mrevrange(args).await,
            "TS.QUERYINDEX" => self.cmd_ts_queryindex(args).await,
            "TS.CREATERULE" => self.cmd_ts_createrule(args).await,
            "TS.DELETERULE" => self.cmd_ts_deleterule(args).await,

            // Graph commands (GRAPH.* namespace)
            "GRAPH.QUERY" => self.cmd_graph_query(args).await,
            "GRAPH.RO_QUERY" => self.cmd_graph_ro_query(args).await,
            "GRAPH.DELETE" => self.cmd_graph_delete(args).await,
            "GRAPH.LIST" => self.cmd_graph_list(args).await,
            "GRAPH.EXPLAIN" => self.cmd_graph_explain(args).await,
            "GRAPH.PROFILE" => self.cmd_graph_profile(args).await,
            "GRAPH.SLOWLOG" => self.cmd_graph_slowlog(args).await,
            "GRAPH.CONFIG" => self.cmd_graph_config(args).await,

            // Server commands
            "INFO" => self.cmd_info(args).await,
            "DBSIZE" => self.cmd_dbsize(args).await,
            "FLUSHDB" => self.cmd_flushdb(args).await,
            "FLUSHALL" => self.cmd_flushall(args).await,
            "COMMAND" => self.cmd_command(args).await,

            _ => {
                warn!("Unknown command: {}", command_name);
                Err(ProtocolError::RespError(format!(
                    "ERR unknown command '{}'",
                    command_name
                )))
            }
        }
    }

    // Connection commands

    async fn cmd_ping(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            Ok(RespValue::simple_string("PONG"))
        } else {
            Ok(args[0].clone())
        }
    }

    async fn cmd_echo(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'echo' command".to_string(),
            ));
        }
        Ok(args[0].clone())
    }

    async fn cmd_select(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'select' command".to_string(),
            ));
        }
        // Redis database selection - we'll just accept it and return OK
        Ok(RespValue::ok())
    }

    async fn cmd_auth(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // AUTH command can have 1 or 2 arguments:
        // AUTH password
        // AUTH username password (Redis 6.0+)
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'auth' command".to_string(),
            ));
        }

        let (username, password) = if args.len() == 1 {
            // Single argument: password only (default user)
            let password = args[0]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid password".to_string()))?;
            ("default".to_string(), password)
        } else {
            // Two arguments: username and password
            let username = args[0]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid username".to_string()))?;
            let password = args[1]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid password".to_string()))?;
            (username, password)
        };

        // For now, we'll implement a simple authentication that accepts any credentials
        // In a real implementation, you would:
        // 1. Check against a user database
        // 2. Validate password hashes
        // 3. Set connection authentication state
        // 4. Apply user permissions/ACLs

        debug!(
            "AUTH attempt for user '{}' (password length: {})",
            username,
            password.len()
        );

        // TODO: Implement actual authentication logic here
        // For now, we'll just accept any authentication attempt
        Ok(RespValue::ok())
    }

    async fn cmd_quit(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // QUIT command should not have any arguments
        if !args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'quit' command".to_string(),
            ));
        }

        debug!("QUIT command received - client requested connection close");

        // Return OK to indicate successful quit
        // The actual connection closing should be handled by the server/connection manager
        // This is just the command response before closing
        Ok(RespValue::ok())
    }

    // String/Key commands

    async fn cmd_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'get' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get KeyValueActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke get_value method on the actor
        let value: Option<String> = actor_ref
            .invoke("get_value", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("GET {} -> {:?}", key, value);
        Ok(value
            .map(RespValue::bulk_string_from_str)
            .unwrap_or(RespValue::null()))
    }

    async fn cmd_set(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'set' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let value = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        // Parse optional arguments (EX, PX, NX, XX)
        let mut expiration_seconds: Option<u64> = None;
        let mut i = 2;
        while i < args.len() {
            if let Some(arg) = args[i].as_string() {
                match arg.to_uppercase().as_str() {
                    "EX" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
                        }
                        expiration_seconds = args[i + 1].as_integer().map(|x| x as u64);
                        i += 2;
                    }
                    "PX" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
                        }
                        if let Some(ms) = args[i + 1].as_integer() {
                            expiration_seconds = Some((ms / 1000) as u64);
                        }
                        i += 2;
                    }
                    _ => i += 1,
                }
            } else {
                i += 1;
            }
        }

        // Get KeyValueActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke set_value method on the actor
        actor_ref
            .invoke::<()>("set_value", vec![value.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        // Set expiration if provided
        if let Some(seconds) = expiration_seconds {
            actor_ref
                .invoke::<()>("set_expiration", vec![seconds.into()])
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;
        }

        debug!(
            "SET {} {} (expiration: {:?})",
            key, value, expiration_seconds
        );
        Ok(RespValue::ok())
    }

    async fn cmd_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'del' command".to_string(),
            ));
        }

        let mut deleted_count = 0i64;
        for arg in args {
            if let Some(key_str) = arg.as_string() {
                // Try to delete as KeyValueActor first
                let key_ref_result = self
                    .orbit_client
                    .actor_reference::<KeyValueActor>(Key::StringKey {
                        key: key_str.clone(),
                    })
                    .await;

                if let Ok(actor_ref) = key_ref_result {
                    // Try to delete the value using the actor method
                    let delete_result: Result<bool, _> =
                        actor_ref.invoke("delete_value", vec![]).await;

                    match delete_result {
                        Ok(existed) => {
                            if existed {
                                deleted_count += 1;
                                debug!("DEL {} (KeyValue) -> deleted", key_str);
                            } else {
                                debug!("DEL {} (KeyValue) -> key didn't exist", key_str);
                            }
                        }
                        Err(_) => {
                            // Key might not exist or actor might not be accessible
                            debug!("DEL {} -> key not found or inaccessible", key_str);
                        }
                    }
                } else {
                    debug!("DEL {} -> actor reference failed", key_str);
                }
            }
        }

        Ok(RespValue::integer(deleted_count))
    }

    async fn cmd_exists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'exists' command".to_string(),
            ));
        }

        let mut exists_count = 0i64;
        for arg in args {
            if let Some(key_str) = arg.as_string() {
                // Check if KeyValueActor exists and has a value
                let key_ref_result = self
                    .orbit_client
                    .actor_reference::<KeyValueActor>(Key::StringKey {
                        key: key_str.clone(),
                    })
                    .await;

                if let Ok(actor_ref) = key_ref_result {
                    // Check if the key exists and is not expired
                    let exists_result: Result<bool, _> = actor_ref.invoke("exists", vec![]).await;

                    match exists_result {
                        Ok(exists) => {
                            if exists {
                                exists_count += 1;
                                debug!("EXISTS {} -> true", key_str);
                            } else {
                                debug!("EXISTS {} -> false (expired or empty)", key_str);
                            }
                        }
                        Err(_) => {
                            debug!("EXISTS {} -> false (actor invocation failed)", key_str);
                        }
                    }
                } else {
                    debug!("EXISTS {} -> false (no actor reference)", key_str);
                }
            }
        }

        Ok(RespValue::integer(exists_count))
    }

    async fn cmd_ttl(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'ttl' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get KeyValueActor reference and check TTL
        let key_ref_result = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await;

        if let Ok(actor_ref) = key_ref_result {
            let ttl_result: Result<i64, _> = actor_ref.invoke("get_ttl", vec![]).await;

            match ttl_result {
                Ok(ttl) => {
                    debug!("TTL {} -> {}", key_str, ttl);
                    Ok(RespValue::integer(ttl))
                }
                Err(_) => {
                    debug!("TTL {} -> -2 (key doesn't exist)", key_str);
                    Ok(RespValue::integer(-2)) // -2 means key doesn't exist
                }
            }
        } else {
            debug!("TTL {} -> -2 (no actor reference)", key_str);
            Ok(RespValue::integer(-2)) // -2 means key doesn't exist
        }
    }

    async fn cmd_expire(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'expire' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let seconds = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid timeout".to_string()))?;

        if seconds < 0 {
            return Err(ProtocolError::RespError(
                "ERR invalid expire time in 'expire' command".to_string(),
            ));
        }

        // Get KeyValueActor reference and set expiration
        let key_ref_result = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await;

        if let Ok(actor_ref) = key_ref_result {
            // First check if the key exists
            let exists_result: Result<bool, _> = actor_ref.invoke("exists", vec![]).await;

            match exists_result {
                Ok(exists) => {
                    if exists {
                        // Key exists, set expiration
                        let expire_result: Result<(), _> = actor_ref
                            .invoke("set_expiration", vec![(seconds as u64).into()])
                            .await;

                        match expire_result {
                            Ok(_) => {
                                debug!("EXPIRE {} {} -> timeout set", key_str, seconds);
                                Ok(RespValue::integer(1)) // 1 means timeout was set
                            }
                            Err(e) => {
                                debug!(
                                    "EXPIRE {} {} -> failed to set expiration: {}",
                                    key_str, seconds, e
                                );
                                Ok(RespValue::integer(0)) // 0 means key doesn't exist or operation failed
                            }
                        }
                    } else {
                        debug!("EXPIRE {} {} -> key doesn't exist", key_str, seconds);
                        Ok(RespValue::integer(0)) // 0 means key doesn't exist
                    }
                }
                Err(_) => {
                    debug!(
                        "EXPIRE {} {} -> key doesn't exist or inaccessible",
                        key_str, seconds
                    );
                    Ok(RespValue::integer(0)) // 0 means key doesn't exist
                }
            }
        } else {
            debug!("EXPIRE {} {} -> no actor reference", key_str, seconds);
            Ok(RespValue::integer(0)) // 0 means key doesn't exist
        }
    }

    async fn cmd_keys(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'keys' command".to_string(),
            ));
        }

        let pattern = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid pattern".to_string()))?;

        // TODO: Replace with actual OrbitClient directory listing
        debug!("KEYS {} (placeholder implementation)", pattern);
        Ok(RespValue::array(vec![])) // Empty list for now
    }

    // Hash commands

    async fn cmd_hget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hget' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let field = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke hget method on the actor
        let value: Option<String> = actor_ref
            .invoke("hget", vec![field.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("HGET {} {} -> {:?}", key, field, value);
        Ok(value
            .map(RespValue::bulk_string_from_str)
            .unwrap_or(RespValue::null()))
    }

    async fn cmd_hset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len().is_multiple_of(2) {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hset' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut fields_set = 0i64;
        for i in (1..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }

            let field = args[i]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;
            let value = args[i + 1]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

            // Get HashActor reference
            let actor_ref = self
                .orbit_client
                .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            // Invoke hset method on the actor
            let was_new: bool = actor_ref
                .invoke("hset", vec![field.clone().into(), value.clone().into()])
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;

            debug!("HSET {} {} {} -> new: {}", key, field, value, was_new);
            if was_new {
                fields_set += 1;
            }
        }

        Ok(RespValue::integer(fields_set))
    }

    async fn cmd_hgetall(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hgetall' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get all fields and values
        let fields_and_values: Result<Vec<(String, String)>, _> =
            actor_ref.invoke("hgetall", vec![]).await;

        match fields_and_values {
            Ok(pairs) => {
                let mut result = Vec::new();
                for (field, value) in pairs {
                    result.push(RespValue::bulk_string_from_str(&field));
                    result.push(RespValue::bulk_string_from_str(&value));
                }
                debug!(
                    "HGETALL {} -> {} field-value pairs",
                    key_str,
                    result.len() / 2
                );
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!("HGETALL {} -> error: {}", key_str, e);
                Ok(RespValue::array(vec![])) // Return empty array on error
            }
        }
    }

    async fn cmd_hmget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hmget' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let mut result = Vec::new();
        for arg in &args[1..] {
            let field = arg
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;

            // Get field value
            let field_value: Result<Option<String>, _> =
                actor_ref.invoke("hget", vec![field.clone().into()]).await;

            match field_value {
                Ok(Some(value)) => {
                    result.push(RespValue::bulk_string_from_str(&value));
                    debug!("HMGET {} {} -> {}", key, field, value);
                }
                Ok(None) => {
                    result.push(RespValue::null());
                    debug!("HMGET {} {} -> null (field not found)", key, field);
                }
                Err(e) => {
                    debug!("HMGET {} {} -> error: {}", key, field, e);
                    result.push(RespValue::null());
                }
            }
        }

        Ok(RespValue::array(result))
    }

    async fn cmd_hmset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() % 2 == 0 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hmset' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Set field-value pairs
        for i in (1..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }

            let field = args[i]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;
            let value = args[i + 1]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

            // Invoke hset method on the actor
            let _was_new: bool = actor_ref
                .invoke("hset", vec![field.clone().into(), value.clone().into()])
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;

            debug!("HMSET {} {} {}", key, field, value);
        }

        Ok(RespValue::ok())
    }

    async fn cmd_hdel(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hdel' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let mut deleted_count = 0i64;
        for arg in &args[1..] {
            let field = arg
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;

            // Invoke hdel method on the actor
            let was_deleted: Result<bool, _> =
                actor_ref.invoke("hdel", vec![field.clone().into()]).await;

            match was_deleted {
                Ok(true) => {
                    deleted_count += 1;
                    debug!("HDEL {} {} -> deleted", key, field);
                }
                Ok(false) => {
                    debug!("HDEL {} {} -> not found", key, field);
                }
                Err(e) => {
                    debug!("HDEL {} {} -> error: {}", key, field, e);
                }
            }
        }

        Ok(RespValue::integer(deleted_count))
    }

    async fn cmd_hexists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hexists' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let field = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Check if field exists
        let exists: Result<bool, _> = actor_ref
            .invoke("hexists", vec![field.clone().into()])
            .await;

        match exists {
            Ok(true) => {
                debug!("HEXISTS {} {} -> 1 (exists)", key, field);
                Ok(RespValue::integer(1))
            }
            Ok(false) => {
                debug!("HEXISTS {} {} -> 0 (not found)", key, field);
                Ok(RespValue::integer(0))
            }
            Err(e) => {
                debug!("HEXISTS {} {} -> error: {}", key, field, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_hkeys(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hkeys' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get all keys
        let keys: Result<Vec<String>, _> = actor_ref.invoke("hkeys", vec![]).await;

        match keys {
            Ok(field_keys) => {
                let result: Vec<RespValue> = field_keys
                    .into_iter()
                    .map(|key| RespValue::bulk_string_from_str(&key))
                    .collect();
                debug!("HKEYS {} -> {} keys", key, result.len());
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!("HKEYS {} -> error: {}", key, e);
                Ok(RespValue::array(vec![])) // Return empty array on error
            }
        }
    }

    async fn cmd_hvals(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hvals' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get all values
        let values: Result<Vec<String>, _> = actor_ref.invoke("hvals", vec![]).await;

        match values {
            Ok(field_values) => {
                let result: Vec<RespValue> = field_values
                    .into_iter()
                    .map(|val| RespValue::bulk_string_from_str(&val))
                    .collect();
                debug!("HVALS {} -> {} values", key, result.len());
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!("HVALS {} -> error: {}", key, e);
                Ok(RespValue::array(vec![])) // Return empty array on error
            }
        }
    }

    async fn cmd_hlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hlen' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get HashActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get hash length
        let length: Result<usize, _> = actor_ref.invoke("hlen", vec![]).await;

        match length {
            Ok(len) => {
                debug!("HLEN {} -> {}", key, len);
                Ok(RespValue::integer(len as i64))
            }
            Err(e) => {
                debug!("HLEN {} -> error: {}", key, e);
                Ok(RespValue::integer(0)) // Return 0 on error
            }
        }
    }

    // List commands

    async fn cmd_lpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lpush' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut values = Vec::new();
        for arg in &args[1..] {
            if let Some(value) = arg.as_string() {
                values.push(value);
            } else {
                return Err(ProtocolError::RespError("ERR invalid value".to_string()));
            }
        }

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke lpush method on the actor
        let new_length: i64 = actor_ref
            .invoke("lpush", vec![values.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("LPUSH {} {:?} -> length: {}", key, values, new_length);
        Ok(RespValue::integer(new_length))
    }

    async fn cmd_rpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'rpush' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut values = Vec::new();
        for arg in &args[1..] {
            if let Some(value) = arg.as_string() {
                values.push(value);
            } else {
                return Err(ProtocolError::RespError("ERR invalid value".to_string()));
            }
        }

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke rpush method on the actor
        let new_length: Result<usize, _> =
            actor_ref.invoke("rpush", vec![values.clone().into()]).await;

        match new_length {
            Ok(length) => {
                debug!("RPUSH {} {:?} -> length: {}", key, values, length);
                Ok(RespValue::integer(length as i64))
            }
            Err(e) => {
                debug!("RPUSH {} {:?} -> error: {}", key, values, e);
                Err(ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                )))
            }
        }
    }

    async fn cmd_lpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lpop' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let count = if args.len() == 2 {
            args[1].as_integer().unwrap_or(1) as usize
        } else {
            1
        };

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke lpop method on the actor
        let popped_items: Result<Vec<String>, _> =
            actor_ref.invoke("lpop", vec![count.into()]).await;

        match popped_items {
            Ok(items) => {
                debug!("LPOP {} {} -> {:?}", key, count, items);
                if count == 1 {
                    // For single element, return the element or null
                    Ok(items
                        .first()
                        .map(|s| RespValue::bulk_string_from_str(s))
                        .unwrap_or(RespValue::null()))
                } else {
                    // For multiple elements, return array
                    let result: Vec<RespValue> = items
                        .into_iter()
                        .map(|item| RespValue::bulk_string_from_str(&item))
                        .collect();
                    Ok(RespValue::array(result))
                }
            }
            Err(e) => {
                debug!("LPOP {} {} -> error: {}", key, count, e);
                Ok(RespValue::null())
            }
        }
    }

    async fn cmd_rpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'rpop' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let count = if args.len() == 2 {
            args[1].as_integer().unwrap_or(1) as usize
        } else {
            1
        };

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke rpop method on the actor
        let popped_items: Result<Vec<String>, _> =
            actor_ref.invoke("rpop", vec![count.into()]).await;

        match popped_items {
            Ok(items) => {
                debug!("RPOP {} {} -> {:?}", key, count, items);
                if count == 1 {
                    // For single element, return the element or null
                    Ok(items
                        .first()
                        .map(|s| RespValue::bulk_string_from_str(s))
                        .unwrap_or(RespValue::null()))
                } else {
                    // For multiple elements, return array
                    let result: Vec<RespValue> = items
                        .into_iter()
                        .map(|item| RespValue::bulk_string_from_str(&item))
                        .collect();
                    Ok(RespValue::array(result))
                }
            }
            Err(e) => {
                debug!("RPOP {} {} -> error: {}", key, count, e);
                Ok(RespValue::null())
            }
        }
    }

    async fn cmd_lrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lrange' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let start = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid start index".to_string()))?;
        let stop = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid stop index".to_string()))?;

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get list range
        let range_result: Result<Vec<String>, _> = actor_ref
            .invoke("lrange", vec![start.into(), stop.into()])
            .await;

        match range_result {
            Ok(items) => {
                let result: Vec<RespValue> = items
                    .into_iter()
                    .map(|item| RespValue::bulk_string_from_str(&item))
                    .collect();
                debug!(
                    "LRANGE {} {} {} -> {} items",
                    key_str,
                    start,
                    stop,
                    result.len()
                );
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!("LRANGE {} {} {} -> error: {}", key_str, start, stop, e);
                Ok(RespValue::array(vec![])) // Return empty array on error
            }
        }
    }

    async fn cmd_llen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'llen' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke llen method on the actor
        let length: Result<usize, _> = actor_ref.invoke("llen", vec![]).await;

        match length {
            Ok(len) => {
                debug!("LLEN {} -> {}", key, len);
                Ok(RespValue::integer(len as i64))
            }
            Err(e) => {
                debug!("LLEN {} -> error: {}", key, e);
                Ok(RespValue::integer(0)) // Return 0 on error
            }
        }
    }

    async fn cmd_lindex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lindex' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let index = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index".to_string()))?;

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke lindex method on the actor
        let element: Result<Option<String>, _> =
            actor_ref.invoke("lindex", vec![index.into()]).await;

        match element {
            Ok(Some(value)) => {
                debug!("LINDEX {} {} -> {}", key, index, value);
                Ok(RespValue::bulk_string_from_str(&value))
            }
            Ok(None) => {
                debug!("LINDEX {} {} -> null (not found)", key, index);
                Ok(RespValue::null())
            }
            Err(e) => {
                debug!("LINDEX {} {} -> error: {}", key, index, e);
                Ok(RespValue::null())
            }
        }
    }

    async fn cmd_lset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lset' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let index = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index".to_string()))?;
        let value = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke lset method on the actor
        let set_result: Result<bool, _> = actor_ref
            .invoke("lset", vec![index.into(), value.clone().into()])
            .await;

        match set_result {
            Ok(true) => {
                debug!("LSET {} {} {} -> OK", key, index, value);
                Ok(RespValue::ok())
            }
            Ok(false) => {
                debug!("LSET {} {} {} -> index out of range", key, index, value);
                Err(ProtocolError::RespError(
                    "ERR index out of range".to_string(),
                ))
            }
            Err(e) => {
                debug!("LSET {} {} {} -> error: {}", key, index, value, e);
                Err(ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                )))
            }
        }
    }

    async fn cmd_lrem(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'lrem' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let count = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid count".to_string()))?;
        let value = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke lrem method on the actor
        let removed_count: Result<usize, _> = actor_ref
            .invoke("lrem", vec![count.into(), value.clone().into()])
            .await;

        match removed_count {
            Ok(count) => {
                debug!("LREM {} {} {} -> {} removed", key, count, value, count);
                Ok(RespValue::integer(count as i64))
            }
            Err(e) => {
                debug!("LREM {} {} {} -> error: {}", key, count, value, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_ltrim(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'ltrim' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let start = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid start index".to_string()))?;
        let stop = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid stop index".to_string()))?;

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke ltrim method on the actor
        let trim_result: Result<(), _> = actor_ref
            .invoke("ltrim", vec![start.into(), stop.into()])
            .await;

        match trim_result {
            Ok(()) => {
                debug!("LTRIM {} {} {} -> OK", key, start, stop);
                Ok(RespValue::ok())
            }
            Err(e) => {
                debug!("LTRIM {} {} {} -> error: {}", key, start, stop, e);
                Err(ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                )))
            }
        }
    }

    async fn cmd_linsert(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'linsert' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let before_after = args[1].as_string().ok_or_else(|| {
            ProtocolError::RespError("ERR invalid BEFORE|AFTER argument".to_string())
        })?;
        let pivot = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid pivot".to_string()))?;
        let element = args[3]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid element".to_string()))?;

        // Validate before_after argument
        if !matches!(before_after.to_uppercase().as_str(), "BEFORE" | "AFTER") {
            return Err(ProtocolError::RespError("ERR syntax error".to_string()));
        }

        // Get ListActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke linsert method on the actor
        let result_length: Result<i64, _> = actor_ref
            .invoke(
                "linsert",
                vec![
                    before_after.clone().into(),
                    pivot.clone().into(),
                    element.clone().into(),
                ],
            )
            .await;

        match result_length {
            Ok(length) => {
                debug!(
                    "LINSERT {} {} {} {} -> {}",
                    key, before_after, pivot, element, length
                );
                Ok(RespValue::integer(length))
            }
            Err(e) => {
                debug!(
                    "LINSERT {} {} {} {} -> error: {}",
                    key, before_after, pivot, element, e
                );
                Ok(RespValue::integer(-1))
            }
        }
    }

    // Pub/Sub commands

    async fn cmd_publish(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'publish' command".to_string(),
            ));
        }

        let channel = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid channel".to_string()))?;
        let message = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid message".to_string()))?;

        // Get PubSubActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<PubSubActor>(Key::StringKey {
                key: channel.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke publish method on the actor
        let subscriber_count: i64 = actor_ref
            .invoke("publish", vec![message.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!(
            "PUBLISH {} {} -> subscribers: {}",
            channel, message, subscriber_count
        );
        Ok(RespValue::integer(subscriber_count))
    }

    async fn cmd_subscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'subscribe' command".to_string(),
            ));
        }

        // TODO: Implement subscription logic with pub/sub actor
        // This requires connection state management
        let channels: Vec<_> = args.iter().filter_map(|arg| arg.as_string()).collect();

        debug!("SUBSCRIBE {:?} (placeholder implementation)", channels);
        Ok(RespValue::array(vec![
            RespValue::bulk_string_from_str("subscribe"),
            args[0].clone(),
            RespValue::integer(1),
        ]))
    }

    async fn cmd_unsubscribe(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // TODO: Implement unsubscription logic
        debug!("UNSUBSCRIBE (placeholder implementation)");
        Ok(RespValue::array(vec![
            RespValue::bulk_string_from_str("unsubscribe"),
            RespValue::null(),
            RespValue::integer(0),
        ]))
    }

    async fn cmd_psubscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'psubscribe' command".to_string(),
            ));
        }

        // TODO: Implement pattern subscription logic
        debug!("PSUBSCRIBE (placeholder implementation)");
        Ok(RespValue::array(vec![
            RespValue::bulk_string_from_str("psubscribe"),
            args[0].clone(),
            RespValue::integer(1),
        ]))
    }

    async fn cmd_punsubscribe(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // TODO: Implement pattern unsubscription logic
        debug!("PUNSUBSCRIBE (placeholder implementation)");
        Ok(RespValue::array(vec![
            RespValue::bulk_string_from_str("punsubscribe"),
            RespValue::null(),
            RespValue::integer(0),
        ]))
    }

    // Set commands

    async fn cmd_sadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sadd' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut members = Vec::new();
        for arg in &args[1..] {
            if let Some(member) = arg.as_string() {
                members.push(member);
            } else {
                return Err(ProtocolError::RespError("ERR invalid member".to_string()));
            }
        }

        // Get SetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Add members to set
        let added_count: Result<usize, _> =
            actor_ref.invoke("sadd", vec![members.clone().into()]).await;

        match added_count {
            Ok(count) => {
                debug!("SADD {} {:?} -> {} added", key_str, members, count);
                Ok(RespValue::integer(count as i64))
            }
            Err(e) => {
                debug!("SADD {} {:?} -> error: {}", key_str, members, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_srem(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'srem' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut members = Vec::new();
        for arg in &args[1..] {
            if let Some(member) = arg.as_string() {
                members.push(member);
            } else {
                return Err(ProtocolError::RespError("ERR invalid member".to_string()));
            }
        }

        // Get SetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Remove members from set
        let removed_count: Result<usize, _> =
            actor_ref.invoke("srem", vec![members.clone().into()]).await;

        match removed_count {
            Ok(count) => {
                debug!("SREM {} {:?} -> {} removed", key_str, members, count);
                Ok(RespValue::integer(count as i64))
            }
            Err(e) => {
                debug!("SREM {} {:?} -> error: {}", key_str, members, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_smembers(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'smembers' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get SetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get all set members
        let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

        match members_result {
            Ok(members) => {
                let result: Vec<RespValue> = members
                    .into_iter()
                    .map(|member| RespValue::bulk_string_from_str(&member))
                    .collect();
                debug!("SMEMBERS {} -> {} members", key_str, result.len());
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!("SMEMBERS {} -> error: {}", key_str, e);
                Ok(RespValue::array(vec![]))
            }
        }
    }

    async fn cmd_scard(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'scard' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get SetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Get set cardinality (size)
        let size_result: Result<usize, _> = actor_ref.invoke("scard", vec![]).await;

        match size_result {
            Ok(size) => {
                debug!("SCARD {} -> {}", key_str, size);
                Ok(RespValue::integer(size as i64))
            }
            Err(e) => {
                debug!("SCARD {} -> error: {}", key_str, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_sismember(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sismember' command".to_string(),
            ));
        }

        let key_str = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let member = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid member".to_string()))?;

        // Get SetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: key_str.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Check if member exists in set
        let is_member_result: Result<bool, _> = actor_ref
            .invoke("sismember", vec![member.clone().into()])
            .await;

        match is_member_result {
            Ok(is_member) => {
                debug!("SISMEMBER {} {} -> {}", key_str, member, is_member);
                Ok(RespValue::integer(if is_member { 1 } else { 0 }))
            }
            Err(e) => {
                debug!("SISMEMBER {} {} -> error: {}", key_str, member, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_sunion(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sunion' command".to_string(),
            ));
        }

        let mut result_set = std::collections::HashSet::<String>::new();

        for arg in args {
            let key_str = arg
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

            // Get SetActor reference
            let actor_ref = self
                .orbit_client
                .actor_reference::<SetActor>(Key::StringKey {
                    key: key_str.clone(),
                })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            // Get set members
            let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

            if let Ok(members) = members_result {
                for member in members {
                    result_set.insert(member);
                }
                debug!(
                    "SUNION: Added {} members from key {}",
                    result_set.len(),
                    key_str
                );
            } else {
                debug!("SUNION: Failed to get members from key {}", key_str);
            }
        }

        let result: Vec<RespValue> = result_set
            .into_iter()
            .map(|member| RespValue::bulk_string_from_str(&member))
            .collect();

        debug!("SUNION: Final result has {} members", result.len());
        Ok(RespValue::array(result))
    }

    async fn cmd_sinter(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sinter' command".to_string(),
            ));
        }

        let mut result_set: Option<std::collections::HashSet<String>> = None;

        for arg in args {
            let key_str = arg
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

            // Get SetActor reference
            let actor_ref = self
                .orbit_client
                .actor_reference::<SetActor>(Key::StringKey {
                    key: key_str.clone(),
                })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            // Get set members
            let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

            if let Ok(members) = members_result {
                let current_set: std::collections::HashSet<String> = members.into_iter().collect();

                match result_set {
                    None => {
                        result_set = Some(current_set);
                        debug!(
                            "SINTER: Initialized with {} members from key {}",
                            result_set.as_ref().unwrap().len(),
                            key_str
                        );
                    }
                    Some(ref mut existing_set) => {
                        let intersection: std::collections::HashSet<String> =
                            existing_set.intersection(&current_set).cloned().collect();
                        *existing_set = intersection;
                        debug!(
                            "SINTER: After intersection with key {}, {} members remain",
                            key_str,
                            existing_set.len()
                        );
                    }
                }
            } else {
                debug!("SINTER: Failed to get members from key {}", key_str);
                // If any set is empty or missing, intersection is empty
                result_set = Some(std::collections::HashSet::new());
                break;
            }
        }

        let result: Vec<RespValue> = result_set
            .unwrap_or_default()
            .into_iter()
            .map(|member| RespValue::bulk_string_from_str(&member))
            .collect();

        debug!("SINTER: Final result has {} members", result.len());
        Ok(RespValue::array(result))
    }

    async fn cmd_sdiff(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'sdiff' command".to_string(),
            ));
        }

        let first_key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get first set
        let actor_ref = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey {
                key: first_key.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let first_members_result: Result<Vec<String>, _> =
            actor_ref.invoke("smembers", vec![]).await;

        let mut result_set: std::collections::HashSet<String> = match first_members_result {
            Ok(members) => members.into_iter().collect(),
            Err(_) => {
                debug!("SDIFF: Failed to get members from first key {}", first_key);
                return Ok(RespValue::array(vec![]));
            }
        };

        debug!(
            "SDIFF: Started with {} members from key {}",
            result_set.len(),
            first_key
        );

        // Remove elements from subsequent sets
        for arg in &args[1..] {
            let key_str = arg
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

            let actor_ref = self
                .orbit_client
                .actor_reference::<SetActor>(Key::StringKey {
                    key: key_str.clone(),
                })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            let members_result: Result<Vec<String>, _> = actor_ref.invoke("smembers", vec![]).await;

            if let Ok(members) = members_result {
                for member in members {
                    result_set.remove(&member);
                }
                debug!(
                    "SDIFF: After removing members from key {}, {} members remain",
                    key_str,
                    result_set.len()
                );
            } else {
                debug!("SDIFF: Failed to get members from key {}", key_str);
            }
        }

        let result: Vec<RespValue> = result_set
            .into_iter()
            .map(|member| RespValue::bulk_string_from_str(&member))
            .collect();

        debug!("SDIFF: Final result has {} members", result.len());
        Ok(RespValue::array(result))
    }

    // Sorted Set commands

    async fn cmd_zadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() % 2 == 0 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zadd' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let mut added_count = 0;

        // Process score-member pairs
        for i in (1..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }

            let score = args[i]
                .as_string()
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| {
                    ProtocolError::RespError("ERR value is not a valid float".to_string())
                })?;
            let member = args[i + 1]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid member".to_string()))?;

            // Invoke zadd method on the actor
            let was_new: Result<bool, _> = actor_ref
                .invoke("zadd", vec![member.clone().into(), score.into()])
                .await;

            match was_new {
                Ok(true) => {
                    added_count += 1;
                    debug!("ZADD {} {} {} -> new member", key, score, member);
                }
                Ok(false) => {
                    debug!("ZADD {} {} {} -> updated score", key, score, member);
                }
                Err(e) => {
                    debug!("ZADD {} {} {} -> error: {}", key, score, member, e);
                }
            }
        }

        Ok(RespValue::integer(added_count))
    }

    async fn cmd_zrem(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrem' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut members = Vec::new();
        for arg in &args[1..] {
            let member = arg
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid member".to_string()))?;
            members.push(member);
        }

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zrem method on the actor
        let removed_count: Result<usize, _> =
            actor_ref.invoke("zrem", vec![members.clone().into()]).await;

        match removed_count {
            Ok(count) => {
                debug!("ZREM {} {:?} -> {} removed", key, members, count);
                Ok(RespValue::integer(count as i64))
            }
            Err(e) => {
                debug!("ZREM {} {:?} -> error: {}", key, members, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_zcard(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zcard' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zcard method on the actor
        let size: Result<usize, _> = actor_ref.invoke("zcard", vec![]).await;

        match size {
            Ok(count) => {
                debug!("ZCARD {} -> {}", key, count);
                Ok(RespValue::integer(count as i64))
            }
            Err(e) => {
                debug!("ZCARD {} -> error: {}", key, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_zscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zscore' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let member = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid member".to_string()))?;

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zscore method on the actor
        let score: Result<Option<f64>, _> = actor_ref
            .invoke("zscore", vec![member.clone().into()])
            .await;

        match score {
            Ok(Some(score_val)) => {
                debug!("ZSCORE {} {} -> {}", key, member, score_val);
                Ok(RespValue::bulk_string_from_str(&score_val.to_string()))
            }
            Ok(None) => {
                debug!("ZSCORE {} {} -> null (not found)", key, member);
                Ok(RespValue::null())
            }
            Err(e) => {
                debug!("ZSCORE {} {} -> error: {}", key, member, e);
                Ok(RespValue::null())
            }
        }
    }

    async fn cmd_zincrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zincrby' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let increment = args[1]
            .as_string()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| {
                ProtocolError::RespError("ERR value is not a valid float".to_string())
            })?;
        let member = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid member".to_string()))?;

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zincrby method on the actor
        let new_score: Result<f64, _> = actor_ref
            .invoke("zincrby", vec![member.clone().into(), increment.into()])
            .await;

        match new_score {
            Ok(score) => {
                debug!("ZINCRBY {} {} {} -> {}", key, increment, member, score);
                Ok(RespValue::bulk_string_from_str(&score.to_string()))
            }
            Err(e) => {
                debug!("ZINCRBY {} {} {} -> error: {}", key, increment, member, e);
                Err(ProtocolError::RespError(format!(
                    "ERR actor invocation failed: {}",
                    e
                )))
            }
        }
    }

    async fn cmd_zrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() > 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrange' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let start = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid start index".to_string()))?;
        let stop = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid stop index".to_string()))?;

        let with_scores = if args.len() == 4 {
            args[3]
                .as_string()
                .map(|s| s.to_uppercase() == "WITHSCORES")
                .unwrap_or(false)
        } else {
            false
        };

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zrange method on the actor
        let range_result: Result<Vec<(String, Option<f64>)>, _> = actor_ref
            .invoke(
                "zrange",
                vec![start.into(), stop.into(), with_scores.into()],
            )
            .await;

        match range_result {
            Ok(members) => {
                let mut result = Vec::new();
                for (member, score_opt) in members {
                    result.push(RespValue::bulk_string_from_str(&member));
                    if let Some(score) = score_opt {
                        result.push(RespValue::bulk_string_from_str(&score.to_string()));
                    }
                }
                debug!(
                    "ZRANGE {} {} {} WITHSCORES:{} -> {} items",
                    key,
                    start,
                    stop,
                    with_scores,
                    result.len()
                );
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!("ZRANGE {} {} {} -> error: {}", key, start, stop, e);
                Ok(RespValue::array(vec![]))
            }
        }
    }

    async fn cmd_zrangebyscore(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() > 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrangebyscore' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let min_score = args[1]
            .as_string()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| {
                ProtocolError::RespError("ERR min value is not a valid float".to_string())
            })?;
        let max_score = args[2]
            .as_string()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| {
                ProtocolError::RespError("ERR max value is not a valid float".to_string())
            })?;

        let with_scores = if args.len() == 4 {
            args[3]
                .as_string()
                .map(|s| s.to_uppercase() == "WITHSCORES")
                .unwrap_or(false)
        } else {
            false
        };

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zrangebyscore method on the actor
        let range_result: Result<Vec<(String, Option<f64>)>, _> = actor_ref
            .invoke(
                "zrangebyscore",
                vec![min_score.into(), max_score.into(), with_scores.into()],
            )
            .await;

        match range_result {
            Ok(members) => {
                let mut result = Vec::new();
                for (member, score_opt) in members {
                    result.push(RespValue::bulk_string_from_str(&member));
                    if let Some(score) = score_opt {
                        result.push(RespValue::bulk_string_from_str(&score.to_string()));
                    }
                }
                debug!(
                    "ZRANGEBYSCORE {} {} {} WITHSCORES:{} -> {} items",
                    key,
                    min_score,
                    max_score,
                    with_scores,
                    result.len()
                );
                Ok(RespValue::array(result))
            }
            Err(e) => {
                debug!(
                    "ZRANGEBYSCORE {} {} {} -> error: {}",
                    key, min_score, max_score, e
                );
                Ok(RespValue::array(vec![]))
            }
        }
    }

    async fn cmd_zcount(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zcount' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let min_score = args[1]
            .as_string()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| {
                ProtocolError::RespError("ERR min value is not a valid float".to_string())
            })?;
        let max_score = args[2]
            .as_string()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| {
                ProtocolError::RespError("ERR max value is not a valid float".to_string())
            })?;

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zcount method on the actor
        let count_result: Result<usize, _> = actor_ref
            .invoke("zcount", vec![min_score.into(), max_score.into()])
            .await;

        match count_result {
            Ok(count) => {
                debug!("ZCOUNT {} {} {} -> {}", key, min_score, max_score, count);
                Ok(RespValue::integer(count as i64))
            }
            Err(e) => {
                debug!("ZCOUNT {} {} {} -> error: {}", key, min_score, max_score, e);
                Ok(RespValue::integer(0))
            }
        }
    }

    async fn cmd_zrank(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'zrank' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let member = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid member".to_string()))?;

        // Get SortedSetActor reference
        let actor_ref = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke zrank method on the actor
        let rank_result: Result<Option<usize>, _> =
            actor_ref.invoke("zrank", vec![member.clone().into()]).await;

        match rank_result {
            Ok(Some(rank)) => {
                debug!("ZRANK {} {} -> {}", key, member, rank);
                Ok(RespValue::integer(rank as i64))
            }
            Ok(None) => {
                debug!("ZRANK {} {} -> null (not found)", key, member);
                Ok(RespValue::null())
            }
            Err(e) => {
                debug!("ZRANK {} {} -> error: {}", key, member, e);
                Ok(RespValue::null())
            }
        }
    }

    // Server commands

    async fn cmd_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let section = if args.is_empty() {
            "default".to_string()
        } else {
            args[0].as_string().unwrap_or("default".to_string())
        };

        let info = format!(
            "# Server\r\n\
             redis_version:7.0.0-orbit\r\n\
             redis_git_sha1:00000000\r\n\
             redis_git_dirty:0\r\n\
             redis_build_id:00000000\r\n\
             redis_mode:standalone\r\n\
             os:Darwin 23.6.0 x86_64\r\n\
             arch_bits:64\r\n\
             multiplexing_api:kqueue\r\n\
             process_id:{}\r\n\
             run_id:orbit-{}\r\n\
             tcp_port:6379\r\n\
             uptime_in_seconds:3600\r\n\
             uptime_in_days:0\r\n\
             hz:10\r\n\
             lru_clock:1234567\r\n\
             config_file:\r\n\
             \r\n\
             # Clients\r\n\
             connected_clients:1\r\n\
             client_longest_output_list:0\r\n\
             client_biggest_input_buf:0\r\n\
             blocked_clients:0\r\n\
             \r\n\
             # Memory\r\n\
             used_memory:1048576\r\n\
             used_memory_human:1.00M\r\n\
             used_memory_rss:2097152\r\n\
             used_memory_peak:2097152\r\n\
             used_memory_peak_human:2.00M\r\n\
             \r\n\
             # Persistence\r\n\
             loading:0\r\n\
             rdb_changes_since_last_save:0\r\n\
             rdb_bgsave_in_progress:0\r\n\
             rdb_last_save_time:1234567890\r\n\
             \r\n\
             # Stats\r\n\
             total_connections_received:1\r\n\
             total_commands_processed:0\r\n\
             instantaneous_ops_per_sec:0\r\n\
             rejected_connections:0\r\n\
             \r\n\
             # Orbit\r\n\
             orbit_mode:protocol_adapter\r\n\
             orbit_actor_count:0\r\n\
             orbit_cluster_nodes:1\r\n",
            std::process::id(),
            chrono::Utc::now().timestamp()
        );

        debug!("INFO (section: {})", section);
        Ok(RespValue::bulk_string_from_str(info))
    }

    async fn cmd_dbsize(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // TODO: Replace with actual OrbitClient active actor count
        debug!("DBSIZE (placeholder implementation)");
        Ok(RespValue::integer(0))
    }

    async fn cmd_flushdb(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // TODO: Replace with actual OrbitClient namespace clearing
        debug!("FLUSHDB (placeholder implementation)");
        Ok(RespValue::ok())
    }

    async fn cmd_command(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // Return list of supported commands
        let commands = vec![
            // Connection
            vec![
                RespValue::bulk_string_from_str("ping"),
                RespValue::integer(-1),
                RespValue::integer(1),
                RespValue::integer(0),
                RespValue::integer(0),
            ],
            vec![
                RespValue::bulk_string_from_str("echo"),
                RespValue::integer(2),
                RespValue::integer(1),
                RespValue::integer(1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("select"),
                RespValue::integer(2),
                RespValue::integer(1),
                RespValue::integer(1),
                RespValue::integer(1),
            ],
            // String
            vec![
                RespValue::bulk_string_from_str("get"),
                RespValue::integer(2),
                RespValue::integer(1),
                RespValue::integer(1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("set"),
                RespValue::integer(-3),
                RespValue::integer(1),
                RespValue::integer(1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("del"),
                RespValue::integer(-2),
                RespValue::integer(1),
                RespValue::integer(-1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("exists"),
                RespValue::integer(-2),
                RespValue::integer(1),
                RespValue::integer(-1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("ttl"),
                RespValue::integer(2),
                RespValue::integer(1),
                RespValue::integer(1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("expire"),
                RespValue::integer(3),
                RespValue::integer(1),
                RespValue::integer(1),
                RespValue::integer(1),
            ],
            vec![
                RespValue::bulk_string_from_str("keys"),
                RespValue::integer(2),
                RespValue::integer(0),
                RespValue::integer(0),
                RespValue::integer(0),
            ],
            // Server
            vec![
                RespValue::bulk_string_from_str("info"),
                RespValue::integer(-1),
                RespValue::integer(0),
                RespValue::integer(0),
                RespValue::integer(0),
            ],
            vec![
                RespValue::bulk_string_from_str("dbsize"),
                RespValue::integer(1),
                RespValue::integer(0),
                RespValue::integer(0),
                RespValue::integer(0),
            ],
            vec![
                RespValue::bulk_string_from_str("command"),
                RespValue::integer(-1),
                RespValue::integer(0),
                RespValue::integer(0),
                RespValue::integer(0),
            ],
        ];

        Ok(RespValue::array(
            commands.into_iter().map(RespValue::array).collect(),
        ))
    }

    // New String/Key commands

    async fn cmd_append(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'append' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let value = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let new_length: usize = actor_ref
            .invoke("append_value", vec![value.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("APPEND {} {} -> {}", key, value, new_length);
        Ok(RespValue::integer(new_length as i64))
    }

    async fn cmd_getrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'getrange' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let start = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid start index".to_string()))?;
        let end = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid end index".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let result: Option<String> = actor_ref
            .invoke("get_range", vec![start.into(), end.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("GETRANGE {} {} {} -> {:?}", key, start, end, result);
        Ok(result
            .map(RespValue::bulk_string_from_str)
            .unwrap_or(RespValue::bulk_string_from_str("")))
    }

    async fn cmd_getset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'getset' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let new_value = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let old_value: Option<String> = actor_ref
            .invoke("get_and_set", vec![new_value.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("GETSET {} {} -> {:?}", key, new_value, old_value);
        Ok(old_value
            .map(RespValue::bulk_string_from_str)
            .unwrap_or(RespValue::null()))
    }

    async fn cmd_mget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'mget' command".to_string(),
            ));
        }

        let mut results = Vec::new();
        for arg in args {
            if let Some(key) = arg.as_string() {
                let actor_ref_result = self
                    .orbit_client
                    .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
                    .await;

                if let Ok(actor_ref) = actor_ref_result {
                    let value: Result<Option<String>, _> =
                        actor_ref.invoke("get_value", vec![]).await;

                    match value {
                        Ok(Some(v)) => results.push(RespValue::bulk_string_from_str(&v)),
                        _ => results.push(RespValue::null()),
                    }
                } else {
                    results.push(RespValue::null());
                }
            } else {
                results.push(RespValue::null());
            }
        }

        debug!("MGET {} keys -> {} results", args.len(), results.len());
        Ok(RespValue::array(results))
    }

    async fn cmd_mset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len().is_multiple_of(2) {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'mset' command".to_string(),
            ));
        }

        for i in (0..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }

            let key = args[i]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
            let value = args[i + 1]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

            let actor_ref = self
                .orbit_client
                .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            actor_ref
                .invoke::<()>("set_value", vec![value.clone().into()])
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR actor invocation failed: {}", e))
                })?;
        }

        debug!("MSET {} pairs", args.len() / 2);
        Ok(RespValue::ok())
    }

    async fn cmd_setex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'setex' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let seconds = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid expire time".to_string()))?;
        let value = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        if seconds <= 0 {
            return Err(ProtocolError::RespError(
                "ERR invalid expire time in 'setex' command".to_string(),
            ));
        }

        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        actor_ref
            .invoke::<()>("set_value", vec![value.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        actor_ref
            .invoke::<()>("set_expiration", vec![(seconds as u64).into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("SETEX {} {} {}", key, seconds, value);
        Ok(RespValue::ok())
    }

    async fn cmd_setrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'setrange' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let offset = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid offset".to_string()))?;
        let value = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

        if offset < 0 {
            return Err(ProtocolError::RespError(
                "ERR offset is out of range".to_string(),
            ));
        }

        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let new_length: usize = actor_ref
            .invoke(
                "set_range",
                vec![(offset as usize).into(), value.clone().into()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("SETRANGE {} {} {} -> {}", key, offset, value, new_length);
        Ok(RespValue::integer(new_length as i64))
    }

    async fn cmd_strlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'strlen' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let length: usize = actor_ref
            .invoke("strlen", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("STRLEN {} -> {}", key, length);
        Ok(RespValue::integer(length as i64))
    }

    async fn cmd_persist(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'persist' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref_result = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = actor_ref_result {
            let had_expiration: Result<bool, _> = actor_ref.invoke("persist", vec![]).await;

            match had_expiration {
                Ok(had_exp) => {
                    debug!("PERSIST {} -> {}", key, if had_exp { 1 } else { 0 });
                    Ok(RespValue::integer(if had_exp { 1 } else { 0 }))
                }
                Err(_) => {
                    debug!("PERSIST {} -> 0 (key doesn't exist)", key);
                    Ok(RespValue::integer(0))
                }
            }
        } else {
            debug!("PERSIST {} -> 0 (no actor reference)", key);
            Ok(RespValue::integer(0))
        }
    }

    async fn cmd_pexpire(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'pexpire' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let milliseconds = args[1]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid timeout".to_string()))?;

        if milliseconds < 0 {
            return Err(ProtocolError::RespError(
                "ERR invalid expire time in 'pexpire' command".to_string(),
            ));
        }

        let actor_ref_result = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = actor_ref_result {
            let exists_result: Result<bool, _> = actor_ref.invoke("exists", vec![]).await;

            match exists_result {
                Ok(exists) => {
                    if exists {
                        let expire_result: Result<(), _> = actor_ref
                            .invoke("set_pexpiration", vec![(milliseconds as u64).into()])
                            .await;

                        match expire_result {
                            Ok(_) => {
                                debug!("PEXPIRE {} {} -> timeout set", key, milliseconds);
                                Ok(RespValue::integer(1))
                            }
                            Err(_) => {
                                debug!(
                                    "PEXPIRE {} {} -> failed to set expiration",
                                    key, milliseconds
                                );
                                Ok(RespValue::integer(0))
                            }
                        }
                    } else {
                        debug!("PEXPIRE {} {} -> key doesn't exist", key, milliseconds);
                        Ok(RespValue::integer(0))
                    }
                }
                Err(_) => {
                    debug!("PEXPIRE {} {} -> key doesn't exist", key, milliseconds);
                    Ok(RespValue::integer(0))
                }
            }
        } else {
            debug!("PEXPIRE {} {} -> no actor reference", key, milliseconds);
            Ok(RespValue::integer(0))
        }
    }

    async fn cmd_pttl(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'pttl' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref_result = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = actor_ref_result {
            let pttl_result: Result<i64, _> = actor_ref.invoke("get_pttl", vec![]).await;

            match pttl_result {
                Ok(pttl) => {
                    debug!("PTTL {} -> {}", key, pttl);
                    Ok(RespValue::integer(pttl))
                }
                Err(_) => {
                    debug!("PTTL {} -> -2 (key doesn't exist)", key);
                    Ok(RespValue::integer(-2))
                }
            }
        } else {
            debug!("PTTL {} -> -2 (no actor reference)", key);
            Ok(RespValue::integer(-2))
        }
    }

    async fn cmd_randomkey(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if !args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'randomkey' command".to_string(),
            ));
        }

        // TODO: Replace with actual OrbitClient key listing functionality
        debug!("RANDOMKEY (placeholder implementation)");
        Ok(RespValue::null()) // Return null for now as we don't have key enumeration
    }

    async fn cmd_rename(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'rename' command".to_string(),
            ));
        }

        let old_key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let new_key = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get the old key's value
        let old_actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey {
                key: old_key.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let value: Option<String> = old_actor_ref
            .invoke("get_value", vec![])
            .await
            .map_err(|_| ProtocolError::RespError(format!("ERR no such key")))?;

        if value.is_none() {
            return Err(ProtocolError::RespError("ERR no such key".to_string()));
        }

        let value = value.unwrap();

        // Set the new key
        let new_actor_ref = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey {
                key: new_key.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        new_actor_ref
            .invoke::<()>("set_value", vec![value.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        // Delete the old key
        old_actor_ref
            .invoke::<()>("delete_value", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        debug!("RENAME {} {} -> OK", old_key, new_key);
        Ok(RespValue::ok())
    }

    async fn cmd_type(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'type' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Try different actor types to determine the type
        // First try KeyValueActor (string type)
        let kv_result = self
            .orbit_client
            .actor_reference::<KeyValueActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = kv_result {
            let exists: Result<bool, _> = actor_ref.invoke("exists", vec![]).await;
            if exists.unwrap_or(false) {
                debug!("TYPE {} -> string", key);
                return Ok(RespValue::simple_string("string"));
            }
        }

        // Try HashActor
        let hash_result = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = hash_result {
            let len: Result<usize, _> = actor_ref.invoke("hlen", vec![]).await;
            if len.unwrap_or(0) > 0 {
                debug!("TYPE {} -> hash", key);
                return Ok(RespValue::simple_string("hash"));
            }
        }

        // Try ListActor
        let list_result = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = list_result {
            let len: Result<usize, _> = actor_ref.invoke("llen", vec![]).await;
            if len.unwrap_or(0) > 0 {
                debug!("TYPE {} -> list", key);
                return Ok(RespValue::simple_string("list"));
            }
        }

        // Try SetActor
        let set_result = self
            .orbit_client
            .actor_reference::<SetActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = set_result {
            let len: Result<usize, _> = actor_ref.invoke("scard", vec![]).await;
            if len.unwrap_or(0) > 0 {
                debug!("TYPE {} -> set", key);
                return Ok(RespValue::simple_string("set"));
            }
        }

        // Try SortedSetActor
        let zset_result = self
            .orbit_client
            .actor_reference::<SortedSetActor>(Key::StringKey { key: key.clone() })
            .await;

        if let Ok(actor_ref) = zset_result {
            let len: Result<usize, _> = actor_ref.invoke("zcard", vec![]).await;
            if len.unwrap_or(0) > 0 {
                debug!("TYPE {} -> zset", key);
                return Ok(RespValue::simple_string("zset"));
            }
        }

        debug!("TYPE {} -> none", key);
        Ok(RespValue::simple_string("none"))
    }

    async fn cmd_unlink(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        // UNLINK is the same as DEL but supposed to be asynchronous
        // For now, we'll implement it the same as DEL
        self.cmd_del(args).await
    }

    // Hash commands

    async fn cmd_hincrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'hincrby' command".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let field = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;
        let increment = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid increment".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<HashActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let result: Result<i64, String> = actor_ref
            .invoke("hincrby", vec![field.clone().into(), increment.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        match result {
            Ok(new_value) => {
                debug!("HINCRBY {} {} {} -> {}", key, field, increment, new_value);
                Ok(RespValue::integer(new_value))
            }
            Err(err_msg) => Err(ProtocolError::RespError(err_msg)),
        }
    }

    // List commands

    async fn cmd_blpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'blpop' command".to_string(),
            ));
        }

        // For now, implement as non-blocking LPOP on the first key
        // TODO: Implement proper blocking behavior
        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let values: Vec<String> = actor_ref
            .invoke("lpop", vec![1i64.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        if values.is_empty() {
            debug!("BLPOP {} -> null (empty list)", key);
            Ok(RespValue::null())
        } else {
            debug!("BLPOP {} -> [{}, {}]", key, key, values[0]);
            Ok(RespValue::array(vec![
                RespValue::bulk_string_from_str(&key),
                RespValue::bulk_string_from_str(&values[0]),
            ]))
        }
    }

    async fn cmd_brpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'brpop' command".to_string(),
            ));
        }

        // For now, implement as non-blocking RPOP on the first key
        // TODO: Implement proper blocking behavior
        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<ListActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let values: Vec<String> = actor_ref
            .invoke("rpop", vec![1i64.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        if values.is_empty() {
            debug!("BRPOP {} -> null (empty list)", key);
            Ok(RespValue::null())
        } else {
            debug!("BRPOP {} -> [{}, {}]", key, key, values[0]);
            Ok(RespValue::array(vec![
                RespValue::bulk_string_from_str(&key),
                RespValue::bulk_string_from_str(&values[0]),
            ]))
        }
    }

    // Pub/Sub commands

    async fn cmd_pubsub(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'pubsub' command".to_string(),
            ));
        }

        let subcommand = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid subcommand".to_string()))?;

        match subcommand.to_uppercase().as_str() {
            "CHANNELS" => {
                // TODO: Implement listing of active channels
                debug!("PUBSUB CHANNELS (placeholder implementation)");
                Ok(RespValue::array(vec![]))
            }
            "NUMSUB" => {
                // TODO: Implement subscriber count for channels
                debug!("PUBSUB NUMSUB (placeholder implementation)");
                Ok(RespValue::array(vec![]))
            }
            "NUMPAT" => {
                // TODO: Implement pattern subscriber count
                debug!("PUBSUB NUMPAT (placeholder implementation)");
                Ok(RespValue::integer(0))
            }
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown PUBSUB subcommand '{}'",
                subcommand
            ))),
        }
    }

    // Server commands

    async fn cmd_flushall(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // TODO: Replace with actual OrbitClient global clearing
        debug!("FLUSHALL (placeholder implementation)");
        Ok(RespValue::ok())
    }

    // Vector parsing utilities

    /// Parse vector data from string format "x1,x2,x3,..." or array format
    fn parse_vector_data(&self, input: &RespValue) -> ProtocolResult<Vec<f32>> {
        match input {
            RespValue::BulkString(s) => {
                let vector_str = std::str::from_utf8(s).map_err(|_| {
                    ProtocolError::RespError("Invalid UTF-8 in vector data".to_string())
                })?;

                let values: Result<Vec<f32>, _> = vector_str
                    .split(',')
                    .map(|s| s.trim().parse::<f32>())
                    .collect();

                values.map_err(|_| {
                    ProtocolError::RespError(
                        "Invalid vector format. Use comma-separated floats: \"1.0,2.0,3.0\""
                            .to_string(),
                    )
                })
            }
            RespValue::Array(arr) => {
                let values: Result<Vec<f32>, _> = arr
                    .iter()
                    .map(|v| {
                        v.as_string()
                            .ok_or_else(|| {
                                ProtocolError::RespError(
                                    "Vector array must contain strings".to_string(),
                                )
                            })
                            .and_then(|s| {
                                s.parse::<f32>().map_err(|_| {
                                    ProtocolError::RespError(
                                        "Invalid float in vector array".to_string(),
                                    )
                                })
                            })
                    })
                    .collect();
                values
            }
            _ => Err(ProtocolError::RespError(
                "Vector data must be string or array".to_string(),
            )),
        }
    }

    /// Parse similarity metric from string
    fn parse_similarity_metric(&self, metric_str: &str) -> ProtocolResult<SimilarityMetric> {
        match metric_str.to_uppercase().as_str() {
            "COSINE" | "COS" => Ok(SimilarityMetric::Cosine),
            "EUCLIDEAN" | "L2" | "EUCL" => Ok(SimilarityMetric::Euclidean),
            "DOT" | "DOTPRODUCT" | "IP" | "INNER" => Ok(SimilarityMetric::DotProduct),
            "MANHATTAN" | "L1" | "MAN" => Ok(SimilarityMetric::Manhattan),
            _ => Err(ProtocolError::RespError(format!(
                "Unknown similarity metric '{}'. Use: COSINE, EUCLIDEAN, DOT, or MANHATTAN",
                metric_str
            ))),
        }
    }

    /// Format vector data for Redis response
    fn format_vector_data(&self, data: &[f32]) -> String {
        data.iter()
            .map(|f| format!("{:.6}", f))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Parse metadata from key-value pairs in command arguments
    fn parse_metadata(
        &self,
        args: &[RespValue],
        start_idx: usize,
    ) -> ProtocolResult<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        let mut i = start_idx;
        while i + 1 < args.len() {
            let key = args[i].as_string().ok_or_else(|| {
                ProtocolError::RespError("Metadata key must be string".to_string())
            })?;
            let value = args[i + 1].as_string().ok_or_else(|| {
                ProtocolError::RespError("Metadata value must be string".to_string())
            })?;

            metadata.insert(key, value);
            i += 2;
        }

        if i < args.len() {
            return Err(ProtocolError::RespError(
                "Odd number of metadata key-value pairs".to_string(),
            ));
        }

        Ok(metadata)
    }

    // VECTOR.* command implementations

    async fn cmd_vector_add(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.ADD' command. Usage: VECTOR.ADD <index> <id> <vector> [key value ...]".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;
        let id = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid vector ID".to_string()))?;

        let vector_data = self.parse_vector_data(&args[2])?;
        let metadata = if args.len() > 3 {
            self.parse_metadata(args, 3)?
        } else {
            HashMap::new()
        };

        let vector = Vector::with_metadata(id.clone(), vector_data, metadata);

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        actor_ref
            .invoke::<()>(
                "add_vector",
                vec![serde_json::to_value(vector).unwrap().into()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to add vector: {}", e)))?;

        debug!("VECTOR.ADD {} {} -> OK", index, id);
        Ok(RespValue::ok())
    }

    async fn cmd_vector_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.GET' command. Usage: VECTOR.GET <index> <id>".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;
        let id = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid vector ID".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let vector: Option<Vector> = actor_ref
            .invoke("get_vector", vec![id.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to get vector: {}", e)))?;

        match vector {
            Some(vec) => {
                debug!("VECTOR.GET {} {} -> found", index, id);

                let mut result = vec![
                    RespValue::bulk_string_from_str(&vec.id),
                    RespValue::bulk_string_from_str(&self.format_vector_data(&vec.data)),
                ];

                // Add metadata
                for (key, value) in vec.metadata {
                    result.push(RespValue::bulk_string_from_str(&key));
                    result.push(RespValue::bulk_string_from_str(&value));
                }

                Ok(RespValue::array(result))
            }
            None => {
                debug!("VECTOR.GET {} {} -> not found", index, id);
                Ok(RespValue::null())
            }
        }
    }

    async fn cmd_vector_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.DEL' command. Usage: VECTOR.DEL <index> <id>".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;
        let id = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid vector ID".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let deleted: bool = actor_ref
            .invoke("remove_vector", vec![id.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to delete vector: {}", e)))?;

        debug!(
            "VECTOR.DEL {} {} -> {}",
            index,
            id,
            if deleted { 1 } else { 0 }
        );
        Ok(RespValue::integer(if deleted { 1 } else { 0 }))
    }

    async fn cmd_vector_stats(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.STATS' command. Usage: VECTOR.STATS <index>".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let stats: VectorStats = actor_ref
            .invoke("get_stats", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to get stats: {}", e)))?;

        debug!("VECTOR.STATS {} -> {} vectors", index, stats.vector_count);

        let result = vec![
            RespValue::bulk_string_from_str("vector_count"),
            RespValue::integer(stats.vector_count as i64),
            RespValue::bulk_string_from_str("index_count"),
            RespValue::integer(stats.index_count as i64),
            RespValue::bulk_string_from_str("avg_dimension"),
            RespValue::bulk_string_from_str(&format!("{:.2}", stats.avg_dimension)),
            RespValue::bulk_string_from_str("min_dimension"),
            RespValue::integer(stats.min_dimension as i64),
            RespValue::bulk_string_from_str("max_dimension"),
            RespValue::integer(stats.max_dimension as i64),
            RespValue::bulk_string_from_str("avg_metadata_keys"),
            RespValue::bulk_string_from_str(&format!("{:.2}", stats.avg_metadata_keys)),
        ];

        Ok(RespValue::array(result))
    }

    async fn cmd_vector_list(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.LIST' command. Usage: VECTOR.LIST <index>".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let ids: Vec<String> = actor_ref
            .invoke("list_vector_ids", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to list vectors: {}", e)))?;

        debug!("VECTOR.LIST {} -> {} IDs", index, ids.len());

        let result: Vec<RespValue> = ids
            .into_iter()
            .map(|id| RespValue::bulk_string_from_str(&id))
            .collect();

        Ok(RespValue::array(result))
    }

    async fn cmd_vector_count(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.COUNT' command. Usage: VECTOR.COUNT <index>".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let count: usize = actor_ref
            .invoke("vector_count", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to get count: {}", e)))?;

        debug!("VECTOR.COUNT {} -> {}", index, count);
        Ok(RespValue::integer(count as i64))
    }

    async fn cmd_vector_search(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.SEARCH' command. Usage: VECTOR.SEARCH <index> <vector> <limit> [METRIC <metric>] [THRESHOLD <threshold>] [key value ...]".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        let query_vector = self.parse_vector_data(&args[1])?;

        let limit = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid limit".to_string()))?;

        if limit <= 0 {
            return Err(ProtocolError::RespError(
                "ERR limit must be positive".to_string(),
            ));
        }

        let mut metric = SimilarityMetric::Cosine;
        let mut threshold = None;
        let mut metadata_filters = HashMap::new();
        let mut i = 3;

        // Parse optional parameters
        while i < args.len() {
            if let Some(param) = args[i].as_string() {
                match param.to_uppercase().as_str() {
                    "METRIC" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError(
                                "ERR METRIC requires a value".to_string(),
                            ));
                        }
                        let metric_str = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError("ERR METRIC value must be string".to_string())
                        })?;
                        metric = self.parse_similarity_metric(&metric_str)?;
                        i += 2;
                    }
                    "THRESHOLD" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError(
                                "ERR THRESHOLD requires a value".to_string(),
                            ));
                        }
                        let threshold_str = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError(
                                "ERR THRESHOLD value must be string".to_string(),
                            )
                        })?;
                        let threshold_val = threshold_str.parse::<f32>().map_err(|_| {
                            ProtocolError::RespError("ERR invalid threshold value".to_string())
                        })?;
                        threshold = Some(threshold_val);
                        i += 2;
                    }
                    _ => {
                        // Parse metadata filters (key-value pairs)
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError(
                                "ERR metadata filters require key-value pairs".to_string(),
                            ));
                        }
                        let key = param.clone();
                        let value = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError(
                                "ERR metadata value must be string".to_string(),
                            )
                        })?;
                        metadata_filters.insert(key, value);
                        i += 2;
                    }
                }
            } else {
                return Err(ProtocolError::RespError(
                    "ERR invalid parameter".to_string(),
                ));
            }
        }

        let search_params = VectorSearchParams {
            query_vector,
            metric,
            limit: limit as usize,
            threshold,
            metadata_filters,
        };

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let results: Vec<VectorSearchResult> = actor_ref
            .invoke(
                "search_vectors",
                vec![serde_json::to_value(search_params).unwrap().into()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR search failed: {}", e)))?;

        debug!("VECTOR.SEARCH {} -> {} results", index, results.len());

        let mut response = Vec::new();
        for result in results {
            let mut item = vec![
                RespValue::bulk_string_from_str(&result.vector.id),
                RespValue::bulk_string_from_str(&format!("{:.6}", result.score)),
                RespValue::bulk_string_from_str(&self.format_vector_data(&result.vector.data)),
            ];

            // Add metadata
            for (key, value) in result.vector.metadata {
                item.push(RespValue::bulk_string_from_str(&key));
                item.push(RespValue::bulk_string_from_str(&value));
            }

            response.push(RespValue::array(item));
        }

        Ok(RespValue::array(response))
    }

    async fn cmd_vector_knn(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() > 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'VECTOR.KNN' command. Usage: VECTOR.KNN <index> <vector> <k> [METRIC <metric>]".to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        let query_vector = self.parse_vector_data(&args[1])?;

        let k = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid k value".to_string()))?;

        if k <= 0 {
            return Err(ProtocolError::RespError(
                "ERR k must be positive".to_string(),
            ));
        }

        let metric = if args.len() == 4 {
            let metric_str = args[3]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR metric must be string".to_string()))?;
            Some(self.parse_similarity_metric(&metric_str)?)
        } else {
            None
        };

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let results: Vec<VectorSearchResult> = actor_ref
            .invoke(
                "knn_search",
                vec![
                    serde_json::to_value(query_vector).unwrap().into(),
                    (k as usize).into(),
                    serde_json::to_value(metric).unwrap().into(),
                ],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR KNN search failed: {}", e)))?;

        debug!("VECTOR.KNN {} k={} -> {} results", index, k, results.len());

        let mut response = Vec::new();
        for result in results {
            let item = vec![
                RespValue::bulk_string_from_str(&result.vector.id),
                RespValue::bulk_string_from_str(&format!("{:.6}", result.score)),
            ];
            response.push(RespValue::array(item));
        }

        Ok(RespValue::array(response))
    }

    // FT.* (RedisSearch-compatible) command implementations

    async fn cmd_ft_create(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'FT.CREATE' command. Usage: FT.CREATE <index> DIM <dimension> [DISTANCE_METRIC <metric>]".to_string(),
            ));
        }

        let index_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        // Parse DIM parameter
        if args[1].as_string().map(|s| s.to_uppercase()) != Some("DIM".to_string()) {
            return Err(ProtocolError::RespError(
                "ERR expected DIM parameter".to_string(),
            ));
        }

        let dimension = args[2]
            .as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid dimension".to_string()))?;

        if dimension <= 0 {
            return Err(ProtocolError::RespError(
                "ERR dimension must be positive".to_string(),
            ));
        }

        let mut metric = SimilarityMetric::Cosine;
        let mut i = 3;

        // Parse optional parameters
        while i + 1 < args.len() {
            if let Some(param) = args[i].as_string() {
                match param.to_uppercase().as_str() {
                    "DISTANCE_METRIC" => {
                        let metric_str = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError(
                                "ERR DISTANCE_METRIC value must be string".to_string(),
                            )
                        })?;
                        metric = self.parse_similarity_metric(&metric_str)?;
                        i += 2;
                    }
                    _ => {
                        return Err(ProtocolError::RespError(format!(
                            "ERR unknown parameter: {}",
                            param
                        )));
                    }
                }
            } else {
                return Err(ProtocolError::RespError(
                    "ERR invalid parameter".to_string(),
                ));
            }
        }

        let index_config = VectorIndexConfig::new(index_name.clone(), dimension as usize, metric);

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey {
                key: index_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        actor_ref
            .invoke::<()>(
                "create_index",
                vec![serde_json::to_value(index_config).unwrap().into()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to create index: {}", e)))?;

        debug!(
            "FT.CREATE {} DIM {} METRIC {:?} -> OK",
            index_name, dimension, metric
        );
        Ok(RespValue::ok())
    }

    async fn cmd_ft_add(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'FT.ADD' command. Usage: FT.ADD <index> <id> <vector> [key value ...]".to_string(),
            ));
        }

        // FT.ADD is equivalent to VECTOR.ADD
        self.cmd_vector_add(args).await
    }

    async fn cmd_ft_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'FT.DEL' command. Usage: FT.DEL <index> <id>"
                    .to_string(),
            ));
        }

        // FT.DEL is equivalent to VECTOR.DEL
        self.cmd_vector_del(args).await
    }

    async fn cmd_ft_search(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'FT.SEARCH' command. Usage: FT.SEARCH <index> <vector> <limit> [DISTANCE_METRIC <metric>] [key value ...]".to_string(),
            ));
        }

        // Convert FT.SEARCH format to VECTOR.SEARCH format
        let mut converted_args = args.to_vec();

        // Replace DISTANCE_METRIC with METRIC for compatibility
        for i in 0..converted_args.len() {
            if let Some(param) = converted_args[i].as_string() {
                if param.to_uppercase() == "DISTANCE_METRIC" {
                    converted_args[i] = RespValue::bulk_string_from_str("METRIC");
                }
            }
        }

        self.cmd_vector_search(&converted_args).await
    }

    async fn cmd_ft_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'FT.INFO' command. Usage: FT.INFO <index>"
                    .to_string(),
            ));
        }

        let index = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index name".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<VectorActor>(Key::StringKey { key: index.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let stats: VectorStats = actor_ref
            .invoke("get_stats", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to get info: {}", e)))?;

        let indices: Vec<VectorIndexConfig> = actor_ref
            .invoke("list_indices", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to list indices: {}", e)))?;

        debug!(
            "FT.INFO {} -> {} vectors, {} indices",
            index, stats.vector_count, stats.index_count
        );

        let mut result = vec![
            RespValue::bulk_string_from_str("index_name"),
            RespValue::bulk_string_from_str(&index),
            RespValue::bulk_string_from_str("num_docs"),
            RespValue::integer(stats.vector_count as i64),
            RespValue::bulk_string_from_str("num_indices"),
            RespValue::integer(stats.index_count as i64),
            RespValue::bulk_string_from_str("avg_dimension"),
            RespValue::integer(stats.avg_dimension as i64),
            RespValue::bulk_string_from_str("dimension_range"),
            RespValue::bulk_string_from_str(&format!(
                "{}-{}",
                stats.min_dimension, stats.max_dimension
            )),
        ];

        // Add index information
        if !indices.is_empty() {
            result.push(RespValue::bulk_string_from_str("indices"));
            let mut index_info = Vec::new();
            for idx in indices {
                index_info.push(RespValue::array(vec![
                    RespValue::bulk_string_from_str(&idx.name),
                    RespValue::integer(idx.dimension as i64),
                    RespValue::bulk_string_from_str(&format!("{:?}", idx.metric)),
                ]));
            }
            result.push(RespValue::array(index_info));
        }

        Ok(RespValue::array(result))
    }

    // Time series parsing utilities

    /// Parse timestamp from string or integer
    fn parse_timestamp(&self, input: &RespValue) -> ProtocolResult<u64> {
        match input {
            RespValue::Integer(ts) => {
                if *ts < 0 {
                    Err(ProtocolError::RespError(
                        "Timestamp cannot be negative".to_string(),
                    ))
                } else {
                    Ok(*ts as u64)
                }
            }
            RespValue::BulkString(s) => {
                let ts_str = std::str::from_utf8(s).map_err(|_| {
                    ProtocolError::RespError("Invalid UTF-8 in timestamp".to_string())
                })?;

                if ts_str == "*" {
                    // Use current timestamp
                    Ok(std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64)
                } else {
                    ts_str.parse::<u64>().map_err(|_| {
                        ProtocolError::RespError("Invalid timestamp format".to_string())
                    })
                }
            }
            _ => Err(ProtocolError::RespError(
                "Timestamp must be integer or string".to_string(),
            )),
        }
    }

    /// Parse value from string or number
    fn parse_value(&self, input: &RespValue) -> ProtocolResult<f64> {
        match input {
            RespValue::Integer(val) => Ok(*val as f64),
            RespValue::BulkString(s) => {
                let val_str = std::str::from_utf8(s)
                    .map_err(|_| ProtocolError::RespError("Invalid UTF-8 in value".to_string()))?;
                val_str
                    .parse::<f64>()
                    .map_err(|_| ProtocolError::RespError("Invalid value format".to_string()))
            }
            _ => Err(ProtocolError::RespError(
                "Value must be number or string".to_string(),
            )),
        }
    }

    /// Parse aggregation function
    fn parse_aggregation(&self, agg_str: &str) -> ProtocolResult<AggregationType> {
        AggregationType::from_str(agg_str)
            .ok_or_else(|| ProtocolError::RespError(format!(
                "Unknown aggregation function '{}'. Use: AVG, SUM, MIN, MAX, COUNT, FIRST, LAST, RANGE, STD",
                agg_str
            )))
    }

    /// Parse duplicate policy
    fn parse_duplicate_policy(&self, policy_str: &str) -> ProtocolResult<DuplicatePolicy> {
        DuplicatePolicy::from_str(policy_str).ok_or_else(|| {
            ProtocolError::RespError(format!(
                "Unknown duplicate policy '{}'. Use: BLOCK, FIRST, LAST, MIN, MAX, SUM",
                policy_str
            ))
        })
    }

    /// Parse time series labels from key-value pairs
    fn parse_ts_labels(
        &self,
        args: &[RespValue],
        start_idx: usize,
    ) -> ProtocolResult<HashMap<String, String>> {
        let mut labels = HashMap::new();

        let mut i = start_idx;
        while i + 1 < args.len() {
            let key = args[i]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("Label key must be string".to_string()))?;
            let value = args[i + 1].as_string().ok_or_else(|| {
                ProtocolError::RespError("Label value must be string".to_string())
            })?;

            labels.insert(key, value);
            i += 2;
        }

        if i < args.len() {
            return Err(ProtocolError::RespError(
                "Odd number of label key-value pairs".to_string(),
            ));
        }

        Ok(labels)
    }

    /// Format sample for response
    fn format_sample(&self, sample: &Sample) -> Vec<RespValue> {
        vec![
            RespValue::integer(sample.timestamp as i64),
            RespValue::bulk_string_from_str(&sample.value.to_string()),
        ]
    }

    /// Parse time series configuration from command arguments
    fn parse_ts_config(
        &self,
        args: &[RespValue],
        start_idx: usize,
    ) -> ProtocolResult<TimeSeriesConfig> {
        let mut config = TimeSeriesConfig::default();
        let mut i = start_idx;

        while i < args.len() {
            if let Some(param) = args[i].as_string() {
                match param.to_uppercase().as_str() {
                    "RETENTION" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError(
                                "RETENTION requires a value".to_string(),
                            ));
                        }
                        let retention_str = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError("RETENTION value must be string".to_string())
                        })?;
                        let retention = retention_str.parse::<u64>().map_err(|_| {
                            ProtocolError::RespError("Invalid retention value".to_string())
                        })?;
                        config.retention = Some(retention);
                        i += 2;
                    }
                    "CHUNK_SIZE" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError(
                                "CHUNK_SIZE requires a value".to_string(),
                            ));
                        }
                        let chunk_size_str = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError("CHUNK_SIZE value must be string".to_string())
                        })?;
                        let chunk_size = chunk_size_str.parse::<usize>().map_err(|_| {
                            ProtocolError::RespError("Invalid chunk size value".to_string())
                        })?;
                        config.chunk_size = Some(chunk_size);
                        i += 2;
                    }
                    "DUPLICATE_POLICY" => {
                        if i + 1 >= args.len() {
                            return Err(ProtocolError::RespError(
                                "DUPLICATE_POLICY requires a value".to_string(),
                            ));
                        }
                        let policy_str = args[i + 1].as_string().ok_or_else(|| {
                            ProtocolError::RespError(
                                "DUPLICATE_POLICY value must be string".to_string(),
                            )
                        })?;
                        config.duplicate_policy = self.parse_duplicate_policy(&policy_str)?;
                        i += 2;
                    }
                    "LABELS" => {
                        // Labels are parsed separately after other parameters
                        break;
                    }
                    _ => {
                        // Unknown parameter, skip
                        i += 1;
                    }
                }
            } else {
                i += 1;
            }
        }

        // Parse labels if LABELS keyword was found
        if let Some(labels_idx) = args.iter().position(|arg| {
            arg.as_string()
                .map(|s| s.to_uppercase() == "LABELS")
                .unwrap_or(false)
        }) {
            if labels_idx + 1 < args.len() {
                config.labels = self.parse_ts_labels(args, labels_idx + 1)?;
            }
        }

        Ok(config)
    }

    // TS.* command implementations

    async fn cmd_ts_create(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.CREATE' command. Usage: TS.CREATE <key> [RETENTION <retentionTime>] [CHUNK_SIZE <size>] [DUPLICATE_POLICY <policy>] [LABELS <label> <value> ...]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let config = self.parse_ts_config(args, 1)?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        actor_ref
            .invoke::<()>(
                "update_config",
                vec![serde_json::to_value(config).unwrap().into()],
            )
            .await
            .map_err(|e| {
                ProtocolError::RespError(format!("ERR failed to create time series: {}", e))
            })?;

        debug!("TS.CREATE {} -> OK", key);
        Ok(RespValue::ok())
    }

    async fn cmd_ts_alter(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.ALTER' command. Usage: TS.ALTER <key> [RETENTION <retentionTime>] [CHUNK_SIZE <size>] [DUPLICATE_POLICY <policy>] [LABELS <label> <value> ...]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let config = self.parse_ts_config(args, 1)?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        actor_ref
            .invoke::<()>(
                "update_config",
                vec![serde_json::to_value(config).unwrap().into()],
            )
            .await
            .map_err(|e| {
                ProtocolError::RespError(format!("ERR failed to alter time series: {}", e))
            })?;

        debug!("TS.ALTER {} -> OK", key);
        Ok(RespValue::ok())
    }

    async fn cmd_ts_add(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.ADD' command. Usage: TS.ADD <key> <timestamp> <value> [RETENTION <retentionTime>] [CHUNK_SIZE <size>] [DUPLICATE_POLICY <policy>] [LABELS <label> <value> ...]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let timestamp = self.parse_timestamp(&args[1])?;
        let value = self.parse_value(&args[2])?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // If additional configuration is provided, update the series config first
        if args.len() > 3 {
            let config = self.parse_ts_config(args, 3)?;
            let _: Result<(), _> = actor_ref
                .invoke(
                    "update_config",
                    vec![serde_json::to_value(config).unwrap().into()],
                )
                .await;
        }

        actor_ref
            .invoke::<()>("add_sample", vec![timestamp.into(), value.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to add sample: {}", e)))?;

        debug!("TS.ADD {} {} {} -> {}", key, timestamp, value, timestamp);
        Ok(RespValue::integer(timestamp as i64))
    }

    async fn cmd_ts_madd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() % 3 != 0 || args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.MADD' command. Usage: TS.MADD <key1> <timestamp1> <value1> [<key2> <timestamp2> <value2> ...]".to_string(),
            ));
        }

        let mut results = Vec::new();
        let mut i = 0;

        while i + 2 < args.len() {
            let key = args[i]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

            let timestamp = self.parse_timestamp(&args[i + 1])?;
            let value = self.parse_value(&args[i + 2])?;

            let actor_ref = self
                .orbit_client
                .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            let result = actor_ref
                .invoke::<()>("add_sample", vec![timestamp.into(), value.into()])
                .await;

            match result {
                Ok(_) => results.push(RespValue::integer(timestamp as i64)),
                Err(e) => results.push(RespValue::bulk_string_from_str(&format!("ERR {}", e))),
            }

            i += 3;
        }

        debug!("TS.MADD -> {} results", results.len());
        Ok(RespValue::array(results))
    }

    async fn cmd_ts_incrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.INCRBY' command. Usage: TS.INCRBY <key> <value> [TIMESTAMP <timestamp>] [RETENTION <retentionTime>] [CHUNK_SIZE <size>] [DUPLICATE_POLICY <policy>] [LABELS <label> <value> ...]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let increment = self.parse_value(&args[1])?;

        let mut timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Parse optional TIMESTAMP parameter
        if let Some(ts_idx) = args.iter().position(|arg| {
            arg.as_string()
                .map(|s| s.to_uppercase() == "TIMESTAMP")
                .unwrap_or(false)
        }) {
            if ts_idx + 1 < args.len() {
                timestamp = self.parse_timestamp(&args[ts_idx + 1])?;
            }
        }

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // If additional configuration is provided, update the series config first
        if args.len() > 2 {
            let config = self.parse_ts_config(args, 2)?;
            let _: Result<(), _> = actor_ref
                .invoke(
                    "update_config",
                    vec![serde_json::to_value(config).unwrap().into()],
                )
                .await;
        }

        let new_value: f64 = actor_ref
            .invoke("increment_by", vec![timestamp.into(), increment.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to increment: {}", e)))?;

        debug!("TS.INCRBY {} {} -> {}", key, increment, new_value);
        Ok(RespValue::integer(timestamp as i64))
    }

    async fn cmd_ts_decrby(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.DECRBY' command. Usage: TS.DECRBY <key> <value> [TIMESTAMP <timestamp>] [RETENTION <retentionTime>] [CHUNK_SIZE <size>] [DUPLICATE_POLICY <policy>] [LABELS <label> <value> ...]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let decrement = self.parse_value(&args[1])? * -1.0; // Negate for decrement

        let mut timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Parse optional TIMESTAMP parameter
        if let Some(ts_idx) = args.iter().position(|arg| {
            arg.as_string()
                .map(|s| s.to_uppercase() == "TIMESTAMP")
                .unwrap_or(false)
        }) {
            if ts_idx + 1 < args.len() {
                timestamp = self.parse_timestamp(&args[ts_idx + 1])?;
            }
        }

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // If additional configuration is provided, update the series config first
        if args.len() > 2 {
            let config = self.parse_ts_config(args, 2)?;
            let _: Result<(), _> = actor_ref
                .invoke(
                    "update_config",
                    vec![serde_json::to_value(config).unwrap().into()],
                )
                .await;
        }

        let new_value: f64 = actor_ref
            .invoke("increment_by", vec![timestamp.into(), decrement.into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to decrement: {}", e)))?;

        debug!("TS.DECRBY {} {} -> {}", key, -decrement, new_value);
        Ok(RespValue::integer(timestamp as i64))
    }

    async fn cmd_ts_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.DEL' command. Usage: TS.DEL <key> <fromTimestamp> <toTimestamp>".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let from_ts = self.parse_timestamp(&args[1])?;
        let to_ts = self.parse_timestamp(&args[2])?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let deleted_count: usize = actor_ref
            .invoke("delete_range", vec![from_ts.into(), to_ts.into()])
            .await
            .map_err(|e| {
                ProtocolError::RespError(format!("ERR failed to delete samples: {}", e))
            })?;

        debug!("TS.DEL {} {} {} -> {}", key, from_ts, to_ts, deleted_count);
        Ok(RespValue::integer(deleted_count as i64))
    }

    async fn cmd_ts_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.GET' command. Usage: TS.GET <key>"
                    .to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let sample: Option<Sample> = actor_ref
            .invoke("get_latest", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to get sample: {}", e)))?;

        match sample {
            Some(s) => {
                debug!("TS.GET {} -> [{}, {}]", key, s.timestamp, s.value);
                Ok(RespValue::array(self.format_sample(&s)))
            }
            None => {
                debug!("TS.GET {} -> null", key);
                Ok(RespValue::null())
            }
        }
    }

    async fn cmd_ts_mget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.MGET' command. Usage: TS.MGET [FILTER <label>=<value> ...] <key1> [<key2> ...]".to_string(),
            ));
        }

        // For simplicity, we'll treat all arguments as keys for now
        // In a full implementation, we'd parse FILTER parameters
        let mut results = Vec::new();

        for arg in args {
            if let Some(key) = arg.as_string() {
                let actor_ref = self
                    .orbit_client
                    .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
                    .await;

                if let Ok(actor_ref) = actor_ref {
                    let sample: Result<Option<Sample>, _> =
                        actor_ref.invoke("get_latest", vec![]).await;

                    match sample {
                        Ok(Some(s)) => {
                            let mut result = vec![RespValue::bulk_string_from_str(&key)];
                            result.extend(self.format_sample(&s));
                            results.push(RespValue::array(result));
                        }
                        _ => {
                            results.push(RespValue::array(vec![
                                RespValue::bulk_string_from_str(&key),
                                RespValue::null(),
                            ]));
                        }
                    }
                } else {
                    results.push(RespValue::array(vec![
                        RespValue::bulk_string_from_str(&key),
                        RespValue::null(),
                    ]));
                }
            }
        }

        debug!("TS.MGET -> {} results", results.len());
        Ok(RespValue::array(results))
    }

    async fn cmd_ts_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.INFO' command. Usage: TS.INFO <key>"
                    .to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let stats: TimeSeriesStats = actor_ref
            .invoke("get_stats", vec![])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to get stats: {}", e)))?;

        debug!("TS.INFO {} -> {} samples", key, stats.total_samples);

        let mut result = vec![
            RespValue::bulk_string_from_str("totalSamples"),
            RespValue::integer(stats.total_samples as i64),
            RespValue::bulk_string_from_str("memoryUsage"),
            RespValue::integer(stats.memory_usage as i64),
            RespValue::bulk_string_from_str("firstTimestamp"),
            stats
                .first_timestamp
                .map(|ts| RespValue::integer(ts as i64))
                .unwrap_or(RespValue::null()),
            RespValue::bulk_string_from_str("lastTimestamp"),
            stats
                .last_timestamp
                .map(|ts| RespValue::integer(ts as i64))
                .unwrap_or(RespValue::null()),
            RespValue::bulk_string_from_str("retentionTime"),
            stats
                .retention_time
                .map(|rt| RespValue::integer(rt as i64))
                .unwrap_or(RespValue::integer(0)),
            RespValue::bulk_string_from_str("chunkSize"),
            stats
                .chunk_size
                .map(|cs| RespValue::integer(cs as i64))
                .unwrap_or(RespValue::integer(4096)),
            RespValue::bulk_string_from_str("duplicatePolicy"),
            RespValue::bulk_string_from_str(stats.duplicate_policy.as_str()),
            RespValue::bulk_string_from_str("labels"),
        ];

        // Add labels
        let mut labels_array = Vec::new();
        for (key, value) in stats.labels {
            labels_array.push(RespValue::bulk_string_from_str(&key));
            labels_array.push(RespValue::bulk_string_from_str(&value));
        }
        result.push(RespValue::array(labels_array));

        Ok(RespValue::array(result))
    }

    async fn cmd_ts_range(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.RANGE' command. Usage: TS.RANGE <key> <fromTimestamp> <toTimestamp> [AGGREGATION <aggregation> <bucketDuration>]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let from_ts = self.parse_timestamp(&args[1])?;
        let to_ts = self.parse_timestamp(&args[2])?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Check for aggregation parameters
        let samples = if let Some(agg_idx) = args.iter().position(|arg| {
            arg.as_string()
                .map(|s| s.to_uppercase() == "AGGREGATION")
                .unwrap_or(false)
        }) {
            if agg_idx + 2 >= args.len() {
                return Err(ProtocolError::RespError(
                    "AGGREGATION requires aggregation function and bucket duration".to_string(),
                ));
            }

            let agg_func_str = args[agg_idx + 1].as_string().ok_or_else(|| {
                ProtocolError::RespError("Aggregation function must be string".to_string())
            })?;
            let bucket_duration_str = args[agg_idx + 2].as_string().ok_or_else(|| {
                ProtocolError::RespError("Bucket duration must be string".to_string())
            })?;

            let aggregation = self.parse_aggregation(&agg_func_str)?;
            let bucket_duration = bucket_duration_str
                .parse::<u64>()
                .map_err(|_| ProtocolError::RespError("Invalid bucket duration".to_string()))?;

            let samples: Vec<Sample> = actor_ref
                .invoke(
                    "get_range_aggregated",
                    vec![
                        from_ts.into(),
                        to_ts.into(),
                        bucket_duration.into(),
                        serde_json::to_value(aggregation).unwrap().into(),
                    ],
                )
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR failed to get aggregated range: {}", e))
                })?;
            samples
        } else {
            let samples: Vec<Sample> = actor_ref
                .invoke("get_range", vec![from_ts.into(), to_ts.into()])
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR failed to get range: {}", e)))?;
            samples
        };

        debug!(
            "TS.RANGE {} {} {} -> {} samples",
            key,
            from_ts,
            to_ts,
            samples.len()
        );

        let result: Vec<RespValue> = samples
            .iter()
            .map(|sample| RespValue::array(self.format_sample(sample)))
            .collect();

        Ok(RespValue::array(result))
    }

    async fn cmd_ts_revrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.REVRANGE' command. Usage: TS.REVRANGE <key> <fromTimestamp> <toTimestamp> [AGGREGATION <aggregation> <bucketDuration>]".to_string(),
            ));
        }

        let key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let from_ts = self.parse_timestamp(&args[1])?;
        let to_ts = self.parse_timestamp(&args[2])?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Check for aggregation parameters
        let samples = if let Some(agg_idx) = args.iter().position(|arg| {
            arg.as_string()
                .map(|s| s.to_uppercase() == "AGGREGATION")
                .unwrap_or(false)
        }) {
            if agg_idx + 2 >= args.len() {
                return Err(ProtocolError::RespError(
                    "AGGREGATION requires aggregation function and bucket duration".to_string(),
                ));
            }

            let agg_func_str = args[agg_idx + 1].as_string().ok_or_else(|| {
                ProtocolError::RespError("Aggregation function must be string".to_string())
            })?;
            let bucket_duration_str = args[agg_idx + 2].as_string().ok_or_else(|| {
                ProtocolError::RespError("Bucket duration must be string".to_string())
            })?;

            let aggregation = self.parse_aggregation(&agg_func_str)?;
            let bucket_duration = bucket_duration_str
                .parse::<u64>()
                .map_err(|_| ProtocolError::RespError("Invalid bucket duration".to_string()))?;

            // Get aggregated samples and then reverse them
            let mut samples: Vec<Sample> = actor_ref
                .invoke(
                    "get_range_aggregated",
                    vec![
                        from_ts.into(),
                        to_ts.into(),
                        bucket_duration.into(),
                        serde_json::to_value(aggregation).unwrap().into(),
                    ],
                )
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR failed to get aggregated range: {}", e))
                })?;
            samples.reverse();
            samples
        } else {
            let samples: Vec<Sample> = actor_ref
                .invoke("get_range_reverse", vec![from_ts.into(), to_ts.into()])
                .await
                .map_err(|e| {
                    ProtocolError::RespError(format!("ERR failed to get reverse range: {}", e))
                })?;
            samples
        };

        debug!(
            "TS.REVRANGE {} {} {} -> {} samples",
            key,
            from_ts,
            to_ts,
            samples.len()
        );

        let result: Vec<RespValue> = samples
            .iter()
            .map(|sample| RespValue::array(self.format_sample(sample)))
            .collect();

        Ok(RespValue::array(result))
    }

    async fn cmd_ts_mrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.MRANGE' command. Usage: TS.MRANGE <fromTimestamp> <toTimestamp> [AGGREGATION <aggregation> <bucketDuration>] [FILTER <label>=<value> ...] <key1> [<key2> ...]".to_string(),
            ));
        }

        let from_ts = self.parse_timestamp(&args[0])?;
        let to_ts = self.parse_timestamp(&args[1])?;

        // For simplicity, treat remaining args as keys
        // In full implementation, would parse FILTER and AGGREGATION parameters
        let mut results = Vec::new();

        for i in 2..args.len() {
            if let Some(key) = args[i].as_string() {
                let actor_ref = self
                    .orbit_client
                    .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
                    .await;

                if let Ok(actor_ref) = actor_ref {
                    let samples: Result<Vec<Sample>, _> = actor_ref
                        .invoke("get_range", vec![from_ts.into(), to_ts.into()])
                        .await;

                    match samples {
                        Ok(samples) => {
                            let sample_arrays: Vec<RespValue> = samples
                                .iter()
                                .map(|sample| RespValue::array(self.format_sample(sample)))
                                .collect();

                            results.push(RespValue::array(vec![
                                RespValue::bulk_string_from_str(&key),
                                RespValue::array(vec![]), // Empty labels array
                                RespValue::array(sample_arrays),
                            ]));
                        }
                        Err(_) => {
                            results.push(RespValue::array(vec![
                                RespValue::bulk_string_from_str(&key),
                                RespValue::array(vec![]),
                                RespValue::array(vec![]),
                            ]));
                        }
                    }
                }
            }
        }

        debug!(
            "TS.MRANGE {} {} -> {} series",
            from_ts,
            to_ts,
            results.len()
        );
        Ok(RespValue::array(results))
    }

    async fn cmd_ts_mrevrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.MREVRANGE' command. Usage: TS.MREVRANGE <fromTimestamp> <toTimestamp> [AGGREGATION <aggregation> <bucketDuration>] [FILTER <label>=<value> ...] <key1> [<key2> ...]".to_string(),
            ));
        }

        let from_ts = self.parse_timestamp(&args[0])?;
        let to_ts = self.parse_timestamp(&args[1])?;

        // For simplicity, treat remaining args as keys
        let mut results = Vec::new();

        for i in 2..args.len() {
            if let Some(key) = args[i].as_string() {
                let actor_ref = self
                    .orbit_client
                    .actor_reference::<TimeSeriesActor>(Key::StringKey { key: key.clone() })
                    .await;

                if let Ok(actor_ref) = actor_ref {
                    let samples: Result<Vec<Sample>, _> = actor_ref
                        .invoke("get_range_reverse", vec![from_ts.into(), to_ts.into()])
                        .await;

                    match samples {
                        Ok(samples) => {
                            let sample_arrays: Vec<RespValue> = samples
                                .iter()
                                .map(|sample| RespValue::array(self.format_sample(sample)))
                                .collect();

                            results.push(RespValue::array(vec![
                                RespValue::bulk_string_from_str(&key),
                                RespValue::array(vec![]), // Empty labels array
                                RespValue::array(sample_arrays),
                            ]));
                        }
                        Err(_) => {
                            results.push(RespValue::array(vec![
                                RespValue::bulk_string_from_str(&key),
                                RespValue::array(vec![]),
                                RespValue::array(vec![]),
                            ]));
                        }
                    }
                }
            }
        }

        debug!(
            "TS.MREVRANGE {} {} -> {} series",
            from_ts,
            to_ts,
            results.len()
        );
        Ok(RespValue::array(results))
    }

    async fn cmd_ts_queryindex(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // For now, return empty list as we don't have a global index
        // In full implementation, would search across all time series actors
        debug!("TS.QUERYINDEX (placeholder implementation)");
        Ok(RespValue::array(vec![]))
    }

    async fn cmd_ts_createrule(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 4 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.CREATERULE' command. Usage: TS.CREATERULE <sourceKey> <destKey> AGGREGATION <aggregation> <bucketDuration> [<retention>]".to_string(),
            ));
        }

        let source_key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid source key".to_string()))?;
        let dest_key = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid destination key".to_string()))?;

        // Find AGGREGATION parameter
        let agg_idx = args
            .iter()
            .position(|arg| {
                arg.as_string()
                    .map(|s| s.to_uppercase() == "AGGREGATION")
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                ProtocolError::RespError("AGGREGATION parameter is required".to_string())
            })?;

        if agg_idx + 2 >= args.len() {
            return Err(ProtocolError::RespError(
                "AGGREGATION requires aggregation function and bucket duration".to_string(),
            ));
        }

        let agg_func_str = args[agg_idx + 1].as_string().ok_or_else(|| {
            ProtocolError::RespError("Aggregation function must be string".to_string())
        })?;
        let bucket_duration_str = args[agg_idx + 2].as_string().ok_or_else(|| {
            ProtocolError::RespError("Bucket duration must be string".to_string())
        })?;

        let aggregation = self.parse_aggregation(&agg_func_str)?;
        let bucket_duration = bucket_duration_str
            .parse::<u64>()
            .map_err(|_| ProtocolError::RespError("Invalid bucket duration".to_string()))?;

        // Parse optional retention
        let retention = if agg_idx + 3 < args.len() {
            let retention_str = args[agg_idx + 3]
                .as_string()
                .ok_or_else(|| ProtocolError::RespError("Retention must be string".to_string()))?;
            Some(
                retention_str
                    .parse::<u64>()
                    .map_err(|_| ProtocolError::RespError("Invalid retention value".to_string()))?,
            )
        } else {
            None
        };

        let rule = CompactionRule {
            dest_key: dest_key.clone(),
            bucket_duration,
            aggregation,
            retention,
        };

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey {
                key: source_key.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        actor_ref
            .invoke::<()>(
                "create_compaction_rule",
                vec![serde_json::to_value(rule).unwrap().into()],
            )
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to create rule: {}", e)))?;

        debug!(
            "TS.CREATERULE {} {} {} {} -> OK",
            source_key, dest_key, agg_func_str, bucket_duration
        );
        Ok(RespValue::ok())
    }

    async fn cmd_ts_deleterule(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'TS.DELETERULE' command. Usage: TS.DELETERULE <sourceKey> <destKey>".to_string(),
            ));
        }

        let source_key = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid source key".to_string()))?;
        let dest_key = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid destination key".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<TimeSeriesActor>(Key::StringKey {
                key: source_key.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let deleted: bool = actor_ref
            .invoke("delete_compaction_rule", vec![dest_key.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR failed to delete rule: {}", e)))?;

        debug!(
            "TS.DELETERULE {} {} -> {}",
            source_key,
            dest_key,
            if deleted { "OK" } else { "not found" }
        );
        Ok(RespValue::ok())
    }

    // GRAPH.* command implementations

    async fn cmd_graph_query(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.QUERY' command. Usage: GRAPH.QUERY <graph_name> <query>".to_string(),
            ));
        }

        let graph_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid graph name".to_string()))?;

        let query = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid query".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<GraphActor>(Key::StringKey {
                key: graph_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let result: crate::cypher::graph_engine::QueryResult = actor_ref
            .invoke("execute_query", vec![query.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR query execution failed: {}", e)))?;

        debug!(
            "GRAPH.QUERY {} -> {} nodes, {} relationships",
            graph_name,
            result.nodes.len(),
            result.relationships.len()
        );

        // Format result as nested array: [header, [rows...]]
        let mut response = Vec::new();

        // Add header
        let header: Vec<RespValue> = result
            .columns
            .into_iter()
            .map(|col| RespValue::bulk_string_from_str(&col))
            .collect();
        response.push(RespValue::array(header));

        // Add result rows
        let mut rows = Vec::new();
        for node in result.nodes {
            let mut row = Vec::new();
            row.push(RespValue::bulk_string_from_str(&format!(
                "{}:{:?}",
                node.id, node.labels
            )));
            rows.push(RespValue::array(row));
        }
        for relationship in result.relationships {
            let mut row = Vec::new();
            row.push(RespValue::bulk_string_from_str(&format!(
                "{}->{}:{}",
                relationship.start_node, relationship.end_node, relationship.rel_type
            )));
            rows.push(RespValue::array(row));
        }

        response.push(RespValue::array(rows));

        // Add statistics (simplified)
        let stats = vec![
            RespValue::bulk_string_from_str("Cached execution"),
            RespValue::bulk_string_from_str(&format!(
                "Query internal execution time: 0.000000 milliseconds"
            )),
        ];
        response.push(RespValue::array(stats));

        Ok(RespValue::array(response))
    }

    async fn cmd_graph_ro_query(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.RO_QUERY' command. Usage: GRAPH.RO_QUERY <graph_name> <query>".to_string(),
            ));
        }

        let graph_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid graph name".to_string()))?;

        let query = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid query".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<GraphActor>(Key::StringKey {
                key: graph_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let result: crate::cypher::graph_engine::QueryResult = actor_ref
            .invoke("execute_read_only_query", vec![query.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR query execution failed: {}", e)))?;

        debug!(
            "GRAPH.RO_QUERY {} -> {} nodes, {} relationships",
            graph_name,
            result.nodes.len(),
            result.relationships.len()
        );

        // Format result similar to GRAPH.QUERY
        let mut response = Vec::new();

        let header: Vec<RespValue> = result
            .columns
            .into_iter()
            .map(|col| RespValue::bulk_string_from_str(&col))
            .collect();
        response.push(RespValue::array(header));

        let mut rows = Vec::new();
        for node in result.nodes {
            let mut row = Vec::new();
            row.push(RespValue::bulk_string_from_str(&format!(
                "{}:{:?}",
                node.id, node.labels
            )));
            rows.push(RespValue::array(row));
        }
        for relationship in result.relationships {
            let mut row = Vec::new();
            row.push(RespValue::bulk_string_from_str(&format!(
                "{}->{}:{}",
                relationship.start_node, relationship.end_node, relationship.rel_type
            )));
            rows.push(RespValue::array(row));
        }

        response.push(RespValue::array(rows));

        let stats = vec![
            RespValue::bulk_string_from_str("Cached execution"),
            RespValue::bulk_string_from_str(&format!(
                "Query internal execution time: 0.000000 milliseconds"
            )),
        ];
        response.push(RespValue::array(stats));

        Ok(RespValue::array(response))
    }

    async fn cmd_graph_delete(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.DELETE' command. Usage: GRAPH.DELETE <graph_name>".to_string(),
            ));
        }

        let graph_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid graph name".to_string()))?;

        // In a full implementation, we'd have a global graph manager
        // For now, we'll simulate deletion by trying to access the actor
        let actor_ref = self
            .orbit_client
            .actor_reference::<GraphActor>(Key::StringKey {
                key: graph_name.clone(),
            })
            .await;

        match actor_ref {
            Ok(_) => {
                // In production, this would actually delete the actor and its data
                debug!("GRAPH.DELETE {} -> OK (simulated)", graph_name);
                Ok(RespValue::ok())
            }
            Err(_) => Err(ProtocolError::RespError(format!(
                "ERR graph '{}' not found",
                graph_name
            ))),
        }
    }

    async fn cmd_graph_list(&self, _args: &[RespValue]) -> ProtocolResult<RespValue> {
        // In a full implementation, this would query a global graph registry
        // For now, we'll return an empty list or simulate some graphs

        debug!("GRAPH.LIST -> returning simulated graph list");

        // Return a simple list of graph names (simulated)
        let graphs = vec![
            RespValue::bulk_string_from_str("demo_graph"),
            RespValue::bulk_string_from_str("social_network"),
        ];

        Ok(RespValue::array(graphs))
    }

    async fn cmd_graph_explain(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.EXPLAIN' command. Usage: GRAPH.EXPLAIN <graph_name> <query>".to_string(),
            ));
        }

        let graph_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid graph name".to_string()))?;

        let query = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid query".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<GraphActor>(Key::StringKey {
                key: graph_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let plan: ExecutionPlan = actor_ref
            .invoke("explain_query", vec![query.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR explain failed: {}", e)))?;

        debug!(
            "GRAPH.EXPLAIN {} -> {} steps, cost {}",
            graph_name,
            plan.steps.len(),
            plan.estimated_cost
        );

        // Format execution plan as array of steps
        let mut steps = Vec::new();
        for (i, step) in plan.steps.iter().enumerate() {
            let step_info = vec![
                RespValue::bulk_string_from_str(&format!("{}: {}", i + 1, step.operation)),
                RespValue::bulk_string_from_str(&step.description),
                RespValue::bulk_string_from_str(&format!(
                    "Estimated rows: {}",
                    step.estimated_rows
                )),
                RespValue::bulk_string_from_str(&format!("Cost: {:.2}", step.estimated_cost)),
            ];
            steps.push(RespValue::array(step_info));
        }

        // Add summary
        let summary = vec![RespValue::bulk_string_from_str(&format!(
            "Total estimated cost: {:.2}",
            plan.estimated_cost
        ))];
        steps.push(RespValue::array(summary));

        Ok(RespValue::array(steps))
    }

    async fn cmd_graph_profile(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.PROFILE' command. Usage: GRAPH.PROFILE <graph_name> <query>".to_string(),
            ));
        }

        let graph_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid graph name".to_string()))?;

        let query = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid query".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<GraphActor>(Key::StringKey {
                key: graph_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let profile: QueryProfile = actor_ref
            .invoke("profile_query", vec![query.clone().into()])
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR profile failed: {}", e)))?;

        debug!(
            "GRAPH.PROFILE {} -> {} ms total",
            graph_name, profile.total_time_ms
        );

        // Format profile as nested arrays with execution plan and metrics
        let mut response = Vec::new();

        // Query result (similar to GRAPH.QUERY)
        response.push(RespValue::array(vec![]));
        response.push(RespValue::array(vec![]));

        // Execution plan with actual metrics
        let mut plan_with_metrics = Vec::new();
        for (i, step) in profile.plan.steps.iter().enumerate() {
            let actual_rows = profile.metrics.rows_processed.get(i).unwrap_or(&0);
            let step_time = profile.metrics.step_times_ms.get(i).unwrap_or(&0);
            let memory_used = profile.metrics.memory_used.get(i).unwrap_or(&0);

            let step_info = vec![
                RespValue::bulk_string_from_str(&format!("{}: {}", i + 1, step.operation)),
                RespValue::bulk_string_from_str(&step.description),
                RespValue::bulk_string_from_str(&format!("Records produced: {}", actual_rows)),
                RespValue::bulk_string_from_str(&format!("Execution time: {} ms", step_time)),
                RespValue::bulk_string_from_str(&format!("Memory used: {} bytes", memory_used)),
            ];
            plan_with_metrics.push(RespValue::array(step_info));
        }

        // Add overall statistics
        let stats = vec![
            RespValue::bulk_string_from_str(&format!(
                "Total execution time: {} ms",
                profile.total_time_ms
            )),
            RespValue::bulk_string_from_str(&format!("Cache hits: {}", profile.metrics.cache_hits)),
            RespValue::bulk_string_from_str(&format!(
                "Cache misses: {}",
                profile.metrics.cache_misses
            )),
        ];
        plan_with_metrics.push(RespValue::array(stats));

        response.push(RespValue::array(plan_with_metrics));

        Ok(RespValue::array(response))
    }

    async fn cmd_graph_slowlog(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.SLOWLOG' command. Usage: GRAPH.SLOWLOG <graph_name>".to_string(),
            ));
        }

        let graph_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid graph name".to_string()))?;

        let actor_ref = self
            .orbit_client
            .actor_reference::<GraphActor>(Key::StringKey {
                key: graph_name.clone(),
            })
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        let slow_queries: Vec<SlowQuery> = actor_ref
            .invoke("get_slow_queries", vec![])
            .await
            .map_err(|e| {
                ProtocolError::RespError(format!("ERR failed to get slow queries: {}", e))
            })?;

        debug!(
            "GRAPH.SLOWLOG {} -> {} slow queries",
            graph_name,
            slow_queries.len()
        );

        // Format slow queries as array of entries
        let mut entries = Vec::new();
        for (i, slow_query) in slow_queries.iter().enumerate() {
            let entry = vec![
                RespValue::integer(i as i64 + 1),         // Entry number
                RespValue::integer(slow_query.timestamp), // Timestamp
                RespValue::integer(slow_query.execution_time_ms as i64), // Duration in microseconds
                RespValue::bulk_string_from_str(&slow_query.query), // Query
            ];
            entries.push(RespValue::array(entry));
        }

        Ok(RespValue::array(entries))
    }

    async fn cmd_graph_config(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPH.CONFIG' command. Usage: GRAPH.CONFIG GET|SET <parameter> [value]".to_string(),
            ));
        }

        let operation = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid operation".to_string()))?;

        match operation.to_uppercase().as_str() {
            "GET" => {
                if args.len() != 2 {
                    return Err(ProtocolError::RespError(
                        "ERR wrong number of arguments for 'GRAPH.CONFIG GET'. Usage: GRAPH.CONFIG GET <parameter>".to_string(),
                    ));
                }

                let parameter = args[1].as_string().ok_or_else(|| {
                    ProtocolError::RespError("ERR invalid parameter name".to_string())
                })?;

                // For config operations, we could have a global config or per-graph config
                // For now, we'll return some default values
                match parameter.to_uppercase().as_str() {
                    "QUERY_TIMEOUT" => Ok(RespValue::bulk_string_from_str("30000")),
                    "MAX_NODES" => Ok(RespValue::bulk_string_from_str("1000000")),
                    "MAX_RELATIONSHIPS" => Ok(RespValue::bulk_string_from_str("10000000")),
                    "PROFILING_ENABLED" => Ok(RespValue::bulk_string_from_str("false")),
                    _ => Err(ProtocolError::RespError(format!(
                        "ERR unknown configuration parameter: {}",
                        parameter
                    ))),
                }
            }
            "SET" => {
                if args.len() != 3 {
                    return Err(ProtocolError::RespError(
                        "ERR wrong number of arguments for 'GRAPH.CONFIG SET'. Usage: GRAPH.CONFIG SET <parameter> <value>".to_string(),
                    ));
                }

                let parameter = args[1].as_string().ok_or_else(|| {
                    ProtocolError::RespError("ERR invalid parameter name".to_string())
                })?;

                let value = args[2].as_string().ok_or_else(|| {
                    ProtocolError::RespError("ERR invalid parameter value".to_string())
                })?;

                debug!("GRAPH.CONFIG SET {} = {}", parameter, value);

                // In a full implementation, this would actually update the configuration
                match parameter.to_uppercase().as_str() {
                    "QUERY_TIMEOUT" | "MAX_NODES" | "MAX_RELATIONSHIPS" | "PROFILING_ENABLED" => {
                        Ok(RespValue::ok())
                    }
                    _ => Err(ProtocolError::RespError(format!(
                        "ERR unknown configuration parameter: {}",
                        parameter
                    ))),
                }
            }
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown config operation: {}",
                operation
            ))),
        }
    }
}
