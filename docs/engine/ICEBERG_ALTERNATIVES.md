# Apache Iceberg Implementation Alternatives for Time Travel

This document evaluates alternative Iceberg implementations in Rust for enabling full time travel functionality in orbit-engine.

## Current Status

**orbit-engine** currently uses `iceberg-rust` (apache/iceberg-rust) which has the following limitations:

- ❌ No `TableMetadata::snapshots()` method to list historical snapshots
- ❌ No `TableMetadata::snapshot_by_timestamp()` for time-based queries
- ❌ No `TableMetadata::snapshot_by_id()` for version-based queries
- ❌ No tag/branch management API
- ✅ Basic table read/write operations work
- ✅ REST catalog support functional

**Impact**: Time travel queries fall back to current snapshot only.

## Alternative Rust-Based Implementations

### 1. **Icelake** - Complete Rust Iceberg Implementation

**Repository**: https://github.com/icelake-io/icelake

**Description**: A complete, native Rust implementation of Apache Iceberg table format. Used in production by Databend.

#### Pros
- ✅ **Native Rust**: No FFI overhead, better integration with Rust ecosystem
- ✅ **Complete Implementation**: Full support for Iceberg spec including snapshots
- ✅ **Production-Ready**: Used by Databend (100K+ stars on GitHub)
- ✅ **Active Development**: Regular commits and updates
- ✅ **Snapshot Support**: Likely has full snapshot/history API
- ✅ **Apache Arrow Integration**: Built-in Arrow support for high performance
- ✅ **Time Travel**: Designed for time travel queries from the ground up

#### Cons
- ⚠️ Less mature than Apache's official iceberg-rust
- ⚠️ Smaller community (fewer contributors)
- ⚠️ May have API differences from official spec

#### API Comparison

```rust
// Potential Icelake API (needs verification)
use icelake::{Table, TableIdentifier};

// Load table
let table = catalog.load_table(&table_ident).await?;

// Access snapshots - LIKELY AVAILABLE
let snapshots = table.snapshots(); // Returns Vec<Snapshot>
let snapshot_at_time = table.snapshot_at_timestamp(timestamp_ms)?;
let snapshot_by_id = table.snapshot_by_id(snapshot_id)?;

// Time travel query
let scan = table.scan()
    .with_snapshot_id(snapshot_id)
    .build()?;
```

**Recommendation**: ⭐⭐⭐⭐⭐ **HIGHLY RECOMMENDED** for evaluation
- Strong candidate for replacing iceberg-rust
- Production-proven by Databend
- Likely has the snapshot APIs we need

---

### 2. **Lakekeeper** - Rust-Native Iceberg REST Catalog

**Repository**: https://github.com/lakekeeper/lakekeeper

**Description**: A Rust-native implementation of the Iceberg REST catalog specification. Acts as middleware between query engines and data storage.

#### Pros
- ✅ **Catalog Management**: Full REST catalog implementation
- ✅ **Multi-Engine Support**: Works with Trino, StarRocks, Dremio, Spark
- ✅ **Metadata Server**: Centralized metadata management
- ✅ **Rust-Native**: High performance, low resource usage
- ✅ **Active Development**: Recently created, actively maintained

#### Cons
- ⚠️ **Not a Table Format Library**: Catalog layer only, not table read/write
- ⚠️ Requires separate query engine for actual data access
- ⚠️ Adds architectural complexity (separate catalog service)

#### Architecture with Lakekeeper

```text
┌─────────────────┐
│  Orbit Engine   │
│   (Protocols)   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     REST API     ┌─────────────────┐
│   Lakekeeper    │◄─────────────────┤  Query Engine   │
│  (Catalog Mgmt) │                  │ (Trino/Spark)   │
└────────┬────────┘                  └─────────────────┘
         │
         ▼
┌─────────────────┐
│  Data Storage   │
│  (S3/MinIO/     │
│   Azure Blob)   │
└─────────────────┘
```

**Use Case**:
- Better for multi-engine deployments
- Not ideal for embedded Iceberg access in orbit-engine
- Useful if we want centralized catalog management across multiple services

**Recommendation**: ⭐⭐⭐ **Consider for Catalog Layer**
- Great for catalog management
- Not a replacement for iceberg-rust (different layer)
- Could be used *alongside* Icelake or iceberg-rust

---

### 3. **Query Engine Integration** (Dremio/Trino/StarRocks)

**Approach**: Use orbit-engine as a protocol adapter layer, delegate Iceberg queries to external query engines.

#### Option A: Trino Integration

```rust
// Orbit Engine → Trino → Iceberg
pub struct TrinoIcebergAdapter {
    trino_client: TrinoClient,
}

impl TrinoIcebergAdapter {
    pub async fn query_with_time_travel(
        &self,
        table: &str,
        timestamp: SystemTime,
    ) -> EngineResult<Vec<RecordBatch>> {
        let sql = format!(
            "SELECT * FROM iceberg.{} FOR TIMESTAMP AS OF '{}'",
            table, timestamp
        );
        self.trino_client.execute_query(&sql).await
    }
}
```

**Pros**:
- ✅ Mature, production-ready Iceberg support
- ✅ Full time travel capabilities out of the box
- ✅ Leverages existing ecosystem
- ✅ Handles query optimization, execution planning

**Cons**:
- ❌ Adds external dependency (Trino cluster)
- ❌ Network overhead for all queries
- ❌ Operational complexity (running Trino)
- ❌ Not suitable for embedded use cases

#### Option B: Dremio Integration

Similar to Trino but with enterprise features:
- ✅ Time travel support
- ✅ Reflections (query acceleration)
- ✅ Semantic layer
- ❌ Commercial licensing for some features

#### Option C: StarRocks Integration

- ✅ Fast OLAP performance
- ✅ Iceberg external catalog support
- ✅ Time travel via AS OF syntax
- ❌ Less mature than Trino/Dremio

**Recommendation**: ⭐⭐ **Not Recommended for Core Use**
- Too heavyweight for orbit-engine's embedded nature
- Better for data warehouse scenarios
- Could be offered as an *optional* backend

---

## Recommended Implementation Strategy

### Phase 1: Evaluate Icelake (Immediate)

1. **Spike/Prototype** (1-2 days):
   ```bash
   cargo add icelake
   ```

2. **Test Snapshot API**:
   ```rust
   // Create test to verify snapshot access
   #[tokio::test]
   async fn test_icelake_snapshots() {
       let table = icelake_catalog.load_table(&ident).await?;
       let snapshots = table.snapshots(); // Does this method exist?
       assert!(snapshots.len() > 0);
   }
   ```

3. **Evaluate API Surface**:
   - Does it have `snapshots()` method?
   - Does it support `snapshot_by_timestamp()`?
   - Is tag/branch management available?
   - What's the migration path from iceberg-rust?

4. **Decision Point**:
   - If Icelake has required APIs → **Migrate to Icelake**
   - If not → Wait for iceberg-rust or contribute upstream

### Phase 2: Hybrid Approach (If Icelake Works)

```rust
// orbit/engine/src/storage/iceberg.rs

#[cfg(feature = "icelake")]
use icelake as iceberg_impl;

#[cfg(not(feature = "icelake"))]
use iceberg as iceberg_impl;

pub struct IcebergColdStore {
    table: Arc<iceberg_impl::Table>,
    // ...
}

impl IcebergColdStore {
    pub async fn query_as_of(
        &self,
        timestamp: SystemTime,
        filter: Option<&FilterPredicate>,
    ) -> EngineResult<Vec<RecordBatch>> {
        #[cfg(feature = "icelake")]
        {
            // Full implementation with Icelake
            let snapshot = self.table
                .snapshot_by_timestamp(timestamp_ms)?;
            // ... actual time travel
        }

        #[cfg(not(feature = "icelake"))]
        {
            // Fallback to current snapshot
            self.scan(filter).await
        }
    }
}
```

### Phase 3: Catalog Management with Lakekeeper (Optional)

If we want centralized metadata management:

```rust
// Separate service
pub struct LakekeeperCatalog {
    lakekeeper_url: String,
}

// Use for:
// - Multi-tenant deployments
// - Sharing metadata across multiple orbit-engine instances
// - Centralized access control
```

---

## Feature Comparison Matrix

| Feature | iceberg-rust | Icelake | Lakekeeper | Query Engines |
|---------|--------------|---------|------------|---------------|
| **Snapshot Listing** | ❌ | ✅ (likely) | N/A | ✅ |
| **Time Travel** | ❌ | ✅ (likely) | N/A | ✅ |
| **Tag/Branch Mgmt** | ❌ | ✅ (likely) | ✅ | ✅ |
| **Table Read/Write** | ✅ | ✅ | ❌ | ✅ |
| **REST Catalog** | ✅ | ✅ | ✅ | ✅ |
| **Native Rust** | ✅ | ✅ | ✅ | ❌ |
| **Embedded Use** | ✅ | ✅ | ❌ | ❌ |
| **Production Ready** | ✅ | ✅ | ⚠️ | ✅ |
| **Apache License** | ✅ | ✅ | ✅ | ⚠️ varies |

---

## Immediate Next Steps

### 1. Investigate Icelake API (Priority: HIGH)

```bash
# Add Icelake dependency to Cargo.toml
cd orbit/engine
cargo add icelake --optional

# Create evaluation test
touch tests/icelake_evaluation.rs
```

### 2. Create API Compatibility Test

```rust
// tests/icelake_evaluation.rs
#[cfg(feature = "icelake")]
#[tokio::test]
async fn evaluate_icelake_snapshot_api() {
    use icelake::*;

    // Test 1: Can we list snapshots?
    // Test 2: Can we query by timestamp?
    // Test 3: Can we access tag/branch metadata?
    // Test 4: Migration complexity from iceberg-rust?
}
```

### 3. Document Findings

Create issue/RFC with findings:
- API compatibility
- Performance comparison
- Migration effort estimate
- Risk assessment

### 4. Decision Matrix

```
IF Icelake has complete snapshot API:
  → Migrate to Icelake (breaking change, major version bump)
  → Enable full time travel immediately

ELSE IF Icelake partial support:
  → Contribute to Icelake project
  → Or wait for iceberg-rust updates

ELSE:
  → Continue with iceberg-rust
  → Contribute upstream to add snapshot APIs
  → Consider Trino integration as interim solution
```

---

## Contributing Upstream

If we stay with iceberg-rust, we should contribute the snapshot API:

**Potential PR to iceberg-rust**:

```rust
// Add to iceberg::spec::table_metadata

impl TableMetadata {
    /// List all snapshots for this table
    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    /// Get snapshot at or before specific timestamp
    pub fn snapshot_by_timestamp(&self, timestamp_ms: i64) -> Option<&Snapshot> {
        self.snapshots()
            .iter()
            .filter(|s| s.timestamp_ms <= timestamp_ms)
            .max_by_key(|s| s.timestamp_ms)
    }

    /// Get snapshot by ID
    pub fn snapshot_by_id(&self, snapshot_id: i64) -> Option<&Snapshot> {
        self.snapshots()
            .iter()
            .find(|s| s.snapshot_id == snapshot_id)
    }
}
```

**Benefits of Contributing**:
- ✅ Improves official Apache project
- ✅ Benefits entire Rust/Iceberg community
- ✅ Ensures long-term API stability
- ✅ Aligns with open source best practices

---

## Conclusion

**Recommended Path Forward**:

1. **Immediate** (Week 1): Evaluate Icelake snapshot API support
2. **Short-term** (Week 2-3):
   - If Icelake works → Plan migration
   - If not → Contribute to iceberg-rust upstream
3. **Medium-term** (Month 2): Implement full time travel with chosen solution
4. **Long-term**: Consider Lakekeeper for multi-tenant catalog management

**Best Case Scenario**:
Icelake has complete snapshot API → Migrate → Enable time travel immediately

**Realistic Scenario**:
Contribute snapshot API to iceberg-rust → Wait for merge → Enable time travel

**Backup Plan**:
Offer optional Trino backend for time travel while waiting for native Rust solution

---

## References

- **Icelake**: https://github.com/icelake-io/icelake
- **Lakekeeper**: https://github.com/lakekeeper/lakekeeper
- **iceberg-rust**: https://github.com/apache/iceberg-rust
- **Databend** (uses Icelake): https://github.com/datafuselabs/databend
- **Apache Iceberg Spec**: https://iceberg.apache.org/spec/

Last Updated: 2025-01-19
