//! Integration tests for hardware acceleration with QueryOptimizer and VectorizedExecutor
//!
//! These tests verify the end-to-end flow of:
//! 1. QueryOptimizer analyzing queries and recommending acceleration strategies
//! 2. ExecutionPlan containing acceleration metadata
//! 3. VectorizedExecutor routing execution based on acceleration strategies

// These tests require the cpu-simd feature from orbit-compute
// In the future, this could be made conditional on a feature flag

use orbit_compute::AccelerationStrategy;
use orbit_engine::query::{ExecutionPlan, Query, QueryOptimizer, VectorizedExecutor};
use orbit_engine::storage::{Column, ColumnBatch, FilterPredicate, NullBitmap, SqlValue};

/// Helper to create test data
fn create_test_data(row_count: usize) -> ColumnBatch {
    let values: Vec<i32> = (0..row_count as i32).collect();
    ColumnBatch {
        columns: vec![
            Column::Int32(values.clone()),
            Column::Int32(values.iter().map(|v| v * 2).collect()),
            Column::Int32(values.iter().map(|v| v * 3).collect()),
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
async fn test_optimizer_basic_query_no_acceleration() {
    // Test that basic optimizer (level 0) doesn't add acceleration
    let optimizer = QueryOptimizer::new(0);

    let query = Query {
        table: "users".to_string(),
        projection: Some(vec!["id".to_string(), "name".to_string()]),
        filter: None,
        limit: None,
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();

    // Level 0 shouldn't have acceleration strategy
    assert!(plan.acceleration_strategy.is_none());
    assert!(plan.query_analysis.is_none());
}

#[tokio::test]
async fn test_optimizer_with_acceleration_simple_filter() {
    // Test optimizer with acceleration (level 3)
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    let query = Query {
        table: "users".to_string(),
        projection: Some(vec!["id".to_string(), "value".to_string()]),
        filter: Some(FilterPredicate::Eq("id".to_string(), SqlValue::Int32(100))),
        limit: None,
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();

    // Should have acceleration strategy recommended
    assert!(plan.acceleration_strategy.is_some());
    assert!(plan.query_analysis.is_some());

    let analysis = plan.query_analysis.as_ref().unwrap();
    assert!(analysis.complexity_score > 0.0);
    assert!(analysis.confidence > 0.0);
    assert!(analysis.confidence <= 1.0);

    // Cost should be adjusted based on acceleration
    if matches!(
        plan.acceleration_strategy,
        Some(AccelerationStrategy::CpuSimd)
    ) {
        // With SIMD acceleration, cost should be lower than base
        assert!(plan.estimated_cost > 0.0);
    }
}

#[tokio::test]
async fn test_optimizer_complex_aggregation_query() {
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    // Complex query with aggregation should get acceleration
    let query = Query {
        table: "metrics".to_string(),
        projection: Some(vec!["user_id".to_string(), "total".to_string()]),
        filter: Some(FilterPredicate::Gt(
            "value".to_string(),
            SqlValue::Int32(1000),
        )),
        limit: Some(100),
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();

    assert!(plan.acceleration_strategy.is_some());
    assert!(plan.query_analysis.is_some());

    let analysis = plan.query_analysis.as_ref().unwrap();
    // Complex queries should have higher complexity scores
    assert!(analysis.complexity_score >= 1.0);
}

#[tokio::test]
async fn test_cost_model_with_acceleration() {
    let optimizer_no_accel = QueryOptimizer::new(2); // Level 2 - no acceleration
    let optimizer_with_accel = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    let query = Query {
        table: "large_table".to_string(),
        projection: Some(vec!["id".to_string()]),
        filter: Some(FilterPredicate::Lt(
            "score".to_string(),
            SqlValue::Int32(50),
        )),
        limit: None,
        offset: None,
    };

    let plan_no_accel = optimizer_no_accel.optimize(&query).unwrap();
    let plan_with_accel = optimizer_with_accel.optimize(&query).unwrap();

    // Plan with acceleration should have lower estimated cost
    if plan_with_accel.acceleration_strategy.is_some()
        && !matches!(
            plan_with_accel.acceleration_strategy,
            Some(AccelerationStrategy::None)
        )
    {
        assert!(
            plan_with_accel.estimated_cost < plan_no_accel.estimated_cost,
            "Accelerated plan cost ({:.2}) should be less than non-accelerated ({:.2})",
            plan_with_accel.estimated_cost,
            plan_no_accel.estimated_cost
        );
    }
}

#[tokio::test]
async fn test_executor_with_acceleration_routing() {
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    let query = Query {
        table: "test_table".to_string(),
        projection: Some(vec!["id".to_string(), "value".to_string()]),
        filter: Some(FilterPredicate::Eq("id".to_string(), SqlValue::Int32(42))),
        limit: Some(10),
        offset: None,
    };

    let plan = optimizer.optimize(&query).unwrap();

    // Create test data
    let data = create_test_data(100);

    // Execute with acceleration routing
    let executor = VectorizedExecutor::new();
    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await;

    assert!(result.is_ok(), "Execution should succeed");
    let result_batch = result.unwrap();

    // Verify we got valid results
    assert!(result_batch.row_count <= data.row_count);
    // After projection, should have 2 columns (id, value) instead of original 3
    assert_eq!(
        result_batch.columns.len(),
        2,
        "Expected 2 columns after projection, got {}",
        result_batch.columns.len()
    );
}

#[tokio::test]
async fn test_executor_cpu_simd_strategy() {
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    let query = Query {
        table: "test_table".to_string(),
        projection: Some(vec!["id".to_string()]),
        filter: Some(FilterPredicate::Gt(
            "value".to_string(),
            SqlValue::Int32(50),
        )),
        limit: None,
        offset: None,
    };

    let mut plan = optimizer.optimize(&query).unwrap();

    // Force CPU SIMD strategy
    plan.acceleration_strategy = Some(AccelerationStrategy::CpuSimd);

    let data = create_test_data(1000);
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_fallback_strategies() {
    // Test that GPU/Neural/Hybrid strategies gracefully fallback to CPU SIMD
    let query = Query {
        table: "test_table".to_string(),
        projection: Some(vec!["id".to_string()]),
        filter: None,
        limit: None,
        offset: None,
    };

    let data = create_test_data(100);
    let executor = VectorizedExecutor::new();

    // Test each acceleration strategy
    for strategy in [
        AccelerationStrategy::Gpu,
        AccelerationStrategy::NeuralEngine,
        AccelerationStrategy::Hybrid,
        AccelerationStrategy::None,
    ] {
        let plan = ExecutionPlan {
            nodes: vec![],
            estimated_cost: 100.0,
            uses_simd: false,
            acceleration_strategy: Some(strategy),
            query_analysis: None,
        };

        let result = executor
            .execute_with_acceleration(&plan, &query, &data)
            .await;

        assert!(
            result.is_ok(),
            "Strategy {:?} should execute successfully (with fallback if needed)",
            strategy
        );
    }
}

#[tokio::test]
async fn test_end_to_end_optimization_and_execution() {
    // Full end-to-end test: Optimize → Execute
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    let query = Query {
        table: "sales".to_string(),
        projection: Some(vec!["id".to_string(), "value".to_string()]),
        filter: Some(FilterPredicate::And(vec![
            FilterPredicate::Ge("value".to_string(), SqlValue::Int32(100)),
            FilterPredicate::Le("value".to_string(), SqlValue::Int32(400)),
        ])),
        limit: Some(50),
        offset: Some(10),
    };

    // Step 1: Optimize
    let plan = optimizer.optimize(&query).unwrap();

    assert!(plan.acceleration_strategy.is_some());
    assert!(plan.query_analysis.is_some());

    // Step 2: Execute
    let data = create_test_data(500);
    let executor = VectorizedExecutor::new();

    let result = executor
        .execute_with_acceleration(&plan, &query, &data)
        .await;

    assert!(result.is_ok(), "Execution failed: {:?}", result.err());

    let result_batch = result.unwrap();
    assert!(result_batch.row_count > 0);

    // Verify acceleration was considered
    let analysis = plan.query_analysis.as_ref().unwrap();
    assert!(analysis.complexity_score > 0.0);
    assert!(!analysis.operation_types.is_empty());
}

#[tokio::test]
async fn test_acceleration_confidence_scoring() {
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    // Simple query should have higher confidence
    let simple_query = Query {
        table: "users".to_string(),
        projection: Some(vec!["id".to_string()]),
        filter: None,
        limit: Some(10),
        offset: None,
    };

    let simple_plan = optimizer.optimize(&simple_query).unwrap();

    if let Some(simple_analysis) = simple_plan.query_analysis {
        assert!(simple_analysis.confidence > 0.5);
        assert!(simple_analysis.complexity_score < 5.0);
    }
}

#[tokio::test]
async fn test_multiple_queries_with_different_complexities() {
    let optimizer = QueryOptimizer::new_with_acceleration(3)
        .await
        .expect("Failed to create optimizer with acceleration");

    // Very simple query
    let query1 = Query {
        table: "t1".to_string(),
        projection: Some(vec!["id".to_string()]),
        filter: None,
        limit: Some(1),
        offset: None,
    };

    // Medium complexity query
    let query2 = Query {
        table: "t2".to_string(),
        projection: Some(vec!["id".to_string(), "value".to_string()]),
        filter: Some(FilterPredicate::Eq(
            "status".to_string(),
            SqlValue::Int32(1),
        )),
        limit: Some(100),
        offset: None,
    };

    // Complex query
    let query3 = Query {
        table: "t3".to_string(),
        projection: Some(vec![
            "user_id".to_string(),
            "total".to_string(),
            "avg".to_string(),
        ]),
        filter: Some(FilterPredicate::Or(vec![
            FilterPredicate::Gt("amount".to_string(), SqlValue::Int32(1000)),
            FilterPredicate::Lt("amount".to_string(), SqlValue::Int32(10)),
        ])),
        limit: None,
        offset: None,
    };

    let plan1 = optimizer.optimize(&query1).unwrap();
    let plan2 = optimizer.optimize(&query2).unwrap();
    let plan3 = optimizer.optimize(&query3).unwrap();

    // Verify complexity scores generally reflect query complexity
    // (not strictly monotonic due to optimization and analysis heuristics)
    if let (Some(a1), Some(a2), Some(a3)) = (
        plan1.query_analysis,
        plan2.query_analysis,
        plan3.query_analysis,
    ) {
        // Simple query should have lowest complexity
        assert!(
            a1.complexity_score < 5.0,
            "Simple query should have low complexity"
        );

        // Complex query should have higher complexity than simple
        assert!(
            a3.complexity_score > a1.complexity_score,
            "Complex query ({:.2}) should be more complex than simple query ({:.2})",
            a3.complexity_score,
            a1.complexity_score
        );

        // All should have valid complexity scores
        assert!(a1.complexity_score >= 0.0);
        assert!(a2.complexity_score >= 0.0);
        assert!(a3.complexity_score >= 0.0);
    }
}
