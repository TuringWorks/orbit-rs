//! Test TiKV-based Redis data persistence provider
//!
//! This example demonstrates the TiKV-based Redis data provider working
//! with basic Redis string operations and TTL support.
//!
//! Prerequisites:
//! 1. TiKV cluster running (can use TiUP for local development):
//!    tiup playground --db 0 --pd 1 --kv 3
//!
//! Usage:
//!   cargo run --package orbit-server --example tikv-redis-test

use orbit_protocols::persistence::redis_data::{RedisDataConfig, RedisDataProvider, RedisValue};
use orbit_protocols::persistence::tikv_redis_provider::TiKVRedisDataProvider;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,orbit_protocols=debug,tikv_client=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Testing TiKV Redis Data Provider...");

    // Configure TiKV connection
    let pd_endpoints = vec!["127.0.0.1:2379".to_string()]; // Default PD endpoint
    let redis_config = RedisDataConfig {
        enable_expiry_cleanup: true,
        cleanup_interval_seconds: 30,
        cleanup_batch_size: 100,
        key_prefix: "orbit_test:".to_string(),
    };

    info!("📡 Connecting to TiKV cluster at: {:?}", pd_endpoints);

    // Create TiKV provider
    let provider = match TiKVRedisDataProvider::new(pd_endpoints, redis_config).await {
        Ok(p) => {
            info!("✅ TiKV Redis data provider created successfully");
            Arc::new(p)
        }
        Err(e) => {
            error!("❌ Failed to connect to TiKV cluster: {}", e);
            warn!("💡 Make sure TiKV is running. You can start a local cluster with:");
            warn!("   tiup playground --db 0 --pd 1 --kv 3");
            warn!("   or use Docker: docker run -p 2379:2379 pingcap/pd:latest");
            return Err(e.into());
        }
    };

    // Initialize the provider
    provider.initialize().await?;
    info!("✅ TiKV Redis data provider initialized");

    // Test basic Redis operations
    info!("🧪 Testing basic Redis operations...");

    // Test SET and GET
    provider
        .set("test_key", RedisValue::new("Hello TiKV!".to_string()))
        .await?;
    info!("✅ SET test_key 'Hello TiKV!'");

    match provider.get("test_key").await? {
        Some(value) => {
            info!("✅ GET test_key -> '{}'", value.data);
            assert_eq!(value.data, "Hello TiKV!");
        }
        None => {
            error!("❌ Failed to retrieve test_key");
            return Err("GET failed".into());
        }
    }

    // Test TTL operations
    provider
        .set(
            "ttl_key",
            RedisValue::with_ttl("This expires in 10 seconds".to_string(), 10),
        )
        .await?;
    info!("✅ SET ttl_key with 10s TTL");

    if let Some(value) = provider.get("ttl_key").await? {
        info!(
            "✅ GET ttl_key -> '{}' (TTL: {} seconds)",
            value.data,
            value.ttl()
        );
    }

    // Test INCREMENT operations
    let new_value = provider.incr("counter", 5).await?;
    info!("✅ INCR counter 5 -> {}", new_value);
    assert_eq!(new_value, 5);

    let new_value = provider.incr("counter", 3).await?;
    info!("✅ INCR counter 3 -> {}", new_value);
    assert_eq!(new_value, 8);

    // Test APPEND operations
    let length = provider.append("test_key", " - Appended!").await?;
    info!("✅ APPEND test_key ' - Appended!' -> length {}", length);

    if let Some(value) = provider.get("test_key").await? {
        info!("✅ GET test_key after append -> '{}'", value.data);
        assert_eq!(value.data, "Hello TiKV! - Appended!");
    }

    // Test EXISTS
    let exists = provider.exists("test_key").await?;
    info!("✅ EXISTS test_key -> {}", exists);
    assert!(exists);

    let exists = provider.exists("nonexistent_key").await?;
    info!("✅ EXISTS nonexistent_key -> {}", exists);
    assert!(!exists);

    // Test SETNX
    let set = provider
        .setnx("new_key", RedisValue::new("I'm new!".to_string()))
        .await?;
    info!("✅ SETNX new_key 'I'm new!' -> {}", set);
    assert!(set);

    let set = provider
        .setnx("new_key", RedisValue::new("Won't work".to_string()))
        .await?;
    info!("✅ SETNX new_key 'Won't work' -> {}", set);
    assert!(!set);

    // Test GETSET
    match provider
        .getset("test_key", RedisValue::new("Replaced value".to_string()))
        .await?
    {
        Some(old_value) => {
            info!(
                "✅ GETSET test_key 'Replaced value' -> old: '{}'",
                old_value
            );
        }
        None => {
            warn!("⚠️ GETSET returned no old value");
        }
    }

    // Test DELETE
    let deleted = provider.delete("test_key").await?;
    info!("✅ DEL test_key -> {}", deleted);
    assert!(deleted);

    let deleted = provider.delete("nonexistent_key").await?;
    info!("✅ DEL nonexistent_key -> {}", deleted);
    assert!(!deleted);

    // Get metrics
    let metrics = provider.metrics().await?;
    info!("📊 Final metrics:");
    info!("   GET operations: {}", metrics.get_operations);
    info!("   SET operations: {}", metrics.set_operations);
    info!("   DELETE operations: {}", metrics.delete_operations);

    // Cleanup
    provider.shutdown().await?;
    info!("✅ TiKV Redis data provider shut down");

    info!("");
    info!("🎉 TiKV Redis provider test completed successfully!");
    info!("✨ All Redis string operations work correctly with TiKV backend");
    info!("🚀 Ready for integration into Orbit server!");

    Ok(())
}
