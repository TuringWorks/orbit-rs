# Phase 3 Enhancement Roadmap

**Status**: Planning
**Target**: Post-MVP Enhancements
**Created**: 2025-01-19

## Overview

This document outlines enhancements planned for Phase 3 of the orbit-engine development. These items are not blocking for the initial release but will significantly improve performance and functionality.

## Performance Optimizations

### 1. Filter Predicate Optimization Across Tiers

**Current Status**: Filters work correctly but are not optimized for each tier

**Enhancement**: Implement efficient filter predicate conversion and pushdown

**Locations**:

- `orbit/engine/src/storage/hybrid.rs:683` - Hot tier filter conversion
- `orbit/engine/src/storage/hybrid.rs:759` - Cold tier filter conversion
- `orbit/engine/src/storage/hybrid.rs:817` - Aggregate filter conversion
- `orbit/engine/src/storage/hybrid.rs:850` - Update filter conversion
- `orbit/engine/src/storage/hybrid.rs:857` - Delete filter conversion

**Implementation Plan**:

```rust
/// Convert StorageFilterPredicate to tier-specific filter format
fn convert_filter_for_tier(
    filter: &StorageFilterPredicate,
    tier: StorageTier,
) -> EngineResult<TierSpecificFilter> {
    match tier {
        StorageTier::Hot => convert_to_simple_filter(filter),
        StorageTier::Warm => convert_to_hybrid_filter(filter),
        StorageTier::Cold => convert_to_iceberg_filter(filter),
    }
}
```

**Benefits**:

- Push predicates down to storage engines
- Reduce data movement
- Improve query performance 10-100x for filtered queries

**Estimated Effort**: 2-3 days

### 2. Iceberg Metadata-Based Row Count

**Current Status**: Returns 0, forcing full scans for COUNT(*) queries

**Enhancement**: Extract row counts from Iceberg snapshot metadata

**Location**: `orbit/engine/src/storage/iceberg.rs:236-249`

**Investigation Required**:

1. Understand Iceberg `Summary` type structure
2. Find total_records or equivalent field
3. Handle manifest-only counts vs file-level counts

**Implementation Plan**:

```rust
pub async fn row_count(&self) -> EngineResult<usize> {
    if let Some(snapshot) = self.table.metadata().current_snapshot() {
        let summary = snapshot.summary();

        // Option 1: Direct field access
        if let Some(total) = summary.total_records {
            return Ok(total as usize);
        }

        // Option 2: Manifest file reading
        let manifest_list = snapshot.manifest_list();
        let count = read_row_counts_from_manifests(manifest_list).await?;
        return Ok(count);
    }

    Ok(0)
}
```

**Benefits**:

- COUNT(*) queries become O(1) instead of O(n)
- 100-1000x speedup for large tables
- Enables table statistics for query optimizer

**Estimated Effort**: 4-6 hours (once API is understood)

## Feature Enhancements

### 3. Connection Pooling Infrastructure

**Current Status**: Commented out, module not implemented

**Enhancement**: Add connection pooling for distributed transactions

**Locations**:

- `orbit/engine/src/transactions/mod.rs:40` - Module import commented
- `orbit/engine/src/transactions/performance.rs:13` - Types commented

**Implementation Plan**:

1. Create `orbit/engine/src/transactions/pooling.rs`
2. Implement connection pool with:
   - Min/max connection limits
   - Connection health checks
   - Automatic reconnection
   - Load balancing across nodes
3. Integrate with transaction coordinator

**Benefits**:

- Reduce connection overhead
- Improve transaction throughput
- Better resource utilization

**Estimated Effort**: 1-2 weeks

### 4. Memory Module PostgreSQL Decoupling

**Current Status**: Module commented out due to PostgreSQL type dependencies

**Enhancement**: Decouple memory.rs from protocol-specific types

**Location**: `orbit/engine/src/storage/mod.rs:20-21`

**Implementation Plan**:

1. Create protocol-agnostic memory storage interface
2. Move PostgreSQL-specific logic to adapter layer
3. Re-enable memory module with clean abstraction

**Benefits**:

- Reusable memory storage across protocols
- Cleaner architecture
- Easier testing

**Estimated Effort**: 2-3 days

### 5. OrbitQL Primary Key Extraction

**Current Status**: Uses empty vector, works but incomplete

**Enhancement**: Extract primary key from table constraints

**Location**: `orbit/engine/src/adapters/orbitql.rs:45`

**Implementation Plan**:

```rust
// Extract primary key from CREATE TABLE constraints
pub fn extract_primary_key(constraints: &[Constraint]) -> Vec<String> {
    for constraint in constraints {
        if let Constraint::PrimaryKey { columns, .. } = constraint {
            return columns.iter().map(|c| c.value.clone()).collect();
        }
    }
    vec![]
}
```

**Benefits**:

- Proper primary key enforcement
- Better query optimization
- Index creation support

**Estimated Effort**: 2-3 hours

## Iceberg Write Path (Major Feature)

### 6. Iceberg Table Writes

**Current Status**: Read-only, write path returns error

**Enhancement**: Full write support for Iceberg cold tier

**Location**: `orbit/engine/src/storage/iceberg.rs:297-312`

**Implementation Plan** (from existing comments):

```rust
pub async fn write(&self, batch: &ColumnBatch) -> EngineResult<()> {
    // 1. Convert ColumnBatch to Arrow RecordBatch
    let arrow_batch = column_batch_to_arrow(batch)?;

    // 2. Build Parquet writer
    let parquet_writer = ParquetWriterBuilder::new(
        WriterProperties::default(),
        schema.clone(),
        None, // partition_key
        file_io.clone(),
        location_generator,
        file_name_generator,
    );

    // 3. Build data file writer
    let mut data_file_writer = DataFileWriterBuilder::new(
        parquet_writer,
        None, // partition_value
        0,    // partition_spec_id
    ).build().await?;

    // 4. Write data
    data_file_writer.write(arrow_batch).await?;
    let data_files = data_file_writer.close().await?;

    // 5. Commit via transaction
    table.new_transaction()
        .fast_append(data_files)
        .commit().await?;

    Ok(())
}
```

**Additional Features**:

- Schema evolution support
- Partition management
- Compaction scheduling
- Snapshot expiration
- Incremental updates

**Benefits**:

- Complete Iceberg integration
- Efficient cold tier writes
- Schema evolution
- Time travel with writes

**Estimated Effort**: 2-3 weeks

## Testing & Quality

### 7. Comprehensive Integration Tests

**Enhancement**: Add integration tests for all Phase 3 features

**Test Coverage**:

- Filter pushdown across all tiers
- Row count accuracy
- Connection pool stress tests
- Memory module functionality
- Primary key constraints
- Iceberg write/read cycles

**Estimated Effort**: 1 week

### 8. Performance Benchmarks

**Enhancement**: Establish performance baselines and regression tests

**Benchmark Suite**:

- Filter pushdown impact
- Row count vs full scan
- Connection pool scalability
- Write throughput (Iceberg)
- Multi-tier query performance

**Estimated Effort**: 3-4 days

## Architecture Improvements

### 9. Query Optimizer Enhancements

**Enhancement**: Cost-based optimization using tier statistics

**Features**:

- Statistics collection per tier
- Cost model for tier selection
- Dynamic query routing
- Adaptive query execution

**Benefits**:

- Automatic tier selection
- Optimal query performance
- Reduced resource usage

**Estimated Effort**: 2-3 weeks

### 10. Automatic Tier Migration

**Enhancement**: Background process for data lifecycle management

**Features**:

- Configurable tier transition policies
- Automatic data migration
- Resource-aware scheduling
- Progress monitoring

**Benefits**:

- Hands-off data management
- Optimal storage costs
- Consistent performance

**Estimated Effort**: 1-2 weeks

## Priority Matrix

| Feature | Impact | Effort | Priority |
|---------|--------|--------|----------|
| Filter Predicate Optimization | High | Low |  **P0** |
| Iceberg Row Count | High | Low |  **P0** |
| OrbitQL Primary Key | Low | Low |  **P1** |
| Iceberg Write Path | High | High |  **P1** |
| Connection Pooling | Medium | Medium |  **P2** |
| Memory Module Decoupling | Low | Low |  **P2** |
| Query Optimizer | High | High |  **P3** |
| Auto Tier Migration | Medium | Medium |  **P3** |
| Integration Tests | High | Medium |  **P1** |
| Benchmarks | Medium | Low |  **P2** |

## Timeline Estimate

- **Month 1**: P0 items (filter optimization, row count)
- **Month 2**: P1 items (primary key, write path, tests)
- **Month 3**: P2 items (pooling, decoupling, benchmarks)
- **Month 4+**: P3 items (optimizer, automation)

## Success Metrics

1. **Performance**:
   - Filter pushdown: 10-100x speedup on filtered queries
   - Row count: O(1) instead of O(n)
   - Write throughput: >10MB/s to Iceberg

2. **Functionality**:
   - All tiers support complex filters
   - Iceberg read/write parity
   - Connection pool handles 1000+ concurrent txns

3. **Quality**:
   - 100% test coverage for new features
   - Zero performance regressions
   - All TODOs resolved

---

**Document Version**: 1.0
**Last Updated**: 2025-01-19
**Next Review**: End of Phase 2
