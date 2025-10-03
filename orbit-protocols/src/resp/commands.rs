//! RESP protocol command handler
//! 
//! Implements Redis-compatible commands that operate on Orbit actors.
//! Each Redis data type (string, hash, list) maps to a corresponding Orbit actor type.

use std::sync::Arc;
use tracing::{debug, warn};

use orbit_client::OrbitClient;
use orbit_shared::Key;
use crate::error::{ProtocolError, ProtocolResult};
use super::{RespValue, actors::{KeyValueActor, HashActor, ListActor, PubSubActor}};

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
            _ => return Err(ProtocolError::RespError("Command must be an array".to_string())),
        };

        if args.is_empty() {
            return Err(ProtocolError::RespError("Empty command".to_string()));
        }

        let command_name = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("Command name must be a string".to_string()))?
            .to_uppercase();

        let args = &args[1..];

        debug!("Executing command: {} with {} args", command_name, args.len());

        match command_name.as_str() {
            // Connection commands
            "PING" => self.cmd_ping(args).await,
            "ECHO" => self.cmd_echo(args).await,
            "SELECT" => self.cmd_select(args).await,
            
            // String/Key commands
            "GET" => self.cmd_get(args).await,
            "SET" => self.cmd_set(args).await,
            "DEL" => self.cmd_del(args).await,
            "EXISTS" => self.cmd_exists(args).await,
            "TTL" => self.cmd_ttl(args).await,
            "EXPIRE" => self.cmd_expire(args).await,
            "KEYS" => self.cmd_keys(args).await,
            
            // Hash commands
            "HGET" => self.cmd_hget(args).await,
            "HSET" => self.cmd_hset(args).await,
            "HGETALL" => self.cmd_hgetall(args).await,
            "HDEL" => self.cmd_hdel(args).await,
            "HEXISTS" => self.cmd_hexists(args).await,
            "HKEYS" => self.cmd_hkeys(args).await,
            "HVALS" => self.cmd_hvals(args).await,
            "HLEN" => self.cmd_hlen(args).await,
            
            // List commands
            "LPUSH" => self.cmd_lpush(args).await,
            "RPUSH" => self.cmd_rpush(args).await,
            "LPOP" => self.cmd_lpop(args).await,
            "RPOP" => self.cmd_rpop(args).await,
            "LRANGE" => self.cmd_lrange(args).await,
            "LLEN" => self.cmd_llen(args).await,
            "LINDEX" => self.cmd_lindex(args).await,
            
            // Pub/Sub commands
            "PUBLISH" => self.cmd_publish(args).await,
            "SUBSCRIBE" => self.cmd_subscribe(args).await,
            "UNSUBSCRIBE" => self.cmd_unsubscribe(args).await,
            "PSUBSCRIBE" => self.cmd_psubscribe(args).await,
            "PUNSUBSCRIBE" => self.cmd_punsubscribe(args).await,
            
            // Server commands
            "INFO" => self.cmd_info(args).await,
            "DBSIZE" => self.cmd_dbsize(args).await,
            "FLUSHDB" => self.cmd_flushdb(args).await,
            "COMMAND" => self.cmd_command(args).await,
            
            _ => {
                warn!("Unknown command: {}", command_name);
                Err(ProtocolError::RespError(format!("ERR unknown command '{}'", command_name)))
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
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'echo' command".to_string()));
        }
        Ok(args[0].clone())
    }

    async fn cmd_select(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'select' command".to_string()));
        }
        // Redis database selection - we'll just accept it and return OK
        Ok(RespValue::ok())
    }

    // String/Key commands

    async fn cmd_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'get' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // Get KeyValueActor reference
        let actor_ref = self.orbit_client.actor_reference::<KeyValueActor>(
            Key::StringKey { key: key.clone() }
        ).await.map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke get_value method on the actor
        let value: Option<String> = actor_ref.invoke("get_value", vec![])
            .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;
        
        debug!("GET {} -> {:?}", key, value);
        Ok(value.map(|v| RespValue::bulk_string_from_str(v)).unwrap_or(RespValue::null()))
    }

    async fn cmd_set(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'set' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let value = args[1].as_string()
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
        let actor_ref = self.orbit_client.actor_reference::<KeyValueActor>(
            Key::StringKey { key: key.clone() }
        ).await.map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke set_value method on the actor
        actor_ref.invoke("set_value", vec![value.clone().into()])
            .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;

        // Set expiration if provided
        if let Some(seconds) = expiration_seconds {
            actor_ref.invoke("set_expiration", vec![seconds.into()])
                .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;
        }

        debug!("SET {} {} (expiration: {:?})", key, value, expiration_seconds);
        Ok(RespValue::ok())
    }

    async fn cmd_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'del' command".to_string()));
        }

        let mut deleted_count = 0i64;
        for arg in args {
            if let Some(key) = arg.as_string() {
                // TODO: Replace with actual OrbitClient deactivation
                debug!("DEL {} (placeholder implementation)", key);
                deleted_count += 1;
            }
        }

        Ok(RespValue::integer(deleted_count))
    }

    async fn cmd_exists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'exists' command".to_string()));
        }

        let mut exists_count = 0i64;
        for arg in args {
            if let Some(key) = arg.as_string() {
                // TODO: Replace with actual OrbitClient existence check
                debug!("EXISTS {} (placeholder implementation)", key);
                // For now, assume all keys exist
                exists_count += 1;
            }
        }

        Ok(RespValue::integer(exists_count))
    }

    async fn cmd_ttl(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'ttl' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // TODO: Replace with actual OrbitClient TTL check
        debug!("TTL {} (placeholder implementation)", key);
        Ok(RespValue::integer(-1)) // -1 means no expiration
    }

    async fn cmd_expire(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'expire' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let seconds = args[1].as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid timeout".to_string()))?;

        // TODO: Replace with actual OrbitClient expiration setting
        debug!("EXPIRE {} {} (placeholder implementation)", key, seconds);
        Ok(RespValue::integer(1)) // 1 means timeout was set
    }

    async fn cmd_keys(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'keys' command".to_string()));
        }

        let pattern = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid pattern".to_string()))?;

        // TODO: Replace with actual OrbitClient directory listing
        debug!("KEYS {} (placeholder implementation)", pattern);
        Ok(RespValue::array(vec![])) // Empty list for now
    }

    // Hash commands

    async fn cmd_hget(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hget' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let field = args[1].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;

        // Get HashActor reference
        let actor_ref = self.orbit_client.actor_reference::<HashActor>(
            Key::StringKey { key: key.clone() }
        ).await.map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke hget method on the actor
        let value: Option<String> = actor_ref.invoke("hget", vec![field.clone().into()])
            .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;
        
        debug!("HGET {} {} -> {:?}", key, field, value);
        Ok(value.map(|v| RespValue::bulk_string_from_str(v)).unwrap_or(RespValue::null()))
    }

    async fn cmd_hset(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() % 2 == 0 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hset' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut fields_set = 0i64;
        for i in (1..args.len()).step_by(2) {
            if i + 1 >= args.len() {
                break;
            }
            
            let field = args[i].as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;
            let value = args[i + 1].as_string()
                .ok_or_else(|| ProtocolError::RespError("ERR invalid value".to_string()))?;

            // Get HashActor reference
            let actor_ref = self.orbit_client.actor_reference::<HashActor>(
                Key::StringKey { key: key.clone() }
            ).await.map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

            // Invoke hset method on the actor
            let was_new: bool = actor_ref.invoke("hset", vec![field.clone().into(), value.clone().into()])
                .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;
            
            debug!("HSET {} {} {} -> new: {}", key, field, value, was_new);
            if was_new {
                fields_set += 1;
            }
        }

        Ok(RespValue::integer(fields_set))
    }

    async fn cmd_hgetall(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hgetall' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // TODO: Replace with actual OrbitClient hash actor invocation
        debug!("HGETALL {} (placeholder implementation)", key);
        Ok(RespValue::array(vec![])) // Empty array for now
    }

    async fn cmd_hdel(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hdel' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut deleted_count = 0i64;
        for arg in &args[1..] {
            if let Some(field) = arg.as_string() {
                // TODO: Replace with actual OrbitClient hash field deletion
                debug!("HDEL {} {} (placeholder implementation)", key, field);
                deleted_count += 1;
            }
        }

        Ok(RespValue::integer(deleted_count))
    }

    async fn cmd_hexists(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hexists' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let field = args[1].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid field".to_string()))?;

        // TODO: Replace with actual OrbitClient hash field existence check
        debug!("HEXISTS {} {} (placeholder implementation)", key, field);
        Ok(RespValue::integer(0)) // Field doesn't exist for now
    }

    async fn cmd_hkeys(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hkeys' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // TODO: Replace with actual OrbitClient hash keys retrieval
        debug!("HKEYS {} (placeholder implementation)", key);
        Ok(RespValue::array(vec![])) // Empty array for now
    }

    async fn cmd_hvals(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hvals' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // TODO: Replace with actual OrbitClient hash values retrieval
        debug!("HVALS {} (placeholder implementation)", key);
        Ok(RespValue::array(vec![])) // Empty array for now
    }

    async fn cmd_hlen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'hlen' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // TODO: Replace with actual OrbitClient hash length retrieval
        debug!("HLEN {} (placeholder implementation)", key);
        Ok(RespValue::integer(0)) // Empty hash for now
    }

    // List commands

    async fn cmd_lpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'lpush' command".to_string()));
        }

        let key = args[0].as_string()
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
        let actor_ref = self.orbit_client.actor_reference::<ListActor>(
            Key::StringKey { key: key.clone() }
        ).await.map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke lpush method on the actor
        let new_length: i64 = actor_ref.invoke("lpush", vec![values.clone().into()])
            .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;
        
        debug!("LPUSH {} {:?} -> length: {}", key, values, new_length);
        Ok(RespValue::integer(new_length))
    }

    async fn cmd_rpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'rpush' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let mut values = Vec::new();
        for arg in &args[1..] {
            if let Some(value) = arg.as_string() {
                values.push(value);
            } else {
                return Err(ProtocolError::RespError("ERR invalid value".to_string()));
            }
        }

        // TODO: Replace with actual OrbitClient list actor invocation
        debug!("RPUSH {} {:?} (placeholder implementation)", key, values);
        Ok(RespValue::integer(values.len() as i64))
    }

    async fn cmd_lpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'lpop' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let count = if args.len() == 2 {
            args[1].as_integer().unwrap_or(1)
        } else {
            1
        };

        // TODO: Replace with actual OrbitClient list actor invocation
        debug!("LPOP {} {} (placeholder implementation)", key, count);
        Ok(RespValue::null()) // No elements to pop for now
    }

    async fn cmd_rpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() || args.len() > 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'rpop' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        let count = if args.len() == 2 {
            args[1].as_integer().unwrap_or(1)
        } else {
            1
        };

        // TODO: Replace with actual OrbitClient list actor invocation
        debug!("RPOP {} {} (placeholder implementation)", key, count);
        Ok(RespValue::null()) // No elements to pop for now
    }

    async fn cmd_lrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 3 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'lrange' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let start = args[1].as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid start index".to_string()))?;
        let stop = args[2].as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid stop index".to_string()))?;

        // TODO: Replace with actual OrbitClient list actor invocation
        debug!("LRANGE {} {} {} (placeholder implementation)", key, start, stop);
        Ok(RespValue::array(vec![])) // Empty list for now
    }

    async fn cmd_llen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 1 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'llen' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;

        // TODO: Replace with actual OrbitClient list actor invocation
        debug!("LLEN {} (placeholder implementation)", key);
        Ok(RespValue::integer(0)) // Empty list for now
    }

    async fn cmd_lindex(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'lindex' command".to_string()));
        }

        let key = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid key".to_string()))?;
        let index = args[1].as_integer()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid index".to_string()))?;

        // TODO: Replace with actual OrbitClient list actor invocation
        debug!("LINDEX {} {} (placeholder implementation)", key, index);
        Ok(RespValue::null()) // No element at index for now
    }

    // Pub/Sub commands

    async fn cmd_publish(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() != 2 {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'publish' command".to_string()));
        }

        let channel = args[0].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid channel".to_string()))?;
        let message = args[1].as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid message".to_string()))?;

        // Get PubSubActor reference
        let actor_ref = self.orbit_client.actor_reference::<PubSubActor>(
            Key::StringKey { key: channel.clone() }
        ).await.map_err(|e| ProtocolError::RespError(format!("ERR actor error: {}", e)))?;

        // Invoke publish method on the actor
        let subscriber_count: i64 = actor_ref.invoke("publish", vec![message.clone().into()])
            .await.map_err(|e| ProtocolError::RespError(format!("ERR actor invocation failed: {}", e)))?;
        
        debug!("PUBLISH {} {} -> subscribers: {}", channel, message, subscriber_count);
        Ok(RespValue::integer(subscriber_count))
    }

    async fn cmd_subscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'subscribe' command".to_string()));
        }

        // TODO: Implement subscription logic with pub/sub actor
        // This requires connection state management
        let channels: Vec<_> = args.iter()
            .filter_map(|arg| arg.as_string())
            .collect();

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
            return Err(ProtocolError::RespError("ERR wrong number of arguments for 'psubscribe' command".to_string()));
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
            vec![RespValue::bulk_string_from_str("ping"), RespValue::integer(-1), RespValue::integer(1), RespValue::integer(0), RespValue::integer(0)],
            vec![RespValue::bulk_string_from_str("echo"), RespValue::integer(2), RespValue::integer(1), RespValue::integer(1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("select"), RespValue::integer(2), RespValue::integer(1), RespValue::integer(1), RespValue::integer(1)],
            // String
            vec![RespValue::bulk_string_from_str("get"), RespValue::integer(2), RespValue::integer(1), RespValue::integer(1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("set"), RespValue::integer(-3), RespValue::integer(1), RespValue::integer(1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("del"), RespValue::integer(-2), RespValue::integer(1), RespValue::integer(-1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("exists"), RespValue::integer(-2), RespValue::integer(1), RespValue::integer(-1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("ttl"), RespValue::integer(2), RespValue::integer(1), RespValue::integer(1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("expire"), RespValue::integer(3), RespValue::integer(1), RespValue::integer(1), RespValue::integer(1)],
            vec![RespValue::bulk_string_from_str("keys"), RespValue::integer(2), RespValue::integer(0), RespValue::integer(0), RespValue::integer(0)],
            // Server
            vec![RespValue::bulk_string_from_str("info"), RespValue::integer(-1), RespValue::integer(0), RespValue::integer(0), RespValue::integer(0)],
            vec![RespValue::bulk_string_from_str("dbsize"), RespValue::integer(1), RespValue::integer(0), RespValue::integer(0), RespValue::integer(0)],
            vec![RespValue::bulk_string_from_str("command"), RespValue::integer(-1), RespValue::integer(0), RespValue::integer(0), RespValue::integer(0)],
        ];

        Ok(RespValue::array(commands.into_iter().map(RespValue::array).collect()))
    }
}