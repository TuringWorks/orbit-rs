---
layout: default
title: LSM Tree Implementation Plan for orbit-rs
category: documentation
---

# LSM Tree Implementation Plan for orbit-rs

## Executive Summary

Implementing a Log Structured Merge Tree (LSM Tree) for orbit-rs will provide:

- **10x faster writes** for actor state updates and lease renewals
- **Guaranteed durability** through Write-Ahead Logging (WAL)
- **Efficient space utilization** through compaction
- **Point-in-time recovery** for catastrophic failures
- **Multi-model data optimization** for diverse orbit-rs workloads

## Architecture Overview

### Core Components

```rust
pub struct OrbitLSMTree {
    // In-memory components
    memtable: Arc<RwLock<MemTable>>,
    immutable_memtables: Arc<RwLock<Vec<MemTable>>>,
    
    // Persistent components  
    wal: WriteAheadLog,
    sstable_manager: SSTableManager,
    compaction_scheduler: CompactionScheduler,
    
    // Configuration
    config: LSMConfig,
    metrics: Arc<LSMMetrics>,
}

pub struct LSMConfig {
    // Memory management
    pub memtable_size_mb: usize,        // Default: 64MB
    pub max_memtables: usize,           // Default: 4
    pub write_buffer_size: usize,       // Default: 16MB
    
    // Compaction strategy
    pub compaction_strategy: CompactionStrategy,
    pub max_levels: usize,              // Default: 7
    pub level_size_multiplier: usize,   // Default: 10
    
    // Durability settings
    pub wal_sync_mode: WALSyncMode,     // Default: PerWrite
    pub snapshot_interval_secs: u64,    // Default: 300 (5 minutes)
    
    // Performance tuning
    pub bloom_filter_bits_per_key: usize, // Default: 10
    pub block_cache_size_mb: usize,        // Default: 256MB
    pub compression: CompressionType,       // Default: Lz4
}
```

### Data Model Integration

```rust
// Orbit-specific key-value layout
pub enum OrbitKey {
    ActorLease { namespace: String, actor_id: String },
    ClusterNode { node_id: String },
    OrbitQLCache { query_hash: u64 },
    TimeSeries { metric: String, timestamp: u64 },
}

pub enum OrbitValue {
    ActorLease(AddressableLease),
    ClusterNode(NodeInfo), 
    QueryResult(CachedQueryResult),
    TimeSeriesPoint(TimeSeriesData),
}

// Specialized serialization for orbit-rs data types
impl Serializable for OrbitKey { /* ... */ }
impl Serializable for OrbitValue { /* ... */ }
```

## Implementation Phases

### Phase 1: Core LSM Engine (4-6 weeks)

#### Week 1-2: MemTable & WAL

```rust
// orbit-server/src/persistence/lsm/memtable.rs
pub struct MemTable {
    data: SkipMap<OrbitKey, OrbitValue>,
    size_bytes: AtomicUsize,
    created_at: Instant,
    sealed: AtomicBool,
}

// orbit-server/src/persistence/lsm/wal.rs
pub struct WriteAheadLog {
    writer: BufWriter<File>,
    sequence_number: AtomicU64,
    sync_mode: WALSyncMode,
}

impl WriteAheadLog {
    pub async fn append(&mut self, key: &OrbitKey, value: &OrbitValue) -> Result<u64> {
        let entry = WALEntry {
            sequence: self.sequence_number.fetch_add(1, Ordering::SeqCst),
            key: key.clone(),
            value: value.clone(),
            timestamp: SystemTime::now(),
        };
        
        self.writer.write_all(&entry.serialize()).await?;
        if self.sync_mode == WALSyncMode::PerWrite {
            self.writer.flush().await?;
        }
        Ok(entry.sequence)
    }
}
```

#### Week 3-4: SSTable Implementation

```rust
// orbit-server/src/persistence/lsm/sstable.rs
pub struct SSTable {
    file_path: PathBuf,
    metadata: SSTableMetadata,
    bloom_filter: BloomFilter,
    index: BlockIndex,
}

pub struct SSTableWriter {
    writer: BufWriter<File>,
    index_builder: BlockIndexBuilder,
    bloom_builder: BloomFilterBuilder,
    compression: CompressionType,
}

impl SSTableWriter {
    pub async fn write_batch(&mut self, entries: Vec<(OrbitKey, OrbitValue)>) -> Result<()> {
        // Sort entries by key for efficient range queries
        let mut sorted_entries = entries;
        sorted_entries.sort_by_key(|(k, _)| k.clone());
        
        for (key, value) in sorted_entries {
            // Add to bloom filter
            self.bloom_builder.add(&key);
            
            // Compress and write block
            let block = self.create_block(&key, &value)?;
            let compressed_block = self.compress_block(block)?;
            
            // Update index
            let offset = self.writer.stream_position().await?;
            self.index_builder.add_entry(key.clone(), offset, compressed_block.len());
            
            self.writer.write_all(&compressed_block).await?;
        }
        
        Ok(())
    }
}
```

#### Week 5-6: Read Path & Bloom Filters

```rust
impl OrbitLSMTree {
    pub async fn get(&self, key: &OrbitKey) -> Result<Option<OrbitValue>> {
        // 1. Check memtable first (fastest)
        if let Some(value) = self.memtable.read().await.get(key) {
            return Ok(Some(value.clone()));
        }
        
        // 2. Check immutable memtables
        for memtable in self.immutable_memtables.read().await.iter() {
            if let Some(value) = memtable.get(key) {
                return Ok(Some(value.clone()));
            }
        }
        
        // 3. Check SSTables (with bloom filter optimization)
        for level in 0..self.config.max_levels {
            for sstable in self.sstable_manager.get_level_tables(level) {
                // Bloom filter check first
                if !sstable.bloom_filter.contains(key) {
                    continue;
                }
                
                if let Some(value) = sstable.get(key).await? {
                    return Ok(Some(value));
                }
            }
        }
        
        Ok(None)
    }
}
```

### Phase 2: Compaction & Space Management (3-4 weeks)

#### Week 1-2: Level-Based Compaction

```rust
// orbit-server/src/persistence/lsm/compaction.rs
pub struct CompactionScheduler {
    strategy: CompactionStrategy,
    running_compactions: Arc<RwLock<HashSet<CompactionTask>>>,
}

pub enum CompactionStrategy {
    Leveled { size_ratio: f64 },
    Tiered { max_tables_per_level: usize },
    Universal { ratio_threshold: f64 },
}

impl CompactionScheduler {
    pub async fn schedule_compaction(&self, sstable_manager: &SSTableManager) -> Option<CompactionTask> {
        match self.strategy {
            CompactionStrategy::Leveled { size_ratio } => {
                self.schedule_leveled_compaction(sstable_manager, size_ratio).await
            }
            _ => todo!("Other compaction strategies"),
        }
    }
    
    async fn schedule_leveled_compaction(
        &self,
        sstable_manager: &SSTableManager,
        size_ratio: f64,
    ) -> Option<CompactionTask> {
        // Find level that exceeds size threshold
        for level in 0..self.config.max_levels - 1 {
            let level_size = sstable_manager.get_level_size(level);
            let max_size = self.calculate_max_level_size(level);
            
            if level_size as f64 > max_size * size_ratio {
                let source_tables = sstable_manager.get_level_tables(level);
                let target_level = level + 1;
                
                return Some(CompactionTask {
                    source_level: level,
                    target_level,
                    source_tables,
                    priority: CompactionPriority::Normal,
                });
            }
        }
        None
    }
}
```

#### Week 3-4: Background Tasks & Metrics

```rust
impl OrbitLSMTree {
    pub async fn start_background_tasks(&self) {
        let compaction_handle = self.start_compaction_thread();
        let wal_sync_handle = self.start_wal_sync_thread();  
        let metrics_handle = self.start_metrics_collection();
        
        // Store handles for clean shutdown
        // ...
    }
    
    async fn start_compaction_thread(&self) -> JoinHandle<()> {
        let scheduler = self.compaction_scheduler.clone();
        let sstable_manager = self.sstable_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                if let Some(task) = scheduler.schedule_compaction(&sstable_manager).await {
                    if let Err(e) = Self::execute_compaction(task).await {
                        error!("Compaction failed: {}", e);
                    }
                }
            }
        })
    }
}
```

### Phase 3: Advanced Features (2-3 weeks)

#### Week 1: Point-in-Time Recovery

```rust
pub struct SnapshotManager {
    snapshot_dir: PathBuf,
    retention_policy: SnapshotRetentionPolicy,
}

impl SnapshotManager {
    pub async fn create_snapshot(&self, lsm_tree: &OrbitLSMTree) -> Result<SnapshotId> {
        let snapshot_id = SnapshotId::new();
        let snapshot_path = self.snapshot_dir.join(snapshot_id.to_string());
        
        // 1. Flush memtable to SSTable
        lsm_tree.force_flush().await?;
        
        // 2. Create hard links to current SSTables (atomic snapshot)
        for sstable in lsm_tree.sstable_manager.get_all_tables() {
            let link_path = snapshot_path.join(sstable.file_name());
            std::fs::hard_link(&sstable.file_path, link_path)?;
        }
        
        // 3. Copy current WAL segment
        std::fs::copy(&lsm_tree.wal.current_segment_path(), 
                     snapshot_path.join("wal.log"))?;
        
        // 4. Write snapshot metadata
        let metadata = SnapshotMetadata {
            id: snapshot_id,
            created_at: SystemTime::now(),
            sstable_count: lsm_tree.sstable_manager.table_count(),
            sequence_number: lsm_tree.wal.current_sequence(),
        };
        
        self.write_snapshot_metadata(&snapshot_path, &metadata).await?;
        Ok(snapshot_id)
    }
}
```

#### Week 2-3: Performance Optimization

```rust
// Specialized optimizations for orbit-rs workloads
pub struct OrbitOptimizations {
    // Actor lease optimization - frequently updated small keys
    actor_lease_cache: LruCache<String, AddressableLease>,
    
    // Cluster heartbeat batching - group frequent updates  
    heartbeat_batch_writer: BatchWriter<NodeHeartbeat>,
    
    // OrbitQL result caching - optimize for read-heavy queries
    query_result_bloom_cache: BloomCache<u64>,
}

impl OrbitLSMTree {
    // Optimized write path for actor leases
    pub async fn update_actor_lease(&self, lease: AddressableLease) -> Result<()> {
        // Use specialized compaction for frequent lease updates
        let key = OrbitKey::ActorLease {
            namespace: lease.namespace.clone(),
            actor_id: lease.addressable_reference.key.to_string(),
        };
        
        // Batch small frequent updates
        if lease.is_heartbeat_update() {
            return self.batch_writer.add_lease_update(key, lease).await;
        }
        
        // Regular write path for full updates
        self.put(key, OrbitValue::ActorLease(lease)).await
    }
    
    // Optimized range scan for time series data
    pub async fn scan_timeseries(&self, 
                                metric: &str, 
                                time_range: Range<u64>) -> Result<Vec<TimeSeriesPoint>> {
        let start_key = OrbitKey::TimeSeries { 
            metric: metric.to_string(), 
            timestamp: time_range.start 
        };
        let end_key = OrbitKey::TimeSeries { 
            metric: metric.to_string(), 
            timestamp: time_range.end 
        };
        
        // Use specialized iterator for time series scans
        self.range_scan(start_key..=end_key)
            .map(|(_, value)| match value {
                OrbitValue::TimeSeriesPoint(point) => point,
                _ => unreachable!(),
            })
            .collect()
    }
}
```

## Integration with Existing Persistence Layer

### New LSM Provider Configuration

```rust
// Add to orbit-server/src/persistence/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistenceConfig {
    // ... existing providers ...
    
    /// LSM Tree-based persistent storage
    LSMTree(LSMTreeConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSMTreeConfig {
    pub data_dir: String,
    pub memtable_size_mb: usize,
    pub compaction_strategy: CompactionStrategy,
    pub wal_sync_mode: WALSyncMode,
    pub enable_snapshots: bool,
    pub snapshot_interval_secs: u64,
    pub max_levels: usize,
    pub bloom_filter_enabled: bool,
    pub compression: CompressionType,
}

impl Default for LSMTreeConfig {
    fn default() -> Self {
        Self {
            data_dir: "/var/lib/orbit/lsm".to_string(),
            memtable_size_mb: 64,
            compaction_strategy: CompactionStrategy::Leveled { size_ratio: 10.0 },
            wal_sync_mode: WALSyncMode::PerWrite,
            enable_snapshots: true,
            snapshot_interval_secs: 300,
            max_levels: 7,
            bloom_filter_enabled: true,
            compression: CompressionType::Lz4,
        }
    }
}
```

### Provider Implementation

```rust
// orbit-server/src/persistence/lsm_provider.rs
pub struct LSMTreeProvider {
    lsm_tree: Arc<OrbitLSMTree>,
    config: LSMTreeConfig,
}

#[async_trait]
impl PersistenceProvider for LSMTreeProvider {
    async fn initialize(&mut self) -> Result<()> {
        self.lsm_tree.open(&self.config.data_dir).await?;
        self.lsm_tree.start_background_tasks().await;
        Ok(())
    }
    
    async fn health_check(&self) -> ProviderHealth {
        // Check if LSM tree is accepting writes and compaction is healthy
        // ...
    }
    
    async fn metrics(&self) -> PersistenceMetrics {
        let lsm_metrics = self.lsm_tree.get_metrics().await;
        PersistenceMetrics {
            read_operations: lsm_metrics.reads,
            write_operations: lsm_metrics.writes, 
            read_latency_avg: lsm_metrics.read_latency_p50,
            write_latency_avg: lsm_metrics.write_latency_p50,
            // ... other metrics
        }
    }
}

#[async_trait]
impl AddressableDirectoryProvider for LSMTreeProvider {
    async fn store_lease(&self, lease: &AddressableLease) -> Result<()> {
        self.lsm_tree.update_actor_lease(lease.clone()).await
    }
    
    async fn get_lease(&self, namespace: &str, key: &str) -> Result<Option<AddressableLease>> {
        let orbit_key = OrbitKey::ActorLease {
            namespace: namespace.to_string(),
            actor_id: key.to_string(),
        };
        
        match self.lsm_tree.get(&orbit_key).await? {
            Some(OrbitValue::ActorLease(lease)) => Ok(Some(lease)),
            _ => Ok(None),
        }
    }
    
    // ... other provider methods
}
```

## Performance Characteristics

### Expected Performance Improvements

| Operation | Current (Memory) | Current (S3) | LSM Tree |
|-----------|------------------|--------------|----------|
| Actor lease write | 1μs | 100ms | 10μs |
| Lease read | 0.5μs | 50ms | 2μs |  
| Range scan (1K items) | 100μs | 5s | 1ms |
| Recovery time | N/A | 30s+ | 5s |
| Storage efficiency | 100% | 100% | 60-80% |

### Memory Usage

- **MemTable**: 64MB default (configurable)
- **Block Cache**: 256MB default (configurable)  
- **Bloom Filters**: ~10 bits per key
- **Index**: ~1% of data size

### Disk Usage  

- **WAL**: ~2x write amplification during normal operation
- **SSTables**: 1.2-1.5x storage amplification after compaction
- **Snapshots**: Hard links (minimal extra space)

## Deployment Scenarios

### Scenario 1: High-Performance Development

```toml
[providers.lsm]
type = "LSMTree"
data_dir = "/tmp/orbit-lsm"  
memtable_size_mb = 32
wal_sync_mode = "Batch"        # Better performance
enable_snapshots = false        # Skip snapshots for dev
```

### Scenario 2: Production with High Durability

```toml
[providers.lsm]
type = "LSMTree"  
data_dir = "/var/lib/orbit/lsm"
memtable_size_mb = 128
wal_sync_mode = "PerWrite"     # Maximum durability
enable_snapshots = true
snapshot_interval_secs = 300   # 5-minute snapshots
compaction_strategy = { type = "Leveled", size_ratio = 10.0 }
```

### Scenario 3: Write-Heavy Actor Systems

```toml
[providers.lsm]
type = "LSMTree"
data_dir = "/nvme/orbit/lsm"
memtable_size_mb = 256         # Larger memtables
max_levels = 5                 # Fewer levels = less compaction 
compaction_strategy = { type = "Universal", ratio_threshold = 4.0 }
compression = "Lz4"            # Fast compression
```

## Migration Strategy

### Phase 1: Parallel Operation

1. Deploy LSM provider alongside existing provider
2. Write to both providers (dual-write mode)
3. Compare results and performance metrics
4. Gradually shift read traffic to LSM provider

### Phase 2: Data Migration

```rust
// Migration utility
pub struct LSMMigration {
    source_provider: Arc<dyn PersistenceProvider>,
    target_lsm: Arc<OrbitLSMTree>,
}

impl LSMMigration {
    pub async fn migrate_all_data(&self) -> Result<MigrationStats> {
        let mut stats = MigrationStats::default();
        
        // Migrate actor leases  
        let leases = self.source_provider.list_all_leases().await?;
        for lease in leases {
            self.target_lsm.update_actor_lease(lease).await?;
            stats.leases_migrated += 1;
        }
        
        // Migrate cluster nodes
        let nodes = self.source_provider.list_all_nodes().await?;  
        for node in nodes {
            let key = OrbitKey::ClusterNode { node_id: node.id.clone() };
            self.target_lsm.put(key, OrbitValue::ClusterNode(node)).await?;
            stats.nodes_migrated += 1;
        }
        
        Ok(stats)
    }
}
```

## Testing Strategy

### Unit Tests

- MemTable operations and overflow handling
- WAL write/recovery scenarios  
- SSTable read/write/compaction
- Bloom filter accuracy
- Key serialization/deserialization

### Integration Tests  

- End-to-end write/read cycles
- Compaction correctness
- Crash recovery scenarios
- Performance regression tests
- Multi-threaded access patterns

### Load Testing

- Sustained write loads (actor lease updates)
- Mixed read/write workloads  
- Memory pressure scenarios
- Compaction during heavy load
- Recovery time measurement

## Success Metrics

### Performance Targets

- **Write latency**: <50μs p99 for actor lease updates
- **Read latency**: <10μs p99 for single key lookups
- **Write throughput**: >100K actor lease updates/sec
- **Recovery time**: <10 seconds for 1GB of data
- **Space amplification**: <2x after compaction

### Reliability Targets  

- **Durability**: Zero data loss with PerWrite sync mode
- **Availability**: 99.99% uptime during compaction
- **Recovery**: 100% data recovery from WAL+SSTables
- **Consistency**: All reads return latest committed writes

## Future Enhancements

### Phase 4: Advanced Features (Future)

1. **Multi-Version Concurrency Control (MVCC)**
   - Snapshot isolation for consistent reads
   - Time-travel queries for audit trails

2. **Distributed LSM Trees**
   - Consistent hashing across multiple nodes
   - Cross-node compaction coordination

3. **Adaptive Compaction**
   - Machine learning-based compaction scheduling
   - Workload-aware level sizing

4. **Specialized Data Types**
   - Native time-series compression (Delta-of-Delta, Gorilla)
   - Graph-specific storage layouts
   - Vector embeddings optimization

## Conclusion

Implementing LSM Trees for orbit-rs will transform it from a memory-centric system to a truly durable, high-performance actor platform. The write-optimized nature of LSM Trees perfectly matches orbit-rs's actor lease management patterns, while providing the durability guarantees needed for production deployments.

**Estimated Timeline**: 10-12 weeks for full implementation
**Resource Requirements**: 1-2 senior Rust developers  
**Expected ROI**: 10x write performance improvement + guaranteed durability

This implementation positions orbit-rs as a truly production-ready actor system capable of handling enterprise-scale workloads while maintaining the performance characteristics that make actor systems attractive.
