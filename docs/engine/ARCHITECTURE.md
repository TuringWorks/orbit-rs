# Orbit Engine Architecture

## Table of Contents

1. [Overview](#overview)
2. [Core Components](#core-components)
3. [Storage Layer](#storage-layer)
4. [Transaction Layer](#transaction-layer)
5. [Query Execution](#query-execution)
6. [Clustering](#clustering)
7. [Protocol Adapters](#protocol-adapters)
8. [Data Flow](#data-flow)
9. [Performance Characteristics](#performance-characteristics)
10. [Design Decisions](#design-decisions)

## Overview

Orbit Engine is a unified storage engine designed to support multiple database protocols (PostgreSQL, Redis, OrbitQL, etc.) through a single, consistent backend. It combines:

- **Tiered storage** for optimal cost/performance balance
- **MVCC transactions** for ACID guarantees
- **Distributed clustering** for high availability
- **Vectorized execution** for analytical queries
- **Protocol adapters** for multi-protocol support

### Design Goals

1. **Protocol Independence**: Support any protocol via adapters
2. **Performance**: SIMD-optimized queries, tiered storage
3. **Scalability**: Horizontal scaling via Raft consensus
4. **Durability**: Multi-tier persistence with replication
5. **Developer Experience**: Simple, ergonomic APIs

## Core Components

### Component Hierarchy

```text
┌─────────────────────────────────────────────────────────┐
│  Layer 1: Protocol Layer (User-Facing)                  │
│  - PostgreSQL Wire Protocol                             │
│  - Redis RESP Protocol                                  │
│  - REST HTTP API                                        │
│  - OrbitQL (Multi-Model Query Language)                 │
│  - AQL/Cypher (Future)                                  │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│  Layer 2: Adapter Layer (Protocol Translation)          │
│  - Type Mapping                                         │
│  - Command Translation                                  │
│  - Error Mapping                                        │
│  - Transaction Bridging                                 │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│  Layer 3: Engine Core (Unified Backend)                 │
│                                                         │
│  ┌──────────┐  ┌────────────┐  ┌────────┐  ┌────────┐   │
│  │ Storage  │  │Transaction │  │ Query  │  │Cluster │   │
│  │          │  │            │  │        │  │        │   │
│  │ Hot      │  │ MVCC       │  │ Vector │  │ Raft   │   │
│  │ Warm     │  │ 2PC        │  │ SIMD   │  │ Replic │   │
│  │ Cold     │  │ Locks      │  │ Optim  │  │ CDC    │   │
│  └──────────┘  └────────────┘  └────────┘  └────────┘   │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│  Layer 4: Storage Backends (Physical Storage)           │
│  - Memory (Hot)                                         │
│  - RocksDB (Warm)                                       │
│  - S3/Azure/MinIO (Cold)                                │
└─────────────────────────────────────────────────────────┘
```

### Module Organization

```text
orbit/engine/src/
├── adapters/         # Protocol adapters
│   ├── postgres.rs   # PostgreSQL adapter
│   ├── redis.rs      # Redis adapter
│   ├── rest.rs       # REST API adapter
│   └── orbitql.rs    # OrbitQL adapter (multi-model queries)
├── storage/          # Storage layer
│   ├── columnar.rs   # Columnar format
│   ├── hybrid.rs     # Tiered storage manager
│   ├── iceberg.rs    # Cold tier (Iceberg)
│   └── config.rs     # Storage configuration
├── transaction/      # Transaction management
│   └── mvcc.rs       # MVCC implementation
├── transactions/     # Distributed transactions
│   ├── core.rs       # Transaction coordinator
│   ├── locks.rs      # Distributed locks
│   └── security.rs   # Auth/audit
├── cluster/          # Clustering
│   ├── consensus.rs  # Raft consensus
│   ├── manager.rs    # Cluster management
│   ├── recovery.rs   # Recovery
│   └── replication.rs# Replication
├── query/            # Query execution
│   ├── execution.rs  # Vectorized executor
│   └── simd/         # SIMD optimizations
├── cdc.rs            # Change Data Capture
├── transaction_log.rs# Transaction logging
└── error.rs          # Error types
```

## Storage Layer

### Tiered Storage Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                    Hot Tier (Memory)                    │
│  - Last 24-48 hours                                     │
│  - Row-based format                                     │
│  - OLTP optimized                                       │
│  - <1ms latency                                         │
│  - HashMap-based indexing                               │
└─────────────────────────────────────────────────────────┘
              ↓ (Age-based migration)
┌─────────────────────────────────────────────────────────┐
│                   Warm Tier (RocksDB)                   │
│  - 2-30 days                                            │
│  - Hybrid format                                        │
│  - Mixed workload                                       │
│  - <10ms latency                                        │
│  - LSM-tree storage                                     │
└─────────────────────────────────────────────────────────┘
              ↓ (Age-based migration)
┌─────────────────────────────────────────────────────────┐
│              Cold Tier (Iceberg/Parquet)                │
│  - >30 days                                             │
│  - Columnar format                                      │
│  - Analytics optimized                                  │
│  - 10-100ms latency                                     │
│  - Object storage (S3/Azure)                            │
│  - ZSTD compression (~9x ratio)                         │
└─────────────────────────────────────────────────────────┘
```

### Storage Traits

```rust
// Base storage engine trait
pub trait StorageEngine {
    async fn initialize(&self) -> EngineResult<()>;
    async fn shutdown(&self) -> EngineResult<()>;
    async fn metrics(&self) -> StorageMetrics;
}

// Table-level operations
pub trait TableStorage: StorageEngine {
    async fn create_table(&self, schema: TableSchema) -> EngineResult<()>;
    async fn insert_row(&self, table: &str, row: Row) -> EngineResult<()>;
    async fn query(&self, table: &str, pattern: AccessPattern)
        -> EngineResult<QueryResult>;
    async fn update(&self, table: &str, filter: FilterPredicate,
        updates: HashMap<String, SqlValue>) -> EngineResult<usize>;
    async fn delete(&self, table: &str, filter: FilterPredicate)
        -> EngineResult<usize>;
}

// Tiered storage operations
pub trait TieredStorage: TableStorage {
    async fn migrate_tier(&self, table: &str, from: StorageTier,
        to: StorageTier) -> EngineResult<MigrationStats>;
    async fn query_tier(&self, table: &str, tier: StorageTier,
        pattern: AccessPattern) -> EngineResult<QueryResult>;
}
```

### Data Migration Strategy

**Automatic Migration**:

- Background process evaluates data age
- Migrates data between tiers based on configured thresholds
- Uses timestamp columns for age determination
- Batch migration to minimize overhead

**Manual Migration**:

- API for explicit tier migration
- Useful for data lifecycle policies
- Supports partial migrations with filters

## Transaction Layer

### MVCC (Multi-Version Concurrency Control)

```text
Transaction Timeline:

T1: BEGIN (snapshot_id=100)
    │
    ├─ Read row X (sees version with xmin<100, xmax>100)
    │
T2: BEGIN (snapshot_id=101)
    │
    ├─ Update row X (creates new version: xmin=101, xmax=∞)
    │
T1: ├─ Read row X (still sees old version: xmin<100)
    │
T2: ├─ COMMIT (version xmin=101 becomes visible to new txns)
    │
T1: ├─ Read row X (still sees old version: snapshot isolation)
    │
    └─ COMMIT

T3: BEGIN (snapshot_id=102)
    └─ Read row X (sees new version: xmin=101 < snapshot_id=102)
```

### Row Versioning

```rust
pub struct RowVersion {
    pub data: HashMap<String, SqlValue>,
    pub xmin: TransactionId,  // Creating transaction
    pub xmax: Option<TransactionId>,  // Deleting transaction
    pub created_at: DateTime<Utc>,
    pub committed: bool,
}

// Visibility rules
fn is_visible(version: &RowVersion, snapshot: SnapshotId) -> bool {
    version.committed
        && version.xmin < snapshot
        && (version.xmax.is_none() || version.xmax.unwrap() > snapshot)
}
```

### Distributed Transactions (2PC)

```text
Coordinator                    Participant A              Participant B
    │                              │                          │
    ├─ BEGIN                       │                          │
    ├─ Prepare ────────────────────┼──────────────────────────┤
    │                              │                          │
    │                          PREPARE                    PREPARE
    │                              │                          │
    │                          Vote YES                   Vote YES
    │  ◄─────────────────. ────────┼───────────────────--─────┤
    │                              │                          │
    ├─ Decision: COMMIT            │                          │
    ├─ Commit ─────────────────────┼──────────────────────────┤
    │                              │                          │
    │                          COMMIT                     COMMIT
    │  ◄─────────────────────--────┼───────────────────--─────┤
    │                              │                          │
    ├─ DONE                        │                          │
```

### Deadlock Detection

```rust
pub struct DeadlockDetector {
    // Wait-for graph: transaction -> waiting for transaction
    wait_graph: Arc<RwLock<HashMap<TransactionId, HashSet<TransactionId>>>>,
}

impl DeadlockDetector {
    // Detect cycles using DFS
    pub fn detect_deadlock(&self, tx_id: TransactionId)
        -> Option<Vec<TransactionId>> {
        // Returns cycle if deadlock detected
    }

    // Resolve by aborting youngest transaction
    pub fn resolve_deadlock(&self, cycle: Vec<TransactionId>)
        -> TransactionId {
        // Returns transaction to abort
    }
}
```

## Query Execution

### Vectorized Execution

```text
Traditional Row-at-a-Time:
┌─────┐    ┌───-──┐   ┌─────┐
│ Row │ →  │Filter│ → │ Agg │
└─────┘    └────-─┘   └─────┘
  1 row      1 row     1 row

Vectorized Batch-at-a-Time:
┌──────────┐    ┌──────────┐    ┌──────────┐
│ Batch    │ →  │ Filter   │ →  │   Agg    │
│ 1024 rows│    │ 1024 rows│    │ 1024 rows│
└──────────┘    └──────────┘    └──────────┘
```

### SIMD Optimization

```rust
// Scalar (1 comparison at a time)
for i in 0..values.len() {
    if values[i] > threshold {
        results.push(i);
    }
}

// SIMD (8 comparisons at a time with AVX2)
for chunk in values.chunks(8) {
    let vec = _mm256_loadu_si256(chunk);
    let threshold_vec = _mm256_set1_epi32(threshold);
    let mask = _mm256_cmpgt_epi32(vec, threshold_vec);
    // Process mask to extract matching indices
}
```

**Performance Gains**:

- 5-10x faster aggregations
- 3-5x faster filters
- 2-3x better compression

### Columnar Format

```text
Row-Based Storage:
┌────┬──────┬───────┐
│ id │ name │ price │
├────┼──────┼───────┤
│ 1  │ A    │ 10.0  │
│ 2  │ B    │ 20.0  │
│ 3  │ C    │ 15.0  │
└────┴──────┴───────┘
[1,A,10.0][2,B,20.0][3,C,15.0]

Columnar Storage:
┌────┬────┬────┐
│ id │ id │ id │
├────┼────┼────┤
│ 1  │ 2  │ 3  │
└────┴────┴────┘
[1,2,3]

┌──────┬──────┬──────┐
│ name │ name │ name │
├──────┼──────┼──────┤
│ A    │ B    │ C    │
└──────┴──────┴──────┘
[A,B,C]

┌───────┬───────┬───────┐
│ price │ price │ price │
├───────┼───────┼───────┤
│ 10.0  │ 20.0  │ 15.0  │
└───────┴───────┴───────┘
[10.0,20.0,15.0]
```

**Benefits**:

- Better compression (similar values together)
- Cache-friendly for column scans
- Skip irrelevant columns
- SIMD-friendly contiguous data

## Clustering

### Raft Consensus

```text
Leader Election:

Node A (Leader)     Node B (Follower)   Node C (Follower)
    │                      │                    │
    ├─ Heartbeat ──────────┼────────────────────┤
    │  (term=5)            │                    │
    │                      │                    │
    │                   (timeout)               │
    │                      │                    │
    │                  RequestVote              │
    │  ◄───────────────────┤                    │
    │                  (term=6)                 │
    │                      │                    │
    ├─ Vote Granted ───────┤                    │
    │                      │                    │
    │                      ├─ RequestVote ──────┤
    │                      │   (term=6)         │
    │                      │                    │
    │                      │  Vote Granted ─────┤
    │                      │                    │
    │                 (becomes leader)          │
```

### Replication

```text
Write Path with Replication:

Client
  │
  ├─ Write Request
  │
  ▼
Leader (Node A)
  │
  ├─ 1. Write to local log
  ├─ 2. Replicate to followers
  │     │
  │     ├─────────────────┬─────────────────┐
  │     ▼                 ▼                 ▼
  │  Node B           Node C           Node D
  │     │                 │                 │
  │     ├─ Write log      ├─ Write log      ├─ Write log
  │     ├─ ACK            ├─ ACK            ├─ ACK
  │     │                 │                 │
  │  ◄──┴─────────────────┴─────────────────┘
  │
  ├─ 3. Wait for quorum (2 of 3)
  ├─ 4. Commit
  │
  ▼
Response to Client
```

### Change Data Capture (CDC)

```rust
pub enum CdcEvent {
    Insert { table: String, row: Row },
    Update { table: String, old: Row, new: Row },
    Delete { table: String, row: Row },
    Ddl { statement: String },
}

// Subscribe to changes
let mut stream = cdc.subscribe("users", CdcFilter::All).await?;
while let Some(event) = stream.next().await {
    match event {
        CdcEvent::Insert { table, row } => {
            // Handle insert
        }
        _ => {}
    }
}
```

## Protocol Adapters

### OrbitQL - Multi-Model Query Language

**OrbitQL** is a unified query language that combines document, graph, time-series, and key-value operations in a single query. It's designed to access all data stored in orbit-engine across hot/warm/cold tiers.

```text
┌─────────────────────────────────────────────────────────────┐
│                    OrbitQL Query Layer                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │  Parser  │→│ Optimizer │→│  Planner  │→│ Executor  │     │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘     │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                   OrbitQL Adapter                           │
│  - Type Mapping (QueryValue ↔ SqlValue)                     │
│  - Filter Conversion (Expression → FilterPredicate)         │
│  - Multi-Model Support (Document/Graph/Time-Series)         │
│  - Tier-Aware Queries (Hot/Warm/Cold)                       │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                   Unified Storage Engine                    │
│  Storage (Hot/Warm/Cold) | Transactions | Clustering        │
└─────────────────────────────────────────────────────────────┘
```

**Key Features:**

- **Multi-Model Queries**: Query documents, graphs, and time-series in one query
- **Cross-Model JOINs**: Relate data between different models seamlessly
- **Tiered Storage Aware**: Automatically accesses hot/warm/cold tiers
- **Live Queries**: Real-time subscriptions with change notifications
- **ACID Transactions**: Multi-model transaction support

**Example - Document Query:**

```orbitql
SELECT * FROM users WHERE age > 18 ORDER BY created_at DESC LIMIT 10;
```

**Example - Graph Traversal:**

```orbitql
SELECT user->follows->user.name AS friends FROM users WHERE user.id = 123;
```

**Example - Time-Series Analytics:**

```orbitql
SELECT
    server_id,
    AVG(metrics[cpu_usage WHERE timestamp > NOW() - 1h]) AS avg_cpu
FROM servers
GROUP BY server_id;
```

**Example - Cross-Model JOIN:**

```orbitql
SELECT
    u.name,
    u->follows->user.name AS friends,
    AVG(m.cpu_usage) AS avg_cpu
FROM users AS u
JOIN metrics AS m ON u.server_id = m.server_id
WHERE m.timestamp > NOW() - 1h
GROUP BY u.id;
```

**Integration:**

```rust
use orbit_engine::adapters::{AdapterContext, OrbitQLAdapter};
use orbit_engine::storage::HybridStorageManager;

let storage = Arc::new(HybridStorageManager::new_in_memory());
let context = AdapterContext::new(storage as Arc<dyn TableStorage>);
let adapter = OrbitQLAdapter::new(context);

// Execute OrbitQL query
let result = adapter.execute_query(
    "SELECT * FROM users WHERE age > 18"
).await?;
```

**See Also:**

- [OrbitQL Integration Guide](ORBITQL.md) - Complete syntax and examples
- [orbitql_example.rs](../examples/orbitql_example.rs) - Working examples

### Adapter Pattern

```rust
pub trait ProtocolAdapter {
    fn protocol_name(&self) -> &'static str;
    async fn initialize(&mut self) -> EngineResult<()>;
    async fn shutdown(&mut self) -> EngineResult<()>;
}

// PostgreSQL adapter
impl PostgresAdapter {
    pub async fn select(&self, table: &str, filter: PostgresFilter)
        -> EngineResult<CommandResult> {
        // 1. Convert PostgreSQL filter to engine FilterPredicate
        let predicate = filter.to_engine_filter_predicate();

        // 2. Create engine AccessPattern
        let pattern = AccessPattern::Scan {
            time_range: None,
            filter: Some(predicate),
        };

        // 3. Execute via engine
        let result = self.context.storage.query(table, pattern).await?;

        // 4. Convert engine result to protocol result
        Ok(CommandResult::from_query_result(result))
    }
}
```

### Type Mapping

| Protocol Type | Engine Type | Notes |
|--------------|-------------|-------|
| PostgreSQL INTEGER | SqlValue::Int32 | 4 bytes |
| PostgreSQL BIGINT | SqlValue::Int64 | 8 bytes |
| PostgreSQL TEXT | SqlValue::String | Variable |
| Redis String | SqlValue::String | Stored in redis_strings table |
| Redis Hash | Multiple rows | Stored in redis_hashes table |
| REST JSON Number | SqlValue::Int64 or Float64 | Based on decimal point |
| OrbitQL STRING | SqlValue::String | UTF-8 text |
| OrbitQL INTEGER | SqlValue::Int64 | 64-bit signed |
| OrbitQL FLOAT | SqlValue::Float64 | IEEE 754 |
| OrbitQL BOOLEAN | SqlValue::Boolean | 1 byte |
| OrbitQL TIMESTAMP | SqlValue::Timestamp | Unix epoch |

## Data Flow

### Write Path

```text
1. Client Request
   ├─ PostgreSQL: INSERT statement
   ├─ Redis: SET command
   ├─ REST: POST /tables/{name}/rows
   └─ OrbitQL: INSERT INTO users VALUES (...)

2. Protocol Adapter
   ├─ Parse request
   ├─ Validate types
   └─ Convert to engine Row

3. Transaction Layer
   ├─ Assign transaction ID
   ├─ Check constraints
   └─ Acquire locks

4. Storage Layer
   ├─ Write to hot tier (memory)
   ├─ Update indexes
   └─ Create MVCC version

5. Replication (if clustered)
   ├─ Replicate to followers
   ├─ Wait for quorum
   └─ Commit

6. Response
   └─ Return success/error to client
```

### Read Path

```text
1. Client Query
   ├─ PostgreSQL: SELECT with WHERE
   ├─ Redis: GET/HGET
   ├─ REST: GET /tables/{name}/rows?filter=...
   └─ OrbitQL: SELECT * FROM users WHERE age > 18

2. Protocol Adapter
   ├─ Parse query
   └─ Convert to AccessPattern

3. Query Optimizer
   ├─ Choose execution strategy
   ├─ Select tier(s) to query
   └─ Build execution plan

4. Storage Layer
   ├─ Query hot tier first
   ├─ Fallback to warm/cold if needed
   └─ Apply MVCC visibility rules

5. Query Executor
   ├─ Vectorized execution
   ├─ SIMD filters/aggregations
   └─ Columnar format in cold tier

6. Protocol Adapter
   ├─ Convert QueryResult to protocol format
   └─ Apply protocol-specific formatting

7. Response
   └─ Return results to client
```

## Performance Characteristics

### Latency by Tier

| Tier | Storage | P50 | P95 | P99 |
|------|---------|-----|-----|-----|
| Hot | Memory | 0.5ms | 1ms | 2ms |
| Warm | RocksDB | 3ms | 8ms | 15ms |
| Cold | S3/Azure | 30ms | 80ms | 150ms |

### Throughput

| Operation | Hot Tier | Warm Tier | Cold Tier |
|-----------|----------|-----------|-----------|
| Point Read | 100K ops/s | 50K ops/s | 1K ops/s |
| Point Write | 80K ops/s | 40K ops/s | N/A (async) |
| Range Scan | 50K rows/s | 30K rows/s | 100K rows/s* |
| Aggregation | 1M rows/s* | 500K rows/s* | 5M rows/s* |

*With SIMD optimization

### Storage Efficiency

| Format | Compression Ratio | Random Access | Scan Speed |
|--------|------------------|---------------|------------|
| Row (Hot) | 1x | Excellent | Good |
| Hybrid (Warm) | 2-3x | Good | Good |
| Columnar (Cold) | 8-10x | Poor | Excellent |

## Design Decisions

### Why Tiered Storage?

**Decision**: Implement automatic tiered storage (hot/warm/cold)

**Rationale**:

- Recent data needs low latency (hot tier)
- Historical data accessed less frequently (cold tier)
- Cost optimization: memory expensive, object storage cheap
- Performance optimization: different formats for different workloads

**Trade-offs**:

- ✅ Lower storage costs (10-100x cheaper for cold tier)
- ✅ Better query performance (SIMD on columnar)
- ❌ Additional complexity in migration logic
- ❌ Potential inconsistency during migration

### Why MVCC over Locking?

**Decision**: Use Multi-Version Concurrency Control

**Rationale**:

- Readers never block writers
- Writers never block readers
- Snapshot isolation provides consistency
- Better concurrency than 2PL (Two-Phase Locking)

**Trade-offs**:

- ✅ Higher concurrency
- ✅ No deadlocks between readers/writers
- ❌ Higher storage overhead (multiple versions)
- ❌ Garbage collection needed for old versions

### Why Protocol Adapters?

**Decision**: Separate adapter layer instead of native multi-protocol

**Rationale**:

- Clean separation of concerns
- Easy to add new protocols
- Unified engine optimizations benefit all protocols
- Shared transaction/storage layer

**Trade-offs**:

- ✅ Single engine to maintain
- ✅ Consistent behavior across protocols
- ✅ Easy protocol extensions
- ❌ Small overhead in type conversion
- ❌ Protocol-specific features may be limited

### Why Raft over Paxos?

**Decision**: Use Raft for distributed consensus

**Rationale**:

- Easier to understand and implement
- Strong leader simplifies design
- Well-tested implementations available
- Better debugging and monitoring

**Trade-offs**:

- ✅ Simpler to reason about
- ✅ Better tooling and documentation
- ❌ Slightly lower throughput than Multi-Paxos
- ❌ Leader bottleneck for writes

### Why Iceberg for Cold Tier?

**Decision**: Use Apache Iceberg for cold tier table format

**Rationale**:

- Schema evolution without rewriting data
- Time travel queries (snapshot isolation)
- Hidden partitioning (optimized queries)
- Multi-engine compatibility (Spark, Trino)
- Metadata-based query optimization

**Trade-offs**:

- ✅ Production-proven at scale
- ✅ Advanced features (time travel, schema evolution)
- ✅ Ecosystem integration
- ❌ Additional dependency
- ❌ Learning curve

---

**Last Updated**: 2025-11-18
**Version**: 1.0
