use orbit_benchmarks::persistence::cow_btree::CowBTreePersistence;
use orbit_benchmarks::persistence::*;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌳 COW B+ Tree Persistence & Recovery Demo");
    println!("==========================================");

    // Use a persistent directory in the current folder
    let data_dir = Path::new("./cow_btree_test_data");

    // Clean up any existing data first
    if data_dir.exists() {
        std::fs::remove_dir_all(data_dir)?;
        println!("🧹 Cleaned up existing test data");
    }

    println!("📁 Data directory: {}", data_dir.display());

    // === PHASE 1: Initial Data Creation ===
    println!("\n🏗️  PHASE 1: Creating Initial Data");
    let mut initial_leases = Vec::new();

    {
        let persistence = CowBTreePersistence::new(data_dir).await?;
        println!("✅ COW B+ Tree initialized");

        // Create some test data
        for i in 0..10 {
            let lease = ActorLease::new(
                Uuid::new_v4(),
                format!("test_actor_{}", i),
                format!("node_{}", i % 3),
                Duration::from_secs(300),
            );

            let metrics = persistence.store_lease(&lease).await?;
            println!(
                "   📝 Stored lease {} in {:.1}μs",
                i,
                metrics.latency.as_micros()
            );
            initial_leases.push(lease);
        }

        // Force WAL flush
        let (snapshot_id, _) = persistence.create_snapshot().await?;
        println!("   📸 Created snapshot: {}", snapshot_id);

        // Show what's in memory
        let stats = persistence.get_stats().await?;
        println!(
            "   📊 Stats: {} keys, {} bytes in memory",
            stats.total_keys, stats.memory_usage_bytes
        );
    } // persistence goes out of scope here, simulating a "crash"

    println!("   💥 Simulated crash (persistence dropped)");

    // === PHASE 2: Check WAL File ===
    println!("\n🔍 PHASE 2: Examining Persistence Files");

    let wal_path = data_dir.join("orbit.wal");
    if wal_path.exists() {
        let wal_size = std::fs::metadata(&wal_path)?.len();
        println!("   ✅ WAL file exists: {} bytes", wal_size);

        // Show first few bytes of WAL file
        let wal_content = std::fs::read(&wal_path)?;
        if !wal_content.is_empty() {
            println!(
                "   📄 WAL file contains {} bytes of data",
                wal_content.len()
            );
            if wal_content.len() >= 100 {
                println!("   📄 First 100 bytes: {:02x?}...", &wal_content[0..100]);
            }
        }
    } else {
        println!("   ❌ No WAL file found - data not persisted!");
        return Ok(());
    }

    // List all files in data directory
    println!("   📂 Files in data directory:");
    for entry in std::fs::read_dir(data_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        println!(
            "      - {}: {} bytes",
            entry.file_name().to_string_lossy(),
            metadata.len()
        );
    }

    // === PHASE 3: Recovery Attempt ===
    println!("\n🔄 PHASE 3: Recovery Attempt");

    {
        let persistence = CowBTreePersistence::new(data_dir).await?;
        println!("   ✅ COW B+ Tree re-initialized");

        // Check if data is recovered
        let stats = persistence.get_stats().await?;
        println!(
            "   📊 After recovery: {} keys, {} bytes in memory",
            stats.total_keys, stats.memory_usage_bytes
        );

        let mut recovered_count = 0;
        for (i, original_lease) in initial_leases.iter().enumerate() {
            match persistence.get_lease(&original_lease.key).await? {
                (Some(recovered_lease), _) => {
                    if recovered_lease == *original_lease {
                        recovered_count += 1;
                        println!("   ✅ Lease {} recovered correctly", i);
                    } else {
                        println!("   ⚠️  Lease {} recovered but data differs", i);
                    }
                }
                (None, _) => {
                    println!("   ❌ Lease {} not found after recovery", i);
                }
            }
        }

        println!(
            "   📈 Recovery Summary: {}/{} leases recovered",
            recovered_count,
            initial_leases.len()
        );

        if recovered_count == 0 {
            println!("   🚨 NO DATA RECOVERED - WAL replay not implemented!");
            println!("   💡 The COW B+ Tree writes to WAL but doesn't replay on startup");
            println!(
                "   💡 This is expected for a prototype - full recovery needs to be implemented"
            );
        } else if recovered_count == initial_leases.len() {
            println!("   🎉 PERFECT RECOVERY - All data restored!");
        } else {
            println!("   ⚠️  PARTIAL RECOVERY - Some data lost");
        }
    }

    // === PHASE 4: WAL Analysis ===
    println!("\n🔬 PHASE 4: WAL Content Analysis");

    if wal_path.exists() {
        let wal_content = std::fs::read(&wal_path)?;
        println!("   📄 WAL file size: {} bytes", wal_content.len());

        // Try to parse the WAL entries
        let mut offset = 0;
        let mut entry_count = 0;

        while offset + 4 <= wal_content.len() {
            // Read entry length
            let len_bytes = &wal_content[offset..offset + 4];
            let entry_len =
                u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]])
                    as usize;
            offset += 4;

            if offset + entry_len <= wal_content.len() {
                let entry_data = &wal_content[offset..offset + entry_len];

                // Try to parse as JSON
                match serde_json::from_slice::<serde_json::Value>(entry_data) {
                    Ok(entry_json) => {
                        println!("   📝 WAL Entry {}: {} bytes", entry_count, entry_len);
                        if let Some(op_type) =
                            entry_json.get("operation").and_then(|op| op.get("type"))
                        {
                            println!("      Operation: {}", op_type);
                        }
                        entry_count += 1;
                    }
                    Err(e) => {
                        println!("   ❌ Failed to parse WAL entry {}: {}", entry_count, e);
                        break;
                    }
                }

                offset += entry_len;
            } else {
                println!("   ⚠️  Incomplete entry at offset {}", offset);
                break;
            }
        }

        println!("   📊 Found {} WAL entries", entry_count);
    }

    // === PHASE 5: Current Implementation Status ===
    println!("\n📋 PHASE 5: Implementation Status");
    println!("   ✅ WAL writing: IMPLEMENTED");
    println!("   ✅ Data serialization: IMPLEMENTED");
    println!("   ✅ In-memory operations: IMPLEMENTED");
    println!("   ❌ WAL replay on startup: NOT IMPLEMENTED");
    println!("   ❌ Tree persistence to disk: NOT IMPLEMENTED");
    println!("   ❌ Crash recovery: NOT IMPLEMENTED");

    println!("\n💡 Next Steps for Full Persistence:");
    println!("   1. Implement WAL replay during CowBTreePersistence::new()");
    println!("   2. Add periodic tree serialization to disk");
    println!("   3. Implement proper crash recovery logic");
    println!("   4. Add checkpointing to reduce WAL replay time");

    println!("\n🎯 Current State: PROTOTYPE with WAL logging");
    println!("   The COW B+ Tree correctly logs all operations to disk");
    println!("   but needs recovery implementation for full persistence.");

    // Clean up
    std::fs::remove_dir_all(data_dir)?;
    println!("\n🧹 Test data cleaned up");

    Ok(())
}
