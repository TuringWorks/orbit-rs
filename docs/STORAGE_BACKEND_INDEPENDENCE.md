# Storage Backend Independence in Orbit-RS

## Quick Answer

**Yes, cloud storage (S3, Azure, GCP, MinIO) and local storage (LSM-Tree, RocksDB, B-Tree) are completely independent.**

Each storage backend is a complete, self-contained persistence solution. You choose **ONE** backend type per deployment - they don't layer or combine.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                 Orbit-RS Framework                      │
│            (AddressableDirectoryProvider)               │
└┬────────────┬─────────────┬─────────────┬─────────────┬-┘
 │            │             │             │             │
 │   Cloud    │   Local     │   Local     │   Local     │
 │  (REST)    │  (Embedded) │ (Embedded)  │ (Embedded)  │
 │            │             │             │             │
 ▼            ▼             ▼             ▼             ▼
┌──────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌────────┐
│    S3    │ │ LSM-Tree  │ │  RocksDB  │ │ B+Tree+   │ │ Memory │
│ Provider │ │ Provider  │ │ Provider  │ │ WAL       │ │Provider│
│          │ │           │ │           │ │ Provider  │ │        │
│ REST API │ │ SSTables  │ │ Column    │ │ COW Nodes │ │ HashMap│
│ Objects  │ │ Memtables │ │ Families  │ │ WAL Files │ │ In-RAM │
└────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └───┬────┘
     │             │             │             │           │
 Internet      Local SSD     Local SSD     Local SSD    RAM
 (AWS S3)       Files         Files         Files      Only
```

## Storage Backend Types

### Cloud Backends (Object Storage)
- **S3, Azure Blob, Google Cloud Storage, MinIO**
- **How they work**: Direct HTTP REST API calls to cloud services
- **Data format**: JSON objects with metadata stored as blobs/objects
- **No local storage**: Completely cloud-based, no local files

**Example**: S3 stores actor leases as objects like:
```
s3://my-bucket/orbit/leases/UserActor/user-123
```

### Local Backends (Embedded Storage)
- **LSM-Tree, RocksDB, B+Tree, Memory**
- **How they work**: Complete storage engines with local data structures
- **Data format**: Custom binary formats optimized for each engine
- **Local files only**: No cloud dependencies

**Example**: LSM-Tree stores data in local files:
```
/data/orbit_lsm_data/
├── memtable_001.sst
├── memtable_002.sst
└── orbit.wal
```

## Code Evidence

**S3 Provider** - Direct REST API:
```rust
impl S3AddressableDirectoryProvider {
    async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        let url = format!("{}/{}/{}", self.config.endpoint, self.config.bucket, key);
        let response = self.client.put(&url).body(data).send().await?;
        // ↑ Direct HTTP call to S3 - no local storage
    }
}
```

**LSM-Tree Provider** - Local data structures:
```rust
impl LsmTreeAddressableProvider {
    async fn store_lease(&self, lease: &AddressableLease) -> OrbitResult<()> {
        // Write to local WAL file
        self.wal.lock().await.append(&key, &value, "INSERT").await?;
        
        // Write to in-memory table
        self.active_memtable.write().unwrap().insert(key, value);
        
        // Manage local SSTables and compaction
        self.maybe_trigger_compaction().await?;
        // ↑ All operations on local files - no cloud involved
    }
}
```

## Configuration Examples

You configure **exactly one** backend per Orbit-RS deployment:

### Option 1: Pure Cloud (S3)
```toml
[server]
persistence_backend = "s3"

[persistence.s3]
endpoint = "https://s3.amazonaws.com"
bucket = "orbit-production-state"

# All data stored in S3 objects
```

### Option 2: Pure Local (LSM-Tree)
```toml
[server]
persistence_backend = "lsm_tree"

[persistence.lsm_tree]
data_dir = "/ssd/orbit_data"
memtable_size_limit = 67108864  # 64MB

# All data stored in local LSM files
```

### Option 3: Pure Local (RocksDB)
```toml
[server]
persistence_backend = "rocksdb"

[persistence.rocksdb]
data_dir = "/nvme/orbit_rocksdb"
enable_wal = true

# All data stored in RocksDB files
```

## Factory Pattern

The `factory.rs` module shows this independence clearly:

```rust
pub async fn create_addressable_provider(
    config: &PersistenceConfig,
) -> OrbitResult<Arc<dyn AddressableDirectoryProvider>> {
    match config {
        // Each case creates a completely different provider
        PersistenceConfig::S3(s3_config) => {
            Ok(Arc::new(S3AddressableDirectoryProvider::new(s3_config.clone())))
        }
        PersistenceConfig::LsmTree(lsm_config) => {
            Ok(Arc::new(LsmTreeAddressableProvider::new(lsm_config.clone())))
        }
        PersistenceConfig::RocksDB(rocks_config) => {
            Ok(Arc::new(RocksDbAddressableProvider::new(rocks_config.clone())))
        }
        // Each provider is self-contained - no mixing
    }
}
```

## Key Takeaways

✅ **Complete Independence**: Cloud and local storage backends share no code or dependencies  
✅ **Single Backend**: Choose one backend type per deployment, not a combination  
✅ **Self-Contained**: Each backend handles all persistence operations internally  
✅ **Different Protocols**: Cloud uses HTTP REST, local uses direct file I/O  
✅ **Different Formats**: Cloud stores JSON objects, local uses optimized binary formats  

❌ **No Layering**: You cannot use S3 "on top of" RocksDB or vice versa  
❌ **No Mixing**: You cannot use multiple backends simultaneously  
❌ **No Fallback**: One backend choice per Orbit-RS instance  

For detailed information, see the complete [Virtual Actor Persistence](VIRTUAL_ACTOR_PERSISTENCE.md) guide.