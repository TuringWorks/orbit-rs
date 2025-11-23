//! TiKV-based Redis data persistence provider implementation
//!
//! This implementation uses TiKV for distributed persistent storage of Redis key-value data
//! with proper TTL handling, ACID transactions, and horizontal scalability.

use crate::protocols::persistence::redis_data::*;
use async_trait::async_trait;
use orbit_shared::{OrbitError, OrbitResult};
use serde_json;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tikv_client::{Key, TransactionClient};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

/// TiKV-based Redis data provider
pub struct TiKVRedisDataProvider {
    client: TransactionClient,
    config: RedisDataConfig,
    key_prefix: String,
    metrics: tokio::sync::RwLock<RedisDataMetrics>,
    cleanup_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl TiKVRedisDataProvider {
    /// Create a new TiKV Redis data provider
    pub async fn new(pd_endpoints: Vec<String>, config: RedisDataConfig) -> OrbitResult<Self> {
        info!(
            "Connecting to TiKV cluster at endpoints: {:?}",
            pd_endpoints
        );

        // Connect to TiKV cluster
        let client = TransactionClient::new(pd_endpoints)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to connect to TiKV: {}", e)))?;

        let key_prefix = config.key_prefix.clone();

        Ok(Self {
            client,
            config,
            key_prefix,
            metrics: tokio::sync::RwLock::new(RedisDataMetrics::default()),
            cleanup_task: tokio::sync::RwLock::new(None),
        })
    }

    /// Format a data key with prefix
    fn format_data_key(&self, key: &str) -> String {
        format!("{}data:{}", self.key_prefix, key)
    }

    /// Format an expiration key with prefix (timestamp:key for sorted iteration)
    fn format_expiration_key(&self, timestamp: u64, key: &str) -> String {
        format!("{}exp:{}:{}", self.key_prefix, timestamp, key)
    }

    /// Parse expiration key to get timestamp and original key
    #[allow(dead_code)] // Utility method for future expiration key parsing
    fn parse_expiration_key(&self, exp_key: &str) -> Option<(u64, String)> {
        let prefix = format!("{}exp:", self.key_prefix);
        if !exp_key.starts_with(&prefix) {
            return None;
        }

        let suffix = &exp_key[prefix.len()..];
        let parts: Vec<&str> = suffix.splitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(timestamp) = parts[0].parse::<u64>() {
                return Some((timestamp, parts[1].to_string()));
            }
        }
        None
    }

    /// Start the background cleanup task
    async fn start_cleanup_task(&self) {
        if !self.config.enable_expiry_cleanup {
            return;
        }

        let client = self.client.clone();
        let config = self.config.clone();
        let key_prefix = self.key_prefix.clone();

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.cleanup_interval_seconds));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                if let Err(e) = Self::perform_cleanup_sync(&client, &config, &key_prefix).await {
                    error!("TiKV Redis cleanup task error: {}", e);
                }
            }
        });

        *self.cleanup_task.write().await = Some(handle);
    }

    /// Perform cleanup of expired keys using TiKV scan and transaction
    async fn perform_cleanup_sync(
        _client: &TransactionClient,
        _config: &RedisDataConfig,
        _key_prefix: &str,
    ) -> OrbitResult<u64> {
        let _now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let _exp_prefix = format!("{}exp:", _key_prefix);
        let data_prefix = format!("{}data:", _key_prefix);

        // For now, we use a simplified approach without scan (TiKV scan API is complex)
        // In a production implementation, you would implement proper scanning
        // This is a placeholder that works for basic functionality
        let expired_keys: Vec<(Vec<u8>, String)> = Vec::new();

        // Note: Proper implementation would:
        // 1. Scan the expiration key range
        // 2. Check timestamps against current time
        // 3. Collect expired keys
        // For now, we return 0 as no scanning is implemented

        // Delete expired keys in a new transaction
        if !expired_keys.is_empty() {
            let mut delete_txn = _client.begin_optimistic().await.map_err(|e| {
                OrbitError::internal(format!("Failed to begin delete transaction: {}", e))
            })?;

            for (exp_key, data_key) in &expired_keys {
                // Delete the expiration key
                delete_txn
                    .delete(Key::from(exp_key.clone()))
                    .await
                    .map_err(|e| {
                        OrbitError::internal(format!("Failed to delete expiration key: {}", e))
                    })?;

                // Delete the data key
                let full_data_key = format!("{}{}", data_prefix, data_key);
                delete_txn
                    .delete(Key::from(full_data_key.as_bytes().to_vec()))
                    .await
                    .map_err(|e| {
                        OrbitError::internal(format!("Failed to delete data key: {}", e))
                    })?;
            }

            delete_txn.commit().await.map_err(|e| {
                OrbitError::internal(format!("Failed to commit delete transaction: {}", e))
            })?;

            debug!("Cleaned up {} expired keys from TiKV", expired_keys.len());
        }

        Ok(expired_keys.len() as u64)
    }

    /// Static helper for parsing expiration keys (for use in async context)
    #[allow(dead_code)] // Static utility method for future use
    fn parse_expiration_key_static(key_prefix: &str, exp_key: &str) -> Option<(u64, String)> {
        let prefix = format!("{}exp:", key_prefix);
        if !exp_key.starts_with(&prefix) {
            return None;
        }

        let suffix = &exp_key[prefix.len()..];
        let parts: Vec<&str> = suffix.splitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(timestamp) = parts[0].parse::<u64>() {
                return Some((timestamp, parts[1].to_string()));
            }
        }
        None
    }
}

#[async_trait]
impl RedisDataProvider for TiKVRedisDataProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        info!("Initializing TiKV Redis data provider");

        // Test the connection with a simple operation
        let test_key = format!("{}__init_test", self.key_prefix);
        let mut txn =
            self.client.begin_optimistic().await.map_err(|e| {
                OrbitError::internal(format!("Failed to test TiKV connection: {}", e))
            })?;

        // Write and read a test value
        txn.put(test_key.as_bytes().to_vec(), b"test".to_vec())
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to write test key: {}", e)))?;

        let _test_result = txn
            .get(test_key.as_bytes().to_vec())
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to read test key: {}", e)))?;

        txn.delete(test_key.as_bytes().to_vec())
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to delete test key: {}", e)))?;

        txn.commit().await.map_err(|e| {
            OrbitError::internal(format!("Failed to commit test transaction: {}", e))
        })?;

        // Start background cleanup task
        self.start_cleanup_task().await;

        info!("TiKV Redis data provider initialized successfully");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        info!("Shutting down TiKV Redis data provider");

        // Stop cleanup task
        if let Some(handle) = self.cleanup_task.write().await.take() {
            handle.abort();
        }

        info!("TiKV Redis data provider shutdown completed");
        Ok(())
    }

    async fn get(&self, key: &str) -> OrbitResult<Option<RedisValue>> {
        let data_key = self.format_data_key(key);

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.get_operations += 1;
        }

        let mut txn = self
            .client
            .begin_optimistic()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to begin transaction: {}", e)))?;

        let result = match txn.get(data_key.as_bytes().to_vec()).await {
            Ok(Some(data)) => {
                let value: RedisValue = serde_json::from_slice(&data).map_err(|e| {
                    OrbitError::internal(format!("Failed to deserialize value: {}", e))
                })?;

                if value.is_expired() {
                    // Key is expired, return None but don't delete here to avoid transaction conflicts
                    Ok(None)
                } else {
                    Ok(Some(value))
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(OrbitError::internal(format!("TiKV get error: {}", e))),
        };

        // For read-only transactions, we can rollback instead of commit
        let _ = txn.rollback().await;

        // If key was expired, clean it up in a separate call
        if matches!(&result, Ok(None)) {
            // Check if it was actually expired by doing the get operation again
            // This is a bit inefficient but avoids transaction conflicts
            let mut recheck_txn = self.client.begin_optimistic().await.map_err(|e| {
                OrbitError::internal(format!("Failed to begin recheck transaction: {}", e))
            })?;
            if let Ok(Some(data)) = recheck_txn.get(data_key.as_bytes().to_vec()).await {
                if let Ok(value) = serde_json::from_slice::<RedisValue>(&data) {
                    if value.is_expired() {
                        // Clean up expired key asynchronously
                        let _ = self.delete(key).await;
                    }
                }
            }
            let _ = recheck_txn.rollback().await;
        }

        result
    }

    async fn set(&self, key: &str, value: RedisValue) -> OrbitResult<()> {
        let data_key = self.format_data_key(key);

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.set_operations += 1;
        }

        // Serialize the value
        let serialized = serde_json::to_vec(&value)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize value: {}", e)))?;

        let mut txn = self
            .client
            .begin_optimistic()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to begin transaction: {}", e)))?;

        // Set the data
        txn.put(data_key.as_bytes().to_vec(), serialized)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to set data: {}", e)))?;

        // Set expiration if specified
        if let Some(expiration) = value.expiration {
            let exp_key = self.format_expiration_key(expiration, key);
            txn.put(exp_key.as_bytes().to_vec(), b"".to_vec())
                .await
                .map_err(|e| OrbitError::internal(format!("Failed to set expiration: {}", e)))?;
        }

        txn.commit()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> OrbitResult<bool> {
        let data_key = self.format_data_key(key);

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.delete_operations += 1;
        }

        let mut txn = self
            .client
            .begin_optimistic()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to begin transaction: {}", e)))?;

        // Check if key exists and get its expiration
        let existed = match txn.get(data_key.as_bytes().to_vec()).await {
            Ok(Some(data)) => {
                let value: RedisValue = serde_json::from_slice(&data).map_err(|e| {
                    OrbitError::internal(format!("Failed to deserialize value: {}", e))
                })?;

                // Delete the data key
                txn.delete(data_key.as_bytes().to_vec())
                    .await
                    .map_err(|e| {
                        OrbitError::internal(format!("Failed to delete data key: {}", e))
                    })?;

                // Delete expiration key if it exists
                if let Some(expiration) = value.expiration {
                    let exp_key = self.format_expiration_key(expiration, key);
                    txn.delete(exp_key.as_bytes().to_vec()).await.map_err(|e| {
                        OrbitError::internal(format!("Failed to delete expiration key: {}", e))
                    })?;
                }

                true
            }
            Ok(None) => false,
            Err(e) => return Err(OrbitError::internal(format!("TiKV get error: {}", e))),
        };
        if existed {
            txn.commit().await.map_err(|e| {
                OrbitError::internal(format!("Failed to commit delete transaction: {}", e))
            })?;
        } else {
            // Rollback if nothing to delete
            let _ = txn.rollback().await;
        }

        Ok(existed)
    }

    async fn exists(&self, key: &str) -> OrbitResult<bool> {
        let data_key = self.format_data_key(key);

        let mut txn = self
            .client
            .begin_optimistic()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to begin transaction: {}", e)))?;

        let result = match txn.get(data_key.as_bytes().to_vec()).await {
            Ok(Some(data)) => {
                let value: RedisValue = serde_json::from_slice(&data).map_err(|e| {
                    OrbitError::internal(format!("Failed to deserialize value: {}", e))
                })?;

                if value.is_expired() {
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            Ok(None) => Ok(false),
            Err(e) => Err(OrbitError::internal(format!("TiKV exists error: {}", e))),
        };

        // Rollback read-only transaction
        let _ = txn.rollback().await;

        result
    }

    async fn mget(&self, keys: &[String]) -> OrbitResult<Vec<Option<RedisValue>>> {
        let mut results = Vec::with_capacity(keys.len());

        // TiKV doesn't have native multi-get, so we use individual gets
        // In a real implementation, you might want to use batch operations or parallel gets
        for key in keys {
            results.push(self.get(key).await?);
        }

        Ok(results)
    }

    async fn mset(&self, values: HashMap<String, RedisValue>) -> OrbitResult<()> {
        // Use a single transaction for all sets
        let mut txn = self
            .client
            .begin_optimistic()
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to begin transaction: {}", e)))?;

        for (key, value) in values {
            let data_key = self.format_data_key(&key);

            // Serialize the value
            let serialized = serde_json::to_vec(&value)
                .map_err(|e| OrbitError::internal(format!("Failed to serialize value: {}", e)))?;

            // Set the data
            txn.put(data_key.as_bytes().to_vec(), serialized)
                .await
                .map_err(|e| OrbitError::internal(format!("Failed to set data: {}", e)))?;

            // Set expiration if specified
            if let Some(expiration) = value.expiration {
                let exp_key = self.format_expiration_key(expiration, &key);
                txn.put(exp_key.as_bytes().to_vec(), b"".to_vec())
                    .await
                    .map_err(|e| {
                        OrbitError::internal(format!("Failed to set expiration: {}", e))
                    })?;
            }
        }

        txn.commit().await.map_err(|e| {
            OrbitError::internal(format!("Failed to commit mset transaction: {}", e))
        })?;

        Ok(())
    }

    async fn keys(&self, _pattern: &str) -> OrbitResult<Vec<String>> {
        // For now, return empty list as implementing TiKV scan properly requires
        // more complex handling. In a production implementation, you would:
        // 1. Use TiKV's scan operation with proper range queries
        // 2. Handle pagination for large key sets
        // 3. Implement pattern matching

        // This could be implemented with TiKV's scan ranges but requires
        // handling the complex async iterator properly
        warn!("KEYS command returns empty - TiKV scan not fully implemented");
        Ok(Vec::new())
    }

    async fn cleanup_expired(&self) -> OrbitResult<u64> {
        let cleaned_count =
            Self::perform_cleanup_sync(&self.client, &self.config, &self.key_prefix).await?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.expired_keys_cleaned += cleaned_count;
        }

        Ok(cleaned_count)
    }

    async fn metrics(&self) -> OrbitResult<RedisDataMetrics> {
        let metrics = self.metrics.read().await.clone();
        // Note: For TiKV, we don't update total_keys and keys_with_ttl in real-time
        // as it would require scanning the entire keyspace, which is expensive.
        // These could be updated periodically or on-demand if needed.
        Ok(metrics)
    }

    async fn incr(&self, key: &str, delta: i64) -> OrbitResult<i64> {
        let current = match self.get(key).await? {
            Some(value) => value
                .data
                .parse::<i64>()
                .map_err(|_| OrbitError::internal("ERR value is not an integer or out of range"))?,
            None => 0,
        };

        let new_value = current + delta;
        let redis_value = RedisValue::new(new_value.to_string());
        self.set(key, redis_value).await?;

        Ok(new_value)
    }

    async fn append(&self, key: &str, append_value: &str) -> OrbitResult<usize> {
        let current = match self.get(key).await? {
            Some(value) => value.data,
            None => String::new(),
        };

        let new_value = format!("{}{}", current, append_value);
        let length = new_value.len();
        let redis_value = RedisValue::new(new_value);
        self.set(key, redis_value).await?;

        Ok(length)
    }

    async fn strlen(&self, key: &str) -> OrbitResult<usize> {
        match self.get(key).await? {
            Some(value) => Ok(value.data.len()),
            None => Ok(0),
        }
    }

    async fn setnx(&self, key: &str, value: RedisValue) -> OrbitResult<bool> {
        if self.exists(key).await? {
            Ok(false)
        } else {
            self.set(key, value).await?;
            Ok(true)
        }
    }

    async fn getset(&self, key: &str, value: RedisValue) -> OrbitResult<Option<String>> {
        let old_value = match self.get(key).await? {
            Some(v) => Some(v.data),
            None => None,
        };

        self.set(key, value).await?;
        Ok(old_value)
    }
}
