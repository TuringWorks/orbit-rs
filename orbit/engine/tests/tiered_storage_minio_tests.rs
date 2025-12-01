//! Tiered Storage Integration Tests with MinIO
//!
//! These tests verify the complete tiered storage workflow including:
//! - Hot → Warm → Cold tier propagation
//! - Cold tier recall (reading data from cold storage)
//! - Tier migration based on access patterns
//! - Multi-tier consistency
//!
//! ## Prerequisites
//!
//! Run MinIO locally:
//! ```bash
//! docker run -d --name minio \
//!   -p 9000:9000 -p 9001:9001 \
//!   -e "MINIO_ROOT_USER=minioadmin" \
//!   -e "MINIO_ROOT_PASSWORD=minioadmin" \
//!   minio/minio server /data --console-address ":9001"
//!
//! # Create the test bucket
//! docker exec minio mc alias set local http://localhost:9000 minioadmin minioadmin
//! docker exec minio mc mb local/orbit-cold-storage
//! ```
//!
//! Or use docker-compose with the provided configuration.

use orbit_engine::unified::{
    ColdBackendType, ColdTierConfig, EvictionPolicy, HotTierConfig, S3Backend,
    S3BackendConfig, StorageTier, TierMigrationConfig, TieredStorageBackend, TieredStorageConfig,
    UnifiedStorageBackend, WarmTierConfig, WritePolicy,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Helper to create a MinIO-enabled tiered storage config
fn create_minio_tiered_config(bucket: &str) -> TieredStorageConfig {
    TieredStorageConfig {
        hot_tier: HotTierConfig {
            enabled: true,
            max_memory_bytes: 1024 * 1024, // 1MB - small to force eviction
            eviction_policy: EvictionPolicy::Lru,
            eviction_high_watermark: 0.8,
            eviction_low_watermark: 0.5,
            default_ttl_secs: None,
            write_through: true,
            read_through: true,
        },
        warm_tier: WarmTierConfig {
            enabled: true,
            data_dir: format!("./test_data/tiered_minio/{}", uuid::Uuid::new_v4()),
            enable_compression: true,
            compression_algorithm: "lz4".to_string(),
            enable_bloom_filter: true,
            bloom_filter_bits: 10,
            enable_wal: false,
            max_background_jobs: 2,
            write_buffer_size: 64 * 1024,
        },
        cold_tier: ColdTierConfig {
            enabled: true,
            backend: ColdBackendType::MinIO,
            bucket: bucket.to_string(),
            prefix: format!("test/{}/", uuid::Uuid::new_v4()),
            data_format: orbit_engine::unified::ColdDataFormat::JsonLines,
            region: Some("us-east-1".to_string()),
            endpoint: Some("http://localhost:9000".to_string()),
            access_key: Some("minioadmin".to_string()),
            secret_key: Some("minioadmin".to_string()),
        },
        migration: TierMigrationConfig {
            enabled: true,
            check_interval_secs: 1, // Fast for testing
            hot_to_warm_idle_secs: 5,
            warm_to_cold_idle_secs: 10,
            promotion_access_count: 3,
            max_concurrent_migrations: 2,
        },
        write_policy: WritePolicy::WriteThrough,
    }
}

/// Setup test: Create the MinIO bucket if it doesn't exist
/// Run this first with: cargo test --test tiered_storage_minio_tests setup_minio_bucket -- --exact --nocapture --ignored
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn setup_minio_bucket() {
    let config = S3BackendConfig::minio(
        "http://localhost:9000",
        "minioadmin",
        "minioadmin",
        "orbit-cold-storage",
    );
    let backend = S3Backend::new(config);
    
    // Try to initialize - this will create the bucket if it doesn't exist
    match backend.initialize().await {
        Ok(_) => println!("✓ Bucket 'orbit-cold-storage' is ready"),
        Err(e) => {
            // Try to write a test file to trigger bucket creation
            println!("Attempting to create bucket via write operation...");
            if let Err(write_err) = backend.put("_setup_test", b"test").await {
                eprintln!("Failed to create bucket: {}", write_err);
                eprintln!("Original error: {}", e);
                panic!("Could not create or access bucket");
            }
            // Clean up test file
            let _ = backend.delete("_setup_test").await;
            println!("✓ Bucket 'orbit-cold-storage' created successfully");
        }
    }
    
    backend.shutdown().await.ok();
}

/// Test basic S3 backend operations with MinIO
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_s3_backend_basic_operations() {
    let config = S3BackendConfig::minio(
        "http://localhost:9000",
        "minioadmin",
        "minioadmin",
        "orbit-cold-storage",
    );
    let backend = S3Backend::new(config);
    backend.initialize().await.expect("Failed to initialize S3 backend");

    let test_key = format!("test-basic-{}", uuid::Uuid::new_v4());
    let test_value = b"Hello, MinIO!";

    // PUT
    backend
        .put(&test_key, test_value)
        .await
        .expect("Failed to put value");

    // GET
    let retrieved = backend.get(&test_key).await.expect("Failed to get value");
    assert_eq!(retrieved, Some(test_value.to_vec()));

    // EXISTS
    assert!(backend.exists(&test_key).await.expect("Failed to check exists"));

    // DELETE
    assert!(backend.delete(&test_key).await.expect("Failed to delete"));
    assert!(!backend.exists(&test_key).await.expect("Failed to check exists after delete"));

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test S3 backend scan prefix
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_s3_backend_scan_prefix() {
    let config = S3BackendConfig::minio(
        "http://localhost:9000",
        "minioadmin",
        "minioadmin",
        "orbit-cold-storage",
    );
    let backend = S3Backend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    let prefix = format!("test-scan-{}/", uuid::Uuid::new_v4());

    // Put multiple values with prefix
    for i in 0..5 {
        backend
            .put(&format!("{}key{}", prefix, i), format!("value{}", i).as_bytes())
            .await
            .expect("Failed to put");
    }

    // Put one value without prefix
    let other_key = format!("other-{}", uuid::Uuid::new_v4());
    backend
        .put(&other_key, b"other value")
        .await
        .expect("Failed to put other");

    // Scan with prefix
    let results = backend
        .scan_prefix(&prefix, None)
        .await
        .expect("Failed to scan");

    assert_eq!(results.len(), 5, "Expected 5 results from scan");

    // Verify all values are correct
    for (key, value) in &results {
        assert!(key.starts_with(&prefix), "Key should start with prefix");
        let expected_suffix = key.strip_prefix(&prefix).unwrap();
        let expected_value = format!("value{}", expected_suffix.strip_prefix("key").unwrap());
        assert_eq!(value, expected_value.as_bytes());
    }

    // Cleanup
    for (key, _) in results {
        backend.delete(&key).await.expect("Failed to cleanup");
    }
    backend.delete(&other_key).await.expect("Failed to cleanup other");

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test Hot → Warm tier propagation via eviction
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_hot_to_warm_propagation() {
    let config = create_minio_tiered_config("orbit-cold-storage");
    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize tiered backend");

    // Write data that will fit in hot tier
    for i in 0..5 {
        let key = format!("hot-test:{}", i);
        let value = format!("value-{}", i);
        backend.put(&key, value.as_bytes()).await.expect("Failed to put");
    }

    // All should be in hot tier initially
    for i in 0..5 {
        let key = format!("hot-test:{}", i);
        assert_eq!(
            backend.get_tier(&key).await,
            Some(StorageTier::Hot),
            "Key {} should be in Hot tier",
            key
        );
    }

    // Now write more data to trigger eviction
    let large_value = vec![0u8; 100 * 1024]; // 100KB per entry
    for i in 0..20 {
        let key = format!("eviction-test:{}", i);
        backend.put(&key, &large_value).await.expect("Failed to put large value");
    }

    // Check metrics for evictions
    let metrics = backend.tiered_metrics().await;
    println!("Evictions: {}, Demotions: {}", metrics.evictions, metrics.demotions);

    // Some entries should have been evicted to warm tier
    assert!(
        metrics.evictions > 0 || metrics.demotions > 0,
        "Expected some evictions to occur"
    );

    // Verify evicted data is still readable (via warm tier)
    for i in 0..5 {
        let key = format!("hot-test:{}", i);
        let value = backend.get(&key).await.expect("Failed to get");
        assert!(value.is_some(), "Data should still be accessible");
    }

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test Warm → Cold archival
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_warm_to_cold_archival() {
    let mut config = create_minio_tiered_config("orbit-cold-storage");
    // Disable hot tier to write directly to warm
    config.hot_tier.enabled = false;
    config.write_policy = WritePolicy::WriteAround;

    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    // Write data directly to warm tier
    for i in 0..5 {
        let key = format!("warm-test:{}", i);
        let value = format!("warm-value-{}", i);
        backend.put(&key, value.as_bytes()).await.expect("Failed to put");

        // Verify in warm tier
        assert_eq!(
            backend.get_tier(&key).await,
            Some(StorageTier::Warm),
            "Key should be in Warm tier"
        );
    }

    // Manually trigger archival to cold tier (in real scenarios, this happens automatically)
    // For testing, we directly archive
    for i in 0..5 {
        let key = format!("warm-test:{}", i);
        backend
            .archive_to_cold(&key)
            .await
            .expect("Failed to archive to cold");
    }

    // Verify data is now in cold tier
    for i in 0..5 {
        let key = format!("warm-test:{}", i);
        assert_eq!(
            backend.get_tier(&key).await,
            Some(StorageTier::Cold),
            "Key should be in Cold tier"
        );

        // Data should still be readable
        let value = backend.get(&key).await.expect("Failed to get from cold");
        assert_eq!(value, Some(format!("warm-value-{}", i).into_bytes()));
    }

    // Check archival metrics
    let metrics = backend.tiered_metrics().await;
    assert!(metrics.archivals >= 5, "Expected at least 5 archivals");

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test cold tier recall (reading data from cold storage)
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_cold_tier_recall() {
    // First, set up data in cold tier directly using S3 backend
    let s3_prefix = format!("test/{}/", uuid::Uuid::new_v4());
    let s3_config = S3BackendConfig {
        endpoint: "http://localhost:9000".to_string(),
        access_key_id: "minioadmin".to_string(),
        secret_access_key: "minioadmin".to_string(),
        region: "us-east-1".to_string(),
        bucket: "orbit-cold-storage".to_string(),
        prefix: s3_prefix.clone(),
        path_style_access: true,
    };

    let s3_backend = S3Backend::new(s3_config.clone());
    s3_backend.initialize().await.expect("Failed to initialize S3");

    // Pre-populate cold storage
    for i in 0..5 {
        let key = format!("cold-data:{}", i);
        let value = format!("{{\"id\": {}, \"data\": \"cold-value-{}\"}}", i, i);
        s3_backend.put(&key, value.as_bytes()).await.expect("Failed to put to S3");
    }

    s3_backend.shutdown().await.ok();

    // Now create tiered storage that uses this cold storage
    let mut config = create_minio_tiered_config("orbit-cold-storage");
    config.cold_tier.prefix = s3_prefix;

    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize tiered");

    // Read data from cold tier (should trigger read-through)
    for i in 0..5 {
        let key = format!("cold-data:{}", i);
        let value = backend.get(&key).await.expect("Failed to get from cold");
        assert!(value.is_some(), "Should be able to read from cold tier");

        let value_str = String::from_utf8(value.unwrap()).expect("Invalid UTF-8");
        assert!(
            value_str.contains(&format!("cold-value-{}", i)),
            "Value should contain expected data"
        );
    }

    // Check cold tier hit metrics
    let metrics = backend.tiered_metrics().await;
    assert!(metrics.cold_tier_hits >= 5, "Expected at least 5 cold tier hits");

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test tier promotion based on access patterns
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_tier_promotion_on_access() {
    let mut config = create_minio_tiered_config("orbit-cold-storage");
    config.migration.promotion_access_count = 3; // Promote after 3 accesses
    config.hot_tier.read_through = true;

    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    // Write data with write-around to put in warm tier
    let key = "promotion-test:1";
    backend.put(key, b"test-value").await.expect("Failed to put");

    // Move to warm tier by evicting from hot
    // In real usage, this would happen via time-based demotion
    // For now, we verify the read-through behavior

    // Access the key multiple times
    for _ in 0..5 {
        let _value = backend.get(key).await.expect("Failed to get");
    }

    // After multiple accesses, the key should be promoted to hot tier
    // (if read-through is enabled and promotion logic is triggered)
    let metrics = backend.tiered_metrics().await;
    println!(
        "Promotions: {}, Hot hits: {}, Warm hits: {}",
        metrics.promotions, metrics.hot_tier_hits, metrics.warm_tier_hits
    );

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test multi-tier data consistency
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_multi_tier_consistency() {
    let config = create_minio_tiered_config("orbit-cold-storage");
    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    let key = "consistency-test:1";
    let initial_value = b"initial-value";
    let updated_value = b"updated-value";

    // Write initial value
    backend.put(key, initial_value).await.expect("Failed to put initial");

    // Update the value
    backend.put(key, updated_value).await.expect("Failed to put updated");

    // Verify the updated value is returned regardless of tier
    let value = backend.get(key).await.expect("Failed to get");
    assert_eq!(value, Some(updated_value.to_vec()), "Should get updated value");

    // Force eviction to warm tier
    let large_value = vec![0u8; 500 * 1024];
    for i in 0..10 {
        backend
            .put(&format!("filler:{}", i), &large_value)
            .await
            .expect("Failed to put filler");
    }

    // Value should still be consistent after eviction
    let value_after_eviction = backend.get(key).await.expect("Failed to get after eviction");
    assert_eq!(
        value_after_eviction,
        Some(updated_value.to_vec()),
        "Should get updated value after eviction"
    );

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test batch operations across tiers
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_batch_operations_across_tiers() {
    let config = create_minio_tiered_config("orbit-cold-storage");
    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    // Batch put
    let entries: Vec<_> = (0..10)
        .map(|i| (format!("batch:{}", i), format!("value-{}", i).into_bytes()))
        .collect();

    backend
        .put_batch(entries.clone())
        .await
        .expect("Failed to batch put");

    // Verify all entries exist
    for (key, expected_value) in &entries {
        let value = backend.get(key).await.expect("Failed to get");
        assert_eq!(value.as_ref(), Some(expected_value), "Batch put value mismatch");
    }

    // Batch delete half
    let keys_to_delete: Vec<_> = (0..5).map(|i| format!("batch:{}", i)).collect();
    let deleted = backend
        .delete_batch(keys_to_delete.clone())
        .await
        .expect("Failed to batch delete");
    assert_eq!(deleted, 5, "Should have deleted 5 entries");

    // Verify deletions
    for key in &keys_to_delete {
        assert!(!backend.exists(key).await.expect("Failed to check exists"));
    }

    // Verify remaining entries
    for i in 5..10 {
        let key = format!("batch:{}", i);
        assert!(backend.exists(&key).await.expect("Failed to check exists"));
    }

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test scan across all tiers
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_scan_across_tiers() {
    let config = create_minio_tiered_config("orbit-cold-storage");
    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    let prefix = format!("scan-test-{}/", uuid::Uuid::new_v4());

    // Put entries (will go to hot tier)
    for i in 0..5 {
        backend
            .put(&format!("{}entry:{}", prefix, i), format!("value-{}", i).as_bytes())
            .await
            .expect("Failed to put");
    }

    // Force some to warm tier
    let large_value = vec![0u8; 300 * 1024];
    for i in 0..10 {
        backend
            .put(&format!("filler:{}", i), &large_value)
            .await
            .expect("Failed to put filler");
    }

    // Scan should find all entries regardless of tier
    let results = backend
        .scan_prefix(&prefix, None)
        .await
        .expect("Failed to scan");

    assert_eq!(results.len(), 5, "Should find all 5 entries");

    // Verify correct values
    for (key, value) in results {
        assert!(key.starts_with(&prefix));
        let idx: usize = key
            .split(':')
            .last()
            .unwrap()
            .parse()
            .expect("Invalid index");
        assert_eq!(value, format!("value-{}", idx).into_bytes());
    }

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test metrics accuracy across tier operations
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_tiered_metrics_accuracy() {
    let config = create_minio_tiered_config("orbit-cold-storage");
    let backend = TieredStorageBackend::new(config);
    backend.initialize().await.expect("Failed to initialize");

    let initial_metrics = backend.tiered_metrics().await;

    // Perform known operations
    for i in 0..10 {
        backend
            .put(&format!("metrics-test:{}", i), b"value")
            .await
            .expect("Failed to put");
    }

    for i in 0..10 {
        backend
            .get(&format!("metrics-test:{}", i))
            .await
            .expect("Failed to get");
    }

    for i in 0..5 {
        backend
            .delete(&format!("metrics-test:{}", i))
            .await
            .expect("Failed to delete");
    }

    let final_metrics = backend.tiered_metrics().await;

    // Verify metrics increments
    assert!(
        final_metrics.base.write_operations >= initial_metrics.base.write_operations + 10,
        "Write operations should have increased by at least 10"
    );
    assert!(
        final_metrics.base.read_operations >= initial_metrics.base.read_operations + 10,
        "Read operations should have increased by at least 10"
    );
    assert!(
        final_metrics.base.delete_operations >= initial_metrics.base.delete_operations + 5,
        "Delete operations should have increased by at least 5"
    );

    println!("Final metrics: {:?}", final_metrics);

    backend.shutdown().await.expect("Failed to shutdown");
}

/// Test graceful shutdown with pending operations
#[tokio::test]
#[ignore = "Requires MinIO running at localhost:9000"]
async fn test_graceful_shutdown() {
    let config = create_minio_tiered_config("orbit-cold-storage");
    let backend = Arc::new(TieredStorageBackend::new(config));
    backend.initialize().await.expect("Failed to initialize");

    let backend_clone = backend.clone();

    // Start background writes
    let write_handle = tokio::spawn(async move {
        for i in 0..100 {
            let _ = backend_clone
                .put(&format!("shutdown-test:{}", i), b"value")
                .await;
            sleep(Duration::from_millis(10)).await;
        }
    });

    // Wait a bit then shutdown
    sleep(Duration::from_millis(200)).await;

    // Shutdown should complete gracefully
    backend.shutdown().await.expect("Failed to shutdown gracefully");

    // Cancel the write task
    write_handle.abort();
}
