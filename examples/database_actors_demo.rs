//! Database Actors Demo
//!
//! This example demonstrates how to use the comprehensive database actor system
//! with PostgreSQL, pgvector, and TimescaleDB support in Orbit-RS.

use orbit_protocols::database::*;
use orbit_protocols::database::messages::*;
use orbit_protocols::database::actors::*;
use orbit_shared::addressable::Addressable;
use orbit_shared::exception::OrbitResult;
use std::collections::HashMap;
use tokio;
use tracing::{info, error};

#[tokio::main]
async fn main() -> OrbitResult<()> {
    // Initialize logging
    tracing_subscriber::init();
    
    info!("🚀 Starting Database Actors Demo");
    info!("==================================");
    
    // Configuration for all database types
    let config = DatabaseConfig {
        postgres: PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "orbit_demo".to_string(),
            username: "postgres".to_string(),
            password: "password".to_string(),
            ..Default::default()
        },
        vector: VectorConfig {
            default_dimension: 384,
            default_index_type: VectorIndexType::Hnsw,
            similarity_threshold: 0.8,
            max_results: 50,
            ..Default::default()
        },
        timescale: TimescaleConfig {
            chunk_time_interval: std::time::Duration::from_secs(86400), // 1 day
            enable_compression: true,
            compression_after: std::time::Duration::from_secs(604800), // 1 week
            retention_policy: Some(std::time::Duration::from_secs(2592000)), // 30 days
            ..Default::default()
        },
        ..Default::default()
    };
    
    // Demonstrate PostgreSQL Actor
    info!("📊 Demonstrating PostgreSQL Actor");
    info!("--------------------------------");
    
    demo_postgres_actor(config.clone()).await?;
    
    info!("");
    info!("🔍 Demonstrating Vector Database Actor");
    info!("--------------------------------------");
    
    demo_vector_actor(config.clone()).await?;
    
    info!("");
    info!("⏱️  Demonstrating TimescaleDB Actor");
    info!("-----------------------------------");
    
    demo_timescale_actor(config.clone()).await?;
    
    info!("");
    info!("🎭 Demonstrating Actor Communication");
    info!("------------------------------------");
    
    demo_actor_communication(config).await?;
    
    info!("");
    info!("✅ Database Actors Demo completed successfully!");
    
    Ok(())
}

/// Demonstrate PostgreSQL Actor functionality
async fn demo_postgres_actor(config: DatabaseConfig) -> OrbitResult<()> {
    let mut postgres_actor = PostgresActor::new("demo-postgres".to_string());
    
    // Initialize the actor
    postgres_actor.initialize(config).await?;
    
    info!("✅ PostgreSQL Actor initialized");
    
    // Test connection
    let is_healthy = postgres_actor.is_healthy().await;
    info!("🏥 Health check: {}", if is_healthy { "HEALTHY" } else { "UNHEALTHY" });
    
    // Execute basic SQL queries
    info!("🔍 Executing basic SQL queries...");
    
    // SELECT 1
    let result = postgres_actor.execute_sql("SELECT 1 as test_column", vec![]).await?;
    info!("   SELECT 1 result: {} rows, {} columns", result.rows.len(), result.columns.len());
    
    // Test schema operations via messages
    let create_table_msg = DatabaseMessage::Schema(SchemaRequest::CreateTable {
        table_definition: TableDefinition {
            name: "demo_users".to_string(),
            schema: Some("public".to_string()),
            columns: vec![
                ColumnDefinition {
                    name: "id".to_string(),
                    data_type: "SERIAL".to_string(),
                    nullable: false,
                    default_value: None,
                    constraints: vec![ColumnConstraint::PrimaryKey],
                },
                ColumnDefinition {
                    name: "name".to_string(),
                    data_type: "TEXT".to_string(),
                    nullable: false,
                    default_value: None,
                    constraints: vec![],
                },
                ColumnDefinition {
                    name: "email".to_string(),
                    data_type: "TEXT".to_string(),
                    nullable: true,
                    default_value: None,
                    constraints: vec![ColumnConstraint::Unique],
                },
                ColumnDefinition {
                    name: "created_at".to_string(),
                    data_type: "TIMESTAMP".to_string(),
                    nullable: false,
                    default_value: Some("NOW()".to_string()),
                    constraints: vec![],
                },
            ],
            constraints: vec![],
            options: HashMap::new(),
        },
        if_not_exists: true,
    });
    
    let response = postgres_actor.handle_message(create_table_msg).await?;
    if let DatabaseResponse::Schema(SchemaResponse::TableCreated { table_name, .. }) = response {
        info!("✅ Created table: {}", table_name);
    }
    
    // Test CREATE EXTENSION
    let extension_msg = DatabaseMessage::Schema(SchemaRequest::CreateExtension {
        extension_name: "vector".to_string(),
        if_not_exists: true,
        schema: None,
        version: None,
    });
    
    let response = postgres_actor.handle_message(extension_msg).await?;
    if let DatabaseResponse::Schema(SchemaResponse::ExtensionCreated { extension_name, version }) = response {
        info!("✅ Created extension: {} (version: {})", extension_name, version);
    }
    
    // Get metrics
    let metrics = postgres_actor.get_metrics().await;
    info!("📊 Metrics - Queries: {:.2}/sec, Avg time: {:.2}ms, Connections: {}", 
          metrics.queries_per_second, metrics.average_query_time_ms, metrics.active_connections);
    
    // Test addressable interface
    let invoke_result = postgres_actor.invoke("execute_sql", vec![
        serde_json::Value::String("SELECT VERSION()".to_string()),
        serde_json::Value::Array(vec![]),
    ]).await?;
    
    info!("🎯 Addressable invoke result received");
    
    // Shutdown
    postgres_actor.shutdown().await?;
    info!("🔒 PostgreSQL Actor shut down");
    
    Ok(())
}

/// Demonstrate Vector Database Actor functionality
async fn demo_vector_actor(config: DatabaseConfig) -> OrbitResult<()> {
    info!("🔍 Vector Database Actor functionality would be implemented here");
    info!("   Features to demonstrate:");
    info!("   • Vector table creation with VECTOR(384) columns");
    info!("   • IVFFLAT and HNSW index creation");
    info!("   • Vector insertion with embeddings");
    info!("   • Similarity search with <->, <#>, <=> operators");
    info!("   • Bulk vector operations and batch processing");
    info!("   • Vector statistics and performance metrics");
    
    // TODO: Implement VectorDatabaseActor
    info!("⚠️  VectorDatabaseActor implementation pending");
    
    Ok(())
}

/// Demonstrate TimescaleDB Actor functionality
async fn demo_timescale_actor(config: DatabaseConfig) -> OrbitResult<()> {
    info!("⏱️  TimescaleDB Actor functionality would be implemented here");
    info!("   Features to demonstrate:");
    info!("   • Hypertable creation for time-series data");
    info!("   • Compression policies for older data");
    info!("   • Retention policies for data lifecycle");
    info!("   • Continuous aggregates for real-time analytics");
    info!("   • Chunk management and statistics");
    info!("   • Background job scheduling");
    
    // TODO: Implement TimescaleActor
    info!("⚠️  TimescaleActor implementation pending");
    
    Ok(())
}

/// Demonstrate Actor Communication patterns
async fn demo_actor_communication(config: DatabaseConfig) -> OrbitResult<()> {
    let mut postgres_actor = PostgresActor::new("communication-postgres".to_string());
    postgres_actor.initialize(config.clone()).await?;
    
    // Demonstrate different message types
    info!("📨 Testing different message types...");
    
    // Health check message
    let health_msg = DatabaseMessage::Health(HealthRequest::CheckHealth);
    let response = postgres_actor.handle_message(health_msg).await?;
    if let DatabaseResponse::Health(HealthResponse::Healthy { checked_at, response_time_ms }) = response {
        info!("✅ Health check: OK ({}ms) at {}", response_time_ms, checked_at.format("%H:%M:%S"));
    }
    
    // Connection status message
    let conn_msg = DatabaseMessage::Connection(ConnectionRequest::TestConnection);
    let response = postgres_actor.handle_message(conn_msg).await?;
    if let DatabaseResponse::Connection(ConnectionResponse::ConnectionOk { database, version, connection_count }) = response {
        info!("✅ Connection test: {} {} ({} connections)", database, version, connection_count);
    }
    
    // Monitoring message
    let monitor_msg = DatabaseMessage::Monitoring(MonitoringRequest::GetMetrics);
    let response = postgres_actor.handle_message(monitor_msg).await?;
    if let DatabaseResponse::Monitoring(MonitoringResponse::Metrics { metrics, collected_at }) = response {
        info!("📊 Metrics collected at {}: {} active connections, {:.2}% cache hit ratio", 
              collected_at.format("%H:%M:%S"), metrics.active_connections, metrics.cache_hit_ratio * 100.0);
    }
    
    // Transaction message
    let tx_msg = DatabaseMessage::Transaction(TransactionRequest::Begin {
        isolation_level: Some(IsolationLevel::ReadCommitted),
        access_mode: Some(TransactionAccessMode::ReadWrite),
        deferrable: Some(false),
    });
    let response = postgres_actor.handle_message(tx_msg).await?;
    if let DatabaseResponse::Transaction(TransactionResponse::Started { transaction_id, isolation_level, started_at }) = response {
        info!("🔄 Transaction started: {} ({:?}) at {}", 
              transaction_id, isolation_level, started_at.format("%H:%M:%S"));
        
        // Commit the transaction
        let commit_msg = DatabaseMessage::Transaction(TransactionRequest::Commit { transaction_id });
        let response = postgres_actor.handle_message(commit_msg).await?;
        if let DatabaseResponse::Transaction(TransactionResponse::Committed { transaction_id, committed_at, duration_ms }) = response {
            info!("✅ Transaction committed: {} in {}ms at {}", 
                  transaction_id, duration_ms, committed_at.format("%H:%M:%S"));
        }
    }
    
    // Query message
    let query_msg = DatabaseMessage::Query(QueryRequest {
        sql: "SELECT 'Hello from Database Actors!' as message".to_string(),
        parameters: vec![],
        context: QueryContext::default(),
        options: QueryOptions {
            explain: true,
            analyze: false,
            include_plan: true,
            ..Default::default()
        },
    });
    
    let response = postgres_actor.handle_message(query_msg).await?;
    if let DatabaseResponse::Query(query_response) = response {
        info!("📝 Query executed in {}ms:", query_response.execution_time_ms);
        for row in &query_response.rows {
            if let Some(serde_json::Value::String(message)) = row.get(0) {
                info!("   📃 Result: {}", message);
            }
        }
        if let Some(plan) = &query_response.query_plan {
            info!("   📋 Query plan: {}", plan);
        }
    }
    
    postgres_actor.shutdown().await?;
    
    Ok(())
}

/// Utility function to demonstrate error handling
#[allow(dead_code)]
async fn demo_error_handling() -> OrbitResult<()> {
    info!("🚨 Demonstrating error handling patterns");
    
    let mut postgres_actor = PostgresActor::new("error-demo".to_string());
    
    // Try to use actor before initialization
    let result = postgres_actor.execute_sql("SELECT 1", vec![]).await;
    match result {
        Ok(_) => info!("   Unexpected success"),
        Err(e) => info!("   ✅ Expected error: {}", e),
    }
    
    // Initialize and try invalid SQL
    let config = DatabaseConfig::default();
    postgres_actor.initialize(config).await?;
    
    let result = postgres_actor.execute_sql("INVALID SQL SYNTAX", vec![]).await;
    match result {
        Ok(_) => info!("   Unexpected success"),
        Err(e) => info!("   ✅ Expected SQL error: {}", e),
    }
    
    postgres_actor.shutdown().await?;
    
    Ok(())
}

/// Performance benchmarking example
#[allow(dead_code)]
async fn demo_performance_benchmark() -> OrbitResult<()> {
    info!("⚡ Running performance benchmark");
    
    let mut postgres_actor = PostgresActor::new("benchmark".to_string());
    let config = DatabaseConfig::default();
    postgres_actor.initialize(config).await?;
    
    let start_time = std::time::Instant::now();
    let query_count = 100;
    
    for i in 0..query_count {
        let sql = format!("SELECT {} as iteration", i);
        let _result = postgres_actor.execute_sql(&sql, vec![]).await?;
    }
    
    let duration = start_time.elapsed();
    let qps = query_count as f64 / duration.as_secs_f64();
    
    info!("📊 Benchmark results:");
    info!("   Queries: {}", query_count);
    info!("   Duration: {:.2}s", duration.as_secs_f64());
    info!("   QPS: {:.2}", qps);
    info!("   Avg latency: {:.2}ms", duration.as_millis() as f64 / query_count as f64);
    
    let metrics = postgres_actor.get_metrics().await;
    info!("   Cache hit ratio: {:.2}%", metrics.cache_hit_ratio * 100.0);
    
    postgres_actor.shutdown().await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_config_creation() {
        let config = DatabaseConfig::default();
        assert_eq!(config.postgres.host, "localhost");
        assert_eq!(config.postgres.port, 5432);
        assert_eq!(config.vector.default_dimension, 384);
        assert!(config.timescale.enable_compression);
    }
    
    #[tokio::test]
    async fn test_postgres_actor_lifecycle() {
        let mut actor = PostgresActor::new("test".to_string());
        assert!(!actor.is_healthy().await);
        
        let config = DatabaseConfig::default();
        actor.initialize(config).await.unwrap();
        assert!(actor.is_healthy().await);
        
        actor.shutdown().await.unwrap();
        assert!(!actor.is_healthy().await);
    }
    
    #[tokio::test]
    async fn test_message_serialization() {
        let query_msg = DatabaseMessage::Query(QueryRequest {
            sql: "SELECT 1".to_string(),
            parameters: vec![],
            context: QueryContext::default(),
            options: QueryOptions::default(),
        });
        
        let json = serde_json::to_string(&query_msg).unwrap();
        let _deserialized: DatabaseMessage = serde_json::from_str(&json).unwrap();
    }
}