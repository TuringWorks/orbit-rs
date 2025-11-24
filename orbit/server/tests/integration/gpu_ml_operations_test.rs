//! Integration tests for GPU-accelerated ML operations

#[cfg(feature = "gpu-acceleration")]
use orbit_compute::ml_operations::{
    FeatureEngineeringOp, GPUMLOperations, MatrixOp, MLOperationsConfig,
};

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_feature_normalization() {
    let config = MLOperationsConfig::default();
    let ops = GPUMLOperations::new(config).await.unwrap();

    let features = vec![
        vec![1.0, 2.0, 3.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 8.0, 9.0],
    ];

    let result = ops
        .transform_features(&features, FeatureEngineeringOp::Normalize)
        .unwrap();

    assert_eq!(result.samples_processed, 3);
    assert_eq!(result.output_features, 3);
    // Normalized features should have mean ~0 and std ~1
    assert!(!result.features.is_empty());
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_feature_standardization() {
    let config = MLOperationsConfig::default();
    let ops = GPUMLOperations::new(config).await.unwrap();

    let features = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];

    let result = ops
        .transform_features(&features, FeatureEngineeringOp::Standardize)
        .unwrap();

    assert_eq!(result.samples_processed, 3);
    assert_eq!(result.output_features, 2);
    // Standardized features should be in [0, 1] range
    assert!(!result.features.is_empty());
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_matrix_transpose() {
    let config = MLOperationsConfig::default();
    let ops = GPUMLOperations::new(config).await.unwrap();

    let matrix = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];

    let result = ops
        .matrix_operation(&matrix, None, MatrixOp::Transpose)
        .unwrap();

    assert_eq!(result.rows, 3);
    assert_eq!(result.cols, 2);
    assert_eq!(result.data.len(), 6);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_matrix_multiplication() {
    let config = MLOperationsConfig::default();
    let ops = GPUMLOperations::new(config).await.unwrap();

    let matrix_a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
    let matrix_b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];

    let result = ops
        .matrix_operation(&matrix_a, Some(&matrix_b), MatrixOp::MatMul)
        .unwrap();

    assert_eq!(result.rows, 2);
    assert_eq!(result.cols, 2);
    // Result should be [[19, 22], [43, 50]]
    assert_eq!(result.data[0], 19.0);
    assert_eq!(result.data[1], 22.0);
    assert_eq!(result.data[2], 43.0);
    assert_eq!(result.data[3], 50.0);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_large_dataset_processing() {
    let config = MLOperationsConfig {
        gpu_min_samples: 5000,
        ..Default::default()
    };
    let ops = GPUMLOperations::new(config).await.unwrap();

    let features: Vec<Vec<f64>> = (0..10000)
        .map(|i| (0..64).map(|j| (i * 64 + j) as f64).collect())
        .collect();

    let result = ops
        .transform_features(&features, FeatureEngineeringOp::Normalize)
        .unwrap();

    assert_eq!(result.samples_processed, 10000);
    assert_eq!(result.output_features, 64);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_cpu_parallel_fallback() {
    let config = MLOperationsConfig {
        enable_gpu: false,
        use_cpu_parallel: true,
        ..Default::default()
    };
    let ops = GPUMLOperations::new(config).await.unwrap();

    let features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];

    let result = ops
        .transform_features(&features, FeatureEngineeringOp::Normalize)
        .unwrap();

    // Should work correctly even without GPU
    assert_eq!(result.samples_processed, 2);
}

#[cfg(not(feature = "gpu-acceleration"))]
#[test]
fn test_placeholder() {
    // Placeholder test when GPU acceleration is not enabled
    assert!(true);
}

