//! Example RESP (Redis) protocol server
//!
//! This example demonstrates how to use the orbit-protocols crate to create
//! a Redis-compatible server that can be accessed by any Redis client.
//!
//! Usage:
//!   cargo run --example resp-server
//!
//! Then connect with a Redis client:
//!   redis-cli -h localhost -p 6379
//!   > PING
//! > PONG
//!   > SET mykey "Hello Orbit"
//! > OK
//!   > GET mykey
//! > "Hello Orbit"

use std::error::Error;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use orbit_client::OrbitClient;
use orbit_protocols::resp::RespServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,orbit_protocols=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Starting Orbit RESP (Redis) Protocol Server...");

    // Create Orbit client
    // Note: In this example, we're using a placeholder OrbitClient
    // In a real deployment, this would connect to an actual Orbit cluster
    let orbit_client = match create_orbit_client().await {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create OrbitClient: {}", e);
            // For demonstration purposes, create a mock client
            create_mock_orbit_client().await?
        }
    };

    info!("✅ Connected to Orbit cluster");

    // Create RESP server
    let server = RespServer::new("127.0.0.1:6379", orbit_client);

    info!("🌐 RESP server listening on 127.0.0.1:6379");
    info!("📡 Redis clients can now connect using: redis-cli -h localhost -p 6379");
    info!("");
    info!("Try these commands:");
    info!("  > PING");
    info!("  > ECHO \"Hello Orbit\"");
    info!("  > SET mykey \"Hello World\"");
    info!("  > GET mykey");
    info!("  > HSET user:1 name \"Alice\" age \"30\"");
    info!("  > HGETALL user:1");
    info!("  > LPUSH queue item1 item2 item3");
    info!("  > LRANGE queue 0 -1");
    info!("  > INFO");
    info!("");

    // Start the server (this will block)
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}

/// Attempt to create a real OrbitClient
async fn create_orbit_client() -> Result<OrbitClient, Box<dyn Error>> {
    // Try to connect to a real Orbit cluster
    // This would typically connect to orbit-server running on localhost or cluster
    let client = OrbitClient::builder()
        .with_namespace("redis-demo")
        .with_server_urls(vec!["http://localhost:50056".to_string()]) // Default orbit-server gRPC port
        .build()
        .await?;

    Ok(client)
}

/// Create a mock OrbitClient for demonstration purposes
async fn create_mock_orbit_client() -> Result<OrbitClient, Box<dyn Error>> {
    info!("⚠️  Using offline OrbitClient for demonstration");
    info!("   In production, ensure orbit-server is running and accessible");

    // Create an offline OrbitClient that doesn't require server connections
    let client = OrbitClient::builder()
        .with_namespace("redis-demo-offline")
        .with_offline_mode(true)
        .build()
        .await?;

    Ok(client)
}
