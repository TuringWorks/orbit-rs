---
layout: default
title: Alternative Persistence Implementations for orbit-rs
category: documentation
---

# Alternative Persistence Implementations for orbit-rs

## Executive Summary

After analyzing orbit-rs's specific requirements for actor lease management, cluster coordination, and catastrophic failure recovery, there are several alternatives to LSM Trees that may be **superior** for this use case:

1. **🥇 Copy-on-Write B+ Trees** - Best overall fit
2. **🥈 Hybrid WAL + Memory-Mapped Files** - Excellent performance 
3. **🥉 Append-Only Log with Periodic Snapshots** - Simplest implementation
4. **🔄 Actor-Specific Persistence Layer** - Most specialized
5. **🏗️ RocksDB/LevelDB Integration** - Proven solution

## Detailed Analysis

### 1. 🥇 **Copy-on-Write B+ Trees** (BEST FIT)

```rust
pub struct CowBTreePersistence {
    // Immutable B+ tree with COW semantics
    current_tree: Arc<BTreeNode>,
    wal: WriteAheadLog,
    snapshots: SnapshotManager,
    
    // Optimizations for actor leases
    lease_cache: LruCache<ActorKey, LeaseEntry>,
    dirty_pages: BitSet,
}
```

#### Why Perfect for orbit-rs:
- **Actor lease updates** → Single node modifications with COW
- **Point-in-time snapshots** → Natural with immutable trees  
- **Range queries** → Native B+ tree strength
- **Memory efficiency** → Share unchanged pages between versions
- **Crash recovery** → WAL replay onto last consistent tree

#### Performance Characteristics:
```
Write latency:     5-20μs (single page COW)
Read latency:      1-3μs (cached pages)  
Memory overhead:   Only modified pages duplicated
Recovery time:     2-5 seconds (WAL replay)
Space efficiency:  85-95% (shared pages)
```

#### Implementation Phases:
```rust
// Phase 1: Core COW B+ Tree
pub struct BTreeNode {
    keys: Vec<ActorKey>,
    values: Vec<ActorValue>, 
    children: Vec<Arc<BTreeNode>>, // COW children
    version: u64,
    dirty: bool,
}

// Phase 2: Actor-optimized operations
impl CowBTreePersistence {
    pub async fn update_actor_lease(&self, lease: ActorLease) -> Result<()> {
        // COW update - only modified path from root to leaf
        let new_root = self.current_tree.clone_and_update(
            &lease.key(), 
            &lease.into()
        ).await?;
        
        // Atomic swap
        self.current_tree = Arc::new(new_root);
        self.wal.append(&lease).await?;
        Ok(())
    }
}
```

---

### 2. 🥈 **Hybrid WAL + Memory-Mapped Files** 

```rust
pub struct MMapPersistence {
    // Memory-mapped file for hot data
    mmap_region: MMapRegion,
    wal: WriteAheadLog,
    
    // Actor lease optimization
    lease_table: MMapHashTable<ActorKey, LeaseEntry>,
    cluster_table: MMapHashTable<NodeId, NodeInfo>,
}
```

#### Why Excellent for orbit-rs:
- **Zero-copy reads** → Memory-mapped lease table
- **Atomic writes** → WAL ensures consistency
- **Fast recovery** → Memory map + WAL replay
- **OS-optimized** → Leverage kernel page cache
- **Simple implementation** → Less complexity than LSM

#### Performance Characteristics:
```
Write latency:     2-8μs (memory write + WAL)
Read latency:      0.5-2μs (memory read)
Memory usage:      Shared with OS page cache
Recovery time:     1-3 seconds
Space efficiency:  90-98% (no amplification)
```

#### Implementation:
```rust
impl MMapPersistence {
    pub async fn initialize(data_file: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(data_file)?;
            
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        
        Ok(Self {
            mmap_region: MMapRegion::new(mmap),
            wal: WriteAheadLog::new("orbit.wal").await?,
            lease_table: MMapHashTable::new(&mmap_region, LEASE_OFFSET)?,
            cluster_table: MMapHashTable::new(&mmap_region, CLUSTER_OFFSET)?,
        })
    }
    
    pub async fn store_lease(&self, lease: &ActorLease) -> Result<()> {
        // 1. Write to WAL first (durability)
        self.wal.append(lease).await?;
        
        // 2. Update memory-mapped hash table (performance) 
        self.lease_table.insert(&lease.key(), lease)?;
        
        // 3. Async sync to disk
        self.schedule_sync();
        Ok(())
    }
}
```

---

### 3. 🥉 **Append-Only Log + Periodic Snapshots**

```rust
pub struct AppendOnlyPersistence {
    // Main log for all operations
    append_log: AppendLog,
    
    // Periodic snapshots for fast recovery
    snapshot_manager: SnapshotManager,
    
    // In-memory indices for fast access
    lease_index: HashMap<ActorKey, LogOffset>,
    cluster_index: HashMap<NodeId, LogOffset>,
}
```

#### Why Great for orbit-rs:
- **Simplest implementation** → Easiest to get right
- **Excellent write performance** → Pure append operations
- **Natural audit trail** → All changes logged
- **Easy replication** → Just stream the log
- **Predictable performance** → No compaction stalls

#### Trade-offs:
- **Read amplification** → May need to read multiple log entries
- **Space usage** → Grows until snapshot+cleanup
- **Recovery time** → Proportional to log size

#### Implementation:
```rust
impl AppendOnlyPersistence {
    pub async fn store_lease(&self, lease: &ActorLease) -> Result<()> {
        let log_entry = LogEntry {
            sequence: self.next_sequence(),
            timestamp: SystemTime::now(),
            operation: Operation::UpdateLease(lease.clone()),
        };
        
        // Append to log (main durability mechanism)
        let offset = self.append_log.append(&log_entry).await?;
        
        // Update in-memory index (for fast reads)
        self.lease_index.insert(lease.key(), offset);
        
        Ok(())
    }
    
    pub async fn get_lease(&self, key: &ActorKey) -> Result<Option<ActorLease>> {
        // Fast path: check index
        if let Some(offset) = self.lease_index.get(key) {
            let entry = self.append_log.read_at(*offset).await?;
            return Ok(Some(entry.into_lease()?));
        }
        
        Ok(None)
    }
    
    // Background snapshot creation
    pub async fn create_snapshot(&self) -> Result<SnapshotId> {
        let snapshot = Snapshot::new();
        
        // Serialize current state
        for (key, offset) in &self.lease_index {
            let entry = self.append_log.read_at(*offset).await?;
            snapshot.add_lease(key, &entry.into_lease()?);
        }
        
        let snapshot_id = self.snapshot_manager.save(snapshot).await?;
        
        // Can now truncate log before this point
        self.append_log.truncate_before(self.current_sequence()).await?;
        
        Ok(snapshot_id)
    }
}
```

---

### 4. 🔄 **Actor-Specific Persistence Layer**

The most specialized approach - build persistence around actor system semantics:

```rust
pub struct ActorPersistence {
    // Separate stores optimized for different data patterns
    lease_store: LeaseStore,        // High-frequency updates
    cluster_store: ClusterStore,    // Medium-frequency updates  
    query_cache: QueryCacheStore,   // Read-heavy with TTL
    timeseries_store: TSStore,      // Append-only time series
}

pub struct LeaseStore {
    // Optimized specifically for actor lease patterns
    active_leases: DashMap<ActorKey, LeaseEntry>,
    lease_log: CircularBuffer<LeaseUpdate>, 
    checkpoint_writer: CheckpointWriter,
}

impl LeaseStore {
    pub async fn update_lease(&self, lease: ActorLease) -> Result<()> {
        let update = LeaseUpdate {
            key: lease.key(),
            lease: lease.clone(),
            timestamp: SystemTime::now(),
            update_type: if lease.is_renewal() { 
                UpdateType::Renewal 
            } else { 
                UpdateType::FullUpdate 
            },
        };
        
        // Different handling based on update type
        match update.update_type {
            UpdateType::Renewal => {
                // Fast path - just update timestamp in memory
                self.active_leases.get_mut(&lease.key())
                    .map(|mut entry| entry.last_renewed = update.timestamp);
                    
                // Batch renewals for disk persistence
                self.lease_log.push_renewal(update);
            }
            UpdateType::FullUpdate => {
                // Full update - immediate persistence
                self.active_leases.insert(lease.key(), lease.into());
                self.lease_log.push_update(update);
                self.checkpoint_writer.schedule_checkpoint();
            }
        }
        
        Ok(())
    }
}
```

#### Benefits:
- **Maximum optimization** for each data type
- **Minimal overhead** - no generic abstractions
- **Actor-aware** scheduling and batching
- **Predictable latency** - no cross-workload interference

---

### 5. 🏗️ **RocksDB/LevelDB Integration** 

Use battle-tested LSM implementation with orbit-rs optimizations:

```rust
pub struct RocksDBPersistence {
    db: rocksdb::DB,
    // Column families for different data types
    cf_leases: rocksdb::ColumnFamily,
    cf_cluster: rocksdb::ColumnFamily,
    cf_queries: rocksdb::ColumnFamily,
    
    // orbit-rs specific optimizations
    lease_write_options: WriteOptions, // Optimized for frequent updates
    read_options: ReadOptions,
}

impl RocksDBPersistence {
    pub fn new(path: &Path) -> Result<Self> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        
        // Optimize for actor lease workload
        opts.set_write_buffer_size(64 * 1024 * 1024);  // 64MB memtables
        opts.set_max_write_buffer_number(4);
        opts.set_target_file_size_base(64 * 1024 * 1024);
        
        // Optimize compaction for frequent updates
        opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
        opts.set_level_compaction_dynamic_level_bytes(true);
        
        let cf_descriptors = vec![
            rocksdb::ColumnFamilyDescriptor::new("leases", opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new("cluster", opts.clone()),
            rocksdb::ColumnFamilyDescriptor::new("queries", opts.clone()),
        ];
        
        let db = rocksdb::DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
        
        Ok(Self {
            db,
            cf_leases: db.cf_handle("leases").unwrap(),
            cf_cluster: db.cf_handle("cluster").unwrap(), 
            cf_queries: db.cf_handle("queries").unwrap(),
            lease_write_options: Self::lease_write_options(),
            read_options: ReadOptions::default(),
        })
    }
    
    fn lease_write_options() -> WriteOptions {
        let mut opts = WriteOptions::default();
        opts.set_sync(true);  // Ensure durability for lease updates
        opts.disable_wal(false);  // Keep WAL for recovery
        opts
    }
    
    pub async fn store_lease(&self, lease: &ActorLease) -> Result<()> {
        let key = lease.key().to_bytes();
        let value = lease.serialize()?;
        
        self.db.put_cf_opt(&self.cf_leases, &self.lease_write_options, key, value)?;
        Ok(())
    }
}
```

---

## 📊 **Comparative Analysis**

| Implementation | Write Latency | Read Latency | Recovery Time | Complexity | Memory Usage |
|----------------|---------------|--------------|---------------|------------|--------------|
| **COW B+ Trees** | 5-20μs | 1-3μs | 2-5s | Medium | Low (shared) |
| **MMap + WAL** | 2-8μs | 0.5-2μs | 1-3s | Low | Very Low |
| **Append Log** | 1-5μs | 5-50μs | 5-30s | Very Low | Medium |
| **Actor-Specific** | 1-10μs | 0.5-2μs | 1-5s | High | Medium |
| **RocksDB** | 10-50μs | 2-10μs | 3-10s | Low | High |
| **LSM Trees** | 10-50μs | 2-5μs | 5-15s | High | High |

## 🎯 **Recommendation**

For orbit-rs, I recommend the **Copy-on-Write B+ Trees** approach because:

### Perfect Fit for Actor Systems:
1. **Lease updates are localized** → COW minimizes copying
2. **Snapshot requirements** → Natural with immutable trees
3. **Range queries needed** → B+ trees excel here
4. **Memory efficiency critical** → COW shares unchanged data
5. **Fast recovery essential** → Small WAL + tree restoration

### Implementation Priority:
```rust
// Phase 1: Basic COW B+ Tree (4 weeks)
// Phase 2: Actor lease optimizations (2 weeks)  
// Phase 3: Snapshot management (2 weeks)
// Phase 4: Performance tuning (2 weeks)
// Total: 10 weeks vs 12 weeks for LSM
```

### Fallback Option:
If development resources are limited, **MMap + WAL** provides 80% of the benefits with 40% of the complexity.

## 🚀 **Next Steps**

1. **Prototype COW B+ Tree** core operations (1 week)
2. **Benchmark against current Memory provider** (actor lease workload)
3. **Compare with RocksDB baseline** (using existing proven solution)
4. **Decision point**: Custom COW implementation vs RocksDB integration

The actor system's specific access patterns (frequent small updates, occasional range queries, snapshot requirements) make COW B+ Trees the theoretically optimal choice, but RocksDB provides a battle-tested fallback with good-enough performance.