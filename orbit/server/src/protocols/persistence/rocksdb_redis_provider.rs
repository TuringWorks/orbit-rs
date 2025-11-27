//! RocksDB-backed Redis data persistence provider implementation
#![cfg(feature = "storage-rocksdb")]

//! This implementation uses RocksDB for persistent storage of Redis key-value data
//! with proper TTL handling and background cleanup.

use crate::protocols::persistence::redis_data::*;
use async_trait::async_trait;
use orbit_shared::{OrbitError, OrbitResult};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::time::Duration;

/// RocksDB column families for Redis data
const CF_DATA: &str = "redis_data";
const CF_EXPIRATION: &str = "redis_expiration";
const CF_METADATA: &str = "redis_metadata";

/// RocksDB-backed Redis data provider
pub struct RocksDbRedisDataProvider {
    db: Arc<DB>,
    config: RedisDataConfig,
    metrics: tokio::sync::RwLock<RedisDataMetrics>,
    cleanup_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl RocksDbRedisDataProvider {
    /// Create a new RocksDB Redis data provider
    pub fn new<P: AsRef<Path>>(path: P, config: RedisDataConfig) -> OrbitResult<Self> {
        // Configure RocksDB options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Define column families
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_DATA, Options::default()),
            ColumnFamilyDescriptor::new(CF_EXPIRATION, Options::default()),
            ColumnFamilyDescriptor::new(CF_METADATA, Options::default()),
        ];

        // Open database with column families
        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)
            .map_err(|e| OrbitError::internal(format!("Failed to open RocksDB: {}", e)))?;

        Ok(Self {
            db: Arc::new(db),
            config,
            metrics: tokio::sync::RwLock::new(RedisDataMetrics::default()),
            cleanup_task: tokio::sync::RwLock::new(None),
        })
    }

    /// Get the data column family handle
    fn data_cf(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_DATA)
            .expect("Data column family should exist")
    }

    /// Get the expiration column family handle
    fn expiration_cf(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_EXPIRATION)
            .expect("Expiration column family should exist")
    }

    /// Get the metadata column family handle
    #[allow(dead_code)] // Reserved for future metadata operations
    fn metadata_cf(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_METADATA)
            .expect("Metadata column family should exist")
    }

    /// Start the background cleanup task
    async fn start_cleanup_task(&self) {
        if !self.config.enable_expiry_cleanup {
            return;
        }

        let db = Arc::clone(&self.db);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.cleanup_interval_seconds));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                // Perform cleanup synchronously to avoid Send/Sync issues
                match Self::perform_cleanup_sync(&db, &config) {
                    Ok(cleaned_count) => {
                        if cleaned_count > 0 {
                            tracing::debug!(
                                "Background cleanup: {} expired keys removed",
                                cleaned_count
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("Redis cleanup task error: {}", e);
                    }
                }
            }
        });

        *self.cleanup_task.write().await = Some(handle);
    }

    /// Perform synchronous cleanup of expired keys (no async operations)
    fn perform_cleanup_sync(db: &DB, config: &RedisDataConfig) -> OrbitResult<u64> {
        let data_cf = db
            .cf_handle(CF_DATA)
            .ok_or_else(|| OrbitError::internal("Data column family not found"))?;
        let exp_cf = db
            .cf_handle(CF_EXPIRATION)
            .ok_or_else(|| OrbitError::internal("Expiration column family not found"))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut expired_keys = Vec::new();
        let mut processed = 0;

        // Iterate through expiration index to find expired keys
        let iter = db.iterator_cf(exp_cf, rocksdb::IteratorMode::Start);
        for item in iter {
            if processed >= config.cleanup_batch_size {
                break;
            }

            let (key, _value) =
                item.map_err(|e| OrbitError::internal(format!("RocksDB iteration error: {}", e)))?;

            // Key format: "timestamp:actual_key"
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }

            let timestamp: u64 = parts[0].parse().unwrap_or(u64::MAX);
            if timestamp <= now {
                expired_keys.push((key.to_vec(), parts[1].to_string()));
            } else {
                // Since expiration keys are sorted by timestamp, we can stop here
                break;
            }

            processed += 1;
        }

        // Remove expired keys
        let cleaned_count = expired_keys.len() as u64;
        if !expired_keys.is_empty() {
            let mut batch = rocksdb::WriteBatch::default();
            for (exp_key, data_key) in &expired_keys {
                batch.delete_cf(data_cf, data_key);
                batch.delete_cf(exp_cf, exp_key);
            }

            db.write(batch).map_err(|e| {
                OrbitError::internal(format!("Failed to delete expired keys: {}", e))
            })?;

            tracing::debug!("Cleaned up {} expired keys", expired_keys.len());
        }

        Ok(cleaned_count)
    }

    /// Format expiration key for indexing
    fn format_expiration_key(timestamp: u64, key: &str) -> String {
        format!("{}:{}", timestamp, key)
    }

    /// Parse expiration key to get timestamp and original key
    #[allow(dead_code)] // Utility method for future expiration key parsing
    fn parse_expiration_key(exp_key: &str) -> Option<(u64, &str)> {
        let parts: Vec<&str> = exp_key.splitn(2, ':').collect();
        if parts.len() == 2 {
            parts[0].parse().ok().map(|ts| (ts, parts[1]))
        } else {
            None
        }
    }
}

#[async_trait]
impl RedisDataProvider for RocksDbRedisDataProvider {
    async fn initialize(&self) -> OrbitResult<()> {
        tracing::info!("Initializing RocksDB Redis data provider");

        // Start background cleanup task
        self.start_cleanup_task().await;

        tracing::info!("RocksDB Redis data provider initialized successfully");
        Ok(())
    }

    async fn shutdown(&self) -> OrbitResult<()> {
        tracing::info!("Shutting down RocksDB Redis data provider");

        // Stop cleanup task
        if let Some(handle) = self.cleanup_task.write().await.take() {
            handle.abort();
        }

        tracing::info!("RocksDB Redis data provider shutdown completed");
        Ok(())
    }

    async fn get(&self, key: &str) -> OrbitResult<Option<RedisValue>> {
        // Get data without holding column family handle across await
        let data_result = {
            let data_cf = self.data_cf();
            self.db.get_cf(data_cf, key)
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.get_operations += 1;
        }

        match data_result {
            Ok(Some(data)) => {
                let value: RedisValue = serde_json::from_slice(&data).map_err(|e| {
                    OrbitError::internal(format!("Failed to deserialize value: {}", e))
                })?;

                if value.is_expired() {
                    // Key is expired, clean it up
                    let _ = self.delete(key).await;
                    Ok(None)
                } else {
                    Ok(Some(value))
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(OrbitError::internal(format!("RocksDB get error: {}", e))),
        }
    }

    async fn set(&self, key: &str, value: RedisValue) -> OrbitResult<()> {
        // Serialize the value
        let serialized = serde_json::to_vec(&value)
            .map_err(|e| OrbitError::internal(format!("Failed to serialize value: {}", e)))?;

        // Prepare write batch without holding CF handles across await
        let write_result = {
            let data_cf = self.data_cf();
            let exp_cf = self.expiration_cf();

            let mut batch = rocksdb::WriteBatch::default();
            batch.put_cf(data_cf, key, &serialized);

            // Handle expiration
            if let Some(expiration) = value.expiration {
                let exp_key = Self::format_expiration_key(expiration, key);
                batch.put_cf(exp_cf, exp_key, b"");
            }

            self.db.write(batch)
        };

        // Update metrics after the write
        {
            let mut metrics = self.metrics.write().await;
            metrics.set_operations += 1;
        }

        write_result.map_err(|e| OrbitError::internal(format!("RocksDB write error: {}", e)))
    }

    async fn delete(&self, key: &str) -> OrbitResult<bool> {
        // Get existing value and prepare delete batch without holding CF handles across await
        let (existed, delete_result) = {
            let data_cf = self.data_cf();
            let exp_cf = self.expiration_cf();

            match self.db.get_cf(data_cf, key) {
                Ok(Some(data)) => {
                    let value: RedisValue = serde_json::from_slice(&data).map_err(|e| {
                        OrbitError::internal(format!("Failed to deserialize value: {}", e))
                    })?;

                    let mut batch = rocksdb::WriteBatch::default();
                    batch.delete_cf(data_cf, key);

                    // Remove expiration entry if it exists
                    if let Some(expiration) = value.expiration {
                        let exp_key = Self::format_expiration_key(expiration, key);
                        batch.delete_cf(exp_cf, exp_key);
                    }

                    let result = self
                        .db
                        .write(batch)
                        .map_err(|e| OrbitError::internal(format!("RocksDB delete error: {}", e)));

                    (true, result)
                }
                Ok(None) => (false, Ok(())),
                Err(e) => return Err(OrbitError::internal(format!("RocksDB get error: {}", e))),
            }
        };

        // Update metrics after the operation
        {
            let mut metrics = self.metrics.write().await;
            metrics.delete_operations += 1;
        }

        delete_result?;
        Ok(existed)
    }

    async fn exists(&self, key: &str) -> OrbitResult<bool> {
        let data_result = {
            let data_cf = self.data_cf();
            self.db.get_cf(data_cf, key)
        };

        match data_result {
            Ok(Some(data)) => {
                let value: RedisValue = serde_json::from_slice(&data).map_err(|e| {
                    OrbitError::internal(format!("Failed to deserialize value: {}", e))
                })?;

                if value.is_expired() {
                    // Clean up expired key
                    let _ = self.delete(key).await;
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            Ok(None) => Ok(false),
            Err(e) => Err(OrbitError::internal(format!("RocksDB exists error: {}", e))),
        }
    }

    async fn mget(&self, keys: &[String]) -> OrbitResult<Vec<Option<RedisValue>>> {
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            results.push(self.get(key).await?);
        }

        Ok(results)
    }

    async fn mset(&self, values: HashMap<String, RedisValue>) -> OrbitResult<()> {
        for (key, value) in values {
            self.set(&key, value).await?;
        }
        Ok(())
    }

    async fn keys(&self, _pattern: &str) -> OrbitResult<Vec<String>> {
        let keys = {
            let data_cf = self.data_cf();
            let mut keys = Vec::new();

            // Simple implementation - return all keys (ignoring pattern for now)
            let iter = self.db.iterator_cf(data_cf, rocksdb::IteratorMode::Start);
            for item in iter {
                let (key, _) = item
                    .map_err(|e| OrbitError::internal(format!("RocksDB iteration error: {}", e)))?;
                keys.push(String::from_utf8_lossy(&key).to_string());
            }

            keys
        };

        Ok(keys)
    }

    async fn cleanup_expired(&self) -> OrbitResult<u64> {
        let cleaned_count = Self::perform_cleanup_sync(&self.db, &self.config)?;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.expired_keys_cleaned += cleaned_count;
        }

        Ok(cleaned_count)
    }

    async fn metrics(&self) -> OrbitResult<RedisDataMetrics> {
        let (total_keys, keys_with_ttl) = {
            let data_cf = self.data_cf();
            let mut total_keys = 0;
            let mut keys_with_ttl = 0;

            let iter = self.db.iterator_cf(data_cf, rocksdb::IteratorMode::Start);
            for item in iter {
                let (_, value_data) = item
                    .map_err(|e| OrbitError::internal(format!("RocksDB iteration error: {}", e)))?;

                if let Ok(value) = serde_json::from_slice::<RedisValue>(&value_data) {
                    if !value.is_expired() {
                        total_keys += 1;
                        if value.expiration.is_some() {
                            keys_with_ttl += 1;
                        }
                    }
                }
            }

            (total_keys, keys_with_ttl)
        };

        let mut metrics = self.metrics.read().await.clone();
        metrics.total_keys = total_keys;
        metrics.keys_with_ttl = keys_with_ttl;

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
