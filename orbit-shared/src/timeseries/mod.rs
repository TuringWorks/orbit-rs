//! # Orbit Time Series Engine
//!
//! A high-performance time series database implementation supporting:
//! - Redis TimeSeries integration for high-speed ingestion
//! - PostgreSQL TimescaleDB-like features with hypertables
//! - Multi-terabyte storage with compression and optimization
//! - Advanced querying with aggregations and windowing functions

pub mod core;
pub mod storage;
pub mod compression;
pub mod query;
pub mod redis;
pub mod postgresql;
pub mod aggregation;
pub mod retention;
pub mod partitioning;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Time series identifier type
pub type SeriesId = Uuid;

/// Timestamp type using nanosecond precision
pub type Timestamp = i64;

/// Core time series value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeSeriesValue {
    Float(f64),
    Integer(i64),
    String(String),
    Boolean(bool),
    Null,
}

/// Time series data point
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: Timestamp,
    pub value: TimeSeriesValue,
    pub labels: HashMap<String, String>,
}

/// Time series metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesMetadata {
    pub series_id: SeriesId,
    pub name: String,
    pub labels: HashMap<String, String>,
    pub retention_policy: Option<RetentionPolicy>,
    pub compression_policy: Option<CompressionPolicy>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Retention policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub duration_seconds: u64,
    pub downsampling_rules: Vec<DownsamplingRule>,
}

/// Downsampling rule for data aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsamplingRule {
    pub source_age_seconds: u64,
    pub dest_age_seconds: u64,
    pub aggregation_type: AggregationType,
}

/// Compression policy for storage optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionPolicy {
    pub compression_type: CompressionType,
    pub chunk_size: u64,
    pub compress_after_seconds: u64,
}

/// Supported compression algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    Delta,
    DoubleDelta,
    Gorilla,
    Lz4,
    Zstd,
}

/// Aggregation types for queries and downsampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Sum,
    Average,
    Min,
    Max,
    Count,
    StdDev,
    First,
    Last,
    Percentile(f64),
}

/// Time series query range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Timestamp,
    pub end: Timestamp,
}

/// Query result for time series data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub series_id: SeriesId,
    pub metadata: TimeSeriesMetadata,
    pub data_points: Vec<DataPoint>,
    pub total_points: usize,
    pub execution_time_ms: u64,
}

/// Time series storage backend types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    Memory,
    Redis,
    PostgreSQL,
    HybridRedisPostgres,
    CustomDisk,
}

/// Main time series engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesConfig {
    pub storage_backend: StorageBackend,
    pub redis_config: Option<redis::RedisConfig>,
    pub postgresql_config: Option<postgresql::PostgreSQLConfig>,
    pub memory_limit_mb: u64,
    pub default_retention_policy: RetentionPolicy,
    pub default_compression_policy: CompressionPolicy,
    pub enable_metrics: bool,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
}

impl Default for TimeSeriesConfig {
    fn default() -> Self {
        Self {
            storage_backend: StorageBackend::Memory,
            redis_config: None,
            postgresql_config: None,
            memory_limit_mb: 1024, // 1GB default
            default_retention_policy: RetentionPolicy {
                duration_seconds: 86400 * 30, // 30 days
                downsampling_rules: vec![],
            },
            default_compression_policy: CompressionPolicy {
                compression_type: CompressionType::Delta,
                chunk_size: 1024,
                compress_after_seconds: 3600, // 1 hour
            },
            enable_metrics: true,
            batch_size: 1000,
            flush_interval_ms: 5000, // 5 seconds
        }
    }
}

/// Convert DateTime to nanosecond timestamp
pub fn datetime_to_timestamp(dt: DateTime<Utc>) -> Timestamp {
    dt.timestamp_nanos_opt().unwrap_or(0)
}

/// Convert nanosecond timestamp to DateTime
pub fn timestamp_to_datetime(ts: Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp_nanos(ts)
}