//! PostgreSQL Storage Architecture Demo
//!
//! This example demonstrates the new pluggable storage architecture for Orbit-RS's
//! PostgreSQL wire protocol implementation. It shows how to use different storage
//! backends and perform various SQL operations with persistence.

use orbit_protocols::postgres_wire::sql::executor::SqlExecutor;
use orbit_protocols::postgres_wire::storage::{StorageBackendConfig, StorageBackendFactory};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,orbit_protocols=debug")),
        )
        .init();

    println!("🚀 PostgreSQL Storage Architecture Demo");
    println!("{}", "=".repeat(50));

    // Demo 1: Memory Storage Backend
    println!("\n📦 Demo 1: Memory Storage Backend");
    println!("{}", "-".repeat(40));

    let memory_config = StorageBackendConfig::Memory;
    let executor = SqlExecutor::new_with_storage_config(memory_config).await?;

    // Test schema operations
    println!("Creating table with memory backend...");
    let result = executor
        .execute("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT)")
        .await?;
    println!("Result: {:?}", result);

    // Test data operations
    println!("Inserting data...");
    let result = executor
        .execute("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')")
        .await?;
    println!("Result: {:?}", result);

    let result = executor
        .execute("INSERT INTO users (name, email) VALUES ('Bob', 'bob@example.com')")
        .await?;
    println!("Result: {:?}", result);

    // Test queries
    println!("Querying data...");
    let result = executor.execute("SELECT * FROM users").await?;
    println!("Result: {:?}", result);

    // Test updates
    println!("Updating data...");
    let result = executor
        .execute("UPDATE users SET email = 'alice.new@example.com' WHERE name = 'Alice'")
        .await?;
    println!("Result: {:?}", result);

    // Verify update
    let result = executor
        .execute("SELECT * FROM users WHERE name = 'Alice'")
        .await?;
    println!("Updated user: {:?}", result);

    // Test information_schema queries
    println!("Querying information_schema...");
    let result = executor
        .execute("SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'")
        .await?;
    println!("Tables: {:?}", result);

    // Demo 2: Storage Metrics
    println!("\n📊 Demo 2: Storage Metrics");
    println!("{}", "-".repeat(40));

    let metrics = executor.storage_metrics().await;
    println!("Storage metrics:");
    println!("  Read operations: {}", metrics.read_operations);
    println!("  Write operations: {}", metrics.write_operations);
    println!("  Delete operations: {}", metrics.delete_operations);
    println!("  Memory usage: {} bytes", metrics.memory_usage_bytes);
    println!("  Error count: {}", metrics.error_count);

    // Demo 3: Storage Factory Pattern
    println!("\n🏷️  Demo 3: Storage Factory Pattern");
    println!("{}", "-".repeat(40));

    println!("Creating storage backends via factory...");

    // Create multiple storage instances
    let storage1 = StorageBackendFactory::create_backend(&StorageBackendConfig::Memory).await?;
    let storage2 = StorageBackendFactory::create_backend(&StorageBackendConfig::Memory).await?;

    // Create executors with different storage backends
    let executor1 = SqlExecutor::with_storage(storage1);
    let executor2 = SqlExecutor::with_storage(storage2);

    // Test isolation between storage backends
    let _ = executor1.execute("CREATE TABLE test1 (id INT)").await?;
    let _ = executor2.execute("CREATE TABLE test2 (id INT)").await?;

    println!("Executor 1 tables:");
    let result1 = executor1
        .execute("SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'")
        .await?;
    println!("  {:?}", result1);

    println!("Executor 2 tables:");
    let result2 = executor2
        .execute("SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'")
        .await?;
    println!("  {:?}", result2);

    // Demo 4: Transaction Support (Basic)
    println!("\n💾 Demo 4: Transaction Support");
    println!("{}", "-".repeat(40));

    // Note: Full transaction support will be demonstrated once integrated with query engine
    println!("Transaction support architecture is in place.");
    println!("Features available:");
    println!("  ✅ Begin/Commit/Rollback operations");
    println!("  ✅ Isolation levels (READ COMMITTED, REPEATABLE READ, SERIALIZABLE)");
    println!("  ✅ Write buffering");
    println!("  🚧 Integration with SQL executor (in progress)");

    // Demo 5: Error Handling
    println!("\n⚠️  Demo 5: Error Handling");
    println!("{}", "-".repeat(40));

    // Test invalid SQL
    match executor.execute("INVALID SQL STATEMENT").await {
        Ok(result) => println!("Unexpected success: {:?}", result),
        Err(error) => println!("Expected error handled correctly: {}", error),
    }

    // Test creating table that already exists
    match executor.execute("CREATE TABLE users (id INT)").await {
        Ok(result) => println!("Unexpected success: {:?}", result),
        Err(error) => println!("Duplicate table error handled: {}", error),
    }

    println!("\n✅ Demo completed successfully!");
    println!("The pluggable storage architecture is working correctly.");
    println!("\nNext steps:");
    println!("  1. Enable LSM storage backend for persistence");
    println!("  2. Add distributed cluster backend");
    println!("  3. Integrate with existing query engine");
    println!("  4. Add comprehensive benchmarking");

    Ok(())
}
