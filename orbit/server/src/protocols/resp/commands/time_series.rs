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

fn get_time_series() -> &'static Arc<RwLock<HashMap<String, TimeSeries>>> {
    TIME_SERIES.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
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

    /// TS.RANGE key fromTimestamp toTimestamp [COUNT count]
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

        // Parse optional arguments
        let mut i = 3;
        while i < args.len() {
            let arg = self.get_string_arg(args, i, "TS.RANGE")?.to_uppercase();
            if arg == "COUNT" {
                i += 1;
                count = Some(self.get_int_arg(args, i, "TS.RANGE")? as usize);
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

        let mut samples = ts.range(from, to);
        if let Some(limit) = count {
            samples.truncate(limit);
        }

        let result: Vec<RespValue> = samples
            .iter()
            .map(|dp| {
                RespValue::Array(vec![
                    RespValue::Integer(dp.timestamp),
                    RespValue::bulk_string_from_str(&dp.value.to_string()),
                ])
            })
            .collect();

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
}
