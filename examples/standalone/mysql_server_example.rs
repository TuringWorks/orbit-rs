//! MySQL protocol adapter server example
//!
//! This example demonstrates how to start a MySQL-compatible server using Orbit's MySQL protocol adapter.
//! You can connect to it using any MySQL client (mysql CLI, MySQL Workbench, etc.)
//!
//! # Usage
//!
//! ```bash
//! cargo run --example mysql_server_example
//! ```
//!
//! Then connect with a MySQL client:
//!
//! ```bash
//! mysql -h 127.0.0.1 -P 3306 -u orbit
//! ```

use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Orbit MySQL Protocol Adapter...");

    // Configure MySQL adapter
    let config = MySqlConfig {
        listen_addr: "127.0.0.1:3306".parse()?,
        max_connections: 1000,
        authentication_enabled: false, // Disable auth for demo
        server_version: "Orbit-DB 1.0.0 (MySQL-compatible)".to_string(),
    };

    println!("Configuration:");
    println!("  - Address: {}", config.listen_addr);
    println!("  - Max Connections: {}", config.max_connections);
    println!("  - Authentication: {}", if config.authentication_enabled { "Enabled" } else { "Disabled" });
    println!("  - Server Version: {}", config.server_version);
    println!();

    // Create and start MySQL adapter
    let adapter = MySqlAdapter::new(config).await?;

    println!("MySQL server started successfully!");
    println!("Connect with: mysql -h 127.0.0.1 -P 3306 -u orbit");
    println!();

    // Run server (this blocks)
    adapter.start().await?;

    Ok(())
}
