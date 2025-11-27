# Protocol-Specific Persistence Tuning Guide

This guide provides detailed tuning recommendations for optimizing persistence performance across all Orbit-RS protocol implementations.

## Table of Contents

1. [Overview](#overview)
2. [AQL/ArangoDB Protocol](#aql-arangodb-protocol)
3. [Cypher/Neo4j Protocol](#cypher-neo4j-protocol)
4. [GraphRAG Protocol](#graphrag-protocol)
5. [PostgreSQL Wire Protocol](#postgresql-wire-protocol)
6. [General RocksDB Tuning](#general-rocksdb-tuning)
7. [Monitoring and Profiling](#monitoring-and-profiling)

---

## Overview

All protocol implementations in Orbit-RS use RocksDB as their persistence layer. Each protocol has unique data access patterns and storage requirements that benefit from specific tuning approaches.

### Common Storage Architecture

```
orbit-data/
├── aql/
│   └── rocksdb/           # AQL storage
├── cypher/
│   └── rocksdb/           # Cypher storage
├── graphrag/
│   └── rocksdb/           # GraphRAG storage
└── postgres/
    └── rocksdb/           # PostgreSQL storage
```

---

## AQL/ArangoDB Protocol

### Column Families

The AQL protocol uses the following column families:

- `collections` - Collection metadata
- `documents` - Document data
- `edges` - Graph edge data
- `graphs` - Graph metadata
- `metadata` - System metadata

### Performance Characteristics

**Read-Heavy Workloads:**
- Document queries with filters
- Collection scans
- Graph traversals

**Write-Heavy Workloads:**
- Bulk document inserts
- Document updates
- Edge creation

### Tuning Recommendations

#### 1. Block Cache Size

For document-heavy workloads, increase block cache:

```rust
// In storage initialization
let mut opts = Options::default();
opts.set_block_cache_size(512 * 1024 * 1024); // 512 MB
```

**Recommendation:**
- Small datasets (<10GB): 256 MB
- Medium datasets (10-100GB): 512 MB - 1 GB
- Large datasets (>100GB): 2 GB - 4 GB

#### 2. Write Buffer Size

For high insert rates:

```rust
opts.set_write_buffer_size(128 * 1024 * 1024); // 128 MB
opts.set_max_write_buffer_number(4);
opts.set_min_write_buffer_number_to_merge(2);
```

**Recommendation:**
- Low write rate (<1K docs/sec): 64 MB, 2 buffers
- Medium write rate (1K-10K docs/sec): 128 MB, 4 buffers
- High write rate (>10K docs/sec): 256 MB, 6 buffers

#### 3. Compression

For document storage with JSON data:

```rust
use rocksdb::DBCompressionType;
opts.set_compression_type(DBCompressionType::Lz4);
```

**Compression Options:**
- `Lz4` - Fast, moderate compression (recommended for most cases)
- `Zstd` - Better compression, slower (for archival data)
- `Snappy` - Fastest, least compression (for hot data)

#### 4. Bloom Filters

Enable bloom filters for document lookups:

```rust
let mut cf_opts = Options::default();
cf_opts.set_bloom_filter(10, false); // 10 bits per key
```

#### 5. Index Optimization

For filtered queries:

```rust
// Enable prefix extraction for collection-based queries
opts.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(8));
```

### Workload-Specific Tuning

#### Document-Heavy Analytics

```rust
// Optimize for large scans
opts.set_max_open_files(1000);
opts.set_keep_log_file_num(10);
opts.increase_parallelism(num_cpus::get() as i32);
```

#### Real-Time CRUD Operations

```rust
// Optimize for low latency
opts.set_write_buffer_size(64 * 1024 * 1024);
opts.set_target_file_size_base(64 * 1024 * 1024);
opts.set_level_zero_file_num_compaction_trigger(4);
```

---

## Cypher/Neo4j Protocol

### Column Families

- `nodes` - Graph nodes with properties
- `relationships` - Graph edges with properties
- `metadata` - Graph metadata

### Performance Characteristics

**Graph Traversal Patterns:**
- Multi-hop traversals
- Pattern matching
- Shortest path queries

**Optimization Focus:**
- Node/relationship lookup speed
- Cache hit rate for hot nodes
- Efficient relationship indexing

### Tuning Recommendations

#### 1. Node Cache Optimization

Cypher workloads benefit from aggressive caching:

```rust
// Larger block cache for frequently accessed nodes
opts.set_block_cache_size(1024 * 1024 * 1024); // 1 GB
opts.optimize_for_point_lookup(512); // 512 MB block cache
```

#### 2. Relationship Indexing

For traversal-heavy workloads:

```rust
// Enable prefix bloom for relationship lookups
let mut rel_cf_opts = Options::default();
rel_cf_opts.set_prefix_extractor(
    rocksdb::SliceTransform::create_fixed_prefix(16)
);
rel_cf_opts.set_bloom_filter(10, false);
```

#### 3. Compaction Strategy

For graph data:

```rust
opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
opts.set_level_compaction_dynamic_level_bytes(true);
opts.set_max_background_jobs(4);
```

#### 4. Memory Management

For large graphs:

```rust
// Limit memory usage
opts.set_max_total_wal_size(256 * 1024 * 1024);
opts.set_db_write_buffer_size(512 * 1024 * 1024);
```

### Graph Size Recommendations

| Graph Size | Block Cache | Write Buffer | Max Open Files |
|------------|-------------|--------------|----------------|
| Small (<1M nodes) | 256 MB | 64 MB | 100 |
| Medium (1M-10M nodes) | 1 GB | 128 MB | 500 |
| Large (>10M nodes) | 2-4 GB | 256 MB | 1000 |

---

## GraphRAG Protocol

### Column Families

- `nodes` - Entity nodes with embeddings
- `relationships` - Entity relationships
- `embeddings` - Vector embeddings index
- `metadata` - Knowledge graph metadata

### Performance Characteristics

**Unique Requirements:**
- Large embedding vectors (768-1536 dimensions)
- Semantic search operations
- Entity extraction and linking

### Tuning Recommendations

#### 1. Embedding Storage Optimization

Embeddings are large binary blobs:

```rust
// Disable compression for embeddings CF
let mut emb_cf_opts = Options::default();
emb_cf_opts.set_compression_type(DBCompressionType::None);
emb_cf_opts.set_block_cache_size(2048 * 1024 * 1024); // 2 GB
```

#### 2. Write-Ahead Log (WAL)

For entity extraction workloads:

```rust
opts.set_wal_size_limit_mb(512);
opts.set_wal_ttl_seconds(3600); // 1 hour
opts.set_manual_wal_flush(false); // Auto flush
```

#### 3. Bulk Loading

For initial knowledge graph construction:

```rust
// Disable WAL during bulk load
opts.set_disable_auto_compactions(true);
opts.set_write_buffer_size(512 * 1024 * 1024);
opts.set_max_write_buffer_number(6);

// After bulk load:
// db.compact_range(None::<&[u8]>, None::<&[u8]>);
// opts.set_disable_auto_compactions(false);
```

#### 4. Vector Search Optimization

For embedding-based queries:

```rust
// Optimize for large value reads
opts.set_block_size(32 * 1024); // 32 KB blocks
opts.set_cache_index_and_filter_blocks(true);
```

### Knowledge Graph Size Recommendations

| KG Size | Embedding Cache | Entity Cache | Relationship Cache |
|---------|-----------------|--------------|---------------------|
| Small (<100K entities) | 512 MB | 256 MB | 128 MB |
| Medium (100K-1M entities) | 2 GB | 512 MB | 256 MB |
| Large (>1M entities) | 4-8 GB | 1 GB | 512 MB |

---

## PostgreSQL Wire Protocol

### Column Families

- `tables` - Table metadata
- `rows` - Row data
- `indexes` - Index data
- `jsonb` - JSONB document storage

### Tuning Recommendations

#### 1. JSONB Storage

For JSONB-heavy workloads:

```rust
let mut jsonb_cf_opts = Options::default();
jsonb_cf_opts.set_compression_type(DBCompressionType::Zstd);
jsonb_cf_opts.set_zstd_max_train_bytes(100 * 1024 * 1024); // 100 MB
```

#### 2. Index Performance

For indexed queries:

```rust
let mut index_cf_opts = Options::default();
index_cf_opts.set_bloom_filter(10, false);
index_cf_opts.set_optimize_filters_for_hits(true);
```

#### 3. Transaction Support

For ACID compliance:

```rust
opts.set_atomic_flush(true);
opts.set_enable_pipelined_write(false);
```

---

## General RocksDB Tuning

### CPU and Threading

```rust
// Match CPU core count
opts.increase_parallelism(num_cpus::get() as i32);
opts.set_max_background_jobs(num_cpus::get() as i32);
```

### SSD vs HDD

**SSD Configuration:**
```rust
opts.set_use_direct_io_for_flush_and_compaction(true);
opts.set_use_direct_reads(true);
opts.set_compaction_readahead_size(2 * 1024 * 1024); // 2 MB
```

**HDD Configuration:**
```rust
opts.set_compaction_readahead_size(8 * 1024 * 1024); // 8 MB
opts.set_level_zero_slowdown_writes_trigger(20);
opts.set_level_zero_stop_writes_trigger(30);
```

### Memory Budget

Total memory budget should be distributed:
- 50% - Block cache
- 25% - Write buffers
- 15% - OS page cache
- 10% - Application overhead

Example for 8 GB system:
```rust
opts.set_block_cache_size(4 * 1024 * 1024 * 1024); // 4 GB
opts.set_write_buffer_size(256 * 1024 * 1024); // 256 MB
opts.set_max_write_buffer_number(8); // 2 GB total
```

---

## Monitoring and Profiling

### Key Metrics to Monitor

1. **Cache Hit Rate**
   - Target: >90% for read-heavy workloads
   - Monitor: `rocksdb.block.cache.hit` / `rocksdb.block.cache.miss`

2. **Write Stalls**
   - Target: <1% of write operations
   - Monitor: `rocksdb.stall.micros`

3. **Compaction Stats**
   - Monitor: `rocksdb.compaction.times.micros`
   - Check: Compaction shouldn't block writes

4. **Memory Usage**
   - Monitor: Block cache + write buffers + memtables
   - Stay within budget

### Performance Testing

```bash
# Test read performance
rocksdb_ldb --db=/path/to/rocksdb scan --max_keys=100000

# Test write performance
db_bench --benchmarks=fillseq --num=1000000

# Check database stats
rocksdb_ldb --db=/path/to/rocksdb dump_live_files
```

### Profiling Tools

1. **RocksDB Statistics**
```rust
opts.enable_statistics();
// Periodically check: db.get_statistics()
```

2. **Flame Graphs**
```bash
cargo flamegraph --bin orbit-server
```

3. **Perf Analysis**
```bash
perf record -g target/release/orbit-server
perf report
```

---

## Quick Reference: Common Scenarios

### High Read Throughput
- Increase block cache: 2-4 GB
- Enable bloom filters
- Use Lz4 compression
- Set parallelism to CPU count

### High Write Throughput
- Increase write buffers: 256 MB x 6
- Disable compression temporarily
- Increase level 0 file trigger
- Use direct I/O on SSD

### Low Latency
- Smaller write buffers: 64 MB
- Aggressive compaction: level 0 trigger = 4
- SSD with direct I/O
- Keep working set in cache

### Large Dataset (>100 GB)
- Leveled compaction with dynamic sizing
- Large block cache: 4+ GB
- High parallelism
- Monitor compaction carefully

---

## References

- [RocksDB Tuning Guide](https://github.com/facebook/rocksdb/wiki/RocksDB-Tuning-Guide)
- [RocksDB FAQ](https://github.com/facebook/rocksdb/wiki/RocksDB-FAQ)
- Orbit-RS Protocol Storage Documentation: `docs/STORAGE_ARCHITECTURE_CURRENT.md`
