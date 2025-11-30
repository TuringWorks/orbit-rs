//! Time series command handlers for Redis RESP protocol
//!
//! This module implements RedisTimeSeries-compatible commands:
//! - TS.CREATE - Create a time series
//! - TS.ADD - Add a sample to a time series
//! - TS.GET - Get the last sample
//! - TS.RANGE - Get samples in a time range
//! - TS.MRANGE - Get samples from multiple time series
//! - TS.INFO - Get time series metadata
//! - TS.DEL - Delete samples in a range

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::ProtocolResult;
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Time series configuration
#[derive(Debug, Clone)]
pub struct TimeSeriesConfig {
    /// Name/key of the time series
    pub name: String,
    /// Retention period in milliseconds (0 = no retention)
    pub retention_ms: u64,
    /// Labels for filtering
    pub labels: HashMap<String, String>,
    /// Duplicate policy
    pub duplicate_policy: DuplicatePolicy,
}

/// Policy for handling duplicate timestamps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicatePolicy {
    /// Block duplicates (error)
    Block,
    /// Keep first value
    First,
    /// Keep last value
    Last,
    /// Keep minimum value
    Min,
    /// Keep maximum value
    Max,
    /// Sum values
    Sum,
}

impl DuplicatePolicy {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BLOCK" => Some(Self::Block),
            "FIRST" => Some(Self::First),
            "LAST" => Some(Self::Last),
            "MIN" => Some(Self::Min),
            "MAX" => Some(Self::Max),
            "SUM" => Some(Self::Sum),
            _ => None,
        }
    }
}

/// Aggregation type for time series queries and compaction rules
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationType {
    /// Average of values
    Avg,
    /// Sum of values
    Sum,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// Range (max - min)
    Range,
    /// Count of values
    Count,
    /// First value in bucket
    First,
    /// Last value in bucket
    Last,
    /// Standard deviation (population)
    StdP,
    /// Variance (population)
    VarP,
    /// Time-weighted average
    Twa,
}

impl AggregationType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "AVG" => Some(Self::Avg),
            "SUM" => Some(Self::Sum),
            "MIN" => Some(Self::Min),
            "MAX" => Some(Self::Max),
            "RANGE" => Some(Self::Range),
            "COUNT" => Some(Self::Count),
            "FIRST" => Some(Self::First),
            "LAST" => Some(Self::Last),
            "STD.P" => Some(Self::StdP),
            "VAR.P" => Some(Self::VarP),
            "TWA" => Some(Self::Twa),
            _ => None,
        }
    }

    /// Apply aggregation to a list of values
    fn aggregate(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        match self {
            AggregationType::Avg => values.iter().sum::<f64>() / values.len() as f64,
            AggregationType::Sum => values.iter().sum(),
            AggregationType::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
            AggregationType::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            AggregationType::Range => {
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                max - min
            }
            AggregationType::Count => values.len() as f64,
            AggregationType::First => values[0],
            AggregationType::Last => values[values.len() - 1],
            AggregationType::StdP => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance =
                    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                variance.sqrt()
            }
            AggregationType::VarP => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
            }
            AggregationType::Twa => {
                // Simplified TWA - just return average for now
                values.iter().sum::<f64>() / values.len() as f64
            }
        }
    }
}

/// Compaction rule for automatic downsampling
#[derive(Debug, Clone)]
pub struct CompactionRule {
    /// Source time series key
    pub source_key: String,
    /// Destination time series key
    pub dest_key: String,
    /// Aggregation type
    pub aggregation: AggregationType,
    /// Bucket duration in milliseconds
    pub bucket_duration_ms: i64,
}

/// A single data point in the time series
#[derive(Debug, Clone, Copy)]
struct DataPoint {
    timestamp: i64,
    value: f64,
}

/// Time series storage
struct TimeSeries {
    config: TimeSeriesConfig,
    /// Samples stored in timestamp order (BTreeMap for efficient range queries)
    samples: BTreeMap<i64, f64>,
    /// Total sample count (including deleted)
    total_samples: u64,
}

impl TimeSeries {
    fn new(config: TimeSeriesConfig) -> Self {
        Self {
            config,
            samples: BTreeMap::new(),
            total_samples: 0,
        }
    }

    /// Add a sample with duplicate handling
    fn add_sample(&mut self, timestamp: i64, value: f64) -> Result<(), String> {
        if let Some(existing) = self.samples.get(&timestamp) {
            match self.config.duplicate_policy {
                DuplicatePolicy::Block => {
                    return Err(format!(
                        "TSDB: duplicate timestamp {} already exists",
                        timestamp
                    ));
                }
                DuplicatePolicy::First => {
                    return Ok(()); // Keep existing
                }
                DuplicatePolicy::Last => {
                    self.samples.insert(timestamp, value);
                }
                DuplicatePolicy::Min => {
                    if value < *existing {
                        self.samples.insert(timestamp, value);
                    }
                }
                DuplicatePolicy::Max => {
                    if value > *existing {
                        self.samples.insert(timestamp, value);
                    }
                }
                DuplicatePolicy::Sum => {
                    self.samples.insert(timestamp, existing + value);
                }
            }
        } else {
            self.samples.insert(timestamp, value);
            self.total_samples += 1;
        }

        // Apply retention if configured
        if self.config.retention_ms > 0 {
            let cutoff = timestamp - self.config.retention_ms as i64;
            self.samples.retain(|ts, _| *ts >= cutoff);
        }

        Ok(())
    }

    /// Get samples in a time range
    fn range(&self, from: i64, to: i64) -> Vec<DataPoint> {
        self.samples
            .range(from..=to)
            .map(|(&ts, &val)| DataPoint {
                timestamp: ts,
                value: val,
            })
            .collect()
    }

    /// Get the last sample
    fn last(&self) -> Option<DataPoint> {
        self.samples
            .iter()
            .next_back()
            .map(|(&ts, &val)| DataPoint {
                timestamp: ts,
                value: val,
            })
    }

    /// Delete samples in a range
    fn delete_range(&mut self, from: i64, to: i64) -> usize {
        let to_remove: Vec<i64> = self.samples.range(from..=to).map(|(&ts, _)| ts).collect();
        let count = to_remove.len();
        for ts in to_remove {
            self.samples.remove(&ts);
        }
        count
    }
}

/// Global time series storage
static TIME_SERIES: std::sync::OnceLock<Arc<RwLock<HashMap<String, TimeSeries>>>> =
    std::sync::OnceLock::new();

/// Global compaction rules storage
static COMPACTION_RULES: std::sync::OnceLock<Arc<RwLock<Vec<CompactionRule>>>> =
    std::sync::OnceLock::new();

fn get_time_series() -> &'static Arc<RwLock<HashMap<String, TimeSeries>>> {
    TIME_SERIES.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

fn get_compaction_rules() -> &'static Arc<RwLock<Vec<CompactionRule>>> {
    COMPACTION_RULES.get_or_init(|| Arc::new(RwLock::new(Vec::new())))
}

/// Handler for time series commands
pub struct TimeSeriesCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
}

impl TimeSeriesCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    /// TS.CREATE key [RETENTION retention] [LABELS label value ...] [DUPLICATE_POLICY policy]
    async fn cmd_ts_create(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.create' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "TS.CREATE")?;
        let mut retention_ms = 0u64;
        let mut labels = HashMap::new();
        let mut duplicate_policy = DuplicatePolicy::Block;

        // Parse optional arguments
        let mut i = 1;
        while i < args.len() {
            let arg = self.get_string_arg(args, i, "TS.CREATE")?.to_uppercase();
            match arg.as_str() {
                "RETENTION" => {
                    i += 1;
                    retention_ms = self.get_int_arg(args, i, "TS.CREATE")? as u64;
                }
                "LABELS" => {
                    i += 1;
                    // Collect label-value pairs
                    while i + 1 < args.len() {
                        let label_arg = self.get_string_arg(args, i, "TS.CREATE")?;
                        // Check if this is another keyword
                        if label_arg.to_uppercase() == "RETENTION"
                            || label_arg.to_uppercase() == "DUPLICATE_POLICY"
                        {
                            i -= 1;
                            break;
                        }
                        let label = label_arg;
                        let value = self.get_string_arg(args, i + 1, "TS.CREATE")?;
                        labels.insert(label, value);
                        i += 2;
                    }
                    continue; // Don't increment i again
                }
                "DUPLICATE_POLICY" => {
                    i += 1;
                    let policy_str = self.get_string_arg(args, i, "TS.CREATE")?;
                    duplicate_policy = DuplicatePolicy::from_str(&policy_str).ok_or_else(|| {
                        crate::protocols::error::ProtocolError::RespError(format!(
                            "ERR unknown duplicate policy '{}'",
                            policy_str
                        ))
                    })?;
                }
                _ => {}
            }
            i += 1;
        }

        let config = TimeSeriesConfig {
            name: key.clone(),
            retention_ms,
            labels,
            duplicate_policy,
        };

        let ts_storage = get_time_series();
        let mut ts_guard = ts_storage.write().await;

        if ts_guard.contains_key(&key) {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: key '{}' already exists",
                key
            )));
        }

        ts_guard.insert(key.clone(), TimeSeries::new(config));

        info!(
            "Created time series '{}' with retention={}ms",
            key, retention_ms
        );
        Ok(RespValue::ok())
    }

    /// TS.ADD key timestamp value [RETENTION retention] [LABELS label value ...]
    async fn cmd_ts_add(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.add' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "TS.ADD")?;
        let timestamp_str = self.get_string_arg(args, 1, "TS.ADD")?;
        let value = self.get_float_arg(args, 2, "TS.ADD")?;

        // Handle "*" for current timestamp
        let timestamp = if timestamp_str == "*" {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        } else {
            timestamp_str.parse::<i64>().map_err(|_| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR invalid timestamp '{}'",
                    timestamp_str
                ))
            })?
        };

        let ts_storage = get_time_series();
        let mut ts_guard = ts_storage.write().await;

        // Auto-create time series if it doesn't exist
        if !ts_guard.contains_key(&key) {
            let config = TimeSeriesConfig {
                name: key.clone(),
                retention_ms: 0,
                labels: HashMap::new(),
                duplicate_policy: DuplicatePolicy::Last,
            };
            ts_guard.insert(key.clone(), TimeSeries::new(config));
        }

        let ts = ts_guard.get_mut(&key).unwrap();
        ts.add_sample(timestamp, value)
            .map_err(|e| crate::protocols::error::ProtocolError::RespError(format!("ERR {}", e)))?;

        debug!("TS.ADD {} {} {} -> OK", key, timestamp, value);
        Ok(RespValue::Integer(timestamp))
    }

    /// TS.GET key
    async fn cmd_ts_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("TS.GET", args, 1)?;

        let key = self.get_string_arg(args, 0, "TS.GET")?;

        let ts_storage = get_time_series();
        let ts_guard = ts_storage.read().await;

        let ts = ts_guard.get(&key).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: key '{}' does not exist",
                key
            ))
        })?;

        if let Some(dp) = ts.last() {
            Ok(RespValue::Array(vec![
                RespValue::Integer(dp.timestamp),
                RespValue::bulk_string_from_str(&dp.value.to_string()),
            ]))
        } else {
            Ok(RespValue::Array(vec![]))
        }
    }

    /// TS.RANGE key fromTimestamp toTimestamp [COUNT count] [AGGREGATION aggregationType bucketDuration]
    async fn cmd_ts_range(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.range' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "TS.RANGE")?;
        let from = self.parse_timestamp_arg(args, 1, "TS.RANGE")?;
        let to = self.parse_timestamp_arg(args, 2, "TS.RANGE")?;

        let mut count: Option<usize> = None;
        let mut aggregation: Option<AggregationType> = None;
        let mut bucket_duration: Option<i64> = None;

        // Parse optional arguments
        let mut i = 3;
        while i < args.len() {
            let arg = self.get_string_arg(args, i, "TS.RANGE")?.to_uppercase();
            match arg.as_str() {
                "COUNT" => {
                    i += 1;
                    count = Some(self.get_int_arg(args, i, "TS.RANGE")? as usize);
                }
                "AGGREGATION" => {
                    i += 1;
                    let agg_type_str = self.get_string_arg(args, i, "TS.RANGE")?;
                    aggregation =
                        Some(AggregationType::from_str(&agg_type_str).ok_or_else(|| {
                            crate::protocols::error::ProtocolError::RespError(format!(
                                "ERR unknown aggregation type '{}'",
                                agg_type_str
                            ))
                        })?);
                    i += 1;
                    bucket_duration = Some(self.get_int_arg(args, i, "TS.RANGE")?);
                }
                _ => {}
            }
            i += 1;
        }

        let ts_storage = get_time_series();
        let ts_guard = ts_storage.read().await;

        let ts = ts_guard.get(&key).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: key '{}' does not exist",
                key
            ))
        })?;

        let samples = ts.range(from, to);

        // Apply aggregation if specified
        let result: Vec<RespValue> =
            if let (Some(agg), Some(bucket_ms)) = (aggregation, bucket_duration) {
                // Group samples into buckets and aggregate
                let mut buckets: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
                for dp in &samples {
                    let bucket_start = (dp.timestamp / bucket_ms) * bucket_ms;
                    buckets.entry(bucket_start).or_default().push(dp.value);
                }

                let mut aggregated: Vec<RespValue> = buckets
                    .iter()
                    .map(|(&bucket_ts, values)| {
                        let agg_value = agg.aggregate(values);
                        RespValue::Array(vec![
                            RespValue::Integer(bucket_ts),
                            RespValue::bulk_string_from_str(&agg_value.to_string()),
                        ])
                    })
                    .collect();

                if let Some(limit) = count {
                    aggregated.truncate(limit);
                }
                aggregated
            } else {
                // No aggregation - return raw samples
                let mut raw: Vec<RespValue> = samples
                    .iter()
                    .map(|dp| {
                        RespValue::Array(vec![
                            RespValue::Integer(dp.timestamp),
                            RespValue::bulk_string_from_str(&dp.value.to_string()),
                        ])
                    })
                    .collect();

                if let Some(limit) = count {
                    raw.truncate(limit);
                }
                raw
            };

        debug!(
            "TS.RANGE {} {} {} -> {} samples",
            key,
            from,
            to,
            result.len()
        );
        Ok(RespValue::Array(result))
    }

    /// TS.MRANGE fromTimestamp toTimestamp FILTER filter...
    async fn cmd_ts_mrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 4 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.mrange' command".to_string(),
            ));
        }

        let from = self.parse_timestamp_arg(args, 0, "TS.MRANGE")?;
        let to = self.parse_timestamp_arg(args, 1, "TS.MRANGE")?;

        // Parse FILTER clause
        let mut filters: Vec<(String, String)> = Vec::new();
        let mut i = 2;
        while i < args.len() {
            let arg = self.get_string_arg(args, i, "TS.MRANGE")?.to_uppercase();
            if arg == "FILTER" {
                i += 1;
                while i < args.len() {
                    let filter_str = self.get_string_arg(args, i, "TS.MRANGE")?;
                    // Parse label=value format
                    if let Some((label, value)) = filter_str.split_once('=') {
                        filters.push((label.to_string(), value.to_string()));
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        let ts_storage = get_time_series();
        let ts_guard = ts_storage.read().await;

        let mut result = Vec::new();

        for (key, ts) in ts_guard.iter() {
            // Check if time series matches all filters
            let matches = filters.iter().all(|(label, value)| {
                ts.config
                    .labels
                    .get(label)
                    .map(|v| v == value)
                    .unwrap_or(false)
            });

            if matches || filters.is_empty() {
                let samples = ts.range(from, to);
                let samples_resp: Vec<RespValue> = samples
                    .iter()
                    .map(|dp| {
                        RespValue::Array(vec![
                            RespValue::Integer(dp.timestamp),
                            RespValue::bulk_string_from_str(&dp.value.to_string()),
                        ])
                    })
                    .collect();

                // Build labels array
                let labels_resp: Vec<RespValue> = ts
                    .config
                    .labels
                    .iter()
                    .flat_map(|(k, v)| {
                        vec![
                            RespValue::bulk_string_from_str(k),
                            RespValue::bulk_string_from_str(v),
                        ]
                    })
                    .collect();

                result.push(RespValue::Array(vec![
                    RespValue::bulk_string_from_str(key),
                    RespValue::Array(labels_resp),
                    RespValue::Array(samples_resp),
                ]));
            }
        }

        debug!("TS.MRANGE {} {} -> {} series", from, to, result.len());
        Ok(RespValue::Array(result))
    }

    /// TS.INFO key
    async fn cmd_ts_info(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("TS.INFO", args, 1)?;

        let key = self.get_string_arg(args, 0, "TS.INFO")?;

        let ts_storage = get_time_series();
        let ts_guard = ts_storage.read().await;

        let ts = ts_guard.get(&key).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: key '{}' does not exist",
                key
            ))
        })?;

        let first_timestamp = ts.samples.keys().next().copied().unwrap_or(0);
        let last_timestamp = ts.samples.keys().next_back().copied().unwrap_or(0);

        let labels: Vec<RespValue> = ts
            .config
            .labels
            .iter()
            .flat_map(|(k, v)| {
                vec![
                    RespValue::bulk_string_from_str(k),
                    RespValue::bulk_string_from_str(v),
                ]
            })
            .collect();

        Ok(RespValue::Array(vec![
            RespValue::bulk_string_from_str("totalSamples"),
            RespValue::Integer(ts.total_samples as i64),
            RespValue::bulk_string_from_str("memoryUsage"),
            RespValue::Integer((ts.samples.len() * 16) as i64), // Approximate
            RespValue::bulk_string_from_str("firstTimestamp"),
            RespValue::Integer(first_timestamp),
            RespValue::bulk_string_from_str("lastTimestamp"),
            RespValue::Integer(last_timestamp),
            RespValue::bulk_string_from_str("retentionTime"),
            RespValue::Integer(ts.config.retention_ms as i64),
            RespValue::bulk_string_from_str("chunkCount"),
            RespValue::Integer(1), // Single chunk for now
            RespValue::bulk_string_from_str("labels"),
            RespValue::Array(labels),
        ]))
    }

    /// TS.DEL key fromTimestamp toTimestamp
    async fn cmd_ts_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.del' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "TS.DEL")?;
        let from = self.parse_timestamp_arg(args, 1, "TS.DEL")?;
        let to = self.parse_timestamp_arg(args, 2, "TS.DEL")?;

        let ts_storage = get_time_series();
        let mut ts_guard = ts_storage.write().await;

        let ts = ts_guard.get_mut(&key).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: key '{}' does not exist",
                key
            ))
        })?;

        let deleted = ts.delete_range(from, to);
        debug!("TS.DEL {} {} {} -> {} deleted", key, from, to, deleted);

        Ok(RespValue::Integer(deleted as i64))
    }

    /// TS.MADD key timestamp value [key timestamp value ...]
    async fn cmd_ts_madd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 || args.len() % 3 != 0 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.madd' command".to_string(),
            ));
        }

        let ts_storage = get_time_series();
        let mut ts_guard = ts_storage.write().await;

        let mut results = Vec::new();
        let mut i = 0;

        while i + 2 < args.len() {
            let key = self.get_string_arg(args, i, "TS.MADD")?;
            let timestamp_str = self.get_string_arg(args, i + 1, "TS.MADD")?;
            let value = self.get_float_arg(args, i + 2, "TS.MADD")?;

            let timestamp = if timestamp_str == "*" {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0)
            } else {
                timestamp_str.parse::<i64>().unwrap_or(0)
            };

            // Auto-create time series if needed
            if !ts_guard.contains_key(&key) {
                let config = TimeSeriesConfig {
                    name: key.clone(),
                    retention_ms: 0,
                    labels: HashMap::new(),
                    duplicate_policy: DuplicatePolicy::Last,
                };
                ts_guard.insert(key.clone(), TimeSeries::new(config));
            }

            if let Some(ts) = ts_guard.get_mut(&key) {
                match ts.add_sample(timestamp, value) {
                    Ok(_) => results.push(RespValue::Integer(timestamp)),
                    Err(e) => results.push(RespValue::Error(e)),
                }
            }

            i += 3;
        }

        Ok(RespValue::Array(results))
    }

    /// TS.CREATERULE sourceKey destKey AGGREGATION aggregationType bucketDuration
    async fn cmd_ts_createrule(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 5 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.createrule' command".to_string(),
            ));
        }

        let source_key = self.get_string_arg(args, 0, "TS.CREATERULE")?;
        let dest_key = self.get_string_arg(args, 1, "TS.CREATERULE")?;

        // Parse AGGREGATION keyword
        let agg_keyword = self
            .get_string_arg(args, 2, "TS.CREATERULE")?
            .to_uppercase();
        if agg_keyword != "AGGREGATION" {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR syntax error, expected AGGREGATION".to_string(),
            ));
        }

        let agg_type_str = self.get_string_arg(args, 3, "TS.CREATERULE")?;
        let aggregation = AggregationType::from_str(&agg_type_str).ok_or_else(|| {
            crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown aggregation type '{}'",
                agg_type_str
            ))
        })?;

        let bucket_duration_ms = self.get_int_arg(args, 4, "TS.CREATERULE")?;

        // Verify source time series exists
        let ts_storage = get_time_series();
        let ts_guard = ts_storage.read().await;
        if !ts_guard.contains_key(&source_key) {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: source key '{}' does not exist",
                source_key
            )));
        }
        if !ts_guard.contains_key(&dest_key) {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: destination key '{}' does not exist",
                dest_key
            )));
        }
        drop(ts_guard);

        // Check for duplicate rule
        let rules_storage = get_compaction_rules();
        let mut rules_guard = rules_storage.write().await;

        let rule_exists = rules_guard
            .iter()
            .any(|r| r.source_key == source_key && r.dest_key == dest_key);

        if rule_exists {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: rule already exists from '{}' to '{}'",
                source_key, dest_key
            )));
        }

        let rule = CompactionRule {
            source_key: source_key.clone(),
            dest_key: dest_key.clone(),
            aggregation,
            bucket_duration_ms,
        };

        rules_guard.push(rule);

        info!(
            "Created compaction rule: {} -> {} (AGGREGATION {:?} {}ms)",
            source_key, dest_key, aggregation, bucket_duration_ms
        );
        Ok(RespValue::ok())
    }

    /// TS.DELETERULE sourceKey destKey
    async fn cmd_ts_deleterule(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'ts.deleterule' command".to_string(),
            ));
        }

        let source_key = self.get_string_arg(args, 0, "TS.DELETERULE")?;
        let dest_key = self.get_string_arg(args, 1, "TS.DELETERULE")?;

        let rules_storage = get_compaction_rules();
        let mut rules_guard = rules_storage.write().await;

        let initial_len = rules_guard.len();
        rules_guard.retain(|r| !(r.source_key == source_key && r.dest_key == dest_key));

        if rules_guard.len() == initial_len {
            return Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR TSDB: rule from '{}' to '{}' does not exist",
                source_key, dest_key
            )));
        }

        info!("Deleted compaction rule: {} -> {}", source_key, dest_key);
        Ok(RespValue::ok())
    }

    /// Parse a timestamp argument (handles "-" for min, "+" for max)
    fn parse_timestamp_arg(
        &self,
        args: &[RespValue],
        index: usize,
        cmd: &str,
    ) -> ProtocolResult<i64> {
        let ts_str = self.get_string_arg(args, index, cmd)?;
        match ts_str.as_str() {
            "-" => Ok(i64::MIN),
            "+" => Ok(i64::MAX),
            _ => ts_str.parse::<i64>().map_err(|_| {
                crate::protocols::error::ProtocolError::RespError(format!(
                    "ERR invalid timestamp '{}'",
                    ts_str
                ))
            }),
        }
    }
}

#[async_trait]
impl CommandHandler for TimeSeriesCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name.to_uppercase().as_str() {
            "TS.CREATE" => self.cmd_ts_create(args).await,
            "TS.ADD" => self.cmd_ts_add(args).await,
            "TS.GET" => self.cmd_ts_get(args).await,
            "TS.RANGE" => self.cmd_ts_range(args).await,
            "TS.MRANGE" => self.cmd_ts_mrange(args).await,
            "TS.INFO" => self.cmd_ts_info(args).await,
            "TS.DEL" => self.cmd_ts_del(args).await,
            "TS.MADD" => self.cmd_ts_madd(args).await,
            "TS.CREATERULE" => self.cmd_ts_createrule(args).await,
            "TS.DELETERULE" => self.cmd_ts_deleterule(args).await,
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown time series command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "TS.CREATE",
            "TS.ADD",
            "TS.GET",
            "TS.RANGE",
            "TS.MRANGE",
            "TS.INFO",
            "TS.DEL",
            "TS.MADD",
            "TS.CREATERULE",
            "TS.DELETERULE",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_series_add_get() {
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 0,
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let mut ts = TimeSeries::new(config);

        ts.add_sample(1000, 42.0).unwrap();
        ts.add_sample(2000, 43.0).unwrap();
        ts.add_sample(3000, 44.0).unwrap();

        let last = ts.last().unwrap();
        assert_eq!(last.timestamp, 3000);
        assert!((last.value - 44.0).abs() < 0.001);
    }

    #[test]
    fn test_time_series_range() {
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 0,
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let mut ts = TimeSeries::new(config);

        ts.add_sample(1000, 1.0).unwrap();
        ts.add_sample(2000, 2.0).unwrap();
        ts.add_sample(3000, 3.0).unwrap();
        ts.add_sample(4000, 4.0).unwrap();

        let range = ts.range(1500, 3500);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].timestamp, 2000);
        assert_eq!(range[1].timestamp, 3000);
    }

    #[test]
    fn test_duplicate_policy_last() {
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 0,
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let mut ts = TimeSeries::new(config);

        ts.add_sample(1000, 10.0).unwrap();
        ts.add_sample(1000, 20.0).unwrap(); // Duplicate

        let last = ts.last().unwrap();
        assert!((last.value - 20.0).abs() < 0.001); // Should be last value
    }

    #[test]
    fn test_duplicate_policy_sum() {
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 0,
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Sum,
        };
        let mut ts = TimeSeries::new(config);

        ts.add_sample(1000, 10.0).unwrap();
        ts.add_sample(1000, 20.0).unwrap(); // Duplicate

        let last = ts.last().unwrap();
        assert!((last.value - 30.0).abs() < 0.001); // Should be sum
    }

    #[test]
    fn test_retention() {
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 2000, // 2 second retention
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let mut ts = TimeSeries::new(config);

        ts.add_sample(1000, 1.0).unwrap();
        ts.add_sample(2000, 2.0).unwrap();
        ts.add_sample(4000, 4.0).unwrap(); // Should trigger retention cleanup

        // Sample at 1000 should be removed (4000 - 2000 = 2000 cutoff)
        assert_eq!(ts.samples.len(), 2);
        assert!(!ts.samples.contains_key(&1000));
    }

    #[test]
    fn test_aggregation_avg() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = AggregationType::Avg.aggregate(&values);
        assert!((result - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregation_sum() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = AggregationType::Sum.aggregate(&values);
        assert!((result - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregation_min_max() {
        let values = vec![3.0, 1.0, 4.0, 1.5, 9.0, 2.6];
        assert!((AggregationType::Min.aggregate(&values) - 1.0).abs() < 0.001);
        assert!((AggregationType::Max.aggregate(&values) - 9.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregation_range() {
        let values = vec![3.0, 1.0, 4.0, 1.5, 9.0, 2.6];
        let result = AggregationType::Range.aggregate(&values);
        assert!((result - 8.0).abs() < 0.001); // 9.0 - 1.0
    }

    #[test]
    fn test_aggregation_count() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = AggregationType::Count.aggregate(&values);
        assert!((result - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregation_first_last() {
        let values = vec![10.0, 20.0, 30.0, 40.0];
        assert!((AggregationType::First.aggregate(&values) - 10.0).abs() < 0.001);
        assert!((AggregationType::Last.aggregate(&values) - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregation_std_var() {
        // For values [2, 4, 4, 4, 5, 5, 7, 9], mean=5, variance=4, std=2
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let var = AggregationType::VarP.aggregate(&values);
        let std = AggregationType::StdP.aggregate(&values);
        assert!((var - 4.0).abs() < 0.001);
        assert!((std - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_aggregation_type_parsing() {
        assert_eq!(AggregationType::from_str("AVG"), Some(AggregationType::Avg));
        assert_eq!(AggregationType::from_str("avg"), Some(AggregationType::Avg));
        assert_eq!(AggregationType::from_str("SUM"), Some(AggregationType::Sum));
        assert_eq!(AggregationType::from_str("MIN"), Some(AggregationType::Min));
        assert_eq!(AggregationType::from_str("MAX"), Some(AggregationType::Max));
        assert_eq!(
            AggregationType::from_str("COUNT"),
            Some(AggregationType::Count)
        );
        assert_eq!(
            AggregationType::from_str("FIRST"),
            Some(AggregationType::First)
        );
        assert_eq!(
            AggregationType::from_str("LAST"),
            Some(AggregationType::Last)
        );
        assert_eq!(
            AggregationType::from_str("RANGE"),
            Some(AggregationType::Range)
        );
        assert_eq!(
            AggregationType::from_str("STD.P"),
            Some(AggregationType::StdP)
        );
        assert_eq!(
            AggregationType::from_str("VAR.P"),
            Some(AggregationType::VarP)
        );
        assert_eq!(AggregationType::from_str("INVALID"), None);
    }

    #[test]
    fn test_aggregation_empty_values() {
        let values: Vec<f64> = vec![];
        // All aggregations should return 0 for empty input
        assert!((AggregationType::Avg.aggregate(&values) - 0.0).abs() < 0.001);
        assert!((AggregationType::Sum.aggregate(&values) - 0.0).abs() < 0.001);
        assert!((AggregationType::Count.aggregate(&values) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_compaction_rule_creation() {
        let rule = CompactionRule {
            source_key: "temperature:raw".to_string(),
            dest_key: "temperature:hourly".to_string(),
            aggregation: AggregationType::Avg,
            bucket_duration_ms: 3600000,
        };

        assert_eq!(rule.source_key, "temperature:raw");
        assert_eq!(rule.dest_key, "temperature:hourly");
        assert_eq!(rule.aggregation, AggregationType::Avg);
        assert_eq!(rule.bucket_duration_ms, 3600000);
    }

    #[test]
    fn test_aggregation_bucketing() {
        // Test the bucketing logic used in TS.RANGE with AGGREGATION
        let bucket_ms: i64 = 1000; // 1 second buckets

        // Samples at various timestamps
        let samples = vec![
            (100, 10.0),  // bucket 0
            (500, 20.0),  // bucket 0
            (1100, 30.0), // bucket 1000
            (1500, 40.0), // bucket 1000
            (2100, 50.0), // bucket 2000
        ];

        // Group into buckets
        let mut buckets: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
        for (ts, val) in &samples {
            let bucket_start = (ts / bucket_ms) * bucket_ms;
            buckets.entry(bucket_start).or_default().push(*val);
        }

        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets.get(&0).unwrap(), &vec![10.0, 20.0]);
        assert_eq!(buckets.get(&1000).unwrap(), &vec![30.0, 40.0]);
        assert_eq!(buckets.get(&2000).unwrap(), &vec![50.0]);

        // Test aggregation on buckets
        let bucket_0_avg = AggregationType::Avg.aggregate(buckets.get(&0).unwrap());
        assert!((bucket_0_avg - 15.0).abs() < 0.001);

        let bucket_1000_avg = AggregationType::Avg.aggregate(buckets.get(&1000).unwrap());
        assert!((bucket_1000_avg - 35.0).abs() < 0.001);
    }

    #[test]
    fn test_time_series_range_with_aggregation_logic() {
        // Test the full range + aggregation flow
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 0,
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let mut ts = TimeSeries::new(config);

        // Add samples across multiple hour buckets (3600000ms = 1 hour)
        // Hour 0: 0-3599999
        ts.add_sample(0, 10.0).unwrap();
        ts.add_sample(1800000, 20.0).unwrap(); // 30 min
                                               // Hour 1: 3600000-7199999
        ts.add_sample(3600000, 30.0).unwrap();
        ts.add_sample(5400000, 40.0).unwrap(); // 1.5 hours
                                               // Hour 2: 7200000-10799999
        ts.add_sample(7200000, 50.0).unwrap();

        let samples = ts.range(0, 10000000);
        assert_eq!(samples.len(), 5);

        // Aggregate with 1-hour buckets
        let bucket_ms: i64 = 3600000;
        let mut buckets: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
        for dp in &samples {
            let bucket_start = (dp.timestamp / bucket_ms) * bucket_ms;
            buckets.entry(bucket_start).or_default().push(dp.value);
        }

        assert_eq!(buckets.len(), 3);

        // Hour 0 avg: (10 + 20) / 2 = 15
        let hour0_avg = AggregationType::Avg.aggregate(buckets.get(&0).unwrap());
        assert!((hour0_avg - 15.0).abs() < 0.001);

        // Hour 1 avg: (30 + 40) / 2 = 35
        let hour1_avg = AggregationType::Avg.aggregate(buckets.get(&3600000).unwrap());
        assert!((hour1_avg - 35.0).abs() < 0.001);

        // Hour 2 avg: 50
        let hour2_avg = AggregationType::Avg.aggregate(buckets.get(&7200000).unwrap());
        assert!((hour2_avg - 50.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_ts_create_and_add_via_storage() {
        // Test time series storage directly
        let ts_storage = get_time_series();
        let mut ts_guard = ts_storage.write().await;

        let key = format!(
            "test_ts_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let config = TimeSeriesConfig {
            name: key.clone(),
            retention_ms: 86400000, // 1 day
            labels: {
                let mut labels = HashMap::new();
                labels.insert("location".to_string(), "office".to_string());
                labels.insert("sensor".to_string(), "temperature".to_string());
                labels
            },
            duplicate_policy: DuplicatePolicy::Last,
        };

        ts_guard.insert(key.clone(), TimeSeries::new(config));
        assert!(ts_guard.contains_key(&key));

        let ts = ts_guard.get_mut(&key).unwrap();
        ts.add_sample(1000, 23.5).unwrap();
        ts.add_sample(2000, 24.0).unwrap();
        ts.add_sample(3000, 23.8).unwrap();

        assert_eq!(ts.samples.len(), 3);

        let last = ts.last().unwrap();
        assert_eq!(last.timestamp, 3000);
        assert!((last.value - 23.8).abs() < 0.001);

        // Cleanup
        ts_guard.remove(&key);
    }

    #[tokio::test]
    async fn test_compaction_rules_storage() {
        let rules_storage = get_compaction_rules();
        let mut rules_guard = rules_storage.write().await;

        let initial_count = rules_guard.len();

        // Add a test rule
        let test_rule = CompactionRule {
            source_key: "test_source_unique".to_string(),
            dest_key: "test_dest_unique".to_string(),
            aggregation: AggregationType::Avg,
            bucket_duration_ms: 3600000,
        };

        rules_guard.push(test_rule);
        assert_eq!(rules_guard.len(), initial_count + 1);

        // Find the rule
        let found = rules_guard
            .iter()
            .find(|r| r.source_key == "test_source_unique" && r.dest_key == "test_dest_unique");
        assert!(found.is_some());
        assert_eq!(found.unwrap().aggregation, AggregationType::Avg);

        // Remove the rule
        rules_guard.retain(|r| {
            !(r.source_key == "test_source_unique" && r.dest_key == "test_dest_unique")
        });
        assert_eq!(rules_guard.len(), initial_count);
    }

    #[test]
    fn test_time_series_delete_range() {
        let config = TimeSeriesConfig {
            name: "test".to_string(),
            retention_ms: 0,
            labels: HashMap::new(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let mut ts = TimeSeries::new(config);

        ts.add_sample(1000, 1.0).unwrap();
        ts.add_sample(2000, 2.0).unwrap();
        ts.add_sample(3000, 3.0).unwrap();
        ts.add_sample(4000, 4.0).unwrap();
        ts.add_sample(5000, 5.0).unwrap();

        assert_eq!(ts.samples.len(), 5);

        // Delete samples between 2000 and 4000 (inclusive)
        let deleted = ts.delete_range(2000, 4000);
        assert_eq!(deleted, 3); // 2000, 3000, 4000

        assert_eq!(ts.samples.len(), 2);
        assert!(ts.samples.contains_key(&1000));
        assert!(ts.samples.contains_key(&5000));
        assert!(!ts.samples.contains_key(&3000));
    }

    #[test]
    fn test_time_series_with_labels() {
        let mut labels = HashMap::new();
        labels.insert("location".to_string(), "office".to_string());
        labels.insert("floor".to_string(), "3".to_string());

        let config = TimeSeriesConfig {
            name: "temp:office:3".to_string(),
            retention_ms: 0,
            labels: labels.clone(),
            duplicate_policy: DuplicatePolicy::Last,
        };
        let ts = TimeSeries::new(config);

        assert_eq!(
            ts.config.labels.get("location"),
            Some(&"office".to_string())
        );
        assert_eq!(ts.config.labels.get("floor"), Some(&"3".to_string()));
        assert_eq!(ts.config.labels.len(), 2);
    }
}
