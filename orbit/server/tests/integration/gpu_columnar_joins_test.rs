//! Integration tests for GPU-accelerated columnar joins

#[cfg(feature = "gpu-acceleration")]
use orbit_compute::columnar_joins::{
    ColumnarColumn, ColumnarJoinsConfig, GPUColumnarJoins, JoinType,
};

#[cfg(feature = "gpu-acceleration")]
fn create_test_left_columns() -> Vec<ColumnarColumn> {
    vec![
        ColumnarColumn {
            name: "id".to_string(),
            i32_values: Some(vec![1, 2, 3, 4, 5]),
            i64_values: None,
            f64_values: None,
            string_values: None,
            null_bitmap: None,
        },
        ColumnarColumn {
            name: "name".to_string(),
            string_values: Some(vec![
                "Alice".to_string(),
                "Bob".to_string(),
                "Charlie".to_string(),
                "David".to_string(),
                "Eve".to_string(),
            ]),
            i32_values: None,
            i64_values: None,
            f64_values: None,
            null_bitmap: None,
        },
    ]
}

#[cfg(feature = "gpu-acceleration")]
fn create_test_right_columns() -> Vec<ColumnarColumn> {
    vec![
        ColumnarColumn {
            name: "id".to_string(),
            i32_values: Some(vec![2, 3, 6, 7]),
            i64_values: None,
            f64_values: None,
            string_values: None,
            null_bitmap: None,
        },
        ColumnarColumn {
            name: "score".to_string(),
            f64_values: Some(vec![95.5, 87.0, 92.0, 88.5]),
            i32_values: None,
            i64_values: None,
            string_values: None,
            null_bitmap: None,
        },
    ]
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_inner_join() {
    let config = ColumnarJoinsConfig::default();
    let joins = GPUColumnarJoins::new(config).await.unwrap();

    let left = create_test_left_columns();
    let right = create_test_right_columns();

    let results = joins
        .hash_join(&left, &right, "id", "id", JoinType::Inner)
        .unwrap();

    // Should match id=2 and id=3
    assert_eq!(results.len(), 2);
    assert!(!results[0].left_values.is_empty());
    assert!(!results[0].right_values.is_empty());
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_left_outer_join() {
    let config = ColumnarJoinsConfig::default();
    let joins = GPUColumnarJoins::new(config).await.unwrap();

    let left = create_test_left_columns();
    let right = create_test_right_columns();

    let results = joins
        .hash_join(&left, &right, "id", "id", JoinType::LeftOuter)
        .unwrap();

    // Should have 5 rows (all left rows)
    assert_eq!(results.len(), 5);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_large_dataset_join() {
    let config = ColumnarJoinsConfig {
        gpu_min_rows: 5000,
        ..Default::default()
    };
    let joins = GPUColumnarJoins::new(config).await.unwrap();

    let left_size = 10000;
    let right_size = 5000;

    let left = vec![
        ColumnarColumn {
            name: "id".to_string(),
            i32_values: Some((0..left_size as i32).collect()),
            i64_values: None,
            f64_values: None,
            string_values: None,
            null_bitmap: None,
        },
    ];

    let right = vec![
        ColumnarColumn {
            name: "id".to_string(),
            i32_values: Some((0..right_size as i32).map(|i| i * 2).collect()),
            i64_values: None,
            f64_values: None,
            string_values: None,
            null_bitmap: None,
        },
    ];

    let results = joins
        .hash_join(&left, &right, "id", "id", JoinType::Inner)
        .unwrap();

    // Should have matches for even IDs
    assert!(!results.is_empty());
    assert!(results.len() <= right_size);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_cpu_parallel_fallback() {
    let config = ColumnarJoinsConfig {
        enable_gpu: false,
        use_cpu_parallel: true,
        ..Default::default()
    };
    let joins = GPUColumnarJoins::new(config).await.unwrap();

    let left = create_test_left_columns();
    let right = create_test_right_columns();

    let results = joins
        .hash_join(&left, &right, "id", "id", JoinType::Inner)
        .unwrap();

    // Should work correctly even without GPU
    assert_eq!(results.len(), 2);
}

#[cfg(feature = "gpu-acceleration")]
#[tokio::test]
async fn test_string_join_keys() {
    let config = ColumnarJoinsConfig::default();
    let joins = GPUColumnarJoins::new(config).await.unwrap();

    let left = vec![ColumnarColumn {
        name: "key".to_string(),
        string_values: Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
        i32_values: None,
        i64_values: None,
        f64_values: None,
        null_bitmap: None,
    }];

    let right = vec![ColumnarColumn {
        name: "key".to_string(),
        string_values: Some(vec!["b".to_string(), "c".to_string(), "d".to_string()]),
        i32_values: None,
        i64_values: None,
        f64_values: None,
        null_bitmap: None,
    }];

    let results = joins
        .hash_join(&left, &right, "key", "key", JoinType::Inner)
        .unwrap();

    // Should match "b" and "c"
    assert_eq!(results.len(), 2);
}

#[cfg(not(feature = "gpu-acceleration"))]
#[test]
fn test_placeholder() {
    // Placeholder test when GPU acceleration is not enabled
    assert!(true);
}

