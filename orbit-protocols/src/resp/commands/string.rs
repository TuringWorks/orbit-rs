//! String command handlers for Redis RESP protocol
//!
//! This module implements Redis string commands like GET, SET, DEL, etc.

use super::traits::{BaseHandler, CommandHandler};
use crate::resp::types::RespType;
use async_trait::async_trait;
use orbit_shared::error_utils::{app_error, parse_error, ErrorContext, Result};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct StringHandler {
    base: BaseHandler,
}

impl StringHandler {
    pub fn new(base: BaseHandler) -> Self {
        Self { base }
    }

    /// Parse TTL value from arguments
    fn parse_ttl(args: &[RespType]) -> Result<Option<Duration>> {
        if args.len() >= 2 {
            match &args[1] {
                RespType::BulkString(ttl_str) => {
                    let ttl: u64 = ttl_str
                        .parse()
                        .map_err(|_| parse_error!("Invalid TTL value", ttl_str))?;
                    Ok(Some(Duration::from_secs(ttl)))
                }
                RespType::Integer(ttl) => {
                    if *ttl >= 0 {
                        Ok(Some(Duration::from_secs(*ttl as u64)))
                    } else {
                        Err(parse_error!("TTL cannot be negative"))
                    }
                }
                _ => Err(parse_error!("Invalid TTL type")),
            }
        } else {
            Ok(None)
        }
    }

    /// Parse SET command options (EX, PX, NX, XX)
    fn parse_set_options(args: &[RespType]) -> Result<(Option<Duration>, Option<String>)> {
        let mut ttl = None;
        let mut condition = None;

        let mut i = 2; // Start after key and value
        while i < args.len() {
            match &args[i] {
                RespType::BulkString(opt) => match opt.to_uppercase().as_str() {
                    "EX" => {
                        if i + 1 >= args.len() {
                            return Err(parse_error!("EX requires a value"));
                        }
                        ttl = Some(
                            Self::parse_ttl(&args[i + 1..])?
                                .ok_or_else(|| parse_error!("Invalid EX value"))?,
                        );
                        i += 2;
                    }
                    "PX" => {
                        if i + 1 >= args.len() {
                            return Err(parse_error!("PX requires a value"));
                        }
                        if let Some(ms_ttl) = Self::parse_ttl(&args[i + 1..])? {
                            ttl = Some(Duration::from_millis(ms_ttl.as_secs() * 1000));
                        }
                        i += 2;
                    }
                    "NX" => {
                        condition = Some("NX".to_string());
                        i += 1;
                    }
                    "XX" => {
                        condition = Some("XX".to_string());
                        i += 1;
                    }
                    _ => return Err(parse_error!("Unknown SET option", opt)),
                },
                _ => return Err(parse_error!("Invalid SET option type")),
            }
        }

        Ok((ttl, condition))
    }
}

#[async_trait]
impl CommandHandler for StringHandler {
    async fn handle_command(&self, command: &str, args: &[RespType]) -> Result<RespType> {
        match command.to_uppercase().as_str() {
            "GET" => self.handle_get(args).await,
            "SET" => self.handle_set(args).await,
            "DEL" => self.handle_del(args).await,
            "EXISTS" => self.handle_exists(args).await,
            "STRLEN" => self.handle_strlen(args).await,
            "GETSET" => self.handle_getset(args).await,
            "MGET" => self.handle_mget(args).await,
            "MSET" => self.handle_mset(args).await,
            "INCR" => self.handle_incr(args).await,
            "DECR" => self.handle_decr(args).await,
            "INCRBY" => self.handle_incrby(args).await,
            "DECRBY" => self.handle_decrby(args).await,
            "APPEND" => self.handle_append(args).await,
            "SETEX" => self.handle_setex(args).await,
            "SETNX" => self.handle_setnx(args).await,
            _ => Err(app_error(&format!("Unknown string command: {}", command))),
        }
    }

    fn validate_args(&self, command: &str, args: &[RespType]) -> Result<()> {
        let min_args = match command.to_uppercase().as_str() {
            "GET" | "DEL" | "EXISTS" | "STRLEN" | "INCR" | "DECR" => 1,
            "SET" | "GETSET" | "INCRBY" | "DECRBY" | "APPEND" | "SETNX" => 2,
            "SETEX" => 3,
            "MGET" | "MSET" => 1, // At least one key/value pair
            _ => 0,
        };

        if args.len() < min_args {
            return Err(parse_error!(&format!(
                "Wrong number of arguments for '{}' command",
                command
            )));
        }

        // Validate key types for commands that expect string keys
        for (i, arg) in args.iter().enumerate() {
            match command.to_uppercase().as_str() {
                "GET" | "SET" | "DEL" | "EXISTS" | "STRLEN" | "GETSET" | "INCR" | "DECR"
                | "INCRBY" | "DECRBY" | "APPEND" | "SETEX" | "SETNX"
                    if i == 0 =>
                {
                    if !matches!(arg, RespType::BulkString(_)) {
                        return Err(parse_error!("Key must be a string"));
                    }
                }
                "MGET" | "MSET" => {
                    if !matches!(arg, RespType::BulkString(_)) {
                        return Err(parse_error!("Keys must be strings"));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

// Individual command implementations
impl StringHandler {
    async fn handle_get(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("GET", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from GET command")?;

        match self.base.client.get(&key).await {
            Ok(Some(value)) => Ok(RespType::BulkString(value)),
            Ok(None) => Ok(RespType::Null),
            Err(e) => Err(app_error(&format!("GET operation failed: {}", e))),
        }
    }

    async fn handle_set(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("SET", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from SET command")?;
        let value = self
            .extract_string(&args[1])
            .context("Failed to extract value from SET command")?;

        let (ttl, condition) = Self::parse_set_options(args)?;

        // Handle conditional SET operations
        match condition.as_deref() {
            Some("NX") => {
                // SET IF NOT EXISTS
                let exists = self
                    .base
                    .client
                    .exists(&key)
                    .await
                    .map_err(|e| app_error(&format!("EXISTS check failed: {}", e)))?;
                if exists {
                    return Ok(RespType::Null);
                }
            }
            Some("XX") => {
                // SET IF EXISTS
                let exists = self
                    .base
                    .client
                    .exists(&key)
                    .await
                    .map_err(|e| app_error(&format!("EXISTS check failed: {}", e)))?;
                if !exists {
                    return Ok(RespType::Null);
                }
            }
            _ => {}
        }

        // Perform the SET operation
        match ttl {
            Some(duration) => {
                self.base
                    .client
                    .set_with_expiry(&key, &value, duration)
                    .await
                    .map_err(|e| app_error(&format!("SET with expiry failed: {}", e)))?;
            }
            None => {
                self.base
                    .client
                    .set(&key, &value)
                    .await
                    .map_err(|e| app_error(&format!("SET operation failed: {}", e)))?;
            }
        }

        Ok(RespType::SimpleString("OK".to_string()))
    }

    async fn handle_del(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("DEL", args)?;

        let mut deleted = 0i64;
        for arg in args {
            let key = self
                .extract_string(arg)
                .context("Failed to extract key from DEL command")?;

            match self.base.client.delete(&key).await {
                Ok(true) => deleted += 1,
                Ok(false) => {} // Key didn't exist
                Err(e) => return Err(app_error(&format!("DEL operation failed: {}", e))),
            }
        }

        Ok(RespType::Integer(deleted))
    }

    async fn handle_exists(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("EXISTS", args)?;

        let mut count = 0i64;
        for arg in args {
            let key = self
                .extract_string(arg)
                .context("Failed to extract key from EXISTS command")?;

            match self.base.client.exists(&key).await {
                Ok(true) => count += 1,
                Ok(false) => {}
                Err(e) => return Err(app_error(&format!("EXISTS operation failed: {}", e))),
            }
        }

        Ok(RespType::Integer(count))
    }

    async fn handle_strlen(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("STRLEN", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from STRLEN command")?;

        match self.base.client.get(&key).await {
            Ok(Some(value)) => Ok(RespType::Integer(value.len() as i64)),
            Ok(None) => Ok(RespType::Integer(0)),
            Err(e) => Err(app_error(&format!("STRLEN operation failed: {}", e))),
        }
    }

    async fn handle_getset(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("GETSET", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from GETSET command")?;
        let new_value = self
            .extract_string(&args[1])
            .context("Failed to extract value from GETSET command")?;

        // Get the old value first
        let old_value = match self.base.client.get(&key).await {
            Ok(value) => value,
            Err(e) => return Err(app_error(&format!("GETSET get operation failed: {}", e))),
        };

        // Set the new value
        self.base
            .client
            .set(&key, &new_value)
            .await
            .map_err(|e| app_error(&format!("GETSET set operation failed: {}", e)))?;

        match old_value {
            Some(value) => Ok(RespType::BulkString(value)),
            None => Ok(RespType::Null),
        }
    }

    async fn handle_mget(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("MGET", args)?;

        let mut results = Vec::new();

        for arg in args {
            let key = self
                .extract_string(arg)
                .context("Failed to extract key from MGET command")?;

            match self.base.client.get(&key).await {
                Ok(Some(value)) => results.push(RespType::BulkString(value)),
                Ok(None) => results.push(RespType::Null),
                Err(e) => {
                    return Err(app_error(&format!(
                        "MGET operation failed for key {}: {}",
                        key, e
                    )))
                }
            }
        }

        Ok(RespType::Array(results))
    }

    async fn handle_mset(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("MSET", args)?;

        if args.len() % 2 != 0 {
            return Err(parse_error!("Wrong number of arguments for MSET"));
        }

        for chunk in args.chunks(2) {
            let key = self
                .extract_string(&chunk[0])
                .context("Failed to extract key from MSET command")?;
            let value = self
                .extract_string(&chunk[1])
                .context("Failed to extract value from MSET command")?;

            self.base
                .client
                .set(&key, &value)
                .await
                .map_err(|e| app_error(&format!("MSET operation failed for key {}: {}", key, e)))?;
        }

        Ok(RespType::SimpleString("OK".to_string()))
    }

    async fn handle_incr(&self, args: &[RespType]) -> Result<RespType> {
        self.handle_incrby_impl(args, 1).await
    }

    async fn handle_decr(&self, args: &[RespType]) -> Result<RespType> {
        self.handle_incrby_impl(args, -1).await
    }

    async fn handle_incrby(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("INCRBY", args)?;

        let increment = self
            .extract_integer(&args[1])
            .context("Failed to extract increment from INCRBY command")?;

        self.handle_incrby_impl(&args[..1], increment).await
    }

    async fn handle_decrby(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("DECRBY", args)?;

        let decrement = self
            .extract_integer(&args[1])
            .context("Failed to extract decrement from DECRBY command")?;

        self.handle_incrby_impl(&args[..1], -decrement).await
    }

    async fn handle_incrby_impl(&self, args: &[RespType], delta: i64) -> Result<RespType> {
        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from increment command")?;

        let current_value = match self.base.client.get(&key).await {
            Ok(Some(value)) => value
                .parse::<i64>()
                .map_err(|_| parse_error!("Value is not an integer", &value))?,
            Ok(None) => 0,
            Err(e) => {
                return Err(app_error(&format!(
                    "Failed to get value for increment: {}",
                    e
                )))
            }
        };

        let new_value = current_value + delta;
        let new_value_str = new_value.to_string();

        self.base
            .client
            .set(&key, &new_value_str)
            .await
            .map_err(|e| app_error(&format!("Failed to set incremented value: {}", e)))?;

        Ok(RespType::Integer(new_value))
    }

    async fn handle_append(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("APPEND", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from APPEND command")?;
        let append_value = self
            .extract_string(&args[1])
            .context("Failed to extract value from APPEND command")?;

        let current_value = self
            .base
            .client
            .get(&key)
            .await
            .map_err(|e| app_error(&format!("APPEND get operation failed: {}", e)))?
            .unwrap_or_default();

        let new_value = format!("{}{}", current_value, append_value);
        let new_length = new_value.len() as i64;

        self.base
            .client
            .set(&key, &new_value)
            .await
            .map_err(|e| app_error(&format!("APPEND set operation failed: {}", e)))?;

        Ok(RespType::Integer(new_length))
    }

    async fn handle_setex(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("SETEX", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from SETEX command")?;
        let ttl_seconds = self
            .extract_integer(&args[1])
            .context("Failed to extract TTL from SETEX command")?;
        let value = self
            .extract_string(&args[2])
            .context("Failed to extract value from SETEX command")?;

        if ttl_seconds <= 0 {
            return Err(parse_error!("TTL must be positive"));
        }

        let ttl = Duration::from_secs(ttl_seconds as u64);

        self.base
            .client
            .set_with_expiry(&key, &value, ttl)
            .await
            .map_err(|e| app_error(&format!("SETEX operation failed: {}", e)))?;

        Ok(RespType::SimpleString("OK".to_string()))
    }

    async fn handle_setnx(&self, args: &[RespType]) -> Result<RespType> {
        self.validate_args("SETNX", args)?;

        let key = self
            .extract_string(&args[0])
            .context("Failed to extract key from SETNX command")?;
        let value = self
            .extract_string(&args[1])
            .context("Failed to extract value from SETNX command")?;

        let exists = self
            .base
            .client
            .exists(&key)
            .await
            .map_err(|e| app_error(&format!("SETNX exists check failed: {}", e)))?;

        if exists {
            Ok(RespType::Integer(0))
        } else {
            self.base
                .client
                .set(&key, &value)
                .await
                .map_err(|e| app_error(&format!("SETNX set operation failed: {}", e)))?;
            Ok(RespType::Integer(1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resp::types::RespType;

    #[test]
    fn test_parse_ttl() {
        let args = vec![
            RespType::BulkString("key".to_string()),
            RespType::BulkString("300".to_string()),
        ];

        let ttl = StringHandler::parse_ttl(&args[1..]).unwrap();
        assert_eq!(ttl, Some(Duration::from_secs(300)));

        let args = vec![
            RespType::BulkString("key".to_string()),
            RespType::Integer(600),
        ];

        let ttl = StringHandler::parse_ttl(&args[1..]).unwrap();
        assert_eq!(ttl, Some(Duration::from_secs(600)));
    }

    #[test]
    fn test_parse_set_options() {
        let args = vec![
            RespType::BulkString("key".to_string()),
            RespType::BulkString("value".to_string()),
            RespType::BulkString("EX".to_string()),
            RespType::BulkString("300".to_string()),
            RespType::BulkString("NX".to_string()),
        ];

        let (ttl, condition) = StringHandler::parse_set_options(&args).unwrap();
        assert_eq!(ttl, Some(Duration::from_secs(300)));
        assert_eq!(condition, Some("NX".to_string()));
    }
}
