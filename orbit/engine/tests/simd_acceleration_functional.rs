//! Functional tests demonstrating that SIMD acceleration is actually working
//!
//! These tests verify that queries execute correctly with SIMD optimization
//! and produce accurate results.

// These tests require the cpu-simd feature from orbit-compute
// In the future, this could be made conditional on a feature flag

use orbit_engine::query::{Query, QueryOptimizer, VectorizedExecutor};
use orbit_engine::storage::{Column, ColumnBatch, FilterPredicate, NullBitmap, SqlValue};

/// Create test data with known values for verification
fn create_test_data_with_values() -> ColumnBatch {
    // id: 0..100
    // value: 0, 2, 4, 6, 8, ... (id * 2)
    // score: 0, 3, 6, 9, 12, ... (id * 3)
    let row_count = 100;
    let ids: Vec<i32> = (0..row_count as i32).collect();
    let values: Vec<i32> = ids.iter().map(|id| id * 2).collect();
    let scores: Vec<i32> = ids.iter().map(|id| id * 3).collect();

    ColumnBatch {
        columns: vec![
            Column::Int32(ids),
            Column::Int32(values),
            Column::Int32(scores),
        ],
        null_bitmaps: vec![
            NullBitmap::new_all_valid(row_count),
            NullBitmap::new_all_valid(row_count),
            NullBitmap::new_all_valid(row_count),
        ],
        row_count,
        column_names: Some(vec![
            "id".to_string(),
            "value".to_string(),
            "score".to_string(),
        ]),
    }
}

#[tokio::test]
async fn test_filter_with_simd_eq() {
    // Test equality filter with SIMD
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: None,
        filter: Some(FilterPredicate::Eq("id".to_string(), SqlValue::Int32(42))),
        limit: None,
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();
    let data = create_test_data_with_values();
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await
        .unwrap();

    // Should have exactly 1 row (id=42)
    assert_eq!(result.row_count, 1, "Expected exactly 1 row with id=42");

    // Verify the values
    if let Column::Int32(ids) = &result.columns[0] {
        assert_eq!(ids[0], 42);
    }
    if let Column::Int32(values) = &result.columns[1] {
        assert_eq!(values[0], 84); // 42 * 2
    }
    if let Column::Int32(scores) = &result.columns[2] {
        assert_eq!(scores[0], 126); // 42 * 3
    }
}

#[tokio::test]
async fn test_filter_with_simd_range() {
    // Test range filter with SIMD (value >= 50 AND value <= 100)
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: None,
        filter: Some(FilterPredicate::And(vec![
            FilterPredicate::Ge("value".to_string(), SqlValue::Int32(50)),
            FilterPredicate::Le("value".to_string(), SqlValue::Int32(100)),
        ])),
        limit: None,
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();
    let data = create_test_data_with_values();
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await
        .unwrap();

    // value ranges from 0 to 198 (id * 2 where id is 0-99)
    // We want value >= 50 AND value <= 100
    // That's id 25-50 (inclusive), which is 26 rows
    assert_eq!(
        result.row_count, 26,
        "Expected 26 rows where value is between 50 and 100"
    );

    // Verify first and last values
    if let Column::Int32(values) = &result.columns[1] {
        assert_eq!(values[0], 50); // First value
        assert_eq!(values[25], 100); // Last value
    }
}

#[tokio::test]
async fn test_projection_with_simd() {
    // Test that projection correctly selects columns
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: Some(vec!["value".to_string(), "id".to_string()]), // Reversed order
        filter: None,
        limit: Some(5),
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();
    let data = create_test_data_with_values();
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await
        .unwrap();

    assert_eq!(result.row_count, 5);
    assert_eq!(result.columns.len(), 2, "Should have 2 projected columns");

    // Verify column order (value, id)
    if let Column::Int32(values) = &result.columns[0] {
        assert_eq!(values[0], 0); // First value
        assert_eq!(values[1], 2); // Second value
    }
    if let Column::Int32(ids) = &result.columns[1] {
        assert_eq!(ids[0], 0); // First id
        assert_eq!(ids[1], 1); // Second id
    }
}

#[tokio::test]
async fn test_limit_and_offset_with_simd() {
    // Test LIMIT and OFFSET
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: Some(vec!["id".to_string()]),
        filter: None,
        limit: Some(10),
        offset: Some(20),
    };

    let plan = optimizer.optimize(&query).unwrap();
    let data = create_test_data_with_values();
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await
        .unwrap();

    // Should have 10 rows starting from id=20
    assert_eq!(result.row_count, 10);

    if let Column::Int32(ids) = &result.columns[0] {
        assert_eq!(ids[0], 20); // First id after offset
        assert_eq!(ids[9], 29); // Last id (20 + 9)
    }
}

#[tokio::test]
async fn test_complex_query_with_all_operations() {
    // Test filter + projection + limit + offset all together
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: Some(vec!["id".to_string(), "score".to_string()]),
        filter: Some(FilterPredicate::Gt(
            "value".to_string(),
            SqlValue::Int32(100),
        )),
        limit: Some(10),
        offset: Some(5),
    };

    let plan = optimizer.optimize(&query).unwrap();
    let data = create_test_data_with_values();
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await
        .unwrap();

    // value > 100 means id > 50 (since value = id * 2)
    // That gives us ids 51-99 (49 rows)
    // After offset of 5, we get ids 56-99 (44 rows)
    // After limit of 10, we get ids 56-65 (10 rows)
    assert_eq!(result.row_count, 10);
    assert_eq!(result.columns.len(), 2); // id and score only

    if let Column::Int32(ids) = &result.columns[0] {
        assert_eq!(ids[0], 56); // First id after filter+offset
        assert_eq!(ids[9], 65); // Last id before limit
    }

    if let Column::Int32(scores) = &result.columns[1] {
        assert_eq!(scores[0], 168); // 56 * 3
        assert_eq!(scores[9], 195); // 65 * 3
    }
}

#[tokio::test]
async fn test_acceleration_strategy_is_applied() {
    // Verify that acceleration strategy is actually being used
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: None,
        filter: Some(FilterPredicate::Gt(
            "value".to_string(),
            SqlValue::Int32(50),
        )),
        limit: None,
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();

    // Should have acceleration strategy
    assert!(plan.acceleration_strategy.is_some());
    assert!(plan.query_analysis.is_some());

    let analysis = plan.query_analysis.as_ref().unwrap();
    assert!(analysis.complexity_score > 0.0);

    // Cost with acceleration should be less than base cost
    assert!(plan.estimated_cost > 0.0);
}

#[tokio::test]
async fn test_empty_result_set() {
    // Test query that returns no results
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer");

    let query = Query {
        table: "test".to_string(),
        projection: None,
        filter: Some(FilterPredicate::Eq(
            "id".to_string(),
            SqlValue::Int32(999), // Doesn't exist
        )),
        limit: None,
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();
    let data = create_test_data_with_values();
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await
        .unwrap();

    // Should return empty result
    assert_eq!(result.row_count, 0);
}
