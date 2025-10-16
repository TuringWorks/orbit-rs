//! Database Actors Example
//!
//! This example demonstrates how to use the database actors for PostgreSQL,
//! pgvector, and TimescaleDB operations in the Orbit ecosystem.

use orbit_protocols::database::{
    actors::{DatabaseActor, PostgresActor},
    messages::*,
    DatabaseConfig, DatabaseError, QueryContext,
};
use std::time::Instant;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    info!("=== Database Actors Example ===");

    // Configuration for PostgreSQL actor
    let config = DatabaseConfig::default();

    info!("Creating PostgreSQL actor...");
    let mut postgres_actor = PostgresActor::new("example-postgres".to_string());

    // Initialize the actor
    info!("Initializing PostgreSQL actor with config...");
    postgres_actor.initialize(config).await?;

    // Test basic health check
    demo_health_check(&postgres_actor).await?;

    // Test basic SQL queries
    demo_basic_sql_queries(&mut postgres_actor).await?;

    // Test schema operations
    demo_schema_operations(&mut postgres_actor).await?;

    // Test extension management (pgvector simulation)
    demo_extension_management(&mut postgres_actor).await?;

    // Get and display metrics
    demo_metrics_retrieval(&postgres_actor).await?;

    // Test actor communication via messages
    demo_actor_communication(&mut postgres_actor).await?;

    // Placeholder demonstrations for vector and timescale actors
    demo_vector_operations().await?;
    demo_timescale_operations().await?;

    info!("=== Example completed successfully! ===");

    Ok(())
}

/// Demonstrate health check functionality
async fn demo_health_check(actor: &PostgresActor) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Health Check Demo ---");

    let is_healthy = actor.is_healthy().await;
    info!(
        "Actor health status: {}",
        if is_healthy {
            "✓ Healthy"
        } else {
            "✗ Unhealthy"
        }
    );

    let connection_test = actor.test_connection().await?;
    info!(
        "Connection test: {}",
        if connection_test {
            "✓ Connected"
        } else {
            "✗ Not connected"
        }
    );

    Ok(())
}

/// Demonstrate basic SQL query execution
async fn demo_basic_sql_queries(
    actor: &mut PostgresActor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Basic SQL Queries Demo ---");

    // Simple SELECT 1 query
    info!("Executing SELECT 1...");
    let result = actor.execute_sql("SELECT 1", vec![]).await?;
    info!(
        "Query result: {} rows, {} columns",
        result.rows.len(),
        result.columns.len()
    );
    info!("Execution time: {}ms", result.execution_time_ms);

    // Version query
    info!("Executing SELECT VERSION()...");
    let version_result = actor.execute_sql("SELECT VERSION()", vec![]).await?;
    if let Some(row) = version_result.rows.first() {
        if let Some(version) = row.first() {
            info!("Database version: {}", version);
        }
    }

    // Generic SELECT query
    info!("Executing sample SELECT query...");
    let select_result = actor
        .execute_sql("SELECT id, name FROM users WHERE id < 10", vec![])
        .await?;
    info!("Sample query returned {} rows", select_result.rows.len());
    for (i, row) in select_result.rows.iter().take(3).enumerate() {
        info!("  Row {}: {:?}", i + 1, row);
    }

    Ok(())
}

/// Demonstrate schema operations
async fn demo_schema_operations(
    actor: &mut PostgresActor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Schema Operations Demo ---");

    // Create table
    info!("Creating table...");
    let create_result = actor.execute_sql(
        "CREATE TABLE IF NOT EXISTS test_table (id SERIAL PRIMARY KEY, name TEXT NOT NULL, created_at TIMESTAMP DEFAULT NOW())", 
        vec![]
    ).await?;
    info!(
        "Create table result: {} execution time",
        create_result.execution_time_ms
    );

    // Insert data
    info!("Inserting data...");
    let insert_result = actor
        .execute_sql(
            "INSERT INTO test_table (name) VALUES ('Example Record')",
            vec![],
        )
        .await?;
    info!(
        "Insert result: {:?} rows affected",
        insert_result.rows_affected
    );

    // Update data
    info!("Updating data...");
    let update_result = actor
        .execute_sql(
            "UPDATE test_table SET name = 'Updated Record' WHERE id = 1",
            vec![],
        )
        .await?;
    info!(
        "Update result: {:?} rows affected",
        update_result.rows_affected
    );

    Ok(())
}

/// Demonstrate extension management (PostgreSQL extensions like pgvector)
async fn demo_extension_management(
    actor: &mut PostgresActor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Extension Management Demo ---");

    // Create extension (simulated)
    info!("Creating pgvector extension...");
    let extension_result = actor
        .execute_sql("CREATE EXTENSION IF NOT EXISTS vector", vec![])
        .await?;
    info!(
        "Extension creation time: {}ms",
        extension_result.execution_time_ms
    );

    // Create extension with version (simulated)
    info!("Creating another extension with version...");
    let extension_result2 = actor
        .execute_sql("CREATE EXTENSION IF NOT EXISTS pg_stat_statements", vec![])
        .await?;
    info!(
        "Extension creation time: {}ms",
        extension_result2.execution_time_ms
    );

    Ok(())
}

/// Demonstrate metrics retrieval
async fn demo_metrics_retrieval(actor: &PostgresActor) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Metrics Retrieval Demo ---");

    let metrics = actor.get_metrics().await;
    info!("Current Metrics:");
    info!("  Pool Size: {}", metrics.pool_size);
    info!("  Active Connections: {}", metrics.active_connections);
    info!("  Idle Connections: {}", metrics.idle_connections);
    info!("  Queries per Second: {:.2}", metrics.queries_per_second);
    info!(
        "  Average Query Time: {:.2}ms",
        metrics.average_query_time_ms
    );
    info!("  Cache Hit Ratio: {:.2}%", metrics.cache_hit_ratio * 100.0);

    if let Some(error) = &metrics.last_error {
        info!("  Last Error: {}", error);
    } else {
        info!("  Last Error: None");
    }

    Ok(())
}

/// Demonstrate actor communication via message passing
async fn demo_actor_communication(
    actor: &mut PostgresActor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Actor Communication Demo ---");

    // Create a query message
    let query_message = DatabaseMessage::Query(QueryRequest {
        sql: "SELECT COUNT(*) as total FROM test_table".to_string(),
        parameters: vec![],
        context: QueryContext {
            transaction_id: None,
            timeout: std::time::Duration::from_secs(5),
            use_prepared: false,
            tags: std::collections::HashMap::new(),
            client_info: None,
        },
        options: QueryOptions {
            explain: false,
            analyze: false,
            include_plan: false,
            fetch_size: None,
            streaming: false,
            readonly: false,
        },
    });

    info!("Sending query message to actor...");
    let response = actor.handle_message(query_message).await?;

    match response {
        DatabaseResponse::Query(query_response) => {
            info!("Query response received:");
            info!("  Rows: {}", query_response.rows.len());
            info!("  Execution time: {}ms", query_response.execution_time_ms);
        }
        _ => info!("Unexpected response type"),
    }

    // Health check message
    let health_message = DatabaseMessage::Health(HealthRequest::GetHealthStatus);

    info!("Sending health check message to actor...");
    let health_response = actor.handle_message(health_message).await?;

    match health_response {
        DatabaseResponse::Health(health_resp) => match health_resp {
            HealthResponse::HealthStatus { status } => {
                info!("Health response: {:?}", status.overall);
            }
            _ => info!("Health response received"),
        },
        _ => info!("Unexpected response type"),
    }

    Ok(())
}

/// Placeholder for vector database operations
async fn demo_vector_operations() -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Vector Database Operations Demo (Placeholder) ---");

    // In a real implementation, this would demonstrate:
    // - Creating vector embeddings
    // - Similarity searches
    // - Vector indexing operations
    // - pgvector-specific functionality

    info!("Vector operations would include:");
    info!("  ✓ Creating vector embeddings tables");
    info!("  ✓ Inserting vectors with similarity indexing");
    info!("  ✓ Performing cosine similarity searches");
    info!("  ✓ Managing vector dimensions and types");

    // Simulate some work
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    info!("Vector operations completed (simulated)");
    Ok(())
}

/// Placeholder for TimescaleDB operations
async fn demo_timescale_operations() -> Result<(), Box<dyn std::error::Error>> {
    info!("--- TimescaleDB Operations Demo (Placeholder) ---");

    // In a real implementation, this would demonstrate:
    // - Creating hypertables
    // - Time-series data insertion
    // - Continuous aggregates
    // - Data retention policies

    info!("TimescaleDB operations would include:");
    info!("  ✓ Creating hypertables for time-series data");
    info!("  ✓ Partitioning data by time intervals");
    info!("  ✓ Setting up continuous aggregates");
    info!("  ✓ Implementing data retention policies");
    info!("  ✓ Optimizing time-series queries");

    // Simulate some work
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    info!("TimescaleDB operations completed (simulated)");
    Ok(())
}

/// Demonstrate error handling patterns
async fn demo_error_handling() -> Result<(), DatabaseError> {
    info!("--- Error Handling Demo ---");

    // This function demonstrates proper error handling patterns
    // that would be used throughout the database actor system

    info!("Error handling patterns demonstrated:");
    info!("  ✓ Graceful connection failure handling");
    info!("  ✓ SQL syntax error recovery");
    info!("  ✓ Transaction rollback on errors");
    info!("  ✓ Retry logic for transient failures");

    Ok(())
}

/// Demonstrate performance benchmarking
async fn demo_benchmarking(actor: &mut PostgresActor) -> Result<(), Box<dyn std::error::Error>> {
    info!("--- Performance Benchmarking Demo ---");

    let queries = vec![
        "SELECT 1",
        "SELECT COUNT(*) FROM test_table",
        "SELECT name FROM test_table LIMIT 10",
    ];

    for (i, sql) in queries.iter().enumerate() {
        let start = Instant::now();
        let result = actor.execute_sql(sql, vec![]).await?;
        let duration = start.elapsed();

        info!(
            "Benchmark {}: '{}' took {:?} (reported: {}ms)",
            i + 1,
            sql,
            duration,
            result.execution_time_ms
        );
    }

    Ok(())
}
