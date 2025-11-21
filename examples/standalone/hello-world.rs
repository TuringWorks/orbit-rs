use orbit_server::persistence::config::PersistenceProviderConfig;
use orbit_server::OrbitServer;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("🚀 Hello World from Orbit-RS!");

    // Create the server using the builder pattern
    info!("🔧 Creating Orbit server...");
    let mut server = OrbitServer::builder()
        .with_namespace("hello-world")
        .with_bind_address("127.0.0.1")
        .with_port(8080)
        .with_lease_duration(Duration::from_secs(300))
        .with_max_addressables(1000)
        .with_persistence(PersistenceProviderConfig::default_memory())
        .with_redis_enabled(false)    // Disable Redis for this simple example
        .with_postgres_enabled(false) // Disable PostgreSQL for this simple example
        .build()
        .await?;

    // Register some basic addressable types
    info!("📝 Registering addressable types...");
    server
        .register_addressable_type("HelloActor".to_string())
        .await?;
    server
        .register_addressable_type("GreetingActor".to_string())
        .await?;

    // Display server information
    info!("✅ Server ready!");
    info!("   Node ID: {:?}", server.node_info().id);
    info!("   Namespace: hello-world");
    info!("   Address: 127.0.0.1:8080");
    info!("   Addressable types: {:?}", server.node_info().capabilities.addressable_types);

    // Start the server in a background task
    info!("🌟 Starting Orbit server...");
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("❌ Server error: {}", e);
        }
    });

    // Give the server a moment to start
    sleep(Duration::from_millis(500)).await;

    info!("🎉 Hello World example is running!");
    info!("   The Orbit server is now active and ready to accept connections");
    info!("   Press Ctrl+C to stop the server");

    // Wait for the server (or until interrupted)
    let _ = server_handle.await;

    info!("👋 Goodbye from Orbit-RS!");
    Ok(())
}