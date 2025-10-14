use orbit_server::persistence::config::PersistenceProviderConfig;
use orbit_server::OrbitServer;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Orbit Hello World example");

    // Create and configure the server using the builder pattern
    info!("Starting Orbit server on port 8080...");
    let mut server = OrbitServer::builder()
        .with_namespace("hello-world")
        .with_bind_address("0.0.0.0")
        .with_port(8080)
        .with_lease_duration(Duration::from_secs(300))
        .with_max_addressables(1000)
        .with_persistence(PersistenceProviderConfig::default_memory())
        .with_redis_enabled(false) // Disable Redis for this simple example
        .with_postgres_enabled(false) // Disable PostgreSQL for this simple example
        .build()
        .await?;

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
