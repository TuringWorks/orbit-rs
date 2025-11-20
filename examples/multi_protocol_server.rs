//! Multi-Protocol Server Example
//!
//! This example demonstrates running multiple protocol adapters simultaneously:
//! - CQL (Cassandra) on port 9042
//! - MySQL on port 3306
//!
//! Clients can connect using either protocol to interact with the same underlying Orbit storage.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example multi_protocol_server
//! ```
//!
//! Then connect with either:
//! ```bash
//! # CQL
//! cqlsh 127.0.0.1 9042
//!
//! # MySQL
//! mysql -h 127.0.0.1 -P 3306 -u orbit
//! ```

use orbit_protocols::cql::{CqlAdapter, CqlConfig};
use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║      Orbit Multi-Protocol Server                          ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Configure CQL adapter
    let cql_config = CqlConfig {
        listen_addr: "127.0.0.1:9042".parse()?,
        max_connections: 1000,
        authentication_enabled: false,
        protocol_version: 4,
    };

    // Configure MySQL adapter
    let mysql_config = MySqlConfig {
        listen_addr: "127.0.0.1:3306".parse()?,
        max_connections: 1000,
        authentication_enabled: false,
        server_version: "8.0.0-Orbit".to_string(),
    };

    println!("📋 Configuration:");
    println!("   ┌─ CQL (Cassandra)");
    println!("   │  • Address: {}", cql_config.listen_addr);
    println!("   │  • Protocol Version: {}", cql_config.protocol_version);
    println!("   │  • Max Connections: {}", cql_config.max_connections);
    println!("   │  • Authentication: {}", if cql_config.authentication_enabled { "Enabled" } else { "Disabled" });
    println!("   │");
    println!("   └─ MySQL");
    println!("      • Address: {}", mysql_config.listen_addr);
    println!("      • Server Version: {}", mysql_config.server_version);
    println!("      • Max Connections: {}", mysql_config.max_connections);
    println!("      • Authentication: {}", if mysql_config.authentication_enabled { "Enabled" } else { "Disabled" });
    println!();

    // Create adapters
    let cql_adapter = CqlAdapter::new(cql_config).await?;
    let mysql_adapter = MySqlAdapter::new(mysql_config).await?;

    println!("🚀 Starting protocol adapters...");
    println!();

    // Spawn CQL server in background
    let cql_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> = tokio::spawn(async move {
        println!("✅ CQL adapter started on port 9042");
        cql_adapter.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });

    // Spawn MySQL server in background
    let mysql_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> = tokio::spawn(async move {
        println!("✅ MySQL adapter started on port 3306");
        mysql_adapter.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });

    println!("════════════════════════════════════════════════════════════");
    println!("🎯 Servers running! Connect with:");
    println!();
    println!("   CQL (Cassandra):");
    println!("   └─ cqlsh 127.0.0.1 9042");
    println!();
    println!("   MySQL:");
    println!("   └─ mysql -h 127.0.0.1 -P 3306 -u orbit");
    println!();
    println!("Press Ctrl+C to stop");
    println!("════════════════════════════════════════════════════════════");
    println!();

    // Wait for both servers (will run indefinitely until interrupted)
    tokio::select! {
        result = cql_handle => {
            match result {
                Ok(Ok(())) => println!("CQL server exited successfully"),
                Ok(Err(e)) => eprintln!("CQL server error: {}", e),
                Err(e) => eprintln!("CQL server panic: {}", e),
            }
        }
        result = mysql_handle => {
            match result {
                Ok(Ok(())) => println!("MySQL server exited successfully"),
                Ok(Err(e)) => eprintln!("MySQL server error: {}", e),
                Err(e) => eprintln!("MySQL server panic: {}", e),
            }
        }
    }

    Ok(())
}
