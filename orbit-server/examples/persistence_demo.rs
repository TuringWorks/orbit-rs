//! Orbit-server persistence backends demonstration
//!
//! This example shows how to use the different persistence backends
//! (Memory, COW B+Tree, LSM-Tree, RocksDB) with the orbit-server.

use orbit_server::persistence::*;
use orbit_server::persistence::factory::*;
use orbit_shared::*;
use std::sync::Arc;
use tokio::time::Instant;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();
    
    println!("=== Orbit-Server Persistence Backends Demo ===\n");
    
    // Demo each backend
    demo_memory_backend().await?;
    demo_cow_btree_backend().await?;
    demo_lsm_tree_backend().await?;
    demo_rocksdb_backend().await?;
    
    // Performance comparison
    performance_comparison().await?;
    
    println!("✅ All persistence backends working correctly!");
    Ok(())
}

async fn demo_memory_backend() -> OrbitResult<()> {
    println!("🧠 Memory Backend Demo");
    let config = PersistenceConfig::Memory(MemoryConfig::default());
    
    let provider = create_addressable_provider(&config).await?;
    let lease = create_test_lease();
    
    let start = Instant::now();
    provider.store_lease(&lease).await?;
    let store_time = start.elapsed();
    
    let start = Instant::now();
    let retrieved = provider.get_lease(&lease.reference).await?;
    let get_time = start.elapsed();
    
    assert!(retrieved.is_some());
    println!("  ✓ Store: {:?}, Get: {:?}", store_time, get_time);
    
    let metrics = provider.metrics().await;
    println!("  📊 Metrics: {} reads, {} writes\n", metrics.read_operations, metrics.write_operations);
    
    Ok(())
}

async fn demo_cow_btree_backend() -> OrbitResult<()> {
    println!("🌳 COW B+Tree Backend Demo");
    let config = PersistenceConfig::CowBTree(cow_btree::CowBTreeConfig {
        data_dir: "./demo_cow_data".to_string(),
        ..Default::default()
    });
    
    let provider = create_addressable_provider(&config).await?;
    let lease = create_test_lease();
    
    let start = Instant::now();
    provider.store_lease(&lease).await?;
    let store_time = start.elapsed();
    
    let start = Instant::now();
    let retrieved = provider.get_lease(&lease.reference).await?;
    let get_time = start.elapsed();
    
    assert!(retrieved.is_some());
    println!("  ✓ Store: {:?}, Get: {:?}", store_time, get_time);
    
    let health = provider.health_check().await;
    println!("  🏥 Health: {:?}", health);
    
    provider.shutdown().await?;
    println!("  📊 COW B+Tree backend working correctly\n");
    
    Ok(())
}

async fn demo_lsm_tree_backend() -> OrbitResult<()> {
    println!("📚 LSM-Tree Backend Demo");
    let config = PersistenceConfig::LsmTree(lsm_tree::LsmTreeConfig {
        data_dir: "./demo_lsm_data".to_string(),
        ..Default::default()
    });
    
    let provider = create_addressable_provider(&config).await?;
    let lease = create_test_lease();
    
    let start = Instant::now();
    provider.store_lease(&lease).await?;
    let store_time = start.elapsed();
    
    let start = Instant::now();
    let retrieved = provider.get_lease(&lease.reference).await?;
    let get_time = start.elapsed();
    
    assert!(retrieved.is_some());
    println!("  ✓ Store: {:?}, Get: {:?}", store_time, get_time);
    
    let health = provider.health_check().await;
    println!("  🏥 Health: {:?}", health);
    
    provider.shutdown().await?;
    println!("  📊 LSM-Tree backend working correctly\n");
    
    Ok(())
}

async fn demo_rocksdb_backend() -> OrbitResult<()> {
    println!("🗻 RocksDB Backend Demo");
    let config = PersistenceConfig::RocksDB(rocksdb::RocksDbConfig {
        data_dir: "./demo_rocksdb_data".to_string(),
        ..Default::default()
    });
    
    let provider = create_addressable_provider(&config).await?;
    let lease = create_test_lease();
    
    let start = Instant::now();
    provider.store_lease(&lease).await?;
    let store_time = start.elapsed();
    
    let start = Instant::now();
    let retrieved = provider.get_lease(&lease.reference).await?;
    let get_time = start.elapsed();
    
    assert!(retrieved.is_some());
    println!("  ✓ Store: {:?}, Get: {:?}", store_time, get_time);
    
    let health = provider.health_check().await;
    println!("  🏥 Health: {:?}", health);
    
    provider.shutdown().await?;
    println!("  📊 RocksDB backend working correctly\n");
    
    Ok(())
}

async fn performance_comparison() -> OrbitResult<()> {
    println!("⚡ Performance Comparison");
    println!("{:<12} {:<12} {:<12} {:<12} {:<12}", "Backend", "Writes", "Write Avg", "Reads", "Read Avg");
    println!("{}", "=".repeat(60));
    
    let backends = vec![
        ("Memory", PersistenceConfig::Memory(MemoryConfig::default())),
        ("COW B+Tree", PersistenceConfig::CowBTree(cow_btree::CowBTreeConfig {
            data_dir: "./perf_cow_data".to_string(),
            ..Default::default()
        })),
        ("LSM-Tree", PersistenceConfig::LsmTree(lsm_tree::LsmTreeConfig {
            data_dir: "./perf_lsm_data".to_string(),
            ..Default::default()
        })),
        ("RocksDB", PersistenceConfig::RocksDB(rocksdb::RocksDbConfig {
            data_dir: "./perf_rocksdb_data".to_string(),
            ..Default::default()
        })),
    ];
    
    let num_operations = 10;
    
    for (name, config) in backends {
        let provider = create_addressable_provider(&config).await?;
        let mut write_times = Vec::new();
        let mut read_times = Vec::new();
        
        // Write benchmark
        for i in 0..num_operations {
            let lease = AddressableLease {
                reference: AddressableReference {
                    actor_type: "test_actor".to_string(),
                    actor_id: format!("test_id_{}", i),
                },
                node_id: NodeId::from("test_node"),
                endpoint: "127.0.0.1:8080".to_string(),
                lease_duration: chrono::Duration::minutes(10),
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
                metadata: std::collections::HashMap::new(),
            };
            
            let start = Instant::now();
            provider.store_lease(&lease).await?;
            write_times.push(start.elapsed());
        }
        
        // Read benchmark
        for i in 0..num_operations {
            let reference = AddressableReference {
                actor_type: "test_actor".to_string(),
                actor_id: format!("test_id_{}", i),
            };
            
            let start = Instant::now();
            let _result = provider.get_lease(&reference).await?;
            read_times.push(start.elapsed());
        }
        
        let avg_write: f64 = write_times.iter().map(|d| d.as_nanos() as f64).sum::<f64>() / write_times.len() as f64 / 1000.0;
        let avg_read: f64 = read_times.iter().map(|d| d.as_nanos() as f64).sum::<f64>() / read_times.len() as f64 / 1000.0;
        
        println!("{:<12} {:<12} {:<12.1} {:<12} {:<12.1}", 
            name, 
            write_times.len(), 
            avg_write,
            read_times.len(), 
            avg_read
        );
        
        provider.shutdown().await.ok();
    }
    
    println!("\n💡 Performance Notes:");
    println!("  • Memory: Fastest but not persistent across restarts");
    println!("  • COW B+Tree: Good balance of speed and persistence with COW semantics");  
    println!("  • LSM-Tree: Optimized for write-heavy workloads with compaction");
    println!("  • RocksDB: Production-ready with ACID guarantees and rich features\n");
    
    Ok(())
}

async fn demo_configuration_methods() -> OrbitResult<()> {
    println!("⚙️  Configuration Methods Demo");
    
    // 1. Environment variable configuration
    std::env::set_var("ORBIT_PERSISTENCE_BACKEND", "cow_btree");
    std::env::set_var("ORBIT_COW_DATA_DIR", "./env_cow_data");
    
    let env_config = load_config_from_env()?;
    println!("  ✓ Environment config loaded: {:?}", match env_config {
        PersistenceConfig::CowBTree(_) => "COW B+Tree",
        _ => "Other",
    });
    
    // 2. Builder pattern configuration
    let builder_config = PersistenceConfigBuilder::new()
        .backend("lsm_tree")
        .data_dir("./builder_lsm_data")
        .build()?;
    
    println!("  ✓ Builder config created: {:?}", match builder_config {
        PersistenceConfig::LsmTree(_) => "LSM-Tree",
        _ => "Other",
    });
    
    // 3. Registry initialization
    let registry = initialize_registry(&env_config, &builder_config).await?;
    
    let addressable_provider = registry.get_default_addressable_provider().await?;
    let cluster_provider = registry.get_default_cluster_provider().await?;
    
    println!("  ✓ Registry initialized with both providers");
    
    // Test the providers
    let lease = create_test_lease();
    addressable_provider.store_lease(&lease).await?;
    let retrieved = addressable_provider.get_lease(&lease.reference).await?;
    assert!(retrieved.is_some());
    
    let node = create_test_node();
    cluster_provider.store_node(&node).await?;
    let retrieved_node = cluster_provider.get_node(&node.id).await?;
    assert!(retrieved_node.is_some());
    
    println!("  ✅ Configuration methods demo completed\n");
    
    Ok(())
}

fn create_test_lease() -> AddressableLease {
    AddressableLease {
        reference: AddressableReference {
            actor_type: "demo_actor".to_string(),
            actor_id: Uuid::new_v4().to_string(),
        },
        node_id: NodeId::from("demo_node"),
        endpoint: "127.0.0.1:8080".to_string(),
        lease_duration: chrono::Duration::minutes(5),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        metadata: std::collections::HashMap::new(),
    }
}

fn create_test_node() -> NodeInfo {
    NodeInfo {
        id: NodeId::from(format!("node_{}", Uuid::new_v4())),
        address: "127.0.0.1:8081".to_string(),
        status: NodeStatus::Active,
        capabilities: vec!["persistence".to_string()],
        load_factor: 0.5,
        last_heartbeat: chrono::Utc::now(),
        lease: Some(NodeLease {
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
            lease_duration: chrono::Duration::minutes(10),
        }),
        metadata: std::collections::HashMap::new(),
    }
}