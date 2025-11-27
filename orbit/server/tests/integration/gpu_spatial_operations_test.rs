//! Integration tests for GPU-accelerated spatial operations
//!
//! Tests the integration of GPU-accelerated spatial operations and verifies
//! correctness and performance routing.

#![cfg(feature = "gpu-acceleration")]

use orbit_compute::spatial_operations::{
    GPUSpatialOperations, GPUPoint, GPUPolygon, SpatialOperationsConfig,
};

#[tokio::test]
async fn test_spatial_distance_cpu() {
    let config = SpatialOperationsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUSpatialOperations::new(config).await.unwrap();

    let query = GPUPoint { x: 0.0, y: 0.0 };
    let candidates = vec![
        GPUPoint { x: 3.0, y: 4.0 }, // Distance = 5.0
        GPUPoint { x: 0.0, y: 0.0 }, // Distance = 0.0
        GPUPoint { x: 1.0, y: 1.0 }, // Distance = sqrt(2) ≈ 1.414
    ];

    let results = engine.batch_distance(&query, &candidates).await.unwrap();

    assert_eq!(results.len(), 3);
    assert!((results[0].distance - 5.0).abs() < 0.01);
    assert!((results[1].distance - 0.0).abs() < 0.01);
    assert!((results[2].distance - 1.414).abs() < 0.1);
}

#[tokio::test]
async fn test_spatial_distance_sphere_cpu() {
    let config = SpatialOperationsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUSpatialOperations::new(config).await.unwrap();

    // San Francisco to New York (approximately)
    let query = GPUPoint {
        x: -122.4194,
        y: 37.7749,
    };
    let candidates = vec![GPUPoint {
        x: -74.0060,
        y: 40.7128,
    }];

    let results = engine
        .batch_distance_sphere(&query, &candidates)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    // Distance should be approximately 4139 km
    assert!(results[0].distance > 4000000.0); // > 4000 km
    assert!(results[0].distance < 4300000.0); // < 4300 km
}

#[tokio::test]
async fn test_point_in_polygon_cpu() {
    let config = SpatialOperationsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUSpatialOperations::new(config).await.unwrap();

    // Create a square polygon
    let polygon = GPUPolygon {
        exterior_ring: vec![
            GPUPoint { x: 0.0, y: 0.0 },
            GPUPoint { x: 10.0, y: 0.0 },
            GPUPoint { x: 10.0, y: 10.0 },
            GPUPoint { x: 0.0, y: 10.0 },
        ],
        interior_rings: vec![],
    };

    let points = vec![
        GPUPoint { x: 5.0, y: 5.0 },   // Inside
        GPUPoint { x: 15.0, y: 5.0 },  // Outside
        GPUPoint { x: -5.0, y: -5.0 }, // Outside
    ];

    let results = engine
        .batch_point_in_polygon(&points, &polygon)
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    assert!(results[0].result);  // Inside
    assert!(!results[1].result); // Outside
    assert!(!results[2].result); // Outside
}

#[tokio::test]
async fn test_point_in_polygon_with_hole() {
    let config = SpatialOperationsConfig {
        enable_gpu: false,
        ..Default::default()
    };
    let engine = GPUSpatialOperations::new(config).await.unwrap();

    // Create a square polygon with a hole
    let polygon = GPUPolygon {
        exterior_ring: vec![
            GPUPoint { x: 0.0, y: 0.0 },
            GPUPoint { x: 10.0, y: 0.0 },
            GPUPoint { x: 10.0, y: 10.0 },
            GPUPoint { x: 0.0, y: 10.0 },
        ],
        interior_rings: vec![vec![
            GPUPoint { x: 2.0, y: 2.0 },
            GPUPoint { x: 8.0, y: 2.0 },
            GPUPoint { x: 8.0, y: 8.0 },
            GPUPoint { x: 2.0, y: 8.0 },
        ]],
    };

    let points = vec![
        GPUPoint { x: 5.0, y: 5.0 },  // Inside exterior, inside hole (should be outside)
        GPUPoint { x: 1.0, y: 1.0 },  // Inside exterior, outside hole (should be inside)
        GPUPoint { x: 15.0, y: 15.0 }, // Outside exterior (should be outside)
    ];

    let results = engine
        .batch_point_in_polygon(&points, &polygon)
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    assert!(!results[0].result); // Inside hole, so outside polygon
    assert!(results[1].result);  // Inside exterior, outside hole
    assert!(!results[2].result); // Outside exterior
}

#[tokio::test]
async fn test_spatial_operations_config() {
    let config = SpatialOperationsConfig::default();
    assert!(config.enable_gpu);
    assert_eq!(config.gpu_min_geometries, 1000);
    assert!(config.use_cpu_parallel);
}

#[tokio::test]
async fn test_large_dataset_cpu_parallel() {
    // Create a larger dataset to trigger parallel processing
    let config = SpatialOperationsConfig {
        enable_gpu: false,
        use_cpu_parallel: true,
        ..Default::default()
    };
    let engine = GPUSpatialOperations::new(config).await.unwrap();

    let query = GPUPoint { x: 0.0, y: 0.0 };
    let candidates: Vec<GPUPoint> = (0..200)
        .map(|i| GPUPoint {
            x: i as f64 * 0.1,
            y: i as f64 * 0.1,
        })
        .collect();

    let results = engine.batch_distance(&query, &candidates).await.unwrap();

    assert_eq!(results.len(), 200);
    // Results should be sorted by index
    for (idx, result) in results.iter().enumerate() {
        assert_eq!(result.index, idx);
    }
}

