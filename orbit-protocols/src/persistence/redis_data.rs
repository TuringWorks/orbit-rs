//! Redis data persistence provider for key-value storage with TTL support
//!
//! This module provides persistence for Redis-style key-value data with proper
//! TTL (Time-To-Live) support and background expiration cleanup.

use async_trait::async_trait;
use orbit_shared::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Redis value with optional expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisValue {
    pub data: String,
    pub expiration: Option<u64>, // Unix timestamp in seconds
}

impl RedisValue {
    pub fn new(data: String) -> Self {
        Self {
            data,
            expiration: None,
        }
    }

    pub fn with_ttl(data: String, ttl_seconds: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            data,
            expiration: Some(now + ttl_seconds),
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.expiration {
            None => false,
            Some(exp) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                now >= exp
            }
        }
    }

    pub fn ttl(&self) -> i64 {
        match self.expiration {
            None => -1, // No expiration
            Some(exp) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if exp > now {
                    (exp - now) as i64
                } else {
                    -2 // Expired
                }
            }
        }
    }

    pub fn set_ttl(&mut self, ttl_seconds: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.expiration = Some(now + ttl_seconds);
    }

    pub fn persist(&mut self) -> bool {
        let had_expiration = self.expiration.is_some();
        self.expiration = None;
        had_expiration
    }
}

/// Statistics for Redis data operations
#[derive(Debug, Clone, Default)]
pub struct RedisDataMetrics {
    pub get_operations: u64,
    pub set_operations: u64,
    pub delete_operations: u64,
    pub expired_keys_cleaned: u64,
    pub total_keys: usize,
    pub keys_with_ttl: usize,
}

/// Provider trait for Redis key-value data persistence
#[async_trait]
pub trait RedisDataProvider: Send + Sync {
    /// Initialize the provider
    async fn initialize(&self) -> OrbitResult<()>;

    /// Shutdown the provider gracefully
    async fn shutdown(&self) -> OrbitResult<()>;

    /// Get a value by key
    async fn get(&self, key: &str) -> OrbitResult<Option<RedisValue>>;

    /// Set a value with optional TTL
    async fn set(&self, key: &str, value: RedisValue) -> OrbitResult<()>;

    /// Delete a key
    async fn delete(&self, key: &str) -> OrbitResult<bool>;

    /// Check if a key exists (and is not expired)
    async fn exists(&self, key: &str) -> OrbitResult<bool>;

    /// Get multiple values at once
    async fn mget(&self, keys: &[String]) -> OrbitResult<Vec<Option<RedisValue>>>;

    /// Set multiple values at once
    async fn mset(&self, values: HashMap<String, RedisValue>) -> OrbitResult<()>;

    /// Get all keys matching a pattern (for debugging/admin)
    async fn keys(&self, pattern: &str) -> OrbitResult<Vec<String>>;

    /// Clean up expired keys
    async fn cleanup_expired(&self) -> OrbitResult<u64>;

    /// Get current metrics
    async fn metrics(&self) -> OrbitResult<RedisDataMetrics>;

    /// Increment a numeric value atomically
    async fn incr(&self, key: &str, delta: i64) -> OrbitResult<i64>;

    /// Append to a string value
    async fn append(&self, key: &str, value: &str) -> OrbitResult<usize>;

    /// Get the length of a string value
    async fn strlen(&self, key: &str) -> OrbitResult<usize>;

    /// Set a value only if it doesn't exist
    async fn setnx(&self, key: &str, value: RedisValue) -> OrbitResult<bool>;

    /// Get current value and set a new one atomically
    async fn getset(&self, key: &str, value: RedisValue) -> OrbitResult<Option<String>>;
}

/// Configuration for Redis data providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisDataConfig {
    /// Enable automatic cleanup of expired keys
    pub enable_expiry_cleanup: bool,
    /// Interval between expiry cleanup runs (in seconds)
    pub cleanup_interval_seconds: u64,
    /// Maximum number of keys to check per cleanup run
    pub cleanup_batch_size: usize,
    /// Prefix for Redis data keys in the storage backend
    pub key_prefix: String,
}

impl Default for RedisDataConfig {
    fn default() -> Self {
        Self {
            enable_expiry_cleanup: true,
            cleanup_interval_seconds: 60, // Clean up every minute
            cleanup_batch_size: 1000,     // Check up to 1000 keys per run
            key_prefix: "redis:".to_string(),
        }
    }
}

/// In-memory implementation for testing
pub struct MemoryRedisDataProvider {
    data: tokio::sync::RwLock<HashMap<String, RedisValue>>,
    config: RedisDataConfig,
    metrics: tokio::sync::RwLock<RedisDataMetrics>,
}

impl MemoryRedisDataProvider {
    pub fn new(config: RedisDataConfig) -> Self {
        Self {
            data: tokio::sync::RwLock::new(HashMap::new()),
            config,
            metrics: tokio::sync::RwLock::new(RedisDataMetrics::default()),
        }
    }
}

#[async_trait]
impl RedisDataProvider for MemoryRedisDataProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        tracing::info!("Memory Redis data provider initialized");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        tracing::info!("Memory Redis data provider shutdown");
        Ok(())
    }

    async fn get(&self, key: &str) -> OrbitResult<Option<RedisValue>> {
        let data = self.data.read().await;
        let mut metrics = self.metrics.write().await;
        metrics.get_operations += 1;

        if let Some(value) = data.get(key) {
            if value.is_expired() {
                Ok(None)
            } else {
                Ok(Some(value.clone()))
            }
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: &str, value: RedisValue) -> OrbitResult<()> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;
        metrics.set_operations += 1;

        data.insert(key.to_string(), value);
        metrics.total_keys = data.len();
        metrics.keys_with_ttl = data.values().filter(|v| v.expiration.is_some()).count();

        Ok(())
    }

    async fn delete(&self, key: &str) -> OrbitResult<bool> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;
        metrics.delete_operations += 1;

        let existed = data.remove(key).is_some();
        metrics.total_keys = data.len();
        metrics.keys_with_ttl = data.values().filter(|v| v.expiration.is_some()).count();

        Ok(existed)
    }

    async fn exists(&self, key: &str) -> OrbitResult<bool> {
        let data = self.data.read().await;
        if let Some(value) = data.get(key) {
            Ok(!value.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn mget(&self, keys: &[String]) -> OrbitResult<Vec<Option<RedisValue>>> {
        let data = self.data.read().await;
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            let value = data.get(key);
            match value {
                Some(v) if !v.is_expired() => results.push(Some(v.clone())),
                _ => results.push(None),
            }
        }

        Ok(results)
    }

    async fn mset(&self, values: HashMap<String, RedisValue>) -> OrbitResult<()> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;

        for (key, value) in values {
            data.insert(key, value);
        }

        metrics.total_keys = data.len();
        metrics.keys_with_ttl = data.values().filter(|v| v.expiration.is_some()).count();

        Ok(())
    }

    async fn keys(&self, _pattern: &str) -> OrbitResult<Vec<String>> {
        let data = self.data.read().await;
        // Simple implementation - return all keys (ignoring pattern for now)
        Ok(data.keys().cloned().collect())
    }

    async fn cleanup_expired(&self) -> OrbitResult<u64> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;

        let expired_keys: Vec<String> = data
            .iter()
            .filter(|(_, value)| value.is_expired())
            .map(|(key, _)| key.clone())
            .collect();

        let count = expired_keys.len() as u64;
        for key in expired_keys {
            data.remove(&key);
        }

        metrics.expired_keys_cleaned += count;
        metrics.total_keys = data.len();
        metrics.keys_with_ttl = data.values().filter(|v| v.expiration.is_some()).count();

        Ok(count)
    }

    async fn metrics(&self) -> OrbitResult<RedisDataMetrics> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    async fn incr(&self, key: &str, delta: i64) -> OrbitResult<i64> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;

        let current = match data.get(key) {
            Some(value) if !value.is_expired() => value
                .data
                .parse::<i64>()
                .map_err(|_| OrbitError::internal("ERR value is not an integer or out of range"))?,
            _ => 0,
        };

        let new_value = current + delta;
        let redis_value = RedisValue::new(new_value.to_string());
        data.insert(key.to_string(), redis_value);

        metrics.set_operations += 1;
        metrics.total_keys = data.len();

        Ok(new_value)
    }

    async fn append(&self, key: &str, value: &str) -> OrbitResult<usize> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;

        let current = match data.get(key) {
            Some(v) if !v.is_expired() => v.data.clone(),
            _ => String::new(),
        };

        let new_value = format!("{}{}", current, value);
        let length = new_value.len();
        let redis_value = RedisValue::new(new_value);
        data.insert(key.to_string(), redis_value);

        metrics.set_operations += 1;
        metrics.total_keys = data.len();

        Ok(length)
    }

    async fn strlen(&self, key: &str) -> OrbitResult<usize> {
        let data = self.data.read().await;
        match data.get(key) {
            Some(value) if !value.is_expired() => Ok(value.data.len()),
            _ => Ok(0),
        }
    }

    async fn setnx(&self, key: &str, value: RedisValue) -> OrbitResult<bool> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;

        let exists = match data.get(key) {
            Some(v) => !v.is_expired(),
            None => false,
        };

        if exists {
            Ok(false)
        } else {
            data.insert(key.to_string(), value);
            metrics.set_operations += 1;
            metrics.total_keys = data.len();
            Ok(true)
        }
    }

    async fn getset(&self, key: &str, value: RedisValue) -> OrbitResult<Option<String>> {
        let mut data = self.data.write().await;
        let mut metrics = self.metrics.write().await;

        let old_value = match data.get(key) {
            Some(v) if !v.is_expired() => Some(v.data.clone()),
            _ => None,
        };

        data.insert(key.to_string(), value);
        metrics.set_operations += 1;
        metrics.total_keys = data.len();

        Ok(old_value)
    }
}
