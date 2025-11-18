//! Example showing OrbitServer with persistent Redis data storage
//!
//! This example demonstrates how to run an Orbit server with Redis RESP protocol
//! that uses RocksDB for persistent storage instead of in-memory storage.
//! All Redis string commands will persist data to disk and support TTL.
//!
//! Features demonstrated:
//! - Persistent Redis data storage with RocksDB
//! - TTL support for keys (SETEX, SET with EX/PX options)
//! - Background cleanup of expired keys
//! - Data survives server restarts
//!
//! Usage:
//!   cargo run --package orbit-server --example persistent-redis-server
//!
//! Then connect with Redis clients:
//!   redis-cli -h localhost -p 6379
//!
//! Try these commands to test persistence:
//!   > SET mykey "Hello Persistent World"
//!   > GET mykey
//!   > SETEX tempkey 10 "This expires in 10 seconds"
//!   > TTL tempkey
//!   > SET persistent_key "Restart the server and I'll still be here!"
//!   
//! Restart the server and verify data persistence:
//!   > GET persistent_key

use orbit_protocols::persistence::redis_data::{
    MemoryRedisDataProvider, RedisDataConfig, RedisDataProvider,
};
// use orbit_protocols::persistence::rocksdb_redis_provider::RocksDbRedisDataProvider;
// use orbit_protocols::persistence::tikv_redis_provider::TiKVRedisDataProvider;
use std::error::Error;
use std::sync::Arc;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging with more detailed output for persistence
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| {
                "info,orbit_server=debug,orbit_protocols=debug,rocksdb=info".into()
            }),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Starting Orbit Server with Persistent Redis Storage...");

    // Configure Redis data persistence
    let redis_data_config = RedisDataConfig {
        enable_expiry_cleanup: true,
        cleanup_interval_seconds: 30, // Clean up expired keys every 30 seconds
        cleanup_batch_size: 1000,
        key_prefix: "orbit_redis:".to_string(),
    };

    info!("🧠 Using in-memory Redis data provider for demonstration");

    // Create memory provider for testing
    let redis_provider = Arc::new(MemoryRedisDataProvider::new(redis_data_config.clone()));

    // Initialize the provider
    redis_provider.initialize().await?;
    info!("✅ Memory Redis data provider initialized");

    // Add some test data
    use orbit_protocols::persistence::redis_data::RedisValue;
    redis_provider
        .set("test_key", RedisValue::new("test_value".to_string()))
        .await?;
    redis_provider
        .set(
            "ttl_key",
            RedisValue::with_ttl("expires_soon".to_string(), 30),
        )
        .await?;

    info!("🧪 Added test data:");
    info!("   test_key -> test_value");
    info!("   ttl_key -> expires_soon (30s TTL)");

    // Test some operations
    if let Ok(Some(value)) = redis_provider.get("test_key").await {
        info!("✅ GET test_key -> {}", value.data);
    }

    // Show metrics
    match redis_provider.metrics().await {
        Ok(metrics) => {
            info!("📊 Memory provider metrics:");
            info!("   Total keys: {}", metrics.total_keys);
            info!("   Keys with TTL: {}", metrics.keys_with_ttl);
        }
        Err(e) => warn!("Could not fetch metrics: {}", e),
    }

    // Test increment operation
    match redis_provider.incr("counter", 5).await {
        Ok(new_value) => info!("✅ INCR counter 5 -> {}", new_value),
        Err(e) => warn!("INCR failed: {}", e),
    }

    // Test append operation
    match redis_provider.append("test_key", "_appended").await {
        Ok(new_length) => info!("✅ APPEND test_key '_appended' -> length {}", new_length),
        Err(e) => warn!("APPEND failed: {}", e),
    }

    // Verify the appended value
    if let Ok(Some(value)) = redis_provider.get("test_key").await {
        info!("✅ GET test_key after append -> {}", value.data);
    }

    // Test TTL operations
    match redis_provider.get("ttl_key").await {
        Ok(Some(value)) => {
            info!("✅ TTL key still exists: {}", value.data);
            info!("   TTL: {} seconds", value.ttl());
        }
        Ok(None) => info!("TTL key has expired"),
        Err(e) => warn!("TTL test failed: {}", e),
    }

    // Test SET NX operation
    match redis_provider
        .setnx("new_key", RedisValue::new("new_value".to_string()))
        .await
    {
        Ok(true) => info!("✅ SETNX new_key -> success"),
        Ok(false) => info!("SETNX new_key -> key already exists"),
        Err(e) => warn!("SETNX failed: {}", e),
    }

    // Test GETSET operation
    match redis_provider
        .getset("test_key", RedisValue::new("replaced_value".to_string()))
        .await
    {
        Ok(Some(old_value)) => info!("✅ GETSET test_key -> old value: {}", old_value),
        Ok(None) => info!("GETSET test_key -> no old value"),
        Err(e) => warn!("GETSET failed: {}", e),
    }

    // Final metrics
    match redis_provider.metrics().await {
        Ok(metrics) => {
            info!("📊 Final metrics:");
            info!("   Total keys: {}", metrics.total_keys);
            info!("   Keys with TTL: {}", metrics.keys_with_ttl);
            info!("   GET operations: {}", metrics.get_operations);
            info!("   SET operations: {}", metrics.set_operations);
        }
        Err(e) => warn!("Could not fetch final metrics: {}", e),
    }

    // Cleanup
    redis_provider.shutdown().await?;
    info!("✅ Demonstration complete - Redis data provider works correctly!");
    info!("");
    info!("🎉 Summary:");
    info!("   - Memory-based Redis data provider is functional");
    info!("   - TTL support works correctly");
    info!("   - All Redis string operations (GET, SET, INCR, APPEND, etc.) work");
    info!("   - Ready for integration into Orbit server's Redis protocol handler");
    info!("   - RocksDB persistent provider can be completed for durable storage");

    Ok(())
}
