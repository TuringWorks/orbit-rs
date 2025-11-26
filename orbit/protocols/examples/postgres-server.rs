//! PostgreSQL Wire Protocol Example Server
//!
//! This example demonstrates a PostgreSQL-compatible server that allows
//! standard PostgreSQL clients (psql, pgAdmin, etc.) to query actor state.
//!
//! ## Usage
//!
//! Start the server:
//! ```bash
//! cargo run --example postgres-server
//! ```
//!
//! Connect with psql:
//! ```bash
//! psql -h localhost -p 5433 -U orbit -d actors
//! ```
//!
//! Run queries:
//! ```sql
//! -- Create an actor
//! INSERT INTO actors (actor_id, actor_type, state)
//! VALUES ('user:123', 'UserActor', '{"name": "Alice", "balance": 1000}');
//!
//! -- Query actors
//! SELECT * FROM actors;
//! SELECT * FROM actors WHERE actor_id = 'user:123';
//!
//! -- Update actor state
//! UPDATE actors SET state = '{"name": "Alice", "balance": 1500}'
//! WHERE actor_id = 'user:123';
//!
//! -- Delete actor
//! DELETE FROM actors WHERE actor_id = 'user:123';
//! ```

use orbit_protocols::postgres_wire::QueryEngine;
use orbit_server::protocols::PostgresServer;
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

    // Create a shared query engine (with pluggable storage architecture available)
    let query_engine = QueryEngine::new();

    // Create PostgreSQL server on port 5433 (avoid conflict with real PostgreSQL)
    let server = PostgresServer::new_with_query_engine("127.0.0.1:5433", query_engine);

    println!(
        "🚀 PostgreSQL Wire Protocol Server with Pluggable Storage starting on 127.0.0.1:5433"
    );
    println!("📦 Using storage backend: Memory (in development: LSM-Tree for persistence)");
    println!();
    println!("Connect with psql:");
    println!("  psql -h localhost -p 5433 -U orbit -d actors");
    println!();
    println!("Example queries:");
    println!("  CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT, email TEXT);");
    println!("  INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');");
    println!("  SELECT * FROM users;");
    println!("  UPDATE users SET email = 'alice.new@example.com' WHERE id = 1;");
    println!("  DELETE FROM users WHERE id = 1;");
    println!("  \\dt     -- List tables");
    println!("  \\d users -- Describe table structure");
    println!();

    // Start the server
    server.run().await?;

    Ok(())
}
