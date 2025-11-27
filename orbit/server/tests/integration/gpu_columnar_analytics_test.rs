//! Integration tests for GPU-accelerated columnar analytics
//!
//! Tests the integration of GPU-accelerated columnar aggregations and verifies
//! correctness and performance routing.

#![cfg(feature = "gpu-acceleration")]

use orbit_compute::columnar_analytics::{
    AggregateFunction, ColumnarAnalyticsConfig, GPUColumnarAnalytics,
};

#[tokio::test]
async fn test_aggregate_i32_sum_cpu() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![1, 2, 3, 4, 5];
    let result = engine
        .aggregate_i32(&values, None, AggregateFunction::Sum)
        .await
        .unwrap();

    assert_eq!(result.value, Some(15.0));
    assert_eq!(result.count, 5);
}

#[tokio::test]
async fn test_aggregate_i32_avg_cpu() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![10, 20, 30, 40, 50];
    let result = engine
        .aggregate_i32(&values, None, AggregateFunction::Avg)
        .await
        .unwrap();

    assert_eq!(result.value, Some(30.0));
    assert_eq!(result.count, 5);
}

#[tokio::test]
async fn test_aggregate_i32_min_max_cpu() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![5, 2, 8, 1, 9];
    let min_result = engine
        .aggregate_i32(&values, None, AggregateFunction::Min)
        .await
        .unwrap();
    let max_result = engine
        .aggregate_i32(&values, None, AggregateFunction::Max)
        .await
        .unwrap();

    assert_eq!(min_result.value, Some(1.0));
    assert_eq!(max_result.value, Some(9.0));
}

#[tokio::test]
async fn test_aggregate_i32_count_cpu() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![1, 2, 3, 4, 5];
    let result = engine
        .aggregate_i32(&values, None, AggregateFunction::Count)
        .await
        .unwrap();

    assert_eq!(result.value, Some(5.0));
    assert_eq!(result.count, 5);
}

#[tokio::test]
async fn test_aggregate_i32_with_nulls() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![1, 2, 3, 4, 5];
    // Create null bitmap: first and last are null (bits 0 and 4 set)
    let null_bitmap = vec![0b00010001u8]; // Bits 0 and 4 are null

    let result = engine
        .aggregate_i32(&values, Some(&null_bitmap), AggregateFunction::Sum)
        .await
        .unwrap();

    // Should sum only values at indices 1, 2, 3 (2 + 3 + 4 = 9)
    assert_eq!(result.value, Some(9.0));
}

#[tokio::test]
async fn test_aggregate_f64_sum_cpu() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![1.5, 2.5, 3.5, 4.5, 5.5];
    let result = engine
        .aggregate_f64(&values, None, AggregateFunction::Sum)
        .await
        .unwrap();

    assert!((result.value.unwrap() - 17.5).abs() < 0.01);
    assert_eq!(result.count, 5);
}

#[tokio::test]
async fn test_aggregate_f64_avg_cpu() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    let result = engine
        .aggregate_f64(&values, None, AggregateFunction::Avg)
        .await
        .unwrap();

    assert!((result.value.unwrap() - 30.0).abs() < 0.01);
    assert_eq!(result.count, 5);
}

#[tokio::test]
async fn test_large_dataset_cpu_parallel() {
    // Create a larger dataset to trigger parallel processing
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        use_cpu_parallel: true,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values: Vec<i32> = (0..10000).collect();
    let result = engine
        .aggregate_i32(&values, None, AggregateFunction::Sum)
        .await
        .unwrap();

    // Sum of 0..9999 = 49995000
    let expected_sum: i64 = (0..10000).sum::<i32>() as i64;
    assert_eq!(result.value, Some(expected_sum as f64));
    assert_eq!(result.count, 10000);
}

#[tokio::test]
async fn test_columnar_config_defaults() {
    let config = ColumnarAnalyticsConfig::default();
    assert!(config.enable_gpu);
    assert_eq!(config.gpu_min_rows, 10000);
    assert!(config.use_cpu_parallel);
}

#[tokio::test]
async fn test_all_null_values() {
    let config = ColumnarAnalyticsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUColumnarAnalytics::new(config).await.unwrap();

    let values = vec![1, 2, 3, 4, 5];
    // All values are null
    let null_bitmap = vec![0b11111111u8];

    let result = engine
        .aggregate_i32(&values, Some(&null_bitmap), AggregateFunction::Sum)
        .await
        .unwrap();

    // Should return None when all values are null
    assert_eq!(result.value, None);
}

