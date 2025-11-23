# Orbit Server Initialization Issues

## Summary

The orbit-server is currently missing critical initialization for several key components:

## Issues Identified

### 1. Persistence Layer Not Initialized ❌

**Location:** `orbit/server/src/main.rs`

**Problem:**
- RocksDB persistence layer is never initialized
- Storage instances created but `.initialize()` never called
- No data directory setup
- No persistence provider registry initialization

**Current Code (lines 227-240):**
```rust
let tiered_config = HybridStorageConfig { ... };
let mysql_storage = Arc::new(TieredTableStorage::with_config(tiered_config.clone()));
let cql_storage = Arc::new(TieredTableStorage::with_config(tiered_config));
```

**Missing:**
```rust
// Should call initialize() on storage layers
mysql_storage.initialize().await?;
cql_storage.initialize().await?;

// Should set up RocksDB paths
let db_path = PathBuf::from("/var/lib/orbit/data");
let rocksdb_storage = RocksDbTableStorage::new(&db_path)?;
rocksdb_storage.initialize().await?;
```

### 2. PostgreSQL Wire Protocol Not Started ❌

**Location:** `orbit/server/src/main.rs`

**Problem:**
- Only MySQL (port 3306) and CQL (port 9042) adapters are started
- PostgreSQL wire protocol (port 5432) is completely missing
- PostgresServer and QueryEngine are imported but never used (line 12 of server.rs)

**Current Code:**
- MySQL adapter: lines 246-260
- CQL adapter: lines 262-277
- **PostgreSQL: MISSING**

**Should Add:**
```rust
// PostgreSQL Wire Protocol
use orbit_protocols::postgres_wire::{PostgresServer, PostgresConfig};

let postgres_config = PostgresConfig {
    listen_addr: format!("{}:5432", args.bind).parse()?,
    max_connections: 1000,
    // ... other config
};

let postgres_storage = Arc::new(TieredTableStorage::with_config(tiered_config.clone()));
postgres_storage.initialize().await?;

let postgres_adapter = PostgresServer::new_with_storage(postgres_config, postgres_storage).await?;
let postgres_handle = tokio::spawn(async move {
    info!("[PostgreSQL] PostgreSQL protocol adapter started on port 5432");
    postgres_adapter.start().await.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
});
protocol_handles.push(postgres_handle);
```

### 3. Cluster Configuration Not Initialized ❌

**Location:** `orbit/server/src/main.rs`

**Problem:**
- Seed nodes are parsed from CLI (line 102, 204-208)
- ClusterManager exists in server.rs (line 3) but never used in main.rs
- No Raft consensus initialization
- No cluster joining/discovery

**Current Code:**
```rust
if !args.seed_nodes.is_empty() {
    info!("[Seed Nodes] Seed nodes: {:?}", args.seed_nodes);
} else {
    info!("[Seed Nodes] Starting as seed node (no seed nodes configured)");
}
// BUT NOTHING HAPPENS WITH SEED NODES!
```

**Should Add:**
```rust
use orbit_shared::consensus::{RaftConfig, RaftNode};
use orbit_shared::cluster_manager::ClusterManager;

// Initialize Raft consensus
let raft_config = RaftConfig {
    node_id: args.node_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
    listen_addr: format!("{}:{}", args.bind, args.grpc_port + 1).parse()?,
    seed_nodes: args.seed_nodes.clone(),
    ..Default::default()
};

let raft_node = RaftNode::new(raft_config).await?;
raft_node.start().await?;

// Initialize cluster manager
let cluster_manager = ClusterManager::new(raft_node);
cluster_manager.initialize().await?;

if !args.seed_nodes.is_empty() {
    info!("[Cluster] Joining cluster via seed nodes: {:?}", args.seed_nodes);
    cluster_manager.join_cluster(&args.seed_nodes).await?;
} else {
    info!("[Cluster] Bootstrapping new cluster as seed node");
    cluster_manager.bootstrap().await?;
}
```

### 4. Cloud Persistence (S3/Azure Cold Storage) Not Configured ❌

**Location:** `orbit/server/src/main.rs`

**Problem:**
- HybridStorageConfig is created with cold tier enabled (line 232)
- But no S3 or Azure storage backend is configured
- Cold tier will fail when trying to migrate data

**Current Code (lines 227-232):**
```rust
let tiered_config = HybridStorageConfig {
    hot_to_warm_threshold: Duration::from_secs(48 * 60 * 60),
    warm_to_cold_threshold: Duration::from_secs(30 * 24 * 60 * 60),
    auto_tiering: true,  // ← Will try to use cold tier!
    background_migration: true,
};
```

**Should Add:**
```rust
use orbit_protocols::postgres_wire::sql::execution::storage_config::{StorageConfig, AzureConfig, S3Config};

// Configure cloud storage backend
let storage_backend = if let Ok(azure_conn) = std::env::var("AZURE_STORAGE_CONNECTION_STRING") {
    StorageBackend::Azure(AzureConfig::from_connection_string(&azure_conn)?)
} else if let Ok(s3_bucket) = std::env::var("S3_BUCKET") {
    StorageBackend::S3(S3Config {
        bucket: s3_bucket,
        region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        ..Default::default()
    })
} else {
    warn!("[Cold Storage] No cloud storage configured - cold tier disabled");
    tiered_config.auto_tiering = false;
    StorageBackend::None
};

// Initialize cold storage if configured
if storage_backend != StorageBackend::None {
    info!("[Cold Storage] Cold tier configured with {:?}", storage_backend);
    // Pass storage_backend to HybridStorageManager
}
```

### 5. Persistence Provider Registry Not Used ❌

**Location:** `orbit/server/src/server.rs` line 5

**Problem:**
- PersistenceProviderRegistry is imported but never initialized
- No persistence providers registered
- Actor state persistence will not work

**Should Add to main.rs:**
```rust
use orbit_server::persistence::{PersistenceProviderRegistry, PersistenceProvider};

// Initialize persistence registry
let mut persistence_registry = PersistenceProviderRegistry::new();

// Register RocksDB persistence
let rocksdb_provider = RocksDbPersistenceProvider::new("/var/lib/orbit/rocksdb")?;
persistence_registry.register("rocksdb", Box::new(rocksdb_provider))?;

// Register cloud persistence if configured
if let Ok(azure_conn) = std::env::var("AZURE_STORAGE_CONNECTION_STRING") {
    let azure_provider = AzurePersistenceProvider::new(&azure_conn)?;
    persistence_registry.register("azure", Box::new(azure_provider))?;
}

// Pass registry to server
server.set_persistence_registry(persistence_registry);
```

### 6. Protocol Config Not Used ❌

**Location:** `orbit/server/src/server.rs` lines 21-67

**Problem:**
- ProtocolConfig struct exists with all protocol flags
- But main.rs doesn't use it - protocols are hardcoded
- No way to disable protocols via config

**Current:** Protocols always start on fixed ports
**Should:** Use config.protocols to control which protocols start

### 7. Storage Metrics and Monitoring Not Initialized ❌

**Problem:**
- Storage has metrics capability (TieredTableStorage has metrics)
- But metrics endpoint not configured
- No Prometheus integration started

**Should Add:**
```rust
use metrics_exporter_prometheus::PrometheusBuilder;

// Initialize Prometheus metrics
let prometheus_handle = PrometheusBuilder::new()
    .with_http_listener(format!("{}:{}", args.bind, args.metrics_port).parse()?)
    .install_recorder()?;

info!("[Metrics] Prometheus metrics exposed on port {}", args.metrics_port);
```

## Recommended Fix Order

1. **Initialize persistence layers** (storage.initialize())
2. **Add PostgreSQL wire protocol adapter**
3. **Configure and start cluster manager with Raft**
4. **Add cloud storage backend configuration**
5. **Initialize persistence provider registry**
6. **Use ProtocolConfig from OrbitServerConfig**
7. **Start Prometheus metrics endpoint**

## Impact

Without these initializations:
- ✅ Server starts (gRPC, MySQL, CQL work)
- ❌ No data persistence across restarts
- ❌ No PostgreSQL protocol support
- ❌ No cluster coordination
- ❌ Cold tier migrations will fail
- ❌ Actor state not persisted
- ❌ Metrics not exposed

## Files Needing Changes

1. `orbit/server/src/main.rs` - Primary initialization logic
2. `orbit/server/src/server.rs` - OrbitServerBuilder to use configs
3. `orbit/server/src/lib.rs` - Export necessary types

## Example Configuration File Needed

Create `config/orbit-server.toml`:

```toml
[server]
namespace = "default"
bind_address = "0.0.0.0"
port = 50051
node_id = "auto"

[cluster]
seed_nodes = []
raft_port = 50052

[persistence]
provider = "rocksdb"
data_dir = "/var/lib/orbit/data"

[persistence.cold_storage]
enabled = true
backend = "azure"  # or "s3"
connection_string = "${AZURE_STORAGE_CONNECTION_STRING}"

[protocols]
redis_enabled = true
redis_port = 6379
postgres_enabled = true
postgres_port = 5432
mysql_enabled = true
mysql_port = 3306
cql_enabled = true
cql_port = 9042

[metrics]
prometheus_port = 9090
```

## Next Steps

Would you like me to:
1. Implement the PostgreSQL Wire Protocol initialization?
2. Add cluster/Raft initialization?
3. Configure cloud cold storage backends?
4. Implement TOML config file loading?
5. All of the above?
