//! Workload Predictor
//!
//! Time series forecasting for workload prediction and resource planning.

use anyhow::Result as OrbitResult;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Workload forecast
#[derive(Debug, Clone)]
pub struct WorkloadForecast {
    pub predicted_cpu: f64,
    pub predicted_memory: f64,
    pub predicted_io: f64,
    pub confidence: f64,
    pub time_horizon: tokio::time::Duration,
    pub forecast_points: Vec<ForecastPoint>,
}

/// Forecast point at a specific time
#[derive(Debug, Clone)]
pub struct ForecastPoint {
    pub timestamp: std::time::SystemTime,
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub io_utilization: f64,
    pub confidence: f64,
}

/// Workload measurement
#[derive(Debug, Clone)]
pub struct WorkloadMeasurement {
    pub timestamp: std::time::SystemTime,
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub io_utilization: f64,
    pub query_count: u64,
    pub transaction_count: u64,
}

/// Time series forecasting model for workload prediction
pub struct WorkloadPredictor {
    /// Historical workload data (rolling window)
    workload_history: Arc<RwLock<VecDeque<WorkloadMeasurement>>>,
    /// Maximum history size
    max_history_size: usize,
    /// Seasonal pattern detector
    seasonal_patterns: Arc<RwLock<SeasonalPatterns>>,
}

/// Seasonal patterns detected in workload
#[derive(Debug, Clone, Default)]
pub struct SeasonalPatterns {
    /// Hourly patterns (24 hours)
    hourly_pattern: Vec<f64>,
    /// Daily patterns (7 days)
    _daily_pattern: Vec<f64>,
    /// Weekly patterns (52 weeks)
    _weekly_pattern: Vec<f64>,
}

impl WorkloadPredictor {
    /// Create a new workload predictor
    pub fn new(max_history_size: usize) -> Self {
        Self {
            workload_history: Arc::new(RwLock::new(VecDeque::with_capacity(max_history_size))),
            max_history_size,
            seasonal_patterns: Arc::new(RwLock::new(SeasonalPatterns::default())),
        }
    }

    /// Record a workload measurement
    pub async fn record_measurement(&self, measurement: WorkloadMeasurement) -> OrbitResult<()> {
        let mut history = self.workload_history.write().await;

        // Add new measurement
        history.push_back(measurement);

        // Trim if exceeds max size
        while history.len() > self.max_history_size {
            history.pop_front();
        }

        // Update seasonal patterns periodically
        if history.len() % 100 == 0 {
            self.update_seasonal_patterns(&history).await?;
        }

        Ok(())
    }

    /// Forecast workload for given time horizon
    pub async fn forecast_workload(
        &self,
        horizon: tokio::time::Duration,
    ) -> OrbitResult<WorkloadForecast> {
        let history = self.workload_history.read().await;

        if history.is_empty() {
            // No history - return default forecast
            return Ok(WorkloadForecast {
                predicted_cpu: 50.0,
                predicted_memory: 50.0,
                predicted_io: 50.0,
                confidence: 0.5,
                time_horizon: horizon,
                forecast_points: vec![],
            });
        }

        // Calculate base forecast using moving average
        let recent_avg = self.calculate_recent_average(&history);

        // Apply seasonal adjustments
        let seasonal_patterns = self.seasonal_patterns.read().await;
        let adjusted_forecast =
            self.apply_seasonal_adjustments(&recent_avg, &seasonal_patterns, horizon);

        // Generate forecast points
        let forecast_points =
            self.generate_forecast_points(&recent_avg, &seasonal_patterns, horizon);

        Ok(WorkloadForecast {
            predicted_cpu: adjusted_forecast.cpu,
            predicted_memory: adjusted_forecast.memory,
            predicted_io: adjusted_forecast.io,
            confidence: self.calculate_confidence(&history),
            time_horizon: horizon,
            forecast_points,
        })
    }

    /// Calculate recent average workload
    fn calculate_recent_average(&self, history: &VecDeque<WorkloadMeasurement>) -> WorkloadAverage {
        if history.is_empty() {
            return WorkloadAverage {
                cpu: 0.0,
                memory: 0.0,
                io: 0.0,
            };
        }

        // Use last 100 measurements or all if less
        let recent_count = history.len().min(100);
        let recent: Vec<_> = history.iter().rev().take(recent_count).collect();

        let cpu_sum: f64 = recent.iter().map(|m| m.cpu_utilization).sum();
        let memory_sum: f64 = recent.iter().map(|m| m.memory_utilization).sum();
        let io_sum: f64 = recent.iter().map(|m| m.io_utilization).sum();

        WorkloadAverage {
            cpu: cpu_sum / recent_count as f64,
            memory: memory_sum / recent_count as f64,
            io: io_sum / recent_count as f64,
        }
    }

    /// Apply seasonal adjustments to forecast
    fn apply_seasonal_adjustments(
        &self,
        base: &WorkloadAverage,
        patterns: &SeasonalPatterns,
        _horizon: tokio::time::Duration,
    ) -> WorkloadAverage {
        // Simple seasonal adjustment based on current hour
        let now = std::time::SystemTime::now();
        let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        let hours_since_epoch = duration.as_secs() / 3600;
        let current_hour = (hours_since_epoch % 24) as usize;

        let hourly_factor = if current_hour < patterns.hourly_pattern.len() {
            patterns.hourly_pattern[current_hour]
        } else {
            1.0
        };

        WorkloadAverage {
            cpu: base.cpu * hourly_factor,
            memory: base.memory * hourly_factor,
            io: base.io * hourly_factor,
        }
    }

    /// Generate forecast points over time horizon
    fn generate_forecast_points(
        &self,
        base: &WorkloadAverage,
        patterns: &SeasonalPatterns,
        horizon: tokio::time::Duration,
    ) -> Vec<ForecastPoint> {
        let mut points = Vec::new();
        let now = std::time::SystemTime::now();
        let interval_secs = 300; // 5 minute intervals
        let num_points = (horizon.as_secs() / interval_secs).min(100);

        for i in 0..num_points {
            let timestamp = now + tokio::time::Duration::from_secs(i * interval_secs);
            let hours_ahead = (i * interval_secs) / 3600;
            let hour_index = (hours_ahead % 24) as usize;

            let hourly_factor = if hour_index < patterns.hourly_pattern.len() {
                patterns.hourly_pattern[hour_index]
            } else {
                1.0
            };

            points.push(ForecastPoint {
                timestamp,
                cpu_utilization: base.cpu * hourly_factor,
                memory_utilization: base.memory * hourly_factor,
                io_utilization: base.io * hourly_factor,
                confidence: 0.7, // Base confidence
            });
        }

        points
    }

    /// Calculate forecast confidence based on history
    fn calculate_confidence(&self, history: &VecDeque<WorkloadMeasurement>) -> f64 {
        if history.len() < 10 {
            return 0.3; // Low confidence with little data
        }

        if history.len() < 100 {
            return 0.5; // Medium confidence
        }

        // High confidence with sufficient history
        0.8
    }

    /// Update seasonal patterns from history
    async fn update_seasonal_patterns(
        &self,
        history: &VecDeque<WorkloadMeasurement>,
    ) -> OrbitResult<()> {
        if history.len() < 24 {
            return Ok(()); // Need at least 24 hours of data
        }

        let mut patterns = self.seasonal_patterns.write().await;

        // Calculate hourly patterns
        let mut hourly_sums = vec![0.0; 24];
        let mut hourly_counts = vec![0; 24];

        for measurement in history.iter() {
            let duration = measurement
                .timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap();
            let hours_since_epoch = duration.as_secs() / 3600;
            let hour = (hours_since_epoch % 24) as usize;

            if hour < 24 {
                hourly_sums[hour] += measurement.cpu_utilization;
                hourly_counts[hour] += 1;
            }
        }

        // Calculate averages
        for hour in 0..24 {
            if hourly_counts[hour] > 0 {
                patterns.hourly_pattern[hour] = hourly_sums[hour] / hourly_counts[hour] as f64;
            }
        }

        // Normalize patterns
        let avg_pattern: f64 = patterns.hourly_pattern.iter().sum::<f64>() / 24.0;
        if avg_pattern > 0.0 {
            for hour in 0..24 {
                patterns.hourly_pattern[hour] /= avg_pattern;
            }
        }

        debug!(
            "Updated seasonal patterns from {} measurements",
            history.len()
        );
        Ok(())
    }

    /// Predict resource demand for given time horizon
    pub async fn predict_resource_demand(
        &self,
        horizon: tokio::time::Duration,
    ) -> OrbitResult<ResourceDemand> {
        let forecast = self.forecast_workload(horizon).await?;

        Ok(ResourceDemand {
            cpu_cores: (forecast.predicted_cpu / 100.0 * 8.0).ceil() as u32, // Assume 8 cores
            memory_mb: (forecast.predicted_memory / 100.0 * 16384.0) as u64, // Assume 16GB
            io_bandwidth_mbps: (forecast.predicted_io / 100.0 * 1000.0) as u64, // Assume 1Gbps
            confidence: forecast.confidence,
        })
    }
}

/// Workload average
#[derive(Debug, Clone)]
struct WorkloadAverage {
    cpu: f64,
    memory: f64,
    io: f64,
}

/// Resource demand prediction
#[derive(Debug, Clone)]
pub struct ResourceDemand {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub io_bandwidth_mbps: u64,
    pub confidence: f64,
}

impl Default for WorkloadPredictor {
    fn default() -> Self {
        Self::new(10000) // Default: 10k measurements
    }
}
