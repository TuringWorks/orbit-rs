//! Time Series Module
//!
//! Provides Redis TimeSeries-compatible functionality for storing and querying
//! time series data with automatic aggregation, compaction rules, and label filtering.

use async_trait::async_trait;
use orbit_shared::{Addressable, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single time series data point
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: u64, // Unix timestamp in milliseconds
    pub value: f64,
}

impl Sample {
    pub fn new(timestamp: u64, value: f64) -> Self {
        Self { timestamp, value }
    }

    pub fn now(value: f64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self::new(timestamp, value)
    }
}

/// Time series aggregation functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationType {
    Avg,   // Average
    Sum,   // Sum
    Min,   // Minimum
    Max,   // Maximum
    Count, // Count
    First, // First value
    Last,  // Last value
    Range, // Max - Min
    Std,   // Standard deviation
}

impl AggregationType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "AVG" | "AVERAGE" => Some(Self::Avg),
            "SUM" => Some(Self::Sum),
            "MIN" | "MINIMUM" => Some(Self::Min),
            "MAX" | "MAXIMUM" => Some(Self::Max),
            "COUNT" => Some(Self::Count),
            "FIRST" => Some(Self::First),
            "LAST" => Some(Self::Last),
            "RANGE" => Some(Self::Range),
            "STD" | "STDDEV" => Some(Self::Std),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Avg => "avg",
            Self::Sum => "sum",
            Self::Min => "min",
            Self::Max => "max",
            Self::Count => "count",
            Self::First => "first",
            Self::Last => "last",
            Self::Range => "range",
            Self::Std => "std",
        }
    }

    /// Apply aggregation to a set of samples
    pub fn aggregate(&self, samples: &[Sample]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }

        match self {
            Self::Avg => {
                let sum: f64 = samples.iter().map(|s| s.value).sum();
                sum / samples.len() as f64
            }
            Self::Sum => samples.iter().map(|s| s.value).sum(),
            Self::Min => samples
                .iter()
                .map(|s| s.value)
                .fold(f64::INFINITY, f64::min),
            Self::Max => samples
                .iter()
                .map(|s| s.value)
                .fold(f64::NEG_INFINITY, f64::max),
            Self::Count => samples.len() as f64,
            Self::First => samples.first().map(|s| s.value).unwrap_or(0.0),
            Self::Last => samples.last().map(|s| s.value).unwrap_or(0.0),
            Self::Range => {
                let min = samples
                    .iter()
                    .map(|s| s.value)
                    .fold(f64::INFINITY, f64::min);
                let max = samples
                    .iter()
                    .map(|s| s.value)
                    .fold(f64::NEG_INFINITY, f64::max);
                max - min
            }
            Self::Std => {
                if samples.len() <= 1 {
                    return 0.0;
                }
                let mean = samples.iter().map(|s| s.value).sum::<f64>() / samples.len() as f64;
                let variance = samples
                    .iter()
                    .map(|s| (s.value - mean).powi(2))
                    .sum::<f64>()
                    / samples.len() as f64;
                variance.sqrt()
            }
        }
    }
}

/// Compaction rule for automatic downsampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRule {
    pub dest_key: String,
    pub bucket_duration: u64, // Duration in milliseconds
    pub aggregation: AggregationType,
    pub retention: Option<u64>, // Optional retention for destination series
}

/// Time series configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesConfig {
    pub retention: Option<u64>,            // Retention time in milliseconds
    pub chunk_size: Option<usize>,         // Maximum samples per chunk
    pub duplicate_policy: DuplicatePolicy, // How to handle duplicate timestamps
    pub labels: HashMap<String, String>,   // Key-value labels for the series
    pub encoding: DataEncoding,            // Data encoding type
}

impl Default for TimeSeriesConfig {
    fn default() -> Self {
        Self {
            retention: None,
            chunk_size: Some(4096),
            duplicate_policy: DuplicatePolicy::Block,
            labels: HashMap::new(),
            encoding: DataEncoding::Compressed,
        }
    }
}

/// Policy for handling duplicate timestamps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplicatePolicy {
    Block, // Block duplicate timestamps (default)
    First, // Keep first value
    Last,  // Keep last value
    Min,   // Keep minimum value
    Max,   // Keep maximum value
    Sum,   // Sum values
}

impl DuplicatePolicy {
    pub fn parse(s: &str) -> Option<Self> {
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

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::First => "first",
            Self::Last => "last",
            Self::Min => "min",
            Self::Max => "max",
            Self::Sum => "sum",
        }
    }
}

/// Data encoding types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataEncoding {
    Compressed,
    Uncompressed,
}

impl DataEncoding {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "COMPRESSED" => Some(Self::Compressed),
            "UNCOMPRESSED" => Some(Self::Uncompressed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compressed => "compressed",
            Self::Uncompressed => "uncompressed",
        }
    }
}

/// Time series actor for managing time-based data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesActor {
    /// Time series configuration
    pub config: TimeSeriesConfig,
    /// Ordered samples (timestamp -> value)
    pub samples: BTreeMap<u64, f64>,
    /// Compaction rules for automatic downsampling
    pub compaction_rules: Vec<CompactionRule>,
    /// Series creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
}

impl Default for TimeSeriesActor {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeSeriesActor {
    /// Create a new empty time series
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            config: TimeSeriesConfig::default(),
            samples: BTreeMap::new(),
            compaction_rules: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create time series with custom configuration
    pub fn with_config(config: TimeSeriesConfig) -> Self {
        let mut ts = Self::new();
        ts.config = config;
        ts
    }

    /// Add a sample to the time series
    pub fn add_sample(&mut self, timestamp: u64, value: f64) -> Result<(), String> {
        // Check for duplicate timestamps
        if self.samples.contains_key(&timestamp) {
            match self.config.duplicate_policy {
                DuplicatePolicy::Block => {
                    return Err("Duplicate timestamp not allowed".to_string());
                }
                DuplicatePolicy::First => {
                    // Keep existing value
                    return Ok(());
                }
                DuplicatePolicy::Last => {
                    // Replace with new value
                    self.samples.insert(timestamp, value);
                }
                DuplicatePolicy::Min => {
                    let existing = self.samples.get(&timestamp).unwrap();
                    self.samples.insert(timestamp, existing.min(value));
                }
                DuplicatePolicy::Max => {
                    let existing = self.samples.get(&timestamp).unwrap();
                    self.samples.insert(timestamp, existing.max(value));
                }
                DuplicatePolicy::Sum => {
                    let existing = self.samples.get(&timestamp).unwrap();
                    self.samples.insert(timestamp, existing + value);
                }
            }
        } else {
            self.samples.insert(timestamp, value);
        }

        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Apply retention policy
        self.apply_retention();

        // Apply compaction rules
        self.apply_compaction_rules(timestamp, value);

        Ok(())
    }

    /// Add multiple samples
    pub fn add_samples(&mut self, samples: Vec<Sample>) -> Result<Vec<String>, String> {
        let mut errors = Vec::new();

        for sample in samples {
            if let Err(e) = self.add_sample(sample.timestamp, sample.value) {
                errors.push(format!("{}:{}: {}", sample.timestamp, sample.value, e));
            }
        }

        Ok(errors)
    }

    /// Increment/decrement the latest sample or create new one
    pub fn increment_by(&mut self, timestamp: u64, increment: f64) -> Result<f64, String> {
        // If timestamp is provided and exists, increment that value
        if let Some(existing_value) = self.samples.get(&timestamp) {
            let new_value = existing_value + increment;
            self.samples.insert(timestamp, new_value);
            return Ok(new_value);
        }

        // Otherwise, find the latest sample
        if let Some((&latest_ts, &latest_value)) = self.samples.iter().next_back() {
            // If timestamp is newer than latest, create new sample
            if timestamp > latest_ts {
                let new_value = increment; // Start from increment for new timestamp
                self.add_sample(timestamp, new_value)?;
                Ok(new_value)
            } else {
                // Increment the latest sample
                let new_value = latest_value + increment;
                self.samples.insert(latest_ts, new_value);
                Ok(new_value)
            }
        } else {
            // No samples exist, create first sample
            self.add_sample(timestamp, increment)?;
            Ok(increment)
        }
    }

    /// Get the latest sample
    pub fn get_latest(&self) -> Option<Sample> {
        self.samples
            .iter()
            .next_back()
            .map(|(&timestamp, &value)| Sample::new(timestamp, value))
    }

    /// Get samples within a timestamp range
    pub fn get_range(&self, from_ts: u64, to_ts: u64) -> Vec<Sample> {
        self.samples
            .range(from_ts..=to_ts)
            .map(|(&timestamp, &value)| Sample::new(timestamp, value))
            .collect()
    }

    /// Get samples within a range in reverse order
    pub fn get_range_reverse(&self, from_ts: u64, to_ts: u64) -> Vec<Sample> {
        self.samples
            .range(from_ts..=to_ts)
            .rev()
            .map(|(&timestamp, &value)| Sample::new(timestamp, value))
            .collect()
    }

    /// Get aggregated samples within a range
    pub fn get_range_aggregated(
        &self,
        from_ts: u64,
        to_ts: u64,
        bucket_duration: u64,
        aggregation: AggregationType,
    ) -> Vec<Sample> {
        let samples = self.get_range(from_ts, to_ts);
        self.aggregate_samples(samples, bucket_duration, aggregation)
    }

    /// Aggregate samples into buckets
    pub fn aggregate_samples(
        &self,
        samples: Vec<Sample>,
        bucket_duration: u64,
        aggregation: AggregationType,
    ) -> Vec<Sample> {
        if samples.is_empty() || bucket_duration == 0 {
            return samples;
        }

        let mut buckets: BTreeMap<u64, Vec<Sample>> = BTreeMap::new();

        // Group samples into buckets
        for sample in samples {
            let bucket_start = (sample.timestamp / bucket_duration) * bucket_duration;
            buckets.entry(bucket_start).or_default().push(sample);
        }

        // Aggregate each bucket
        buckets
            .into_iter()
            .map(|(bucket_start, bucket_samples)| {
                let aggregated_value = aggregation.aggregate(&bucket_samples);
                Sample::new(bucket_start, aggregated_value)
            })
            .collect()
    }

    /// Delete samples within a timestamp range
    pub fn delete_range(&mut self, from_ts: u64, to_ts: u64) -> usize {
        let keys_to_remove: Vec<u64> = self
            .samples
            .range(from_ts..=to_ts)
            .map(|(&timestamp, _)| timestamp)
            .collect();

        let count = keys_to_remove.len();
        for key in keys_to_remove {
            self.samples.remove(&key);
        }

        if count > 0 {
            self.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
        }

        count
    }

    /// Create a compaction rule
    pub fn create_compaction_rule(&mut self, rule: CompactionRule) -> Result<(), String> {
        // Check if rule already exists for the destination
        if self
            .compaction_rules
            .iter()
            .any(|r| r.dest_key == rule.dest_key)
        {
            return Err(format!(
                "Compaction rule for '{}' already exists",
                rule.dest_key
            ));
        }

        self.compaction_rules.push(rule);
        Ok(())
    }

    /// Delete a compaction rule
    pub fn delete_compaction_rule(&mut self, dest_key: &str) -> bool {
        let initial_len = self.compaction_rules.len();
        self.compaction_rules
            .retain(|rule| rule.dest_key != dest_key);
        self.compaction_rules.len() < initial_len
    }

    /// Get series statistics
    pub fn get_stats(&self) -> TimeSeriesStats {
        let total_samples = self.samples.len();
        let memory_usage = self.estimate_memory_usage();
        let first_timestamp = self.samples.keys().next().copied();
        let last_timestamp = self.samples.keys().next_back().copied();

        TimeSeriesStats {
            total_samples,
            memory_usage,
            first_timestamp,
            last_timestamp,
            retention_time: self.config.retention,
            chunk_size: self.config.chunk_size,
            duplicate_policy: self.config.duplicate_policy,
            labels: self.config.labels.clone(),
            rules: self.compaction_rules.len(),
            source_key: None, // Will be set by caller if needed
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Check if series matches label filters
    pub fn matches_labels(&self, filters: &HashMap<String, String>) -> bool {
        filters
            .iter()
            .all(|(key, value)| self.config.labels.get(key) == Some(value))
    }

    /// Apply retention policy
    fn apply_retention(&mut self) {
        if let Some(retention_ms) = self.config.retention {
            let cutoff_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            if cutoff_time > retention_ms {
                let expire_before = cutoff_time - retention_ms;
                let keys_to_remove: Vec<u64> = self
                    .samples
                    .range(..expire_before)
                    .map(|(&timestamp, _)| timestamp)
                    .collect();

                for key in keys_to_remove {
                    self.samples.remove(&key);
                }
            }
        }
    }

    /// Apply compaction rules (placeholder - would need access to other series)
    fn apply_compaction_rules(&self, _timestamp: u64, _value: f64) {
        // This would be implemented at the actor manager level
        // where we have access to other time series
    }

    /// Estimate memory usage in bytes
    fn estimate_memory_usage(&self) -> usize {
        // Rough estimate: timestamp (8) + value (8) + overhead per sample
        let sample_size = 16 + 8; // 8 bytes overhead for BTreeMap
        self.samples.len() * sample_size
            + std::mem::size_of::<TimeSeriesConfig>()
            + self
                .config
                .labels
                .iter()
                .map(|(k, v)| k.len() + v.len() + 16)
                .sum::<usize>()
    }

    /// Update time series configuration
    pub fn update_config(&mut self, new_config: TimeSeriesConfig) {
        self.config = new_config;
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Apply new retention policy
        self.apply_retention();
    }
}

/// Time series statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesStats {
    pub total_samples: usize,
    pub memory_usage: usize,
    pub first_timestamp: Option<u64>,
    pub last_timestamp: Option<u64>,
    pub retention_time: Option<u64>,
    pub chunk_size: Option<usize>,
    pub duplicate_policy: DuplicatePolicy,
    pub labels: HashMap<String, String>,
    pub rules: usize,
    pub source_key: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Addressable for TimeSeriesActor {
    fn addressable_type() -> &'static str {
        "TimeSeriesActor"
    }
}

/// Async trait defining time series actor operations
#[async_trait]
pub trait TimeSeriesActorMethods {
    /// Add a sample to the time series
    async fn add_sample(&mut self, timestamp: u64, value: f64) -> OrbitResult<()>;

    /// Add multiple samples
    async fn add_samples(&mut self, samples: Vec<Sample>) -> OrbitResult<Vec<String>>;

    /// Increment/decrement the latest sample
    async fn increment_by(&mut self, timestamp: u64, increment: f64) -> OrbitResult<f64>;

    /// Get the latest sample
    async fn get_latest(&self) -> OrbitResult<Option<Sample>>;

    /// Get samples within a timestamp range
    async fn get_range(&self, from_ts: u64, to_ts: u64) -> OrbitResult<Vec<Sample>>;

    /// Get samples within a range in reverse order
    async fn get_range_reverse(&self, from_ts: u64, to_ts: u64) -> OrbitResult<Vec<Sample>>;

    /// Get aggregated samples within a range
    async fn get_range_aggregated(
        &self,
        from_ts: u64,
        to_ts: u64,
        bucket_duration: u64,
        aggregation: AggregationType,
    ) -> OrbitResult<Vec<Sample>>;

    /// Delete samples within a timestamp range
    async fn delete_range(&mut self, from_ts: u64, to_ts: u64) -> OrbitResult<usize>;

    /// Create a compaction rule
    async fn create_compaction_rule(&mut self, rule: CompactionRule) -> OrbitResult<()>;

    /// Delete a compaction rule
    async fn delete_compaction_rule(&mut self, dest_key: String) -> OrbitResult<bool>;

    /// Get series statistics
    async fn get_stats(&self) -> OrbitResult<TimeSeriesStats>;

    /// Check if series matches label filters
    async fn matches_labels(&self, filters: HashMap<String, String>) -> OrbitResult<bool>;

    /// Update time series configuration
    async fn update_config(&mut self, config: TimeSeriesConfig) -> OrbitResult<()>;
}

/// Implementation of time series actor methods
#[async_trait]
impl TimeSeriesActorMethods for TimeSeriesActor {
    async fn add_sample(&mut self, timestamp: u64, value: f64) -> OrbitResult<()> {
        self.add_sample(timestamp, value)
            .map_err(orbit_shared::OrbitError::internal)
    }

    async fn add_samples(&mut self, samples: Vec<Sample>) -> OrbitResult<Vec<String>> {
        self.add_samples(samples)
            .map_err(orbit_shared::OrbitError::internal)
    }

    async fn increment_by(&mut self, timestamp: u64, increment: f64) -> OrbitResult<f64> {
        self.increment_by(timestamp, increment)
            .map_err(orbit_shared::OrbitError::internal)
    }

    async fn get_latest(&self) -> OrbitResult<Option<Sample>> {
        Ok(self.get_latest())
    }

    async fn get_range(&self, from_ts: u64, to_ts: u64) -> OrbitResult<Vec<Sample>> {
        Ok(self.get_range(from_ts, to_ts))
    }

    async fn get_range_reverse(&self, from_ts: u64, to_ts: u64) -> OrbitResult<Vec<Sample>> {
        Ok(self.get_range_reverse(from_ts, to_ts))
    }

    async fn get_range_aggregated(
        &self,
        from_ts: u64,
        to_ts: u64,
        bucket_duration: u64,
        aggregation: AggregationType,
    ) -> OrbitResult<Vec<Sample>> {
        Ok(self.get_range_aggregated(from_ts, to_ts, bucket_duration, aggregation))
    }

    async fn delete_range(&mut self, from_ts: u64, to_ts: u64) -> OrbitResult<usize> {
        Ok(self.delete_range(from_ts, to_ts))
    }

    async fn create_compaction_rule(&mut self, rule: CompactionRule) -> OrbitResult<()> {
        self.create_compaction_rule(rule)
            .map_err(orbit_shared::OrbitError::internal)
    }

    async fn delete_compaction_rule(&mut self, dest_key: String) -> OrbitResult<bool> {
        Ok(self.delete_compaction_rule(&dest_key))
    }

    async fn get_stats(&self) -> OrbitResult<TimeSeriesStats> {
        Ok(self.get_stats())
    }

    async fn matches_labels(&self, filters: HashMap<String, String>) -> OrbitResult<bool> {
        Ok(self.matches_labels(&filters))
    }

    async fn update_config(&mut self, config: TimeSeriesConfig) -> OrbitResult<()> {
        self.update_config(config);
        Ok(())
    }
}
