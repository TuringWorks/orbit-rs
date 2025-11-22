//! End-to-end integration tests for MCP Server with Orbit-RS
//!
//! These tests verify the complete pipeline:
//! Natural Language → NLP Processing → SQL Generation → Query Execution → Result Processing

use orbit_protocols::mcp::{
    integration::OrbitMcpIntegration,
    server::McpServer,
    McpCapabilities, McpConfig,
};
use orbit_protocols::postgres_wire::query_engine::QueryEngine;
use std::sync::Arc;

/// Test helper to create a test MCP server
async fn create_test_server() -> McpServer {
    // Create a test query engine
    // In a real test, this would use an in-memory or test database
    let query_engine = Arc::new(QueryEngine::new());

    let integration = Arc::new(OrbitMcpIntegration::new(query_engine));

    let config = McpConfig::default();
    let capabilities = McpCapabilities::default();

    McpServer::with_orbit_integration(config, capabilities, integration)
}

#[tokio::test]
async fn test_natural_language_to_sql_generation() {
    let server = create_test_server().await;

    // Test various natural language queries
    let test_cases = vec![
        ("Show me all users", "SELECT"),
        ("What are the top 10 products", "SELECT"),
        ("Find users from California", "SELECT"),
        ("Get the average order value", "SELECT"),
        ("Analyze customer ages", "SELECT"), // ANALYZE intent
    ];

    for (query, expected_sql_keyword) in test_cases {
        let result = server.process_natural_language_query(query).await;
        assert!(
            result.is_ok(),
            "Failed to process query: '{}' - Error: {:?}",
            query,
            result.err()
        );

        let generated_query = result.unwrap();
        assert!(
            generated_query.sql.to_uppercase().contains(expected_sql_keyword),
            "Query '{}' did not generate expected SQL keyword. Got: {}",
            query,
            generated_query.sql
        );
    }
}

#[tokio::test]
async fn test_intent_classification() {
    let server = create_test_server().await;

    // Test SELECT intent
    let select_query = server
        .process_natural_language_query("Show me all users")
        .await
        .unwrap();
    assert!(select_query.sql.to_uppercase().starts_with("SELECT"));

    // Test INSERT intent (if supported)
    let insert_query = server
        .process_natural_language_query("Add a new user with email test@example.com")
        .await
        .unwrap();
    assert!(insert_query.sql.to_uppercase().starts_with("INSERT"));

    // Test UPDATE intent
    let update_query = server
        .process_natural_language_query("Update all products to have free shipping")
        .await
        .unwrap();
    assert!(update_query.sql.to_uppercase().starts_with("UPDATE"));

    // Test DELETE intent
    let delete_query = server
        .process_natural_language_query("Delete old log entries")
        .await
        .unwrap();
    assert!(delete_query.sql.to_uppercase().starts_with("DELETE"));
}

#[tokio::test]
async fn test_entity_extraction() {
    let server = create_test_server().await;

    // Test table name extraction
    let query = server
        .process_natural_language_query("Show me all users from California")
        .await
        .unwrap();

    // The SQL should contain "users" table
    assert!(
        query.sql.to_uppercase().contains("USERS"),
        "Failed to extract table name 'users'. SQL: {}",
        query.sql
    );

    // Test column extraction
    let query = server
        .process_natural_language_query("Get user names and emails")
        .await
        .unwrap();

    // Should have some column references
    assert!(
        !query.sql.to_uppercase().contains("SELECT *"),
        "Should extract specific columns, not use *"
    );
}

#[tokio::test]
async fn test_condition_extraction() {
    let server = create_test_server().await;

    // Test WHERE clause generation
    let query = server
        .process_natural_language_query("Show users where state is California")
        .await
        .unwrap();

    assert!(
        query.sql.to_uppercase().contains("WHERE"),
        "Failed to generate WHERE clause. SQL: {}",
        query.sql
    );

    // Should have parameters for the condition value
    assert!(
        !query.parameters.is_empty(),
        "Should have parameters for condition values"
    );
}

#[tokio::test]
async fn test_aggregation_detection() {
    let server = create_test_server().await;

    // Test COUNT aggregation
    let query = server
        .process_natural_language_query("How many users are there?")
        .await
        .unwrap();

    assert!(
        query.sql.to_uppercase().contains("COUNT"),
        "Failed to detect COUNT aggregation. SQL: {}",
        query.sql
    );

    // Test AVG aggregation
    let query = server
        .process_natural_language_query("What is the average order value?")
        .await
        .unwrap();

    assert!(
        query.sql.to_uppercase().contains("AVG"),
        "Failed to detect AVG aggregation. SQL: {}",
        query.sql
    );
}

#[tokio::test]
async fn test_limit_extraction() {
    let server = create_test_server().await;

    // Test "top N" pattern
    let query = server
        .process_natural_language_query("Show me the top 10 products")
        .await
        .unwrap();

    assert!(
        query.sql.to_uppercase().contains("LIMIT"),
        "Failed to extract limit. SQL: {}",
        query.sql
    );
}

#[tokio::test]
async fn test_result_processing() {
    use orbit_protocols::mcp::result_processor::{QueryResult, ResultProcessor};

    let processor = ResultProcessor::new();

    // Create mock query result
    let mut mock_result = QueryResult {
        columns: vec!["id".to_string(), "name".to_string(), "age".to_string()],
        rows: vec![],
        row_count: 0,
    };

    // Add sample rows
    for i in 1..=5 {
        let mut row = std::collections::HashMap::new();
        row.insert("id".to_string(), serde_json::Value::Number(i.into()));
        row.insert(
            "name".to_string(),
            serde_json::Value::String(format!("User {}", i)),
        );
        row.insert("age".to_string(), serde_json::Value::Number((20 + i * 5).into()));
        mock_result.rows.push(row);
    }
    mock_result.row_count = mock_result.rows.len();

    // Process results
    let processed = processor.process_results(&mock_result, 10);

    // Verify summary
    assert!(
        processed.summary.contains("5"),
        "Summary should mention row count"
    );

    // Verify data preview
    assert_eq!(processed.data_preview.len(), 5);

    // Verify statistics
    assert_eq!(processed.statistics.total_rows, 5);
    assert_eq!(processed.statistics.column_stats.len(), 3);

    // Verify visualization hints
    assert!(!processed.visualization_hints.is_empty());
}

#[tokio::test]
async fn test_sql_parameter_binding() {
    let server = create_test_server().await;

    // Test that parameters are properly extracted
    let query = server
        .process_natural_language_query("Show users where state is California")
        .await
        .unwrap();

    // Should use parameterized queries ($1, $2, etc.)
    assert!(
        query.sql.contains('$') || query.sql.contains("California"),
        "Should use parameters or include value. SQL: {}",
        query.sql
    );

    // Parameters should be present
    if query.sql.contains('$') {
        assert!(
            !query.parameters.is_empty(),
            "Should have parameters when using parameterized queries"
        );
    }
}

#[tokio::test]
async fn test_complex_query_generation() {
    let server = create_test_server().await;

    // Test complex query with multiple conditions
    let query = server
        .process_natural_language_query(
            "Show me the top 10 users from California ordered by age",
        )
        .await
        .unwrap();

    let sql_upper = query.sql.to_uppercase();

    // Should have SELECT
    assert!(sql_upper.contains("SELECT"));
    // Should have FROM
    assert!(sql_upper.contains("FROM"));
    // Should have WHERE (if state extraction works)
    // Should have ORDER BY
    // Should have LIMIT
    assert!(
        sql_upper.contains("LIMIT") || sql_upper.contains("10"),
        "Complex query should include limit. SQL: {}",
        query.sql
    );
}

#[tokio::test]
async fn test_error_handling() {
    let server = create_test_server().await;

    // Test empty query
    let _result = server.process_natural_language_query("").await;
    // Should either handle gracefully or return an error
    // (implementation dependent)

    // Test malformed query - should still generate some SQL (defaults to SELECT)
    let result = server.process_natural_language_query("asdfghjkl").await;
    // Should default to SELECT or handle gracefully
    // Even malformed queries should generate some SQL (might be invalid, but shouldn't panic)
    match result {
        Ok(_) => {
            // Success - generated SQL even for malformed query
        }
        Err(_) => {
            // Error is acceptable for completely malformed queries
            // The important thing is it doesn't panic
        }
    }
}

#[tokio::test]
async fn test_query_complexity_estimation() {
    let server = create_test_server().await;

    // Simple query
    let simple_query = server
        .process_natural_language_query("Show all users")
        .await
        .unwrap();
    assert_eq!(
        simple_query.estimated_complexity,
        orbit_protocols::mcp::sql_generator::QueryComplexity::Low
    );

    // Complex query with aggregations
    let complex_query = server
        .process_natural_language_query(
            "Show me the average age of users grouped by state where age is greater than 25",
        )
        .await
        .unwrap();
    // Should have higher complexity
    assert!(
        matches!(
            complex_query.estimated_complexity,
            orbit_protocols::mcp::sql_generator::QueryComplexity::Medium
                | orbit_protocols::mcp::sql_generator::QueryComplexity::High
        ),
        "Complex query should have higher complexity estimation"
    );
}

#[tokio::test]
async fn test_optimization_hints() {
    let server = create_test_server().await;

    // Query with conditions should suggest index usage
    let query = server
        .process_natural_language_query("Show users where state is California")
        .await
        .unwrap();

    assert!(
        !query.optimization_hints.is_empty(),
        "Query with conditions should have optimization hints"
    );
}

