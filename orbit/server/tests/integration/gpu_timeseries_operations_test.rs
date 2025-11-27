//! Integration tests for GPU-accelerated time-series operations

#[cfg(feature = "gpu-acceleration")]
use orbit_compute::timeseries_operations::{
    GPUTimeSeriesOperations, TimeSeriesAggregation, TimeSeriesOperationsConfig, TimeSeriesPoint,
};

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_aggregate_by_windows_avg() {
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
        TimeSeriesPoint {
            timestamp: 4000,
            value: 40.0,
        },
        TimeSeriesPoint {
            timestamp: 5000,
            value: 50.0,
        },
    ];

    let results = ops
        .aggregate_by_windows(&points, 3000, TimeSeriesAggregation::Avg)
        .unwrap();

    assert!(!results.is_empty());
    // First window (0-3000ms): 10, 20, 30 -> avg = 20.0
    assert_eq!(results[0].value, 20.0);
    assert_eq!(results[0].point_count, 3);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_aggregate_by_windows_sum() {
    let config = TimeSeriesOperationsConfig::default();
    let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

    let points: Vec<TimeSeriesPoint> = (0..100)
        .map(|i| TimeSeriesPoint {
            timestamp: 1000 * i as u64,
            value: i as f64,
        })
        .collect();

    let results = ops
        .aggregate_by_windows(&points, 10000, TimeSeriesAggregation::Sum)
        .unwrap();

    assert!(!results.is_empty());
    // First window (0-10000ms): 0+1+2+...+9 = 45
    assert_eq!(results[0].value, 45.0);
    assert_eq!(results[0].point_count, 10);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_aggregate_by_windows_min_max() {
    let config = TimeSeriesOperationsConfig::default();
    let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

    let points = vec![
        TimeSeriesPoint {
            timestamp: 1000,
            value: 10.0,
        },
        TimeSeriesPoint {
            timestamp: 2000,
            value: 5.0,
        },
        TimeSeriesPoint {
            timestamp: 3000,
            value: 20.0,
        },
    ];

    let results = ops
        .aggregate_by_windows(&points, 5000, TimeSeriesAggregation::Range)
        .unwrap();

    assert!(!results.is_empty());
    // Range = max - min = 20.0 - 5.0 = 15.0
    assert_eq!(results[0].value, 15.0);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_moving_average() {
    let config = TimeSeriesOperationsConfig::default();
    let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

    let points: Vec<TimeSeriesPoint> = (0..100)
        .map(|i| TimeSeriesPoint {
            timestamp: 1000 * i as u64,
            value: i as f64,
        })
        .collect();

    let results = ops.moving_average(&points, 5000).unwrap();

    assert_eq!(results.len(), points.len());
    // First point should have value = 0 (only itself in window)
    assert_eq!(results[0].value, 0.0);
    // Later points should have averages
    assert!(results[50].value > 0.0);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_exponential_moving_average() {
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

    let results = ops.exponential_moving_average(&points, 0.5).unwrap();

    assert_eq!(results.len(), points.len());
    // First value should be unchanged
    assert_eq!(results[0].value, 10.0);
    // Second value: 0.5 * 20.0 + 0.5 * 10.0 = 15.0
    assert_eq!(results[1].value, 15.0);
    // Third value: 0.5 * 30.0 + 0.5 * 15.0 = 22.5
    assert_eq!(results[2].value, 22.5);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_large_dataset_aggregation() {
    let config = TimeSeriesOperationsConfig {
        gpu_min_points: 5000,
        ..Default::default()
    };
    let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

    let points: Vec<TimeSeriesPoint> = (0..10000)
        .map(|i| TimeSeriesPoint {
            timestamp: 1000 * i as u64,
            value: (i as f64).sin(),
        })
        .collect();

    let results = ops
        .aggregate_by_windows(&points, 10000, TimeSeriesAggregation::Avg)
        .unwrap();

    assert!(!results.is_empty());
    assert!(results.len() <= 1000); // Should have fewer windows than points
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_empty_points() {
    let config = TimeSeriesOperationsConfig::default();
    let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

    let points = vec![];
    let results = ops
        .aggregate_by_windows(&points, 1000, TimeSeriesAggregation::Avg)
        .unwrap();

    assert!(results.is_empty());
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_cpu_parallel_fallback() {
    let config = TimeSeriesOperationsConfig {
        enable_gpu: false,
        use_cpu_parallel: true,
        ..Default::default()
    };
    let ops = GPUTimeSeriesOperations::new(config).await.unwrap();

    let points: Vec<TimeSeriesPoint> = (0..5000)
        .map(|i| TimeSeriesPoint {
            timestamp: 1000 * i as u64,
            value: i as f64,
        })
        .collect();

    let results = ops
        .aggregate_by_windows(&points, 5000, TimeSeriesAggregation::Sum)
        .unwrap();

    assert!(!results.is_empty());
    // Should work correctly even without GPU
    assert!(results[0].value > 0.0);
}

#[cfg(not(feature = "gpu-acceleration"))]
#[test]
fn test_placeholder() {
    // Placeholder test when GPU acceleration is not enabled
    assert!(true);
}

