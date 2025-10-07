use orbit_benchmarks::persistence::*;
use orbit_benchmarks::persistence::cow_btree::CowBTreePersistence;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌳 COW B+ Tree Demo for Orbit-rs Actor Leases");
    println!("===============================================");
    
    // Create temporary directory for demo
    let temp_dir = tempfile::tempdir()?;
    println!("📁 Data directory: {}", temp_dir.path().display());
    
    // Initialize COW B+ Tree persistence
    let persistence = CowBTreePersistence::new(temp_dir.path()).await?;
    println!("✅ COW B+ Tree initialized");
    
    // Demo 1: Basic lease operations
    println!("\n📝 Demo 1: Basic Actor Lease Operations");
    let actor_id = Uuid::new_v4();
    let lease = ActorLease::new(
        actor_id,
        "user_session_actor".to_string(),
        "node_1".to_string(),
        Duration::from_secs(300), // 5 minute lease
    );
    
    println!("   Actor ID: {}", actor_id);
    println!("   Actor Type: {}", lease.key.actor_type);
    println!("   Node: {}", lease.node_id);
    
    // Store the lease
    let store_metrics = persistence.store_lease(&lease).await?;
    println!("   ✅ Stored lease in {:.1}μs", store_metrics.latency.as_micros());
    
    // Retrieve the lease
    let (retrieved, get_metrics) = persistence.get_lease(&lease.key).await?;
    println!("   📖 Retrieved lease in {:.1}μs", get_metrics.latency.as_micros());
    
    if let Some(retrieved_lease) = retrieved {
        println!("   ✅ Lease verified: version {}", retrieved_lease.version);
    }
    
    // Demo 2: Lease renewal (common operation in orbit-rs)
    println!("\n🔄 Demo 2: Lease Renewal");
    let mut renewed_lease = lease.clone();
    renewed_lease.renew(Duration::from_secs(600)); // Extend to 10 minutes
    
    let renew_metrics = persistence.store_lease(&renewed_lease).await?;
    println!("   ✅ Renewed lease in {:.1}μs", renew_metrics.latency.as_micros());
    println!("   📈 Version updated: {} -> {}", lease.version, renewed_lease.version);
    println!("   📊 Renewal count: {}", renewed_lease.metadata.renewal_count);
    
    // Demo 3: Multiple actors simulation
    println!("\n👥 Demo 3: Multiple Actor Simulation");
    let mut actor_keys = Vec::new();
    let start_time = std::time::Instant::now();
    
    for i in 0..100 {
        let actor_lease = ActorLease::new(
            Uuid::new_v4(),
            format!("worker_actor_{}", i % 5), // 5 different actor types
            format!("node_{}", i % 3),         // 3 different nodes
            Duration::from_secs(300),
        );
        
        persistence.store_lease(&actor_lease).await?;
        actor_keys.push(actor_lease.key.clone());
        
        if (i + 1) % 25 == 0 {
            println!("   📝 Stored {} actors...", i + 1);
        }
    }
    
    let batch_time = start_time.elapsed();
    println!("   ✅ Stored 100 actors in {:.2}ms", batch_time.as_millis());
    println!("   📊 Average: {:.1}μs per actor", batch_time.as_micros() as f64 / 100.0);
    
    // Demo 4: Range queries for cluster coordination
    println!("\n🔍 Demo 4: Range Queries (Cluster Coordination)");
    let start_key = ActorKey {
        actor_id: Uuid::nil(),
        actor_type: "worker_actor_0".to_string(),
    };
    let end_key = ActorKey {
        actor_id: Uuid::max(),
        actor_type: "worker_actor_2".to_string(),
    };
    
    let (range_results, range_metrics) = persistence.range_query(&start_key, &end_key).await?;
    println!("   🔍 Range query completed in {:.1}μs", range_metrics.latency.as_micros());
    println!("   📊 Found {} actors in range", range_results.len());
    
    // Demo 5: Snapshot creation
    println!("\n📸 Demo 5: Snapshot Creation");
    let (snapshot_id, snapshot_metrics) = persistence.create_snapshot().await?;
    println!("   📸 Snapshot created in {:.1}μs", snapshot_metrics.latency.as_micros());
    println!("   🆔 Snapshot ID: {}", snapshot_id);
    
    // Demo 6: Performance characteristics
    println!("\n📊 Demo 6: Storage Statistics");
    let stats = persistence.get_stats().await?;
    println!("   📝 Total keys: {}", stats.total_keys);
    println!("   💾 Memory usage: {:.2} KB", stats.memory_usage_bytes as f64 / 1024.0);
    println!("   💽 Disk usage: {:.2} KB", stats.disk_usage_bytes as f64 / 1024.0);
    println!("   📏 Avg key size: {} bytes", stats.average_key_size);
    println!("   📐 Avg value size: {} bytes", stats.average_value_size);
    
    // Demo 7: Crash recovery simulation
    println!("\n🔧 Demo 7: Crash Recovery Simulation");
    let recovery_metrics = persistence.simulate_crash_recovery().await?;
    println!("   🔧 Recovery completed in {:.2}ms", recovery_metrics.latency.as_millis());
    println!("   💾 Data read during recovery: {} bytes", recovery_metrics.disk_bytes_read);
    
    // Final verification
    println!("\n🔍 Final Verification");
    let verification_start = std::time::Instant::now();
    let mut found_count = 0;
    
    for key in &actor_keys[..10] { // Check first 10
        if let (Some(_), _) = persistence.get_lease(key).await? {
            found_count += 1;
        }
    }
    
    let verification_time = verification_start.elapsed();
    println!("   ✅ Verified {}/10 actors in {:.2}ms", found_count, verification_time.as_millis());
    
    println!("\n🎉 COW B+ Tree Demo Complete!");
    println!("   📈 Key advantages:");
    println!("     - Fast writes via copy-on-write semantics");
    println!("     - Efficient memory sharing between versions");
    println!("     - Excellent range query performance");
    println!("     - Built-in snapshot capabilities");
    println!("     - Predictable recovery times");
    
    Ok(())
}