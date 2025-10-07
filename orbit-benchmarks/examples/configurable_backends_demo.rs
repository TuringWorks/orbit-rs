use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

use orbit_benchmarks::persistence::{
    config::{PersistenceBackend, PersistenceConfig},
    persistence_factory::{PersistenceConfigBuilder, PersistenceFactory},
    ActorLease, PersistenceError,
};

#[tokio::main]
async fn main() -> Result<(), PersistenceError> {
    let base_dir = Path::new("./configurable_demo_data");

    // Clean up any existing data
    if base_dir.exists() {
        std::fs::remove_dir_all(base_dir).unwrap();
    }

    println!("=== Configurable Persistence Backends Demo ===\n");

    // Test data
    let test_leases = create_test_leases();

    // Demo 1: Environment Variable Configuration
    println!("=== Demo 1: Environment Variable Configuration ===");
    demo_environment_config(&test_leases).await?;

    // Demo 2: File-based Configuration
    println!("\n=== Demo 2: File-based Configuration ===");
    demo_file_config(base_dir, &test_leases).await?;

    // Demo 3: Programmatic Configuration with Builder
    println!("\n=== Demo 3: Programmatic Configuration ===");
    demo_builder_config(base_dir, &test_leases).await?;

    // Demo 4: Performance Comparison
    println!("\n=== Demo 4: Performance Comparison ===");
    demo_performance_comparison(base_dir, &test_leases).await?;

    // Demo 5: Backend Characteristics
    println!("\n=== Demo 5: Backend Characteristics ===");
    demo_backend_characteristics();

    // Clean up
    if base_dir.exists() {
        std::fs::remove_dir_all(base_dir).unwrap();
        println!("\n✓ Cleaned up demo data");
    }

    Ok(())
}

fn create_test_leases() -> Vec<ActorLease> {
    vec![
        ActorLease::new(
            Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap(),
            "web_server".to_string(),
            "node1".to_string(),
            Duration::from_secs(300),
        ),
        ActorLease::new(
            Uuid::parse_str("123e4567-e89b-12d3-a456-426614174001").unwrap(),
            "database".to_string(),
            "node2".to_string(),
            Duration::from_secs(600),
        ),
        ActorLease::new(
            Uuid::parse_str("123e4567-e89b-12d3-a456-426614174002").unwrap(),
            "cache_manager".to_string(),
            "node1".to_string(),
            Duration::from_secs(120),
        ),
    ]
}

async fn demo_environment_config(test_leases: &[ActorLease]) -> Result<(), PersistenceError> {
    println!("Testing environment variable configuration...");

    // Set environment variables
    std::env::set_var("ORBIT_PERSISTENCE_BACKEND", "cow");
    std::env::set_var("ORBIT_DATA_DIR", "./env_config_data");
    std::env::set_var("ORBIT_COW_MAX_KEYS", "32");

    // Load config from environment
    let config = PersistenceConfig::from_env().unwrap();
    println!("✓ Loaded config from environment: {:?}", config.backend);

    // Create provider
    let provider = PersistenceFactory::create_provider(&config).await?;

    // Test basic operations
    let lease = &test_leases[0];
    let write_metrics = provider.store_lease(lease).await?;
    let (retrieved, read_metrics) = provider.get_lease(&lease.key).await?;

    println!("  Write latency: {:?}", write_metrics.latency);
    println!("  Read latency: {:?}", read_metrics.latency);
    println!("  ✓ Data integrity verified: {}", retrieved.is_some());

    // Clean up environment
    std::env::remove_var("ORBIT_PERSISTENCE_BACKEND");
    std::env::remove_var("ORBIT_DATA_DIR");
    std::env::remove_var("ORBIT_COW_MAX_KEYS");

    Ok(())
}

async fn demo_file_config(
    base_dir: &Path,
    test_leases: &[ActorLease],
) -> Result<(), PersistenceError> {
    println!("Testing file-based configuration...");

    // Create configurations for different backends
    let configs = vec![
        (
            "cow_config.json",
            PersistenceConfigBuilder::new()
                .backend(PersistenceBackend::CowBTree)
                .data_dir(base_dir.join("cow_from_file"))
                .cow_max_keys(64)
                .cow_wal_buffer_size(2 * 1024 * 1024) // 2MB
                .build(),
        ),
        (
            "lsm_config.json",
            PersistenceConfigBuilder::new()
                .backend(PersistenceBackend::LsmTree)
                .data_dir(base_dir.join("lsm_from_file"))
                .lsm_memtable_size(32) // 32MB
                .lsm_max_levels(5)
                .lsm_enable_bloom_filters(true)
                .lsm_compaction_threshold(8)
                .build(),
        ),
    ];

    for (filename, config) in configs {
        // Save config to file
        let config_path = base_dir.join(filename);
        tokio::fs::create_dir_all(base_dir).await.unwrap();
        config.to_file(&config_path).unwrap();

        // Load config from file
        let loaded_config = PersistenceConfig::from_file(&config_path).unwrap();
        println!(
            "✓ Loaded {} - Backend: {:?}",
            filename, loaded_config.backend
        );

        // Test the provider
        let provider = PersistenceFactory::create_provider(&loaded_config).await?;
        let lease = &test_leases[1];
        let metrics = provider.store_lease(lease).await?;
        println!("  Write latency: {:?}", metrics.latency);

        let (retrieved, _) = provider.get_lease(&lease.key).await?;
        println!(
            "  ✓ Data verified for {}: {}",
            filename,
            retrieved.is_some()
        );
    }

    Ok(())
}

async fn demo_builder_config(
    base_dir: &Path,
    test_leases: &[ActorLease],
) -> Result<(), PersistenceError> {
    println!("Testing programmatic configuration with builder pattern...");

    // COW B+ Tree with custom configuration
    println!("\n  🌳 COW B+ Tree Configuration:");
    let cow_provider = PersistenceConfigBuilder::new()
        .backend(PersistenceBackend::CowBTree)
        .data_dir(base_dir.join("cow_builder"))
        .cow_max_keys(128)
        .cow_wal_buffer_size(512 * 1024) // 512KB
        .create_provider()
        .await?;

    let lease = &test_leases[0];
    let cow_write = cow_provider.store_lease(lease).await?;
    let (cow_read_result, cow_read) = cow_provider.get_lease(&lease.key).await?;

    println!(
        "    Write: {:?} | Read: {:?} | Success: {}",
        cow_write.latency,
        cow_read.latency,
        cow_read_result.is_some()
    );

    // LSM-Tree with custom configuration
    println!("\n  📚 LSM-Tree Configuration:");
    let lsm_provider = PersistenceConfigBuilder::new()
        .backend(PersistenceBackend::LsmTree)
        .data_dir(base_dir.join("lsm_builder"))
        .lsm_memtable_size(16) // 16MB
        .lsm_max_levels(6)
        .lsm_enable_bloom_filters(true)
        .lsm_compaction_threshold(4)
        .create_provider()
        .await?;

    let lsm_write = lsm_provider.store_lease(lease).await?;
    let (lsm_read_result, lsm_read) = lsm_provider.get_lease(&lease.key).await?;

    println!(
        "    Write: {:?} | Read: {:?} | Success: {}",
        lsm_write.latency,
        lsm_read.latency,
        lsm_read_result.is_some()
    );

    // RocksDB with custom configuration
    println!("\n  🗻 RocksDB Configuration:");
    let rocksdb_provider = PersistenceConfigBuilder::new()
        .backend(PersistenceBackend::RocksDb)
        .data_dir(base_dir.join("rocksdb_builder"))
        .rocksdb_compression(true)
        .rocksdb_block_cache_mb(128)
        .create_provider()
        .await?;

    let rocks_write = rocksdb_provider.store_lease(lease).await?;
    let (rocks_read_result, rocks_read) = rocksdb_provider.get_lease(&lease.key).await?;

    println!(
        "    Write: {:?} | Read: {:?} | Success: {}",
        rocks_write.latency,
        rocks_read.latency,
        rocks_read_result.is_some()
    );

    Ok(())
}

async fn demo_performance_comparison(
    base_dir: &Path,
    test_leases: &[ActorLease],
) -> Result<(), PersistenceError> {
    println!("Comparing performance across all backends...");

    let providers = PersistenceFactory::create_all_providers(base_dir).await?;

    println!(
        "\n{:<12} {:<12} {:<12} {:<12} {:<12}",
        "Backend", "Writes", "Write Avg", "Reads", "Read Avg"
    );
    println!("{}", "=".repeat(60));

    for (name, provider) in providers {
        let mut write_times = Vec::new();
        let mut read_times = Vec::new();

        // Perform multiple operations for better averages
        for lease in test_leases {
            let write_metrics = provider.store_lease(lease).await?;
            write_times.push(write_metrics.latency.as_micros() as f64);

            let (_, read_metrics) = provider.get_lease(&lease.key).await?;
            read_times.push(read_metrics.latency.as_micros() as f64);
        }

        let write_avg = write_times.iter().sum::<f64>() / write_times.len() as f64;
        let read_avg = read_times.iter().sum::<f64>() / read_times.len() as f64;

        println!(
            "{:<12} {:<12} {:<10.1}μs {:<12} {:<10.1}μs",
            name,
            write_times.len(),
            write_avg,
            read_times.len(),
            read_avg
        );
    }

    Ok(())
}

fn demo_backend_characteristics() {
    println!("Backend characteristics and expected performance:\n");

    let backends = vec![
        PersistenceBackend::CowBTree,
        PersistenceBackend::LsmTree,
        PersistenceBackend::RocksDb,
    ];

    println!(
        "{:<12} {:<10} {:<10} {:<8} {:<8} {:<12}",
        "Backend", "Write μs", "Read μs", "Mem Opt", "Zero Copy", "Write Amp"
    );
    println!("{}", "=".repeat(66));

    for backend in backends {
        let name = PersistenceFactory::backend_name(&backend);
        let (write_latency, read_latency) = PersistenceFactory::expected_performance(&backend);
        let (mem_opt, zero_copy, write_amp) = PersistenceFactory::memory_characteristics(&backend);

        println!(
            "{:<12} {:<10} {:<10} {:<8} {:<8} {:<12}",
            name,
            write_latency,
            read_latency,
            if mem_opt { "Yes" } else { "No" },
            if zero_copy { "Yes" } else { "No" },
            if write_amp { "Yes" } else { "No" }
        );
    }

    println!("\n📊 Performance Summary:");
    println!("• COW B+ Tree: Fastest reads (1μs), low write latency (51μs), memory optimized");
    println!("• LSM-Tree: Moderate performance, good for write-heavy workloads");
    println!("• RocksDB: Highest latency but proven stability and features");

    println!("\n🎯 Use Case Recommendations:");
    println!("• COW B+ Tree: Low-latency actor systems, memory-constrained environments");
    println!("• LSM-Tree: Write-heavy workloads, time-series data, log aggregation");
    println!("• RocksDB: Production systems requiring stability and rich features");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_configurable_backends_basic() {
        let temp_dir = tempdir().unwrap();
        let test_leases = create_test_leases();

        // Test that we can create all backends
        let providers = PersistenceFactory::create_all_providers(temp_dir.path())
            .await
            .unwrap();
        assert_eq!(providers.len(), 3);

        // Test that each backend can store and retrieve data
        for (name, provider) in providers {
            let lease = &test_leases[0];
            let write_metrics = provider.store_lease(lease).await.unwrap();
            assert!(write_metrics.success);

            let (retrieved, read_metrics) = provider.get_lease(&lease.key).await.unwrap();
            assert!(read_metrics.success);
            assert_eq!(retrieved.unwrap(), *lease);

            println!("✓ {} backend test passed", name);
        }
    }
}
