//! Tiered Storage Backend for Unified Storage
//!
//! This module implements a multi-tier storage architecture with automatic
//! data movement between Hot (Memory), Warm (RocksDB), and Cold (Cloud) tiers.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        TieredStorageBackend                             │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  ┌──────────────────────────────────────────────────────────────────┐   │
//! │  │                        HOT TIER (Memory)                         │   │
//! │  │  • LRU/LFU eviction policies                                     │   │
//! │  │  • Write-through / Write-back cache                              │   │
//! │  │  • Sub-millisecond latency                                       │   │
//! │  └──────────────────────────────────────────────────────────────────┘   │
//! │                              │                                          │
//! │                              ▼ (promotion/demotion)                     │
//! │  ┌──────────────────────────────────────────────────────────────────┐   │
//! │  │                       WARM TIER (RocksDB)                        │   │
//! │  │  • LSM-tree with compaction                                      │   │
//! │  │  • Compression (LZ4/Snappy/Zstd)                                 │   │
//! │  │  • WAL for durability                                            │   │
//! │  │  • Bloom filters for efficient lookups                           │   │
//! │  └──────────────────────────────────────────────────────────────────┘   │
//! │                              │                                          │
//! │                              ▼ (archival)                               │
//! │  ┌──────────────────────────────────────────────────────────────────┐   │
//! │  │                       COLD TIER (Cloud)                          │   │
//! │  │  • S3/Azure/GCS/MinIO backends                                   │   │
//! │  │  • Parquet/Iceberg format support                                │   │
//! │  │  • Cost-optimized archival                                       │   │
//! │  └──────────────────────────────────────────────────────────────────┘   │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Data Movement Policies
//!
//! - **Promotion**: Cold → Warm → Hot (based on access frequency)
//! - **Demotion**: Hot → Warm → Cold (based on LRU/LFU and idle time)
//! - **Write-through**: Writes go to Hot + Warm for durability
//! - **Read-through**: Misses in Hot tier fetch from Warm/Cold

use super::storage::{
    MemoryBackend, UnifiedStorageBackend, UnifiedStorageMetrics, UnifiedStorageResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Storage tier enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageTier {
    /// Hot tier: In-memory with LRU/LFU eviction
    Hot,
    /// Warm tier: RocksDB with compression
    Warm,
    /// Cold tier: Cloud storage (S3/Azure/GCS)
    Cold,
}

impl std::fmt::Display for StorageTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageTier::Hot => write!(f, "hot"),
            StorageTier::Warm => write!(f, "warm"),
            StorageTier::Cold => write!(f, "cold"),
        }
    }
}

/// Eviction policy for hot tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used
    Lfu,
    /// Time-based expiration
    Ttl,
    /// Adaptive (combines LRU + LFU based on access patterns)
    Adaptive,
}

impl Default for EvictionPolicy {
    fn default() -> Self {
        Self::Lru
    }
}

/// Write policy for tiered storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WritePolicy {
    /// Write to hot tier and warm tier simultaneously (best durability)
    WriteThrough,
    /// Write to hot tier, asynchronously sync to warm tier (best performance)
    WriteBack,
    /// Write only to warm tier, populate hot tier on read
    WriteAround,
}

impl Default for WritePolicy {
    fn default() -> Self {
        Self::WriteThrough
    }
}

/// Configuration for hot tier (memory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotTierConfig {
    /// Enable hot tier
    pub enabled: bool,
    /// Maximum memory in bytes
    pub max_memory_bytes: usize,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// High watermark for eviction (percentage)
    pub eviction_high_watermark: f64,
    /// Low watermark for eviction (percentage)
    pub eviction_low_watermark: f64,
    /// TTL for entries (optional)
    pub default_ttl_secs: Option<u64>,
    /// Enable write-through to warm tier
    pub write_through: bool,
    /// Enable read-through from warm tier
    pub read_through: bool,
}

impl Default for HotTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_memory_bytes: 1024 * 1024 * 1024, // 1GB
            eviction_policy: EvictionPolicy::Lru,
            eviction_high_watermark: 0.9,
            eviction_low_watermark: 0.7,
            default_ttl_secs: None,
            write_through: true,
            read_through: true,
        }
    }
}

/// Configuration for warm tier (RocksDB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmTierConfig {
    /// Enable warm tier
    pub enabled: bool,
    /// Data directory for RocksDB
    pub data_dir: String,
    /// Enable compression
    pub enable_compression: bool,
    /// Compression algorithm
    pub compression_algorithm: String, // "lz4", "snappy", "zstd"
    /// Enable bloom filters
    pub enable_bloom_filter: bool,
    /// Bloom filter bits per key
    pub bloom_filter_bits: usize,
    /// Enable WAL
    pub enable_wal: bool,
    /// Max background compaction jobs
    pub max_background_jobs: i32,
    /// Write buffer size in bytes
    pub write_buffer_size: usize,
}

impl Default for WarmTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            data_dir: "./data/unified/rocksdb".to_string(),
            enable_compression: true,
            compression_algorithm: "lz4".to_string(),
            enable_bloom_filter: true,
            bloom_filter_bits: 10,
            enable_wal: true,
            max_background_jobs: 4,
            write_buffer_size: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// Configuration for cold tier (cloud storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdTierConfig {
    /// Enable cold tier
    pub enabled: bool,
    /// Cloud backend type
    pub backend: ColdBackendType,
    /// Bucket/container name
    pub bucket: String,
    /// Path prefix
    pub prefix: String,
    /// Data format
    pub data_format: ColdDataFormat,
    /// Region
    pub region: Option<String>,
    /// Endpoint (for S3-compatible stores)
    pub endpoint: Option<String>,
    /// Access key (if not using IAM)
    pub access_key: Option<String>,
    /// Secret key (if not using IAM)
    pub secret_key: Option<String>,
}

impl Default for ColdTierConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: ColdBackendType::S3,
            bucket: "orbit-cold-storage".to_string(),
            prefix: "unified/".to_string(),
            data_format: ColdDataFormat::Parquet,
            region: Some("us-east-1".to_string()),
            endpoint: None,
            access_key: None,
            secret_key: None,
        }
    }
}

/// Cold storage backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColdBackendType {
    /// Amazon S3
    S3,
    /// Azure Blob Storage
    Azure,
    /// Google Cloud Storage
    Gcs,
    /// MinIO (S3-compatible)
    MinIO,
    /// Local filesystem (for testing)
    Local,
}

impl Default for ColdBackendType {
    fn default() -> Self {
        Self::S3
    }
}

/// Data format for cold storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColdDataFormat {
    /// Apache Parquet columnar format
    Parquet,
    /// Apache Iceberg table format
    Iceberg,
    /// Raw JSON lines
    JsonLines,
    /// MessagePack binary format
    MessagePack,
}

impl Default for ColdDataFormat {
    fn default() -> Self {
        Self::Parquet
    }
}

/// Configuration for automatic tier migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierMigrationConfig {
    /// Enable automatic tier migration
    pub enabled: bool,
    /// Check interval for tier migration (seconds)
    pub check_interval_secs: u64,
    /// Hot → Warm demotion after idle time (seconds)
    pub hot_to_warm_idle_secs: u64,
    /// Warm → Cold archival after idle time (seconds)
    pub warm_to_cold_idle_secs: u64,
    /// Promote to hot tier after N accesses
    pub promotion_access_count: u32,
    /// Maximum concurrent migrations
    pub max_concurrent_migrations: usize,
}

impl Default for TierMigrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_secs: 60,
            hot_to_warm_idle_secs: 300,    // 5 minutes
            warm_to_cold_idle_secs: 86400, // 24 hours
            promotion_access_count: 10,
            max_concurrent_migrations: 4,
        }
    }
}

/// Complete tiered storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredStorageConfig {
    /// Hot tier configuration
    pub hot_tier: HotTierConfig,
    /// Warm tier configuration
    pub warm_tier: WarmTierConfig,
    /// Cold tier configuration
    pub cold_tier: ColdTierConfig,
    /// Tier migration configuration
    pub migration: TierMigrationConfig,
    /// Write policy
    pub write_policy: WritePolicy,
}

impl Default for TieredStorageConfig {
    fn default() -> Self {
        Self {
            hot_tier: HotTierConfig::default(),
            warm_tier: WarmTierConfig::default(),
            cold_tier: ColdTierConfig::default(),
            migration: TierMigrationConfig::default(),
            write_policy: WritePolicy::WriteThrough,
        }
    }
}

/// Entry metadata for tier management
#[derive(Debug, Clone)]
struct EntryMetadata {
    /// Current storage tier
    tier: StorageTier,
    /// Last access timestamp
    last_access: Instant,
    /// Access count (for LFU)
    access_count: u64,
    /// Entry size in bytes
    size_bytes: usize,
    /// Creation timestamp
    created_at: Instant,
}

impl EntryMetadata {
    fn new(tier: StorageTier, size_bytes: usize) -> Self {
        Self {
            tier,
            last_access: Instant::now(),
            access_count: 1,
            size_bytes,
            created_at: Instant::now(),
        }
    }

    fn touch(&mut self) {
        self.last_access = Instant::now();
        self.access_count += 1;
    }

    fn idle_duration(&self) -> Duration {
        self.last_access.elapsed()
    }
}

/// Tiered storage metrics
#[derive(Debug, Clone, Default)]
pub struct TieredStorageMetrics {
    /// Base metrics
    pub base: UnifiedStorageMetrics,
    /// Hot tier hit count
    pub hot_tier_hits: u64,
    /// Hot tier miss count
    pub hot_tier_misses: u64,
    /// Warm tier hit count
    pub warm_tier_hits: u64,
    /// Warm tier miss count
    pub warm_tier_misses: u64,
    /// Cold tier hit count
    pub cold_tier_hits: u64,
    /// Hot tier current size in bytes
    pub hot_tier_size_bytes: u64,
    /// Warm tier current size in bytes
    pub warm_tier_size_bytes: u64,
    /// Hot tier entry count
    pub hot_tier_entries: u64,
    /// Warm tier entry count
    pub warm_tier_entries: u64,
    /// Promotions from warm to hot
    pub promotions: u64,
    /// Demotions from hot to warm
    pub demotions: u64,
    /// Archivals from warm to cold
    pub archivals: u64,
    /// Evictions from hot tier
    pub evictions: u64,
}

/// Tiered storage backend implementation
///
/// This backend layers multiple storage tiers to optimize for both
/// performance and cost. Frequently accessed data stays in the hot
/// tier (memory), while less frequently accessed data moves to the
/// warm tier (RocksDB) and eventually the cold tier (cloud storage).
pub struct TieredStorageBackend {
    /// Configuration
    config: TieredStorageConfig,
    /// Hot tier (in-memory)
    hot_tier: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// Warm tier (RocksDB-backed, using memory for now)
    warm_tier: Arc<MemoryBackend>,
    /// Cold tier (cloud-backed, using memory for now)
    cold_tier: Arc<MemoryBackend>,
    /// Entry metadata for tier management
    metadata: Arc<RwLock<HashMap<String, EntryMetadata>>>,
    /// Tiered metrics
    tiered_metrics: Arc<RwLock<TieredStorageMetrics>>,
    /// Hot tier current size in bytes
    hot_tier_size: Arc<RwLock<usize>>,
}

impl TieredStorageBackend {
    /// Create a new tiered storage backend with the given configuration
    pub fn new(config: TieredStorageConfig) -> Self {
        info!(
            "[TieredStorage] Creating tiered backend with hot={}, warm={}, cold={}",
            config.hot_tier.enabled, config.warm_tier.enabled, config.cold_tier.enabled
        );

        Self {
            config,
            hot_tier: Arc::new(RwLock::new(HashMap::new())),
            warm_tier: Arc::new(MemoryBackend::new()),
            cold_tier: Arc::new(MemoryBackend::new()),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            tiered_metrics: Arc::new(RwLock::new(TieredStorageMetrics::default())),
            hot_tier_size: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a tiered backend with default configuration
    pub fn with_defaults() -> Self {
        Self::new(TieredStorageConfig::default())
    }

    /// Get which tier a key is stored in
    pub async fn get_tier(&self, key: &str) -> Option<StorageTier> {
        let metadata = self.metadata.read().await;
        metadata.get(key).map(|m| m.tier)
    }

    /// Get tiered storage metrics
    pub async fn tiered_metrics(&self) -> TieredStorageMetrics {
        self.tiered_metrics.read().await.clone()
    }

    /// Check if hot tier needs eviction
    async fn should_evict(&self) -> bool {
        if !self.config.hot_tier.enabled {
            return false;
        }

        let current_size = *self.hot_tier_size.read().await;
        let threshold = (self.config.hot_tier.max_memory_bytes as f64
            * self.config.hot_tier.eviction_high_watermark) as usize;
        current_size >= threshold
    }

    /// Evict entries from hot tier based on eviction policy
    async fn evict_if_needed(&self) -> UnifiedStorageResult<usize> {
        if !self.should_evict().await {
            return Ok(0);
        }

        let target_size = (self.config.hot_tier.max_memory_bytes as f64
            * self.config.hot_tier.eviction_low_watermark) as usize;
        let mut current_size = *self.hot_tier_size.read().await;

        if current_size <= target_size {
            return Ok(0);
        }

        let mut evicted = 0usize;
        let mut to_demote: Vec<(String, Vec<u8>)> = Vec::new();

        // Get candidates for eviction based on policy
        {
            let metadata = self.metadata.read().await;
            let hot = self.hot_tier.read().await;

            let mut candidates: Vec<(&String, &EntryMetadata)> = metadata
                .iter()
                .filter(|(k, m)| m.tier == StorageTier::Hot && hot.contains_key(*k))
                .collect();

            // Sort by eviction policy
            match self.config.hot_tier.eviction_policy {
                EvictionPolicy::Lru => {
                    candidates.sort_by(|a, b| a.1.last_access.cmp(&b.1.last_access));
                }
                EvictionPolicy::Lfu => {
                    candidates.sort_by(|a, b| a.1.access_count.cmp(&b.1.access_count));
                }
                EvictionPolicy::Ttl => {
                    candidates.sort_by(|a, b| a.1.created_at.cmp(&b.1.created_at));
                }
                EvictionPolicy::Adaptive => {
                    // Combine LRU and LFU: score = access_count / idle_time
                    candidates.sort_by(|a, b| {
                        let score_a =
                            a.1.access_count as f64 / (a.1.idle_duration().as_secs_f64() + 1.0);
                        let score_b =
                            b.1.access_count as f64 / (b.1.idle_duration().as_secs_f64() + 1.0);
                        score_a
                            .partial_cmp(&score_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }

            // Select entries to evict
            for (key, meta) in candidates {
                if current_size <= target_size {
                    break;
                }

                if let Some(value) = hot.get(key) {
                    to_demote.push((key.clone(), value.clone()));
                    current_size = current_size.saturating_sub(meta.size_bytes);
                    evicted += 1;
                }
            }
        }

        // Perform demotion to warm tier
        for (key, value) in to_demote {
            self.demote_to_warm(&key, &value).await?;
        }

        debug!("[TieredStorage] Evicted {} entries from hot tier", evicted);

        // Update metrics
        {
            let mut metrics = self.tiered_metrics.write().await;
            metrics.evictions += evicted as u64;
            metrics.demotions += evicted as u64;
        }

        Ok(evicted)
    }

    /// Demote an entry from hot tier to warm tier
    async fn demote_to_warm(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()> {
        // Write to warm tier
        if self.config.warm_tier.enabled {
            self.warm_tier.put(key, value).await?;
        }

        // Remove from hot tier
        {
            let mut hot = self.hot_tier.write().await;
            if let Some(old_value) = hot.remove(key) {
                let mut size = self.hot_tier_size.write().await;
                *size = size.saturating_sub(old_value.len());
            }
        }

        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            if let Some(meta) = metadata.get_mut(key) {
                meta.tier = StorageTier::Warm;
            }
        }

        debug!("[TieredStorage] Demoted key {} to warm tier", key);
        Ok(())
    }

    /// Promote an entry from warm tier to hot tier
    async fn promote_to_hot(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()> {
        // Evict if necessary
        self.evict_if_needed().await?;

        // Write to hot tier
        {
            let mut hot = self.hot_tier.write().await;
            let mut size = self.hot_tier_size.write().await;

            if let Some(old_value) = hot.insert(key.to_string(), value.to_vec()) {
                *size = size.saturating_sub(old_value.len());
            }
            *size += value.len();
        }

        // Update metadata
        {
            let mut metadata = self.metadata.write().await;
            if let Some(meta) = metadata.get_mut(key) {
                meta.tier = StorageTier::Hot;
                meta.touch();
            } else {
                metadata.insert(
                    key.to_string(),
                    EntryMetadata::new(StorageTier::Hot, value.len()),
                );
            }
        }

        // Update metrics
        {
            let mut metrics = self.tiered_metrics.write().await;
            metrics.promotions += 1;
        }

        debug!("[TieredStorage] Promoted key {} to hot tier", key);
        Ok(())
    }

    /// Try to get from hot tier
    async fn get_from_hot(&self, key: &str) -> Option<Vec<u8>> {
        let hot = self.hot_tier.read().await;
        hot.get(key).cloned()
    }

    /// Try to get from warm tier
    async fn get_from_warm(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>> {
        self.warm_tier.get(key).await
    }

    /// Try to get from cold tier
    async fn get_from_cold(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>> {
        self.cold_tier.get(key).await
    }
}

#[async_trait]
impl UnifiedStorageBackend for TieredStorageBackend {
    async fn initialize(&self) -> UnifiedStorageResult<()> {
        info!("[TieredStorage] Initializing tiered storage backend");

        if self.config.hot_tier.enabled {
            info!(
                "[TieredStorage] Hot tier enabled: max_memory={}MB, policy={:?}",
                self.config.hot_tier.max_memory_bytes / (1024 * 1024),
                self.config.hot_tier.eviction_policy
            );
        }

        if self.config.warm_tier.enabled {
            self.warm_tier.initialize().await?;
            info!(
                "[TieredStorage] Warm tier enabled: dir={}, compression={}",
                self.config.warm_tier.data_dir, self.config.warm_tier.compression_algorithm
            );
        }

        if self.config.cold_tier.enabled {
            self.cold_tier.initialize().await?;
            info!(
                "[TieredStorage] Cold tier enabled: backend={:?}, bucket={}",
                self.config.cold_tier.backend, self.config.cold_tier.bucket
            );
        }

        info!("[TieredStorage] Tiered storage initialized successfully");
        Ok(())
    }

    async fn shutdown(&self) -> UnifiedStorageResult<()> {
        info!("[TieredStorage] Shutting down tiered storage backend");

        // Flush hot tier to warm tier for durability
        if self.config.hot_tier.enabled && self.config.warm_tier.enabled {
            let hot = self.hot_tier.read().await;
            for (key, value) in hot.iter() {
                if let Err(e) = self.warm_tier.put(key, value).await {
                    warn!(
                        "[TieredStorage] Failed to flush key {} to warm tier: {}",
                        key, e
                    );
                }
            }
        }

        if self.config.warm_tier.enabled {
            self.warm_tier.shutdown().await?;
        }

        if self.config.cold_tier.enabled {
            self.cold_tier.shutdown().await?;
        }

        info!("[TieredStorage] Tiered storage shutdown complete");
        Ok(())
    }

    async fn get(&self, key: &str) -> UnifiedStorageResult<Option<Vec<u8>>> {
        let mut tiered_metrics = self.tiered_metrics.write().await;
        tiered_metrics.base.read_operations += 1;

        // Try hot tier first
        if self.config.hot_tier.enabled {
            if let Some(value) = self.get_from_hot(key).await {
                // Update access metadata
                {
                    let mut metadata = self.metadata.write().await;
                    if let Some(meta) = metadata.get_mut(key) {
                        meta.touch();
                    }
                }
                tiered_metrics.hot_tier_hits += 1;
                return Ok(Some(value));
            }
            tiered_metrics.hot_tier_misses += 1;
        }

        // Try warm tier
        if self.config.warm_tier.enabled {
            drop(tiered_metrics); // Release lock before async call

            if let Some(value) = self.get_from_warm(key).await? {
                let mut tiered_metrics = self.tiered_metrics.write().await;
                tiered_metrics.warm_tier_hits += 1;
                drop(tiered_metrics);

                // Check if we should promote to hot tier
                let should_promote = {
                    let metadata = self.metadata.read().await;
                    if let Some(meta) = metadata.get(key) {
                        meta.access_count >= self.config.migration.promotion_access_count as u64
                    } else {
                        false
                    }
                };

                if should_promote
                    && self.config.hot_tier.enabled
                    && self.config.hot_tier.read_through
                {
                    let _ = self.promote_to_hot(key, &value).await;
                }

                return Ok(Some(value));
            }

            let mut tiered_metrics = self.tiered_metrics.write().await;
            tiered_metrics.warm_tier_misses += 1;
            drop(tiered_metrics);
        }

        // Try cold tier
        if self.config.cold_tier.enabled {
            if let Some(value) = self.get_from_cold(key).await? {
                let mut tiered_metrics = self.tiered_metrics.write().await;
                tiered_metrics.cold_tier_hits += 1;
                drop(tiered_metrics);

                // Promote to warm tier on cold hit
                if self.config.warm_tier.enabled {
                    let _ = self.warm_tier.put(key, &value).await;
                }

                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    async fn put(&self, key: &str, value: &[u8]) -> UnifiedStorageResult<()> {
        {
            let mut tiered_metrics = self.tiered_metrics.write().await;
            tiered_metrics.base.write_operations += 1;
        }

        match self.config.write_policy {
            WritePolicy::WriteThrough => {
                // Write to both hot and warm tiers
                if self.config.hot_tier.enabled {
                    self.evict_if_needed().await?;

                    let mut hot = self.hot_tier.write().await;
                    let mut size = self.hot_tier_size.write().await;

                    if let Some(old_value) = hot.insert(key.to_string(), value.to_vec()) {
                        *size = size.saturating_sub(old_value.len());
                    }
                    *size += value.len();
                }

                if self.config.warm_tier.enabled {
                    self.warm_tier.put(key, value).await?;
                }

                // Update metadata
                {
                    let mut metadata = self.metadata.write().await;
                    let tier = if self.config.hot_tier.enabled {
                        StorageTier::Hot
                    } else {
                        StorageTier::Warm
                    };
                    metadata.insert(key.to_string(), EntryMetadata::new(tier, value.len()));
                }
            }
            WritePolicy::WriteBack => {
                // Write only to hot tier, will sync to warm tier later
                if self.config.hot_tier.enabled {
                    self.evict_if_needed().await?;

                    let mut hot = self.hot_tier.write().await;
                    let mut size = self.hot_tier_size.write().await;

                    if let Some(old_value) = hot.insert(key.to_string(), value.to_vec()) {
                        *size = size.saturating_sub(old_value.len());
                    }
                    *size += value.len();

                    let mut metadata = self.metadata.write().await;
                    metadata.insert(
                        key.to_string(),
                        EntryMetadata::new(StorageTier::Hot, value.len()),
                    );
                } else if self.config.warm_tier.enabled {
                    self.warm_tier.put(key, value).await?;

                    let mut metadata = self.metadata.write().await;
                    metadata.insert(
                        key.to_string(),
                        EntryMetadata::new(StorageTier::Warm, value.len()),
                    );
                }
            }
            WritePolicy::WriteAround => {
                // Write only to warm tier
                if self.config.warm_tier.enabled {
                    self.warm_tier.put(key, value).await?;

                    let mut metadata = self.metadata.write().await;
                    metadata.insert(
                        key.to_string(),
                        EntryMetadata::new(StorageTier::Warm, value.len()),
                    );
                }
            }
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> UnifiedStorageResult<bool> {
        {
            let mut tiered_metrics = self.tiered_metrics.write().await;
            tiered_metrics.base.delete_operations += 1;
        }

        let mut deleted = false;

        // Delete from all tiers
        if self.config.hot_tier.enabled {
            let mut hot = self.hot_tier.write().await;
            if let Some(old_value) = hot.remove(key) {
                let mut size = self.hot_tier_size.write().await;
                *size = size.saturating_sub(old_value.len());
                deleted = true;
            }
        }

        if self.config.warm_tier.enabled {
            if self.warm_tier.delete(key).await? {
                deleted = true;
            }
        }

        if self.config.cold_tier.enabled {
            if self.cold_tier.delete(key).await? {
                deleted = true;
            }
        }

        // Remove metadata
        {
            let mut metadata = self.metadata.write().await;
            metadata.remove(key);
        }

        Ok(deleted)
    }

    async fn exists(&self, key: &str) -> UnifiedStorageResult<bool> {
        // Check hot tier
        if self.config.hot_tier.enabled {
            let hot = self.hot_tier.read().await;
            if hot.contains_key(key) {
                return Ok(true);
            }
        }

        // Check warm tier
        if self.config.warm_tier.enabled && self.warm_tier.exists(key).await? {
            return Ok(true);
        }

        // Check cold tier
        if self.config.cold_tier.enabled && self.cold_tier.exists(key).await? {
            return Ok(true);
        }

        Ok(false)
    }

    async fn scan_prefix(
        &self,
        prefix: &str,
        limit: Option<usize>,
    ) -> UnifiedStorageResult<Vec<(String, Vec<u8>)>> {
        let mut results: HashMap<String, Vec<u8>> = HashMap::new();

        // Scan cold tier first (lowest priority, will be overwritten)
        if self.config.cold_tier.enabled {
            for (key, value) in self.cold_tier.scan_prefix(prefix, None).await? {
                results.insert(key, value);
            }
        }

        // Scan warm tier (medium priority)
        if self.config.warm_tier.enabled {
            for (key, value) in self.warm_tier.scan_prefix(prefix, None).await? {
                results.insert(key, value);
            }
        }

        // Scan hot tier (highest priority, overwrites)
        if self.config.hot_tier.enabled {
            let hot = self.hot_tier.read().await;
            for (key, value) in hot.iter() {
                if key.starts_with(prefix) {
                    results.insert(key.clone(), value.clone());
                }
            }
        }

        // Sort and limit results
        let mut results: Vec<_> = results.into_iter().collect();
        results.sort_by(|a, b| a.0.cmp(&b.0));

        if let Some(limit) = limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn put_batch(&self, entries: Vec<(String, Vec<u8>)>) -> UnifiedStorageResult<()> {
        for (key, value) in entries {
            self.put(&key, &value).await?;
        }
        Ok(())
    }

    async fn delete_batch(&self, keys: Vec<String>) -> UnifiedStorageResult<u64> {
        let mut count = 0u64;
        for key in keys {
            if self.delete(&key).await? {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn metrics(&self) -> UnifiedStorageMetrics {
        let tiered = self.tiered_metrics.read().await;
        let hot_size = *self.hot_tier_size.read().await;

        UnifiedStorageMetrics {
            read_operations: tiered.base.read_operations,
            write_operations: tiered.base.write_operations,
            delete_operations: tiered.base.delete_operations,
            read_latency_avg: tiered.base.read_latency_avg,
            write_latency_avg: tiered.base.write_latency_avg,
            delete_latency_avg: tiered.base.delete_latency_avg,
            total_records: tiered.hot_tier_entries + tiered.warm_tier_entries,
            total_namespaces: 0,
            memory_usage_bytes: hot_size as u64,
            error_count: tiered.base.error_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tiered_storage_basic_crud() {
        let config = TieredStorageConfig::default();
        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        // Put
        backend.put("test:key1", b"value1").await.unwrap();

        // Get from hot tier
        let result = backend.get("test:key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Exists
        assert!(backend.exists("test:key1").await.unwrap());

        // Delete
        assert!(backend.delete("test:key1").await.unwrap());
        assert!(!backend.exists("test:key1").await.unwrap());

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_tiered_storage_tier_placement() {
        let config = TieredStorageConfig::default();
        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        backend.put("test:key1", b"value1").await.unwrap();

        // Should be in hot tier (write-through)
        let tier = backend.get_tier("test:key1").await;
        assert_eq!(tier, Some(StorageTier::Hot));

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_tiered_storage_scan() {
        let config = TieredStorageConfig::default();
        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        // Add multiple entries
        backend.put("prefix:1", b"value1").await.unwrap();
        backend.put("prefix:2", b"value2").await.unwrap();
        backend.put("prefix:3", b"value3").await.unwrap();
        backend.put("other:1", b"other").await.unwrap();

        // Scan by prefix
        let results = backend.scan_prefix("prefix:", None).await.unwrap();
        assert_eq!(results.len(), 3);

        // Scan with limit
        let results = backend.scan_prefix("prefix:", Some(2)).await.unwrap();
        assert_eq!(results.len(), 2);

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_tiered_storage_eviction() {
        let mut config = TieredStorageConfig::default();
        // Set very small hot tier size to force eviction
        config.hot_tier.max_memory_bytes = 100;
        config.hot_tier.eviction_high_watermark = 0.8;
        config.hot_tier.eviction_low_watermark = 0.5;

        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        // Add entries that exceed hot tier size
        for i in 0..20 {
            backend
                .put(&format!("key{}", i), format!("value{}", i).as_bytes())
                .await
                .unwrap();
        }

        // Some entries should have been evicted to warm tier
        let metrics = backend.tiered_metrics().await;
        assert!(metrics.evictions > 0 || metrics.demotions > 0);

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_write_policies() {
        // Test write-through
        let mut config = TieredStorageConfig::default();
        config.write_policy = WritePolicy::WriteThrough;
        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        backend.put("key1", b"value1").await.unwrap();

        // Should be in hot tier
        assert_eq!(backend.get_tier("key1").await, Some(StorageTier::Hot));

        backend.shutdown().await.unwrap();

        // Test write-around
        let mut config = TieredStorageConfig::default();
        config.write_policy = WritePolicy::WriteAround;
        config.hot_tier.enabled = false; // Disable hot tier for write-around test
        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        backend.put("key2", b"value2").await.unwrap();

        // Should be in warm tier
        assert_eq!(backend.get_tier("key2").await, Some(StorageTier::Warm));

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let config = TieredStorageConfig::default();
        let backend = TieredStorageBackend::new(config);
        backend.initialize().await.unwrap();

        // Batch put
        let entries = vec![
            ("batch:1".to_string(), b"value1".to_vec()),
            ("batch:2".to_string(), b"value2".to_vec()),
            ("batch:3".to_string(), b"value3".to_vec()),
        ];
        backend.put_batch(entries).await.unwrap();

        // Verify all entries exist
        assert!(backend.exists("batch:1").await.unwrap());
        assert!(backend.exists("batch:2").await.unwrap());
        assert!(backend.exists("batch:3").await.unwrap());

        // Batch delete
        let count = backend
            .delete_batch(vec!["batch:1".to_string(), "batch:2".to_string()])
            .await
            .unwrap();
        assert_eq!(count, 2);

        // Verify deletions
        assert!(!backend.exists("batch:1").await.unwrap());
        assert!(!backend.exists("batch:2").await.unwrap());
        assert!(backend.exists("batch:3").await.unwrap());

        backend.shutdown().await.unwrap();
    }
}
