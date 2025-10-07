use std::path::Path;
use tokio;
use orbit_benchmarks::persistence::cow_btree::CowBTreePersistence;
use orbit_benchmarks::persistence::{ActorKey, ActorLease, PersistenceError, PersistenceProvider};
use std::time::{SystemTime, Duration};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), PersistenceError> {
    let data_dir = Path::new("./demo_data");
    
    // Clean up any existing data for a fresh start
    if data_dir.exists() {
        std::fs::remove_dir_all(data_dir).unwrap();
    }
    
    println!("=== Phase 1: Creating initial data ===");
    
    // Create initial persistence instance and add some data
    {
        let mut persistence = CowBTreePersistence::new(data_dir).await?;
        
        // Insert some sample data with known UUIDs for testing
        let test_uuid1 = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let test_uuid2 = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174001").unwrap();
        let test_uuid3 = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174002").unwrap();
        
        let leases = vec![
            ActorLease::new(
                test_uuid1,
                "actor1".to_string(),
                "node1".to_string(),
                Duration::from_secs(60),
            ),
            ActorLease::new(
                test_uuid2,
                "actor2".to_string(),
                "node1".to_string(),
                Duration::from_secs(60),
            ),
            ActorLease::new(
                test_uuid3,
                "actor3".to_string(),
                "node2".to_string(),
                Duration::from_secs(60),
            ),
        ];
        
        let mut stored_keys = Vec::new();
        for lease in &leases {
            stored_keys.push(lease.key.clone());
        }
        
        for lease in leases {
            let metrics = persistence.store_lease(&lease).await?;
            println!("Inserted {:?} - Latency: {:?}, Memory: {} bytes", 
                     lease.key, metrics.latency, metrics.memory_used);
        }
        
        // Query some data to verify it's there
        let (retrieved_lease, _) = persistence.get_lease(&stored_keys[0]).await?;
        if let Some(lease) = retrieved_lease {
            println!("Found lease: {} ({})", lease.key.actor_type, lease.node_id);
        }
        
        // Explicitly flush WAL to disk before dropping persistence instance
        // This simulates a proper shutdown before a crash
        persistence.flush_wal().await?;
        
        println!("Phase 1 complete. Data has been written to WAL.");
    } // persistence instance is dropped, simulating a crash
    
    println!("\n=== Phase 2: Simulating restart and WAL replay ===");
    
    // Create a new persistence instance - this should replay the WAL
    {
        let persistence = CowBTreePersistence::new(data_dir).await?;
        
        println!("New persistence instance created. Checking recovered data...");
        
        // Try to query the same keys that were stored in Phase 1
        let test_uuid1 = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let test_uuid2 = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174001").unwrap();
        let test_uuid3 = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174002").unwrap();
        
        let test_keys = vec![
            ActorKey { actor_id: test_uuid1, actor_type: "actor1".to_string() },
            ActorKey { actor_id: test_uuid2, actor_type: "actor2".to_string() },
            ActorKey { actor_id: test_uuid3, actor_type: "actor3".to_string() },
        ];
        
        for key in &test_keys {
            let (recovered_lease, _) = persistence.get_lease(key).await?;
            match recovered_lease {
                Some(lease) => {
                    println!("✓ Successfully recovered: {} ({})", 
                             lease.key.actor_type, lease.node_id);
                }
                None => {
                    println!("✗ Failed to recover: {:?}", key);
                }
            }
        }
        
        // Add one more entry to verify the system continues to work
        let new_lease = ActorLease::new(
            Uuid::new_v4(),
            "post_recovery".to_string(),
            "node2".to_string(),
            Duration::from_secs(60),
        );
        
        let metrics = persistence.store_lease(&new_lease).await?;
        println!("✓ New insert after recovery successful - Latency: {:?}", metrics.latency);
    }
    
    println!("\n=== Demo completed successfully! ===");
    
    // Clean up
    if data_dir.exists() {
        std::fs::remove_dir_all(data_dir).unwrap();
        println!("Cleaned up demo data directory.");
    }
    
    Ok(())
}