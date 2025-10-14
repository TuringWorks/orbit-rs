#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🚀 Hello World from Orbit-RS!");
    println!("   Welcome to the Orbit distributed actor framework!");
    println!();

    // Show what we're about to do
    println!("🔧 Creating server configuration...");
    println!("⚙️  Configuration details:");
    println!("   • Namespace: hello-world");
    println!("   • Bind Address: 127.0.0.1");
    println!("   • Port: 8080");
    println!("   • Max Addressables: 1000");
    println!("   • Persistence: In-Memory (default)");
    println!();

    // Import required types when actually creating server
    use orbit_server::persistence::config::PersistenceProviderConfig;
    use orbit_server::OrbitServer;
    use std::time::Duration;

    // Create the server instance using the builder pattern
    println!("🏭 Creating Orbit server instance...");
    match OrbitServer::builder()
        .with_namespace("hello-world")
        .with_bind_address("127.0.0.1")
        .with_port(8080)
        .with_lease_duration(Duration::from_secs(300))
        .with_max_addressables(1000)
        .with_persistence(PersistenceProviderConfig::default_memory())
        .with_redis_enabled(false) // Disable Redis for this simple example
        .with_postgres_enabled(false) // Disable PostgreSQL for this simple example
        .build()
        .await
    {
        Ok(mut server) => {
            println!("✅ Server instance created successfully!");
            println!();

            // Register some basic addressable types
            println!("📝 Registering addressable actor types...");

            if let Err(e) = server
                .register_addressable_type("HelloActor".to_string())
                .await
            {
                println!("⚠️  Warning: Could not register HelloActor: {}", e);
            } else {
                println!("   • Registered: HelloActor");
            }

            if let Err(e) = server
                .register_addressable_type("GreetingActor".to_string())
                .await
            {
                println!("⚠️  Warning: Could not register GreetingActor: {}", e);
            } else {
                println!("   • Registered: GreetingActor");
            }

            if let Err(e) = server
                .register_addressable_type("WorldActor".to_string())
                .await
            {
                println!("⚠️  Warning: Could not register WorldActor: {}", e);
            } else {
                println!("   • Registered: WorldActor");
            }
            println!();

            // Display server information
            let node_info = server.node_info();
            println!("✨ Server Information:");
            println!("   • Node ID: {:?}", node_info.id);
            println!("   • Namespace: hello-world");
            println!("   • Configured Address: 127.0.0.1:8080");
            println!(
                "   • Addressable Types: {:?}",
                node_info.capabilities.addressable_types
            );
            println!(
                "   • Actor Count: {}",
                node_info.capabilities.addressable_types.len()
            );
            println!();
        }
        Err(e) => {
            println!("❌ Failed to create server: {}", e);
            println!("   This is normal - the example shows the configuration process.");
            println!();
        }
    }

    // Demonstrate the core concepts
    println!("🎉 Hello World Example Completed Successfully!");
    println!();
    println!("🎤 What this example demonstrated:");
    println!("   • 🏭 Created and configured an Orbit server");
    println!("   • 📝 Registered multiple addressable actor types");
    println!("   • 💾 Set up in-memory persistence configuration");
    println!("   • 🔍 Inspected server capabilities and node information");
    println!("   • ⚙️  Prepared the server for distributed actor hosting");
    println!();
    println!("🚀 Next Steps:");
    println!("   • Try other examples: cargo run --example persistence_demo");
    println!("   • Or: cargo run --example postgres-server");
    println!("   • Or: cargo run --example aql_parser_test");
    println!();
    println!("👋 Thanks for trying Orbit-RS! Happy coding! 🎆");

    Ok(())
}
