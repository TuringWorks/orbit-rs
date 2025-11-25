//! Time series aggregation functions
//!
//! Provides advanced aggregation and analysis functions for time series data:
//! - Moving averages (simple and exponential)
//! - Rate calculations
//! - Derivative computation
//! - Anomaly detection using statistical methods

use super::{DataPoint, TimeSeriesValue, Timestamp};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::time::Duration;

/// Advanced aggregation functions for time series data
pub struct TimeSeriesAggregator;

impl TimeSeriesAggregator {
    /// Calculate moving average with configurable window
    ///
    /// Returns data points with the average value over the specified window.
    /// The window slides through the data, computing the average of all points
    /// that fall within the window duration.
    ///
    /// # Arguments
    /// * `data_points` - Input time series data (must be sorted by timestamp)
    /// * `window_size` - Duration of the sliding window
    ///
    /// # Returns
    /// Vector of data points with averaged values at each original timestamp
    pub fn moving_average(
        data_points: &[DataPoint],
        window_size: Duration,
    ) -> Result<Vec<DataPoint>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let window_ms = window_size.as_millis() as i64;
        let mut result = Vec::with_capacity(data_points.len());

        for (i, point) in data_points.iter().enumerate() {
            let current_ts = point.timestamp;
            let window_start = current_ts - window_ms;

            // Collect values within the window
            let mut sum = 0.0;
            let mut count = 0;

            // Look backward from current point
            for j in (0..=i).rev() {
                if data_points[j].timestamp < window_start {
                    break;
                }
                if let Some(val) = Self::to_f64(&data_points[j].value) {
                    sum += val;
                    count += 1;
                }
            }

            if count > 0 {
                result.push(DataPoint {
                    timestamp: current_ts,
                    value: TimeSeriesValue::Float(sum / count as f64),
                    labels: point.labels.clone(),
                });
            }
        }

        Ok(result)
    }

    /// Calculate exponential weighted moving average
    ///
    /// EWMA gives more weight to recent observations while not discarding
    /// older observations entirely. The formula is:
    /// EWMA_t = alpha * value_t + (1 - alpha) * EWMA_{t-1}
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    /// * `alpha` - Smoothing factor (0 < alpha <= 1). Higher values give more weight to recent values.
    ///
    /// # Returns
    /// Vector of data points with EWMA values
    pub fn exponential_moving_average(
        data_points: &[DataPoint],
        alpha: f64,
    ) -> Result<Vec<DataPoint>> {
        if alpha <= 0.0 || alpha > 1.0 {
            return Err(anyhow!("Alpha must be in range (0, 1]"));
        }

        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(data_points.len());
        let mut ewma: Option<f64> = None;

        for point in data_points {
            let value = match Self::to_f64(&point.value) {
                Some(v) => v,
                None => continue,
            };

            let new_ewma = match ewma {
                Some(prev) => alpha * value + (1.0 - alpha) * prev,
                None => value, // First value initializes the EWMA
            };

            ewma = Some(new_ewma);

            result.push(DataPoint {
                timestamp: point.timestamp,
                value: TimeSeriesValue::Float(new_ewma),
                labels: point.labels.clone(),
            });
        }

        Ok(result)
    }

    /// Calculate rate of change between consecutive points
    ///
    /// Rate is computed as (value[i] - value[i-1]) / time_delta_seconds
    /// This is useful for counter metrics to convert cumulative values to rates.
    ///
    /// # Arguments
    /// * `data_points` - Input time series data (should be sorted by timestamp)
    ///
    /// # Returns
    /// Vector of data points with rate values (one fewer than input)
    pub fn rate(data_points: &[DataPoint]) -> Result<Vec<DataPoint>> {
        if data_points.len() < 2 {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(data_points.len() - 1);

        for i in 1..data_points.len() {
            let prev = &data_points[i - 1];
            let curr = &data_points[i];

            let prev_val = match Self::to_f64(&prev.value) {
                Some(v) => v,
                None => continue,
            };
            let curr_val = match Self::to_f64(&curr.value) {
                Some(v) => v,
                None => continue,
            };

            // Time delta in seconds
            let time_delta_ms = curr.timestamp - prev.timestamp;
            if time_delta_ms <= 0 {
                continue;
            }
            let time_delta_sec = time_delta_ms as f64 / 1000.0;

            let rate = (curr_val - prev_val) / time_delta_sec;

            result.push(DataPoint {
                timestamp: curr.timestamp,
                value: TimeSeriesValue::Float(rate),
                labels: curr.labels.clone(),
            });
        }

        Ok(result)
    }

    /// Calculate derivative (rate of change over time)
    ///
    /// Similar to rate but specifically designed for gauge metrics.
    /// Returns the instantaneous rate of change.
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    ///
    /// # Returns
    /// Vector of data points with derivative values
    pub fn derivative(data_points: &[DataPoint]) -> Result<Vec<DataPoint>> {
        // Derivative is mathematically the same as rate for discrete data
        Self::rate(data_points)
    }

    /// Detect anomalies using statistical methods (Z-score)
    ///
    /// Points are considered anomalies if their Z-score exceeds the threshold.
    /// Z-score = (value - mean) / std_dev
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    /// * `threshold_std_dev` - Number of standard deviations to consider anomalous
    ///
    /// # Returns
    /// Vector of data points that are considered anomalies
    pub fn detect_anomalies(
        data_points: &[DataPoint],
        threshold_std_dev: f64,
    ) -> Result<Vec<DataPoint>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        // Collect all numeric values
        let values: Vec<f64> = data_points
            .iter()
            .filter_map(|p| Self::to_f64(&p.value))
            .collect();

        if values.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate mean
        let mean = values.iter().sum::<f64>() / values.len() as f64;

        // Calculate standard deviation
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        // If std_dev is zero, no anomalies (all values are the same)
        if std_dev == 0.0 {
            return Ok(Vec::new());
        }

        // Find anomalies
        let anomalies: Vec<DataPoint> = data_points
            .iter()
            .filter_map(|point| {
                let value = Self::to_f64(&point.value)?;
                let z_score = (value - mean).abs() / std_dev;
                if z_score > threshold_std_dev {
                    Some(point.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(anomalies)
    }

    /// Calculate percentile value from a set of data points
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    /// * `percentile` - Percentile to calculate (0-100)
    ///
    /// # Returns
    /// The percentile value as f64
    pub fn percentile(data_points: &[DataPoint], percentile: f64) -> Result<f64> {
        if percentile < 0.0 || percentile > 100.0 {
            return Err(anyhow!("Percentile must be between 0 and 100"));
        }

        let mut values: Vec<f64> = data_points
            .iter()
            .filter_map(|p| Self::to_f64(&p.value))
            .collect();

        if values.is_empty() {
            return Err(anyhow!("No numeric values in data points"));
        }

        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let index = (percentile / 100.0 * (values.len() - 1) as f64).round() as usize;
        Ok(values[index.min(values.len() - 1)])
    }

    /// Calculate histogram buckets for data points
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    /// * `bucket_count` - Number of buckets to create
    ///
    /// # Returns
    /// Vector of (bucket_upper_bound, count) tuples
    pub fn histogram(data_points: &[DataPoint], bucket_count: usize) -> Result<Vec<(f64, usize)>> {
        if bucket_count == 0 {
            return Err(anyhow!("Bucket count must be greater than 0"));
        }

        let values: Vec<f64> = data_points
            .iter()
            .filter_map(|p| Self::to_f64(&p.value))
            .collect();

        if values.is_empty() {
            return Ok(Vec::new());
        }

        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        if min == max {
            return Ok(vec![(max, values.len())]);
        }

        let bucket_width = (max - min) / bucket_count as f64;
        let mut buckets = vec![0usize; bucket_count];

        for value in values {
            let bucket_index = ((value - min) / bucket_width).floor() as usize;
            let bucket_index = bucket_index.min(bucket_count - 1);
            buckets[bucket_index] += 1;
        }

        let result: Vec<(f64, usize)> = buckets
            .into_iter()
            .enumerate()
            .map(|(i, count)| (min + (i + 1) as f64 * bucket_width, count))
            .collect();

        Ok(result)
    }

    /// Downsample time series data by averaging values within time buckets
    ///
    /// # Arguments
    /// * `data_points` - Input time series data (must be sorted by timestamp)
    /// * `interval` - Duration of each bucket
    ///
    /// # Returns
    /// Downsampled data points with averaged values
    pub fn downsample(data_points: &[DataPoint], interval: Duration) -> Result<Vec<DataPoint>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let interval_ms = interval.as_millis() as Timestamp;
        if interval_ms == 0 {
            return Err(anyhow!("Interval must be greater than 0"));
        }

        let mut buckets: HashMap<Timestamp, (f64, usize, HashMap<String, String>)> = HashMap::new();

        for point in data_points {
            let value = match Self::to_f64(&point.value) {
                Some(v) => v,
                None => continue,
            };

            // Round down to bucket start
            let bucket_start = (point.timestamp / interval_ms) * interval_ms;

            let entry = buckets
                .entry(bucket_start)
                .or_insert((0.0, 0, point.labels.clone()));
            entry.0 += value;
            entry.1 += 1;
        }

        let mut result: Vec<DataPoint> = buckets
            .into_iter()
            .map(|(ts, (sum, count, labels))| DataPoint {
                timestamp: ts,
                value: TimeSeriesValue::Float(sum / count as f64),
                labels,
            })
            .collect();

        result.sort_by_key(|p| p.timestamp);
        Ok(result)
    }

    /// Calculate the delta between consecutive data points
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    ///
    /// # Returns
    /// Vector of data points with delta values
    pub fn delta(data_points: &[DataPoint]) -> Result<Vec<DataPoint>> {
        if data_points.len() < 2 {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(data_points.len() - 1);

        for i in 1..data_points.len() {
            let prev = &data_points[i - 1];
            let curr = &data_points[i];

            let prev_val = match Self::to_f64(&prev.value) {
                Some(v) => v,
                None => continue,
            };
            let curr_val = match Self::to_f64(&curr.value) {
                Some(v) => v,
                None => continue,
            };

            result.push(DataPoint {
                timestamp: curr.timestamp,
                value: TimeSeriesValue::Float(curr_val - prev_val),
                labels: curr.labels.clone(),
            });
        }

        Ok(result)
    }

    /// Calculate cumulative sum of data points
    ///
    /// # Arguments
    /// * `data_points` - Input time series data
    ///
    /// # Returns
    /// Vector of data points with cumulative sum values
    pub fn cumulative_sum(data_points: &[DataPoint]) -> Result<Vec<DataPoint>> {
        if data_points.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(data_points.len());
        let mut sum = 0.0;

        for point in data_points {
            let value = match Self::to_f64(&point.value) {
                Some(v) => v,
                None => continue,
            };

            sum += value;

            result.push(DataPoint {
                timestamp: point.timestamp,
                value: TimeSeriesValue::Float(sum),
                labels: point.labels.clone(),
            });
        }

        Ok(result)
    }

    /// Convert TimeSeriesValue to f64 for calculations
    fn to_f64(value: &TimeSeriesValue) -> Option<f64> {
        match value {
            TimeSeriesValue::Float(f) => Some(*f),
            TimeSeriesValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<DataPoint> {
        vec![
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
                value: TimeSeriesValue::Float(15.0),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 4000,
                value: TimeSeriesValue::Float(25.0),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 5000,
                value: TimeSeriesValue::Float(30.0),
                labels: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_moving_average() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::moving_average(&data, Duration::from_millis(2500))
            .unwrap();

        assert_eq!(result.len(), 5);

        // First point: just itself (10.0)
        if let TimeSeriesValue::Float(v) = result[0].value {
            assert!((v - 10.0).abs() < 0.001);
        }

        // Third point: should average points within 2500ms window
        // Points at 1000, 2000, 3000 -> values 10, 20, 15
        if let TimeSeriesValue::Float(v) = result[2].value {
            assert!((v - 15.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_exponential_moving_average() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::exponential_moving_average(&data, 0.5).unwrap();

        assert_eq!(result.len(), 5);

        // First point: EWMA = 10.0
        if let TimeSeriesValue::Float(v) = result[0].value {
            assert!((v - 10.0).abs() < 0.001);
        }

        // Second point: EWMA = 0.5 * 20 + 0.5 * 10 = 15.0
        if let TimeSeriesValue::Float(v) = result[1].value {
            assert!((v - 15.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_ewma_invalid_alpha() {
        let data = create_test_data();
        assert!(TimeSeriesAggregator::exponential_moving_average(&data, 0.0).is_err());
        assert!(TimeSeriesAggregator::exponential_moving_average(&data, 1.5).is_err());
    }

    #[test]
    fn test_rate() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::rate(&data).unwrap();

        // 4 rate values for 5 data points
        assert_eq!(result.len(), 4);

        // Rate from 10 to 20 over 1 second = 10/s
        if let TimeSeriesValue::Float(v) = result[0].value {
            assert!((v - 10.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_derivative() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::derivative(&data).unwrap();

        // Same as rate
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_detect_anomalies() {
        // Create data with more normal points to establish baseline, then outlier
        let mut data = Vec::new();

        // Add 20 normal points around 10.0
        for i in 0..20 {
            data.push(DataPoint {
                timestamp: i * 1000,
                value: TimeSeriesValue::Float(10.0 + (i as f64 % 3.0) - 1.0), // 9, 10, 11 cycle
                labels: HashMap::new(),
            });
        }

        // Add an extreme outlier
        data.push(DataPoint {
            timestamp: 20000,
            value: TimeSeriesValue::Float(100.0), // Outlier
            labels: HashMap::new(),
        });

        let anomalies = TimeSeriesAggregator::detect_anomalies(&data, 2.0).unwrap();

        // Should detect the outlier at timestamp 20000
        assert!(!anomalies.is_empty(), "Should detect at least one anomaly");
        assert!(
            anomalies.iter().any(|p| p.timestamp == 20000),
            "Should detect the outlier at timestamp 20000"
        );
    }

    #[test]
    fn test_percentile() {
        let data = create_test_data();

        let p50 = TimeSeriesAggregator::percentile(&data, 50.0).unwrap();
        assert!((p50 - 20.0).abs() < 0.001);

        let p0 = TimeSeriesAggregator::percentile(&data, 0.0).unwrap();
        assert!((p0 - 10.0).abs() < 0.001);

        let p100 = TimeSeriesAggregator::percentile(&data, 100.0).unwrap();
        assert!((p100 - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_histogram() {
        let data = create_test_data();
        let hist = TimeSeriesAggregator::histogram(&data, 2).unwrap();

        assert_eq!(hist.len(), 2);
        // Values: 10, 15, 20, 25, 30 -> range 10-30
        // Bucket 1: 10-20 (10, 15, 20) = 3
        // Bucket 2: 20-30 (25, 30) = 2
        assert_eq!(hist[0].1 + hist[1].1, 5);
    }

    #[test]
    fn test_downsample() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::downsample(&data, Duration::from_millis(2000)).unwrap();

        // Should have fewer points after downsampling
        assert!(result.len() < data.len());
    }

    #[test]
    fn test_delta() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::delta(&data).unwrap();

        assert_eq!(result.len(), 4);

        // Delta from 10 to 20 = 10
        if let TimeSeriesValue::Float(v) = result[0].value {
            assert!((v - 10.0).abs() < 0.001);
        }

        // Delta from 20 to 15 = -5
        if let TimeSeriesValue::Float(v) = result[1].value {
            assert!((v - (-5.0)).abs() < 0.001);
        }
    }

    #[test]
    fn test_cumulative_sum() {
        let data = create_test_data();
        let result = TimeSeriesAggregator::cumulative_sum(&data).unwrap();

        assert_eq!(result.len(), 5);

        // Cumsum: 10, 30, 45, 70, 100
        if let TimeSeriesValue::Float(v) = result[0].value {
            assert!((v - 10.0).abs() < 0.001);
        }
        if let TimeSeriesValue::Float(v) = result[2].value {
            assert!((v - 45.0).abs() < 0.001);
        }
        if let TimeSeriesValue::Float(v) = result[4].value {
            assert!((v - 100.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_empty_data() {
        let empty: Vec<DataPoint> = Vec::new();

        assert!(TimeSeriesAggregator::moving_average(&empty, Duration::from_secs(1)).unwrap().is_empty());
        assert!(TimeSeriesAggregator::exponential_moving_average(&empty, 0.5).unwrap().is_empty());
        assert!(TimeSeriesAggregator::rate(&empty).unwrap().is_empty());
        assert!(TimeSeriesAggregator::detect_anomalies(&empty, 2.0).unwrap().is_empty());
        assert!(TimeSeriesAggregator::delta(&empty).unwrap().is_empty());
        assert!(TimeSeriesAggregator::cumulative_sum(&empty).unwrap().is_empty());
    }

    #[test]
    fn test_integer_values() {
        let data = vec![
            DataPoint {
                timestamp: 1000,
                value: TimeSeriesValue::Integer(10),
                labels: HashMap::new(),
            },
            DataPoint {
                timestamp: 2000,
                value: TimeSeriesValue::Integer(20),
                labels: HashMap::new(),
            },
        ];

        let result = TimeSeriesAggregator::rate(&data).unwrap();
        assert_eq!(result.len(), 1);
        if let TimeSeriesValue::Float(v) = result[0].value {
            assert!((v - 10.0).abs() < 0.001);
        }
    }
}
