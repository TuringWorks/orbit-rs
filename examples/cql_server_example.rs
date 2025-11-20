//! CQL (Cassandra Query Language) Server Example
//!
//! This example demonstrates how to run a CQL-compatible server using Orbit-RS.
//! You can connect to it using cqlsh or any Cassandra driver.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example cql_server_example
//! ```
//!
//! ## Connect with cqlsh
//!
//! ```bash
//! cqlsh localhost 9042
//! ```
//!
//! ## Example CQL commands
//!
//! ```cql
//! -- Create a keyspace
//! CREATE KEYSPACE IF NOT EXISTS my_keyspace
//! WITH REPLICATION = {'class': 'SimpleStrategy', 'replication_factor': 1};
//!
//! -- Use the keyspace
//! USE my_keyspace;
//!
//! -- Create a table
//! CREATE TABLE IF NOT EXISTS users (
//!     user_id UUID PRIMARY KEY,
//!     username TEXT,
//!     email TEXT,
//!     created_at TIMESTAMP
//! );
//!
//! -- Insert data
//! INSERT INTO users (user_id, username, email, created_at)
//! VALUES (uuid(), 'alice', 'alice@example.com', toTimestamp(now()));
//!
//! -- Query data
//! SELECT * FROM users;
//! SELECT username, email FROM users WHERE user_id = <uuid>;
//!
//! -- Update data
//! UPDATE users SET email = 'newemail@example.com' WHERE user_id = <uuid>;
//!
//! -- Delete data
//! DELETE FROM users WHERE user_id = <uuid>;
//! ```

use orbit_protocols::cql::{CqlAdapter, CqlConfig};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Configure CQL server
    let config = CqlConfig {
        listen_addr: "127.0.0.1:9042".parse()?,
        max_connections: 1000,
        authentication_enabled: false,
        protocol_version: 4,
    };

    println!("Starting CQL server on {}", config.listen_addr);
    println!("Connect with: cqlsh localhost 9042");
    println!("\nPress Ctrl+C to stop the server");

    // Create and start CQL adapter
    let adapter = CqlAdapter::new(config).await?;
    adapter.start().await?;

    Ok(())
}
