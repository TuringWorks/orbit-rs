//! Example showing OrbitServer with integrated RESP and PostgreSQL protocols
//!
//! This example demonstrates how to run a unified Orbit server that handles
//! all three protocols simultaneously:
//! - gRPC (port 50051) - for Orbit client connections
//! - Redis RESP (port 6379) - for Redis clients
//! - PostgreSQL (port 5432) - for PostgreSQL clients
//!
//! Usage:
//!   cargo run --example integrated-server
//!
//! Then connect with various clients:
//!   redis-cli -h localhost -p 6379
//!   psql -h localhost -p 5432 -U orbit -d actors
//!   grpcurl -plaintext localhost:50051 ...

use orbit_server::{OrbitServer, ProtocolConfig};
use std::error::Error;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,orbit_server=debug,orbit_protocols=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Starting Unified Orbit Server with All Protocols...");

    // Configure all protocols to be enabled
    let protocol_config = ProtocolConfig {
        redis_enabled: true,
        redis_port: 6379,
        redis_bind_address: "127.0.0.1".to_string(),
        postgres_enabled: true,
        postgres_port: 5432,
        postgres_bind_address: "127.0.0.1".to_string(),
    };

    // Create and configure the server
    let mut server = OrbitServer::builder()
        .with_namespace("integrated-demo")
        .with_port(50051) // gRPC port
        .with_bind_address("127.0.0.1")
        .with_protocols(protocol_config)
        .build()
        .await?;

    // Display server configuration
    let stats = server.stats().await?;
    info!("✅ Server configured successfully!");
    info!("   Namespace: {}", stats.node_id.namespace);
    info!("   Node ID: {}", stats.node_id.key);
    info!("");
    info!("🌐 Protocol Servers:");
    info!("   - gRPC: 127.0.0.1:50051 (Orbit clients)");

    if stats.protocol_stats.redis_enabled {
        info!(
            "   - Redis: {} (redis-cli, Redis clients)",
            stats.protocol_stats.redis_address.unwrap()
        );
    }

    if stats.protocol_stats.postgres_enabled {
        info!(
            "   - PostgreSQL: {} (psql, PostgreSQL clients)",
            stats.protocol_stats.postgres_address.unwrap()
        );
    }

    info!("");
    info!("📡 Client Connection Examples:");
    info!("   Redis:      redis-cli -h localhost -p 6379");
    info!("   PostgreSQL: psql -h localhost -p 5432 -U orbit -d actors");
    info!("   gRPC:       Use OrbitClient or grpcurl localhost:50051");
    info!("");
    info!("💡 Try these Redis commands:");
    info!("   > SET mykey \"Hello Orbit\"");
    info!("   > GET mykey");
    info!("   > HSET user:1 name \"Alice\" age \"30\"");
    info!("   > HGETALL user:1");
    info!("");
    info!("💡 Try these PostgreSQL commands:");
    info!("   CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT);");
    info!("   INSERT INTO users (name) VALUES ('Alice');");
    info!("   SELECT * FROM users;");
    info!("");

    // Start the server (this will block and run all protocols)
    if let Err(e) = server.start().await {
        error!("Server failed: {}", e);
        return Err(e.into());
    }

    Ok(())
}
