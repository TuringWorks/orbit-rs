use orbit_server::persistence::*;
use orbit_shared::*;
use std::sync::Arc;
use tokio;
use tracing;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::init();

    println!("🚀 Orbit Persistence Provider Demo");
    println!("=====================================\n");

    // Example 1: Memory provider with disk backup
    println!("📁 Example 1: Memory Provider with Disk Backup");
    let memory_config = MemoryConfig {
        max_entries: Some(1000),
        disk_backup: Some(DiskBackupConfig {
            path: "/tmp/orbit-demo-backup.json".to_string(),
            sync_interval: 60,
            compression: CompressionType::Gzip,
        }),
    };

    let persistence_config = PersistenceProviderConfig::builder()
        .with_memory("memory", memory_config, true)
        .build()?;

    let registry = persistence_config.create_registry().await?;
    let addressable_provider = registry.get_default_addressable_provider().await?;

    // Create a sample lease
    let reference = AddressableReference {
        addressable_type: "DemoActor".to_string(),
        key: Key::StringKey {
            key: "demo-123".to_string(),
        },
    };

    let lease = AddressableLease {
        reference: reference.clone(),
        node_id: NodeId::generate("demo".to_string()),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        renew_at: chrono::Utc::now() + chrono::Duration::minutes(2),
    };

    // Store and retrieve the lease
    addressable_provider.store_lease(&lease).await?;
    let retrieved = addressable_provider.get_lease(&reference).await?;
    
    match retrieved {
        Some(retrieved_lease) => {
            println!("✅ Successfully stored and retrieved lease for {}", 
                   retrieved_lease.reference.addressable_type);
            println!("   Node: {}", retrieved_lease.node_id);
            println!("   Expires: {}", retrieved_lease.expires_at);
        }
        None => println!("❌ Failed to retrieve lease"),
    }

    println!("✅ Demo completed successfully!");
    Ok(())
}
