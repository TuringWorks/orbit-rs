use orbit_server::{OrbitServer, OrbitServerConfig};
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Orbit Hello World example");

    // Create server configuration
    let server_config = OrbitServerConfig {
        namespace: "hello-world".to_string(),
        bind_address: "0.0.0.0".to_string(),
        port: 8080,
        lease_duration: Duration::from_secs(300),
        cleanup_interval: Duration::from_secs(60),
        max_addressables: Some(1000),
        tags: std::collections::HashMap::new(),
    };

    // Create and start the server
    info!("Starting Orbit server on port 8080...");
    let mut server = OrbitServer::new(server_config).await?;

    // Register some addressable types
    server
        .register_addressable_type("GreeterActor".to_string())
        .await?;
    server
        .register_addressable_type("CounterActor".to_string())
        .await?;

    info!(
        "Server configured with node ID: {:?}",
        server.node_info().id
    );
    info!(
        "Addressable types: {:?}",
        server.node_info().capabilities.addressable_types
    );

    // In this simple example, we'll start the server which will run indefinitely
    // In a real application, you might want to handle graceful shutdown
    info!("Starting gRPC server - this will run until interrupted");
    server.start().await?;

    Ok(())
}
