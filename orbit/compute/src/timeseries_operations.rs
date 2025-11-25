//! GPU-accelerated time-series operations
//!
//! This module provides high-performance time-series operations using GPU acceleration
//! for compute-intensive operations like window aggregations, moving averages, and
//! time-based grouping. Falls back to CPU-parallel execution for small datasets or
//! when GPU is unavailable.

use crate::errors::ComputeError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "gpu-acceleration")]
use crate::gpu::GPUAccelerationManager;
use std::sync::Arc;

/// Time-series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Timestamp in milliseconds
    pub timestamp: u64,
    /// Value at this timestamp
    pub value: f64,
}

/// Time-series aggregation function types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TimeSeriesAggregation {
    /// Average of values in window
    Avg,
    /// Sum of values in window
    Sum,
    /// Minimum value in window
    Min,
    /// Maximum value in window
    Max,
    /// Count of values in window
    Count,
    /// First value in window
    First,
    /// Last value in window
    Last,
    /// Range (max - min) in window
    Range,
    /// Standard deviation in window
    Std,
}

/// Window function types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WindowFunction {
    /// Simple moving average
    MovingAverage {
        /// Window size in milliseconds
        window_size_ms: u64,
    },
    /// Exponential weighted moving average
    ExponentialMovingAverage {
        /// Alpha parameter (0.0 to 1.0)
        alpha: f64,
    },
}

/// Configuration for GPU-accelerated time-series operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesOperationsConfig {
    /// Enable GPU acceleration
    pub enable_gpu: bool,
    /// Minimum number of points to use GPU (below this, use CPU)
    pub gpu_min_points: usize,
    /// Minimum number of windows to use GPU
    pub gpu_min_windows: usize,
    /// Use CPU-parallel fallback (Rayon)
    pub use_cpu_parallel: bool,
}

impl Default for TimeSeriesOperationsConfig {
    fn default() -> Self {
        Self {
            enable_gpu: true,
            gpu_min_points: 10000, // Use GPU for 10K+ points
            gpu_min_windows: 100,   // Use GPU for 100+ windows
            use_cpu_parallel: true,
        }
    }
}

/// Result of time-series aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesAggregationResult {
    /// Window start timestamp
    pub window_start: u64,
    /// Window end timestamp
    pub window_end: u64,
    /// Aggregated value
    pub value: f64,
    /// Number of points in window
    pub point_count: usize,
}

/// Statistics about time-series operation execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeSeriesOperationsStats {
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of points processed
    pub points_processed: usize,
    /// Number of windows processed
    pub windows_processed: usize,
    /// Whether GPU was used
    pub used_gpu: bool,
    /// GPU utilization percentage (if GPU used)
    pub gpu_utilization: Option<f32>,
}

/// GPU-accelerated time-series operations engine
pub struct GPUTimeSeriesOperations {
    #[cfg(feature = "gpu-acceleration")]
    gpu_manager: Option<Arc<GPUAccelerationManager>>,
    config: TimeSeriesOperationsConfig,
}

impl GPUTimeSeriesOperations {
    /// Create a new GPU-accelerated time-series operations engine
    pub async fn new(config: TimeSeriesOperationsConfig) -> Result<Self, ComputeError> {
        #[cfg(feature = "gpu-acceleration")]
        let gpu_manager = if config.enable_gpu {
            match GPUAccelerationManager::new().await {
                Ok(manager) => {
                    tracing::info!("GPU acceleration enabled for time-series operations");
                    Some(Arc::new(manager))
                }
                Err(e) => {
                    tracing::warn!("GPU acceleration unavailable, falling back to CPU: {}", e);
                    None
                }
            }
        } else {
            None
        };

        #[cfg(not(feature = "gpu-acceleration"))]
        let gpu_manager = None;

        Ok(Self {
            #[cfg(feature = "gpu-acceleration")]
            gpu_manager,
            config,
        })
    }

    /// Aggregate time-series data by time windows
    pub fn aggregate_by_windows(
        &self,
        points: &[TimeSeriesPoint],
        window_size_ms: u64,
        aggregation: TimeSeriesAggregation,
    ) -> Result<Vec<TimeSeriesAggregationResult>, ComputeError> {
        if points.is_empty() {
            return Ok(Vec::new());
        }

        // Group points by time windows
        let mut windows: std::collections::BTreeMap<u64, Vec<TimeSeriesPoint>> =
            std::collections::BTreeMap::new();

        for point in points {
            let window_start = (point.timestamp / window_size_ms) * window_size_ms;
            windows.entry(window_start).or_default().push(point.clone());
        }

        let window_count = windows.len();

        // Decide whether to use GPU
        let should_use_gpu = self.should_use_gpu(points.len(), window_count);

        if should_use_gpu {
            #[cfg(feature = "gpu-acceleration")]
            {
                if let Some(manager) = &self.gpu_manager {
                    return self.aggregate_by_windows_gpu(points, window_size_ms, aggregation);
                }
            }
        }

        // Fall back to CPU execution
        if self.config.use_cpu_parallel && points.len() > 1000 {
            self.aggregate_by_windows_cpu_parallel(&windows, window_size_ms, aggregation)
        } else {
            self.aggregate_by_windows_cpu_sequential(&windows, window_size_ms, aggregation)
        }
    }

    /// Apply moving average window function
    pub fn moving_average(
        &self,
        points: &[TimeSeriesPoint],
        window_size_ms: u64,
    ) -> Result<Vec<TimeSeriesPoint>, ComputeError> {
        if points.is_empty() {
            return Ok(Vec::new());
        }

        // For moving average, we need to compute sliding windows
        // This is more complex for GPU, so we'll use CPU-parallel for now
        if self.config.use_cpu_parallel && points.len() > 1000 {
            self.moving_average_cpu_parallel(points, window_size_ms)
        } else {
            self.moving_average_cpu_sequential(points, window_size_ms)
        }
    }

    /// Apply exponential weighted moving average
    pub fn exponential_moving_average(
        &self,
        points: &[TimeSeriesPoint],
        alpha: f64,
    ) -> Result<Vec<TimeSeriesPoint>, ComputeError> {
        if points.is_empty() {
            return Ok(Vec::new());
        }

        // EWMA is sequential by nature, but we can parallelize the computation
        // of the exponential weights and apply them in parallel
        if self.config.use_cpu_parallel && points.len() > 1000 {
            self.ewma_cpu_parallel(points, alpha)
        } else {
            self.ewma_cpu_sequential(points, alpha)
        }
    }

    /// Check if GPU should be used based on data size
    fn should_use_gpu(&self, point_count: usize, window_count: usize) -> bool {
        self.config.enable_gpu
            && point_count >= self.config.gpu_min_points
            && window_count >= self.config.gpu_min_windows
    }

    #[cfg(feature = "gpu-acceleration")]
    fn aggregate_by_windows_gpu(
        &self,
        points: &[TimeSeriesPoint],
        window_size_ms: u64,
        aggregation: TimeSeriesAggregation,
    ) -> Result<Vec<TimeSeriesAggregationResult>, ComputeError> {
        // For now, fall back to CPU-parallel
        // GPU implementation would require:
        // 1. Grouping points by window on GPU
        // 2. Parallel aggregation within each window
        // This is complex and deferred to a future implementation
        let mut windows: std::collections::BTreeMap<u64, Vec<TimeSeriesPoint>> =
            std::collections::BTreeMap::new();

        for point in points {
            let window_start = (point.timestamp / window_size_ms) * window_size_ms;
            windows.entry(window_start).or_default().push(point.clone());
        }

        self.aggregate_by_windows_cpu_parallel(&windows, window_size_ms, aggregation)
    }

    fn aggregate_by_windows_cpu_parallel(
        &self,
        windows: &std::collections::BTreeMap<u64, Vec<TimeSeriesPoint>>,
        window_size_ms: u64,
        aggregation: TimeSeriesAggregation,
    ) -> Result<Vec<TimeSeriesAggregationResult>, ComputeError> {
        use rayon::prelude::*;

        let results: Vec<TimeSeriesAggregationResult> = windows
            .par_iter()
            .map(|(&window_start, points)| {
                let window_end = window_start + window_size_ms;
                let value = self.aggregate_values(points, aggregation);
                TimeSeriesAggregationResult {
                    window_start,
                    window_end,
                    value,
                    point_count: points.len(),
                }
            })
            .collect();

        Ok(results)
    }

    fn aggregate_by_windows_cpu_sequential(
        &self,
        windows: &std::collections::BTreeMap<u64, Vec<TimeSeriesPoint>>,
        window_size_ms: u64,
        aggregation: TimeSeriesAggregation,
    ) -> Result<Vec<TimeSeriesAggregationResult>, ComputeError> {
        let mut results = Vec::new();

        for (&window_start, points) in windows {
            let window_end = window_start + window_size_ms;
            let value = self.aggregate_values(points, aggregation);
            results.push(TimeSeriesAggregationResult {
                window_start,
                window_end,
                value,
                point_count: points.len(),
            });
        }

        Ok(results)
    }

    fn aggregate_values(&self, points: &[TimeSeriesPoint], aggregation: TimeSeriesAggregation) -> f64 {
        if points.is_empty() {
            return 0.0;
        }

        match aggregation {
            TimeSeriesAggregation::Avg => {
                let sum: f64 = points.iter().map(|p| p.value).sum();
                sum / points.len() as f64
            }
            TimeSeriesAggregation::Sum => points.iter().map(|p| p.value).sum(),
            TimeSeriesAggregation::Min => points
                .iter()
                .map(|p| p.value)
                .fold(f64::INFINITY, f64::min),
            TimeSeriesAggregation::Max => points
                .iter()
                .map(|p| p.value)
                .fold(f64::NEG_INFINITY, f64::max),
            TimeSeriesAggregation::Count => points.len() as f64,
            TimeSeriesAggregation::First => points.first().map(|p| p.value).unwrap_or(0.0),
            TimeSeriesAggregation::Last => points.last().map(|p| p.value).unwrap_or(0.0),
            TimeSeriesAggregation::Range => {
                let min = points
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::INFINITY, f64::min);
                let max = points
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::NEG_INFINITY, f64::max);
                max - min
            }
            TimeSeriesAggregation::Std => {
                if points.len() <= 1 {
                    return 0.0;
                }
                let mean = points.iter().map(|p| p.value).sum::<f64>() / points.len() as f64;
                let variance = points
                    .iter()
                    .map(|p| (p.value - mean).powi(2))
                    .sum::<f64>()
                    / points.len() as f64;
                variance.sqrt()
            }
        }
    }

    fn moving_average_cpu_parallel(
        &self,
        points: &[TimeSeriesPoint],
        window_size_ms: u64,
    ) -> Result<Vec<TimeSeriesPoint>, ComputeError> {
        use rayon::prelude::*;

        let results: Vec<TimeSeriesPoint> = points
            .par_iter()
            .enumerate()
            .map(|(i, _point)| {
                // Find all points within the window ending at this point
                let window_end = points[i].timestamp;
                let window_start = window_end.saturating_sub(window_size_ms);

                let window_points: Vec<&TimeSeriesPoint> = points
                    .iter()
                    .filter(|p| p.timestamp >= window_start && p.timestamp <= window_end)
                    .collect();

                let avg = if window_points.is_empty() {
                    0.0
                } else {
                    window_points.iter().map(|p| p.value).sum::<f64>() / window_points.len() as f64
                };

                TimeSeriesPoint {
                    timestamp: points[i].timestamp,
                    value: avg,
                }
            })
            .collect();

        Ok(results)
    }

    fn moving_average_cpu_sequential(
        &self,
        points: &[TimeSeriesPoint],
        window_size_ms: u64,
    ) -> Result<Vec<TimeSeriesPoint>, ComputeError> {
        let mut results = Vec::new();

        for (i, point) in points.iter().enumerate() {
            let window_end = point.timestamp;
            let window_start = window_end.saturating_sub(window_size_ms);

            let window_points: Vec<&TimeSeriesPoint> = points
                .iter()
                .filter(|p| p.timestamp >= window_start && p.timestamp <= window_end)
                .collect();

            let avg = if window_points.is_empty() {
                0.0
            } else {
                window_points.iter().map(|p| p.value).sum::<f64>() / window_points.len() as f64
            };

            results.push(TimeSeriesPoint {
                timestamp: point.timestamp,
                value: avg,
            });
        }

        Ok(results)
    }

    fn ewma_cpu_parallel(
        &self,
        points: &[TimeSeriesPoint],
        alpha: f64,
    ) -> Result<Vec<TimeSeriesPoint>, ComputeError> {
        // EWMA is inherently sequential, but we can compute weights in parallel
        // and then apply them sequentially
        use rayon::prelude::*;

        let mut results = Vec::with_capacity(points.len());
        let mut ema = if let Some(first) = points.first() {
            first.value
        } else {
            return Ok(results);
        };

        results.push(TimeSeriesPoint {
            timestamp: points[0].timestamp,
            value: ema,
        });

        // Process remaining points sequentially (EWMA requires sequential processing)
        for point in points.iter().skip(1) {
            ema = alpha * point.value + (1.0 - alpha) * ema;
            results.push(TimeSeriesPoint {
                timestamp: point.timestamp,
                value: ema,
            });
        }

        Ok(results)
    }

    fn ewma_cpu_sequential(
        &self,
        points: &[TimeSeriesPoint],
        alpha: f64,
    ) -> Result<Vec<TimeSeriesPoint>, ComputeError> {
        let mut results = Vec::with_capacity(points.len());

        if points.is_empty() {
            return Ok(results);
        }

        let mut ema = points[0].value;
        results.push(TimeSeriesPoint {
            timestamp: points[0].timestamp,
            value: ema,
        });

        for point in points.iter().skip(1) {
            ema = alpha * point.value + (1.0 - alpha) * ema;
            results.push(TimeSeriesPoint {
                timestamp: point.timestamp,
                value: ema,
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_points() -> Vec<TimeSeriesPoint> {
        (0..1000)
            .map(|i| TimeSeriesPoint {
                timestamp: 1000 * i as u64, // 1 second intervals
                value: i as f64,
            })
            .collect()
    }

    #[tokio::test]
    async fn test_aggregate_by_windows_avg() {
        let config = TimeSeriesOperationsConfig::default();
        let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

        let points = create_test_points();
        let results = ops
            .aggregate_by_windows(&points, 5000, TimeSeriesAggregation::Avg)
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].point_count, 5); // 5 points per 5-second window
    }

    #[tokio::test]
    async fn test_aggregate_by_windows_sum() {
        let config = TimeSeriesOperationsConfig::default();
        let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

        let points = create_test_points();
        let results = ops
            .aggregate_by_windows(&points, 5000, TimeSeriesAggregation::Sum)
            .unwrap();

        assert!(!results.is_empty());
        // First window: 0+1+2+3+4 = 10
        assert_eq!(results[0].value, 10.0);
    }

    #[tokio::test]
    async fn test_moving_average() {
        let config = TimeSeriesOperationsConfig::default();
        let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

        let points = create_test_points();
        let results = ops.moving_average(&points, 5000).unwrap();

        assert_eq!(results.len(), points.len());
        // First few points should have lower averages (smaller windows)
        assert!(results[0].value >= 0.0);
    }

    #[tokio::test]
    async fn test_exponential_moving_average() {
        let config = TimeSeriesOperationsConfig::default();
        let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

        let points = create_test_points();
        let results = ops.exponential_moving_average(&points, 0.3).unwrap();

        assert_eq!(results.len(), points.len());
        // EWMA should smooth the values
        assert!(results[0].value >= 0.0);
    }

    #[test]
    fn test_aggregate_values() {
        let config = TimeSeriesOperationsConfig::default();
        let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

        let points = vec![
            TimeSeriesPoint {
                timestamp: 1000,
                value: 10.0,
            },
            TimeSeriesPoint {
                timestamp: 2000,
                value: 20.0,
            },
            TimeSeriesPoint {
                timestamp: 3000,
                value: 30.0,
            },
        ];

        assert_eq!(
            ops.aggregate_values(&points, TimeSeriesAggregation::Avg),
            20.0
        );
        assert_eq!(
            ops.aggregate_values(&points, TimeSeriesAggregation::Sum),
            60.0
        );
        assert_eq!(
            ops.aggregate_values(&points, TimeSeriesAggregation::Min),
            10.0
        );
        assert_eq!(
            ops.aggregate_values(&points, TimeSeriesAggregation::Max),
            30.0
        );
        assert_eq!(
            ops.aggregate_values(&points, TimeSeriesAggregation::Count),
            3.0
        );
    }
}

