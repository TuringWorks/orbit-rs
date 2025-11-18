//! Time series storage engine implementations

use super::{
    AggregationType, DataPoint, HashMap, RetentionPolicy, SeriesId, TimeRange, TimeSeriesMetadata,
    TimeSeriesValue, Timestamp, Utc,
};
use crate::timeseries::core::StorageStats;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
#[cfg(test)]
use uuid::Uuid;

/// Storage engine trait for time series data
#[async_trait]
pub trait StorageEngine {
    /// Create a new time series
    async fn create_series(&self, series_id: SeriesId, metadata: &TimeSeriesMetadata)
        -> Result<()>;

    /// Insert a single data point
    async fn insert_point(&self, series_id: SeriesId, data_point: DataPoint) -> Result<()>;

    /// Insert multiple data points in batch
    async fn insert_batch(&self, series_id: SeriesId, data_points: Vec<DataPoint>) -> Result<()>;

    /// Query data points within a time range
    async fn query(&self, series_id: SeriesId, time_range: TimeRange) -> Result<Vec<DataPoint>>;

    /// Query with aggregation
    async fn query_aggregate(
        &self,
        series_id: SeriesId,
        time_range: TimeRange,
        aggregation: AggregationType,
        window_size_seconds: u64,
    ) -> Result<Vec<DataPoint>>;

    /// Update series metadata
    async fn update_series_metadata(
        &self,
        series_id: SeriesId,
        metadata: &TimeSeriesMetadata,
    ) -> Result<()>;

    /// Delete a time series
    async fn delete_series(&self, series_id: SeriesId) -> Result<()>;

    /// Apply retention policy
    async fn apply_retention(
        &self,
        series_id: SeriesId,
        retention_policy: &RetentionPolicy,
    ) -> Result<()>;

    /// Get storage statistics
    async fn get_stats(&self) -> Result<StorageStats>;

    /// Compact storage
    async fn compact(&self) -> Result<()>;
}

/// In-memory storage implementation
pub struct MemoryStorage {
    data: Arc<RwLock<HashMap<SeriesId, BTreeMap<Timestamp, DataPoint>>>>,
    metadata: Arc<RwLock<HashMap<SeriesId, TimeSeriesMetadata>>>,
    memory_limit_bytes: u64,
    current_memory_usage: Arc<RwLock<u64>>,
}

impl MemoryStorage {
    pub fn new(memory_limit_mb: u64) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            memory_limit_bytes: memory_limit_mb * 1024 * 1024,
            current_memory_usage: Arc::new(RwLock::new(0)),
        }
    }

    async fn estimate_point_size(&self, data_point: &DataPoint) -> usize {
        let base_size = std::mem::size_of::<DataPoint>();
        let labels_size: usize = data_point
            .labels
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();
        let value_size = match &data_point.value {
            TimeSeriesValue::String(s) => s.len(),
            _ => 0,
        };
        base_size + labels_size + value_size
    }

    async fn check_memory_limit(&self, additional_bytes: u64) -> Result<()> {
        let current_usage = *self.current_memory_usage.read().await;
        if current_usage + additional_bytes > self.memory_limit_bytes {
            return Err(anyhow::anyhow!("Memory limit exceeded"));
        }
        Ok(())
    }
}

#[async_trait]
impl StorageEngine for MemoryStorage {
    async fn create_series(
        &self,
        series_id: SeriesId,
        metadata: &TimeSeriesMetadata,
    ) -> Result<()> {
        {
            let mut data_map = self.data.write().await;
            data_map.insert(series_id, BTreeMap::new());
        }

        {
            let mut metadata_map = self.metadata.write().await;
            metadata_map.insert(series_id, metadata.clone());
        }

        debug!("Created series {} in memory storage", series_id);
        Ok(())
    }

    async fn insert_point(&self, series_id: SeriesId, data_point: DataPoint) -> Result<()> {
        let point_size = self.estimate_point_size(&data_point).await as u64;
        self.check_memory_limit(point_size).await?;

        {
            let mut data_map = self.data.write().await;
            let series_data = data_map.entry(series_id).or_insert_with(BTreeMap::new);
            series_data.insert(data_point.timestamp, data_point);
        }

        {
            let mut memory_usage = self.current_memory_usage.write().await;
            *memory_usage += point_size;
        }

        Ok(())
    }

    async fn insert_batch(&self, series_id: SeriesId, data_points: Vec<DataPoint>) -> Result<()> {
        let total_size: u64 = {
            let mut total = 0;
            for point in &data_points {
                total += self.estimate_point_size(point).await as u64;
            }
            total
        };

        self.check_memory_limit(total_size).await?;

        {
            let mut data_map = self.data.write().await;
            let series_data = data_map.entry(series_id).or_insert_with(BTreeMap::new);

            for data_point in data_points {
                series_data.insert(data_point.timestamp, data_point);
            }
        }

        {
            let mut memory_usage = self.current_memory_usage.write().await;
            *memory_usage += total_size;
        }

        Ok(())
    }

    async fn query(&self, series_id: SeriesId, time_range: TimeRange) -> Result<Vec<DataPoint>> {
        let data_map = self.data.read().await;

        let series_data = data_map
            .get(&series_id)
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut result = Vec::new();
        for (timestamp, data_point) in series_data.range(time_range.start..=time_range.end) {
            if *timestamp >= time_range.start && *timestamp <= time_range.end {
                result.push(data_point.clone());
            }
        }

        Ok(result)
    }

    async fn query_aggregate(
        &self,
        series_id: SeriesId,
        time_range: TimeRange,
        aggregation: AggregationType,
        window_size_seconds: u64,
    ) -> Result<Vec<DataPoint>> {
        let raw_data = self.query(series_id, time_range).await?;

        if raw_data.is_empty() {
            return Ok(Vec::new());
        }

        // Group data points by time windows
        let window_size_nanos = (window_size_seconds * 1_000_000_000) as i64;
        let mut windows: BTreeMap<i64, Vec<DataPoint>> = BTreeMap::new();

        for point in raw_data {
            let window_start = (point.timestamp / window_size_nanos) * window_size_nanos;
            windows.entry(window_start).or_default().push(point);
        }

        // Aggregate each window
        let mut result = Vec::new();
        for (window_start, points) in windows {
            if let Some(aggregated_value) = aggregate_values(&points, &aggregation) {
                result.push(DataPoint {
                    timestamp: window_start,
                    value: aggregated_value,
                    labels: points.first().map(|p| p.labels.clone()).unwrap_or_default(),
                });
            }
        }

        Ok(result)
    }

    async fn update_series_metadata(
        &self,
        series_id: SeriesId,
        metadata: &TimeSeriesMetadata,
    ) -> Result<()> {
        let mut metadata_map = self.metadata.write().await;
        metadata_map.insert(series_id, metadata.clone());
        Ok(())
    }

    async fn delete_series(&self, series_id: SeriesId) -> Result<()> {
        let memory_freed = {
            let mut data_map = self.data.write().await;
            if let Some(series_data) = data_map.remove(&series_id) {
                series_data.len() as u64 * 128 // Rough estimate
            } else {
                0
            }
        };

        {
            let mut metadata_map = self.metadata.write().await;
            metadata_map.remove(&series_id);
        }

        {
            let mut memory_usage = self.current_memory_usage.write().await;
            *memory_usage = memory_usage.saturating_sub(memory_freed);
        }

        Ok(())
    }

    async fn apply_retention(
        &self,
        series_id: SeriesId,
        retention_policy: &RetentionPolicy,
    ) -> Result<()> {
        let cutoff_time = Utc::now().timestamp_nanos_opt().unwrap_or(0)
            - (retention_policy.duration_seconds as i64 * 1_000_000_000);

        let mut data_map = self.data.write().await;
        if let Some(series_data) = data_map.get_mut(&series_id) {
            let keys_to_remove: Vec<_> =
                series_data.range(..cutoff_time).map(|(k, _)| *k).collect();

            for key in keys_to_remove {
                series_data.remove(&key);
            }
        }

        Ok(())
    }

    async fn get_stats(&self) -> Result<StorageStats> {
        let data_map = self.data.read().await;
        let memory_usage = *self.current_memory_usage.read().await;

        let total_series = data_map.len() as u64;
        let total_data_points: u64 = data_map.values().map(|series| series.len() as u64).sum();

        Ok(StorageStats {
            total_series,
            total_data_points,
            storage_size_bytes: memory_usage,
            memory_usage_bytes: memory_usage,
            compression_ratio: 1.0, // No compression in memory storage
            ingestion_rate_points_per_second: 0.0, // TODO: Track this metric
            query_rate_per_second: 0.0, // TODO: Track this metric
        })
    }

    async fn compact(&self) -> Result<()> {
        // For in-memory storage, compaction might involve:
        // - Removing duplicate timestamps
        // - Organizing data more efficiently
        // For now, this is a no-op
        info!("Memory storage compaction completed (no-op)");
        Ok(())
    }
}

/// Redis TimeSeries storage implementation
pub struct RedisStorage {
    // TODO: Implement Redis TimeSeries client
    _config: crate::timeseries::redis::RedisConfig,
}

impl RedisStorage {
    pub async fn new(_config: &crate::timeseries::redis::RedisConfig) -> Result<Self> {
        // TODO: Initialize Redis connection
        Ok(Self {
            _config: _config.clone(),
        })
    }
}

#[async_trait]
impl StorageEngine for RedisStorage {
    async fn create_series(
        &self,
        _series_id: SeriesId,
        _metadata: &TimeSeriesMetadata,
    ) -> Result<()> {
        // TODO: Implement Redis TimeSeries creation
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn insert_point(&self, _series_id: SeriesId, _data_point: DataPoint) -> Result<()> {
        // TODO: Implement Redis TimeSeries insertion
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn insert_batch(&self, _series_id: SeriesId, _data_points: Vec<DataPoint>) -> Result<()> {
        // TODO: Implement Redis TimeSeries batch insertion
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn query(&self, _series_id: SeriesId, _time_range: TimeRange) -> Result<Vec<DataPoint>> {
        // TODO: Implement Redis TimeSeries query
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn query_aggregate(
        &self,
        _series_id: SeriesId,
        _time_range: TimeRange,
        _aggregation: AggregationType,
        _window_size_seconds: u64,
    ) -> Result<Vec<DataPoint>> {
        // TODO: Implement Redis TimeSeries aggregation
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn update_series_metadata(
        &self,
        _series_id: SeriesId,
        _metadata: &TimeSeriesMetadata,
    ) -> Result<()> {
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn delete_series(&self, _series_id: SeriesId) -> Result<()> {
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn apply_retention(
        &self,
        _series_id: SeriesId,
        _retention_policy: &RetentionPolicy,
    ) -> Result<()> {
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn get_stats(&self) -> Result<StorageStats> {
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }

    async fn compact(&self) -> Result<()> {
        Err(anyhow::anyhow!("Redis storage not yet implemented"))
    }
}

/// PostgreSQL TimescaleDB storage implementation
pub struct PostgreSQLStorage {
    // TODO: Implement PostgreSQL connection
    _config: crate::timeseries::postgresql::PostgreSQLConfig,
}

impl PostgreSQLStorage {
    pub async fn new(_config: &crate::timeseries::postgresql::PostgreSQLConfig) -> Result<Self> {
        // TODO: Initialize PostgreSQL connection
        Ok(Self {
            _config: _config.clone(),
        })
    }
}

#[async_trait]
impl StorageEngine for PostgreSQLStorage {
    async fn create_series(
        &self,
        _series_id: SeriesId,
        _metadata: &TimeSeriesMetadata,
    ) -> Result<()> {
        // TODO: Implement PostgreSQL hypertable creation
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn insert_point(&self, _series_id: SeriesId, _data_point: DataPoint) -> Result<()> {
        // TODO: Implement PostgreSQL insertion
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn insert_batch(&self, _series_id: SeriesId, _data_points: Vec<DataPoint>) -> Result<()> {
        // TODO: Implement PostgreSQL batch insertion
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn query(&self, _series_id: SeriesId, _time_range: TimeRange) -> Result<Vec<DataPoint>> {
        // TODO: Implement PostgreSQL query
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn query_aggregate(
        &self,
        _series_id: SeriesId,
        _time_range: TimeRange,
        _aggregation: AggregationType,
        _window_size_seconds: u64,
    ) -> Result<Vec<DataPoint>> {
        // TODO: Implement PostgreSQL aggregation
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn update_series_metadata(
        &self,
        _series_id: SeriesId,
        _metadata: &TimeSeriesMetadata,
    ) -> Result<()> {
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn delete_series(&self, _series_id: SeriesId) -> Result<()> {
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn apply_retention(
        &self,
        _series_id: SeriesId,
        _retention_policy: &RetentionPolicy,
    ) -> Result<()> {
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn get_stats(&self) -> Result<StorageStats> {
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }

    async fn compact(&self) -> Result<()> {
        Err(anyhow::anyhow!("PostgreSQL storage not yet implemented"))
    }
}

/// Aggregate values according to aggregation type
fn aggregate_values(
    points: &[DataPoint],
    aggregation: &AggregationType,
) -> Option<TimeSeriesValue> {
    if points.is_empty() {
        return None;
    }

    let numeric_values: Vec<f64> = points
        .iter()
        .filter_map(|p| match &p.value {
            TimeSeriesValue::Float(f) => Some(*f),
            TimeSeriesValue::Integer(i) => Some(*i as f64),
            _ => None,
        })
        .collect();

    if numeric_values.is_empty() {
        return Some(points[0].value.clone());
    }

    match aggregation {
        AggregationType::Sum => Some(TimeSeriesValue::Float(numeric_values.iter().sum())),
        AggregationType::Average => Some(TimeSeriesValue::Float(
            numeric_values.iter().sum::<f64>() / numeric_values.len() as f64,
        )),
        AggregationType::Min => numeric_values
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|&v| TimeSeriesValue::Float(v)),
        AggregationType::Max => numeric_values
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|&v| TimeSeriesValue::Float(v)),
        AggregationType::Count => Some(TimeSeriesValue::Integer(points.len() as i64)),
        AggregationType::StdDev => {
            let mean = numeric_values.iter().sum::<f64>() / numeric_values.len() as f64;
            let variance = numeric_values
                .iter()
                .map(|&v| (v - mean).powi(2))
                .sum::<f64>()
                / numeric_values.len() as f64;
            Some(TimeSeriesValue::Float(variance.sqrt()))
        }
        AggregationType::First => Some(points.first()?.value.clone()),
        AggregationType::Last => Some(points.last()?.value.clone()),
        AggregationType::Percentile(p) => {
            let mut sorted = numeric_values.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let index = ((*p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
            sorted.get(index).map(|&v| TimeSeriesValue::Float(v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStorage::new(100); // 100MB limit
        let series_id = Uuid::new_v4();
        let metadata = TimeSeriesMetadata {
            series_id,
            name: "test_series".to_string(),
            labels: HashMap::new(),
            retention_policy: None,
            compression_policy: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Create series
        storage.create_series(series_id, &metadata).await.unwrap();

        // Insert data point
        let data_point = DataPoint {
            timestamp: Utc::now().timestamp_nanos_opt().unwrap_or(0),
            value: TimeSeriesValue::Float(42.0),
            labels: HashMap::new(),
        };

        storage
            .insert_point(series_id, data_point.clone())
            .await
            .unwrap();

        // Query data
        let time_range = TimeRange {
            start: data_point.timestamp - 1000,
            end: data_point.timestamp + 1000,
        };

        let results = storage.query(series_id, time_range).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, TimeSeriesValue::Float(42.0));
    }

    #[tokio::test]
    async fn test_aggregation() {
        let points = vec![
            DataPoint {
                timestamp: 1000,
                value: TimeSeriesValue::Float(10.0),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 2000,
                value: TimeSeriesValue::Float(20.0),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 3000,
                value: TimeSeriesValue::Float(30.0),
                labels: HashMap::new(),
            },
        ];

        let sum = aggregate_values(&points, &AggregationType::Sum).unwrap();
        assert_eq!(sum, TimeSeriesValue::Float(60.0));

        let avg = aggregate_values(&points, &AggregationType::Average).unwrap();
        assert_eq!(avg, TimeSeriesValue::Float(20.0));

        let min = aggregate_values(&points, &AggregationType::Min).unwrap();
        assert_eq!(min, TimeSeriesValue::Float(10.0));

        let max = aggregate_values(&points, &AggregationType::Max).unwrap();
        assert_eq!(max, TimeSeriesValue::Float(30.0));
    }
}
