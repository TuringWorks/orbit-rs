//! Core time series engine implementation

#[cfg(test)]
use super::datetime_to_timestamp;
use super::{
    AggregationType, DataPoint, Deserialize, HashMap, QueryResult, Serialize, SeriesId,
    StorageBackend, TimeRange, TimeSeriesConfig, TimeSeriesMetadata, Utc, Uuid,
};
use crate::timeseries::storage::{MemoryStorage, PostgreSQLStorage, RedisStorage, StorageEngine};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Main time series engine
pub struct TimeSeriesEngine {
    config: TimeSeriesConfig,
    storage: Box<dyn StorageEngine + Send + Sync>,
    series_metadata: Arc<RwLock<HashMap<SeriesId, TimeSeriesMetadata>>>,
    metrics_enabled: bool,
}

impl TimeSeriesEngine {
    /// Create a new time series engine with the given configuration
    pub async fn new(config: TimeSeriesConfig) -> Result<Self> {
        info!(
            "Initializing Time Series Engine with backend: {:?}",
            config.storage_backend
        );

        let storage = Self::create_storage_backend(&config).await?;

        Ok(Self {
            metrics_enabled: config.enable_metrics,
            config,
            storage,
            series_metadata: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create the appropriate storage backend
    async fn create_storage_backend(
        config: &TimeSeriesConfig,
    ) -> Result<Box<dyn StorageEngine + Send + Sync>> {
        match &config.storage_backend {
            StorageBackend::Memory => {
                info!("Using in-memory storage backend");
                Ok(Box::new(MemoryStorage::new(config.memory_limit_mb)))
            }
            StorageBackend::Redis => {
                info!("Using Redis TimeSeries backend");
                let redis_config = config
                    .redis_config
                    .as_ref()
                    .ok_or_else(|| anyhow!("Redis config required for Redis backend"))?;
                Ok(Box::new(RedisStorage::new(redis_config).await?))
            }
            StorageBackend::PostgreSQL => {
                info!("Using PostgreSQL TimescaleDB backend");
                let pg_config = config
                    .postgresql_config
                    .as_ref()
                    .ok_or_else(|| anyhow!("PostgreSQL config required for PostgreSQL backend"))?;
                Ok(Box::new(PostgreSQLStorage::new(pg_config).await?))
            }
            StorageBackend::HybridRedisPostgres => {
                info!("Using hybrid Redis+PostgreSQL backend");
                // TODO: Implement hybrid storage that uses Redis for hot data and PostgreSQL for cold data
                Err(anyhow!("Hybrid storage not yet implemented"))
            }
            StorageBackend::CustomDisk => {
                info!("Using custom disk storage backend");
                // TODO: Implement custom high-performance disk storage
                Err(anyhow!("Custom disk storage not yet implemented"))
            }
        }
    }

    /// Create a new time series
    pub async fn create_series(
        &self,
        name: String,
        labels: HashMap<String, String>,
    ) -> Result<SeriesId> {
        let series_id = Uuid::new_v4();
        let now = Utc::now();

        let metadata = TimeSeriesMetadata {
            series_id,
            name: name.clone(),
            labels: labels.clone(),
            retention_policy: Some(self.config.default_retention_policy.clone()),
            compression_policy: Some(self.config.default_compression_policy.clone()),
            created_at: now,
            updated_at: now,
        };

        // Store metadata
        {
            let mut metadata_map = self.series_metadata.write().await;
            metadata_map.insert(series_id, metadata.clone());
        }

        // Initialize series in storage
        self.storage.create_series(series_id, &metadata).await?;

        info!("Created time series: {} with ID: {}", name, series_id);
        Ok(series_id)
    }

    /// Insert a single data point
    pub async fn insert_point(&self, series_id: SeriesId, data_point: DataPoint) -> Result<()> {
        let start = if self.metrics_enabled {
            Some(Instant::now())
        } else {
            None
        };

        // Validate series exists
        {
            let metadata_map = self.series_metadata.read().await;
            if !metadata_map.contains_key(&series_id) {
                return Err(anyhow!("Series {} does not exist", series_id));
            }
        }

        // Insert the data point
        self.storage.insert_point(series_id, data_point).await?;

        if let Some(start_time) = start {
            let duration = start_time.elapsed();
            debug!("Insert point took: {:?}", duration);
        }

        Ok(())
    }

    /// Insert multiple data points in batch
    pub async fn insert_batch(
        &self,
        series_id: SeriesId,
        data_points: Vec<DataPoint>,
    ) -> Result<()> {
        let start = if self.metrics_enabled {
            Some(Instant::now())
        } else {
            None
        };

        if data_points.is_empty() {
            return Ok(());
        }

        // Validate series exists
        {
            let metadata_map = self.series_metadata.read().await;
            if !metadata_map.contains_key(&series_id) {
                return Err(anyhow!("Series {} does not exist", series_id));
            }
        }

        // Insert batch
        self.storage
            .insert_batch(series_id, data_points.clone())
            .await?;

        if let Some(start_time) = start {
            let duration = start_time.elapsed();
            info!(
                "Batch insert of {} points took: {:?}",
                data_points.len(),
                duration
            );
        }

        Ok(())
    }

    /// Query time series data within a time range
    pub async fn query(&self, series_id: SeriesId, time_range: TimeRange) -> Result<QueryResult> {
        let start = if self.metrics_enabled {
            Some(Instant::now())
        } else {
            None
        };

        // Get metadata
        let metadata = {
            let metadata_map = self.series_metadata.read().await;
            metadata_map
                .get(&series_id)
                .ok_or_else(|| anyhow!("Series {} does not exist", series_id))?
                .clone()
        };

        // Query storage
        let data_points = self.storage.query(series_id, time_range).await?;
        let total_points = data_points.len();

        let execution_time_ms = if let Some(start_time) = start {
            start_time.elapsed().as_millis() as u64
        } else {
            0
        };

        Ok(QueryResult {
            series_id,
            metadata,
            data_points,
            total_points,
            execution_time_ms,
        })
    }

    /// Query with aggregation
    pub async fn query_aggregate(
        &self,
        series_id: SeriesId,
        time_range: TimeRange,
        aggregation: AggregationType,
        window_size_seconds: u64,
    ) -> Result<QueryResult> {
        let start = if self.metrics_enabled {
            Some(Instant::now())
        } else {
            None
        };

        // Get metadata
        let metadata = {
            let metadata_map = self.series_metadata.read().await;
            metadata_map
                .get(&series_id)
                .ok_or_else(|| anyhow!("Series {} does not exist", series_id))?
                .clone()
        };

        // Query and aggregate
        let data_points = self
            .storage
            .query_aggregate(series_id, time_range, aggregation, window_size_seconds)
            .await?;
        let total_points = data_points.len();

        let execution_time_ms = if let Some(start_time) = start {
            start_time.elapsed().as_millis() as u64
        } else {
            0
        };

        Ok(QueryResult {
            series_id,
            metadata,
            data_points,
            total_points,
            execution_time_ms,
        })
    }

    /// List all time series
    pub async fn list_series(&self) -> Result<Vec<TimeSeriesMetadata>> {
        let metadata_map = self.series_metadata.read().await;
        Ok(metadata_map.values().cloned().collect())
    }

    /// Get series metadata
    pub async fn get_series_metadata(&self, series_id: SeriesId) -> Result<TimeSeriesMetadata> {
        let metadata_map = self.series_metadata.read().await;
        metadata_map
            .get(&series_id)
            .ok_or_else(|| anyhow!("Series {} does not exist", series_id))
            .cloned()
    }

    /// Update series metadata
    pub async fn update_series_metadata(
        &self,
        series_id: SeriesId,
        metadata: TimeSeriesMetadata,
    ) -> Result<()> {
        {
            let mut metadata_map = self.series_metadata.write().await;
            if !metadata_map.contains_key(&series_id) {
                return Err(anyhow!("Series {} does not exist", series_id));
            }
            metadata_map.insert(series_id, metadata.clone());
        }

        self.storage
            .update_series_metadata(series_id, &metadata)
            .await?;
        info!("Updated metadata for series: {}", series_id);
        Ok(())
    }

    /// Delete a time series
    pub async fn delete_series(&self, series_id: SeriesId) -> Result<()> {
        // Remove from metadata
        {
            let mut metadata_map = self.series_metadata.write().await;
            if metadata_map.remove(&series_id).is_none() {
                return Err(anyhow!("Series {} does not exist", series_id));
            }
        }

        // Delete from storage
        self.storage.delete_series(series_id).await?;

        info!("Deleted time series: {}", series_id);
        Ok(())
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        self.storage.get_stats().await
    }

    /// Compact storage (run maintenance operations)
    pub async fn compact_storage(&self) -> Result<()> {
        info!("Running storage compaction");
        self.storage.compact().await
    }

    /// Apply retention policies
    pub async fn apply_retention_policies(&self) -> Result<()> {
        info!("Applying retention policies");

        let metadata_map = self.series_metadata.read().await;
        for (series_id, metadata) in metadata_map.iter() {
            if let Some(retention_policy) = &metadata.retention_policy {
                self.storage
                    .apply_retention(*series_id, retention_policy)
                    .await?;
            }
        }

        Ok(())
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_series: u64,
    pub total_data_points: u64,
    pub storage_size_bytes: u64,
    pub memory_usage_bytes: u64,
    pub compression_ratio: f64,
    pub ingestion_rate_points_per_second: f64,
    pub query_rate_per_second: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeseries::{DataPoint, TimeSeriesValue};

    #[tokio::test]
    async fn test_create_series() {
        let config = TimeSeriesConfig::default();
        let engine = TimeSeriesEngine::new(config).await.unwrap();

        let mut labels = HashMap::new();
        labels.insert("host".to_string(), "server1".to_string());
        labels.insert("metric".to_string(), "cpu_usage".to_string());

        let series_id = engine
            .create_series("cpu_usage".to_string(), labels)
            .await
            .unwrap();
        assert!(!series_id.is_nil());

        let metadata = engine.get_series_metadata(series_id).await.unwrap();
        assert_eq!(metadata.name, "cpu_usage");
        assert_eq!(metadata.labels.get("host").unwrap(), "server1");
    }

    #[tokio::test]
    async fn test_insert_and_query() {
        let config = TimeSeriesConfig::default();
        let engine = TimeSeriesEngine::new(config).await.unwrap();

        let series_id = engine
            .create_series("test_metric".to_string(), HashMap::new())
            .await
            .unwrap();

        // Insert some data points
        let now = Utc::now();
        let data_points = vec![
            DataPoint {
                timestamp: datetime_to_timestamp(now),
                value: TimeSeriesValue::Float(100.0),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: datetime_to_timestamp(now + chrono::Duration::seconds(60)),
                value: TimeSeriesValue::Float(150.0),
                labels: HashMap::new(),
            },
        ];

        engine
            .insert_batch(series_id, data_points.clone())
            .await
            .unwrap();

        // Query the data
        let time_range = TimeRange {
            start: datetime_to_timestamp(now - chrono::Duration::minutes(1)),
            end: datetime_to_timestamp(now + chrono::Duration::minutes(2)),
        };

        let result = engine.query(series_id, time_range).await.unwrap();
        assert_eq!(result.total_points, 2);
        assert_eq!(result.data_points.len(), 2);
    }
}
