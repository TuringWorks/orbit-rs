use orbit_benchmarks::persistence::rocksdb_impl::RocksDBPersistence;
use orbit_benchmarks::persistence::*;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🗿 RocksDB Demo for Orbit-rs Actor Leases");
    println!("=========================================");

    // Create temporary directory for demo
    let temp_dir = tempfile::tempdir()?;
    println!("📁 Data directory: {}", temp_dir.path().display());

    // Initialize RocksDB persistence
    let persistence = RocksDBPersistence::new(temp_dir.path()).await?;
    println!("✅ RocksDB initialized with optimized settings");

    // Demo 1: Basic lease operations
    println!("\n📝 Demo 1: Basic Actor Lease Operations");
    let actor_id = Uuid::new_v4();
    let lease = ActorLease::new(
        actor_id,
        "payment_processor_actor".to_string(),
        "node_alpha".to_string(),
        Duration::from_secs(300), // 5 minute lease
    );

    println!("   Actor ID: {}", actor_id);
    println!("   Actor Type: {}", lease.key.actor_type);
    println!("   Node: {}", lease.node_id);

    // Store the lease
    let store_metrics = persistence.store_lease(&lease).await?;
    println!(
        "   ✅ Stored lease in {:.1}μs",
        store_metrics.latency.as_micros()
    );
    println!("   💾 Bytes written: {}", store_metrics.disk_bytes_written);

    // Retrieve the lease
    let (retrieved, get_metrics) = persistence.get_lease(&lease.key).await?;
    println!(
        "   📖 Retrieved lease in {:.1}μs",
        get_metrics.latency.as_micros()
    );
    println!("   💾 Bytes read: {}", get_metrics.disk_bytes_read);

    if let Some(retrieved_lease) = retrieved {
        println!("   ✅ Lease verified: version {}", retrieved_lease.version);
    }

    // Demo 2: High-throughput batch operations
    println!("\n⚡ Demo 2: High-Throughput Batch Operations");
    let mut actor_keys = Vec::new();
    let start_time = std::time::Instant::now();

    for i in 0..1000 {
        let actor_lease = ActorLease::new(
            Uuid::new_v4(),
            format!("batch_actor_{}", i % 10), // 10 different actor types
            format!("node_{}", i % 5),         // 5 different nodes
            Duration::from_secs(300),
        );

        persistence.store_lease(&actor_lease).await?;
        actor_keys.push(actor_lease.key.clone());

        if (i + 1) % 200 == 0 {
            println!("   📝 Stored {} actors...", i + 1);
        }
    }

    let batch_time = start_time.elapsed();
    println!(
        "   ✅ Stored 1000 actors in {:.2}ms",
        batch_time.as_millis()
    );
    println!(
        "   📊 Average: {:.1}μs per actor",
        batch_time.as_micros() as f64 / 1000.0
    );
    println!(
        "   🚀 Throughput: {:.0} ops/sec",
        1000.0 / batch_time.as_secs_f64()
    );

    // Demo 3: Range queries with RocksDB iterators
    println!("\n🔍 Demo 3: Range Queries (RocksDB Iterators)");
    let start_key = ActorKey {
        actor_id: Uuid::nil(),
        actor_type: "batch_actor_3".to_string(),
    };
    let end_key = ActorKey {
        actor_id: Uuid::max(),
        actor_type: "batch_actor_7".to_string(),
    };

    let (range_results, range_metrics) = persistence.range_query(&start_key, &end_key).await?;
    println!(
        "   🔍 Range query completed in {:.1}μs",
        range_metrics.latency.as_micros()
    );
    println!("   📊 Found {} actors in range", range_results.len());
    println!("   💾 Bytes read: {}", range_metrics.disk_bytes_read);

    // Demo 4: Lease renewal patterns
    println!("\n🔄 Demo 4: Lease Renewal Patterns");
    let mut renewal_times = Vec::new();

    for key in actor_keys.iter().take(50) {
        let (lease_opt, _) = persistence.get_lease(key).await?;

        if let Some(mut lease) = lease_opt {
            lease.renew(Duration::from_secs(600));

            let renew_start = std::time::Instant::now();
            persistence.store_lease(&lease).await?;
            renewal_times.push(renew_start.elapsed());
        }
    }

    let avg_renewal = renewal_times.iter().sum::<Duration>() / renewal_times.len() as u32;
    println!("   ✅ Renewed 50 leases");
    println!(
        "   📊 Average renewal time: {:.1}μs",
        avg_renewal.as_micros()
    );

    // Demo 5: RocksDB snapshots
    println!("\n📸 Demo 5: RocksDB Snapshot");
    let (snapshot_id, snapshot_metrics) = persistence.create_snapshot().await?;
    println!(
        "   📸 Snapshot created in {:.1}μs",
        snapshot_metrics.latency.as_micros()
    );
    println!("   🆔 Snapshot ID: {}", snapshot_id);
    println!("   ℹ️  RocksDB snapshots are lightweight (copy-on-write)");

    // Demo 6: Performance and storage stats
    println!("\n📊 Demo 6: RocksDB Storage Statistics");
    let stats = persistence.get_stats().await?;
    println!("   📝 Total keys: {}", stats.total_keys);
    println!(
        "   💾 Total size: {:.2} MB",
        stats.total_size_bytes as f64 / 1024.0 / 1024.0
    );
    println!(
        "   💽 Disk usage: {:.2} MB",
        stats.disk_usage_bytes as f64 / 1024.0 / 1024.0
    );
    println!("   📏 Avg key size: {} bytes", stats.average_key_size);
    println!("   📐 Avg value size: {} bytes", stats.average_value_size);

    // Demo 7: Crash recovery simulation
    println!("\n🔧 Demo 7: RocksDB Crash Recovery");
    let recovery_metrics = persistence.simulate_crash_recovery().await?;
    println!(
        "   🔧 Recovery completed in {:.2}ms",
        recovery_metrics.latency.as_millis()
    );
    println!(
        "   💾 WAL replay data: {} bytes",
        recovery_metrics.disk_bytes_read
    );
    println!("   ℹ️  RocksDB automatically replays WAL on startup");

    // Demo 8: Read performance verification
    println!("\n🔍 Demo 8: Read Performance Verification");
    let verification_start = std::time::Instant::now();
    let mut read_times = Vec::new();

    for key in &actor_keys[..100] {
        // Check first 100
        let read_start = std::time::Instant::now();
        let (lease_opt, _) = persistence.get_lease(key).await?;
        read_times.push(read_start.elapsed());

        if lease_opt.is_none() {
            println!("   ⚠️  Missing lease for key: {:?}", key);
        }
    }

    let verification_time = verification_start.elapsed();
    let avg_read = read_times.iter().sum::<Duration>() / read_times.len() as u32;
    println!(
        "   ✅ Verified 100 actors in {:.2}ms",
        verification_time.as_millis()
    );
    println!("   📊 Average read time: {:.1}μs", avg_read.as_micros());

    println!("\n🎉 RocksDB Demo Complete!");
    println!("   📈 Key advantages:");
    println!("     - Battle-tested LSM-tree implementation");
    println!("     - Excellent write throughput via memtables");
    println!("     - Automatic background compaction");
    println!("     - Bloom filters for fast negative lookups");
    println!("     - Column families for data organization");
    println!("     - Built-in compression and caching");

    Ok(())
}
