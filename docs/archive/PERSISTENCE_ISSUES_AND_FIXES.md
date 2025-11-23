# RocksDB Persistence Creation Failure - Root Causes and Fixes

## Summary

This document identifies the root causes for RocksDB persistence creation failures and why the `data/cold`, `data/hot`, `data/wal`, and `data/warm` folders were empty.

## Root Causes Identified

### 1. **TieredTableStorage was Completely In-Memory**

**Problem**: The `TieredTableStorage` implementation used `HybridStorageManager` which stored all data in-memory using `RowBasedStore` (a `Vec<Row>`). There was no disk persistence mechanism.

**Location**: `orbit/server/src/protocols/common/storage/tiered.rs`

**Impact**: All data written through `TieredTableStorage` (used by MySQL and CQL protocols) was lost on server restart.

### 2. **No Protocol-Specific Persistence Folders**

**Problem**: Only generic directories (`data/hot`, `data/warm`, `data/cold`, `data/wal`, `data/rocksdb`, `data/redis`) were created. Each protocol (PostgreSQL, MySQL, CQL) should have its own persistence folder.

**Location**: `orbit/server/src/main.rs` - `initialize_data_directories()`

**Impact**: Protocols couldn't have isolated persistence, making it difficult to manage and debug protocol-specific data.

### 3. **Hot/Warm/Cold Directories Were Not Used**

**Problem**: The `data/hot`, `data/warm`, and `data/cold` directories were created but never written to. `HybridStorageManager` stored:
- Hot tier: In-memory `RowBasedStore` (no disk persistence)
- Warm tier: In-memory `Option<ColumnBatch>` (no disk persistence)
- Cold tier: In-memory `Option<ColumnBatch>` or `IcebergColdStore` (Iceberg implementation marked as "full implementation pending")

**Location**: 
- `orbit/server/src/protocols/postgres_wire/sql/execution/hybrid.rs` - `HybridStorageManager`
- `orbit/server/src/protocols/postgres_wire/sql/execution/hybrid.rs` - `RowBasedStore`

**Impact**: Data migration from hot→warm→cold tiers didn't persist to disk, so these directories remained empty.

### 4. **WAL Directory Was Not Used**

**Problem**: The `data/wal` directory was created but `TieredTableStorage` didn't use it. RocksDB has its own WAL mechanism, but it wasn't configured to use the `data/wal` directory.

**Impact**: Write-ahead logging wasn't properly configured for tiered storage.

### 5. **RocksDB Was Initialized But Not Used by TieredTableStorage**

**Problem**: A global `RocksDbTableStorage` was initialized at `data/rocksdb` for PostgreSQL, but `TieredTableStorage` (used by MySQL and CQL) didn't use RocksDB at all.

**Location**: `orbit/server/src/main.rs`

**Impact**: MySQL and CQL protocols had no persistence, even though RocksDB was available.

## Fixes Implemented

### 1. **Added RocksDB Persistence to TieredTableStorage**

**Changes**:
- Added `db: Arc<RwLock<Option<Arc<DB>>>>` field to `TieredTableStorage`
- Added `data_dir: Option<PathBuf>` field to store the data directory path
- Created `with_data_dir()` method to initialize `TieredTableStorage` with a data directory
- Updated `initialize()` to open RocksDB with column families:
  - `schemas`: Table schemas
  - `data`: Row data
  - `indexes`: Index metadata
  - `views`: View definitions
  - `schema_defs`: Schema definitions
  - `extensions`: Extension definitions
  - `settings`: Configuration settings

**Location**: `orbit/server/src/protocols/common/storage/tiered.rs`

### 2. **Created Protocol-Specific Persistence Folders**

**Changes**:
- Updated `initialize_data_directories()` to create:
  - `data/postgresql/` - PostgreSQL protocol persistence
  - `data/mysql/` - MySQL protocol persistence
  - `data/cql/` - CQL protocol persistence
  - `data/redis/` - Redis protocol persistence
  - `data/cypher/` - Cypher/Neo4j protocol persistence
  - `data/aql/` - AQL/ArangoDB protocol persistence

**Location**: `orbit/server/src/main.rs`

### 3. **Fixed Redis Persistence Path Inconsistency**

**Problem**: Redis was creating RocksDB files at both `data/redis/` (root) and `data/redis/rocksdb/` (subdirectory), inconsistent with other protocols.

**Fix**:
- Updated `main.rs` to use `data/redis/rocksdb/` consistently
- Updated `server.rs` to use `data/redis/rocksdb/` consistently
- All protocols now follow the same pattern: `data/{protocol}/rocksdb/`

**Location**: `orbit/server/src/main.rs`, `orbit/server/src/server.rs`

### 4. **Updated Protocol Initialization to Use Data Directories**

**Changes**:
- Modified `main.rs` to pass protocol-specific data directories to each `TieredTableStorage`:
  ```rust
  let postgres_storage = Arc::new(TieredTableStorage::with_data_dir(
      postgres_data_dir,
      tiered_config.clone(),
  ));
  let mysql_storage = Arc::new(TieredTableStorage::with_data_dir(
      mysql_data_dir,
      tiered_config.clone(),
  ));
  let cql_storage = Arc::new(TieredTableStorage::with_data_dir(
      cql_data_dir,
      tiered_config,
  ));
  ```

**Location**: `orbit/server/src/main.rs`

### 4. **Added Persistence to Schema and Row Operations**

**Changes**:
- Updated `store_table_schema()` to persist schemas to RocksDB
- Updated `insert_row()` to persist row data to RocksDB in addition to in-memory storage

**Location**: `orbit/server/src/protocols/common/storage/tiered.rs`

### 5. **Implemented Cypher and AQL Persistence**

**Changes**:
- Created `CypherGraphStorage` with RocksDB persistence at `data/cypher/rocksdb/`
- Created `AqlStorage` with RocksDB persistence at `data/aql/rocksdb/`
- Updated `CypherServer` to use storage backend
- Created `AqlServer` with storage backend
- Both servers initialized in `main.rs` with persistence

**Location**: 
- `orbit/server/src/protocols/cypher/storage.rs`
- `orbit/server/src/protocols/aql/storage.rs`
- `orbit/server/src/protocols/cypher/server.rs`
- `orbit/server/src/protocols/aql/server.rs`
- `orbit/server/src/main.rs`

## Remaining Issues

### 1. **Hot/Warm/Cold Tier Persistence**

**Status**: **Pending**

The `HybridStorageManager` still stores hot/warm/cold data in-memory. To fully fix this:

1. **Hot Tier**: Should persist to `data/{protocol}/hot/` using RocksDB or another storage backend
2. **Warm Tier**: Should persist to `data/{protocol}/warm/` using columnar format (Parquet, Arrow, etc.)
3. **Cold Tier**: Should persist to `data/{protocol}/cold/` using Iceberg (when feature enabled) or columnar format

**Location**: `orbit/server/src/protocols/postgres_wire/sql/execution/hybrid.rs`

### 2. **WAL Directory Usage**

**Status**: **Pending**

The `data/wal` directory is created but not used. Options:
- Configure RocksDB to use a separate WAL directory
- Use the WAL directory for a custom write-ahead log implementation
- Remove the WAL directory if not needed

### 3. **Data Loading on Startup**

**Status**: **Partial**

The `load_schemas_from_rocksdb()` method is a placeholder. It should:
- Load all table schemas from RocksDB
- Load all indexes, views, extensions, settings
- Reconstruct `HybridStorageManager` instances for existing tables
- Load row data from RocksDB into hot tier

**Location**: `orbit/server/src/protocols/common/storage/tiered.rs` - `load_schemas_from_rocksdb()`

## Testing Recommendations

1. **Verify Protocol-Specific Folders Are Created**:
   ```bash
   ls -la data/
   # Should show: postgresql/, mysql/, cql/, redis/, hot/, warm/, cold/, wal/, rocksdb/
   ```

2. **Verify RocksDB Files Are Created**:
   ```bash
   ls -la data/postgresql/rocksdb/
   ls -la data/mysql/rocksdb/
   ls -la data/cql/rocksdb/
   # Should show RocksDB database files (CURRENT, MANIFEST, OPTIONS, *.sst, etc.)
   ```

3. **Test Data Persistence**:
   - Insert data via MySQL protocol
   - Restart server
   - Verify data is still present

4. **Monitor Directory Sizes**:
   ```bash
   du -sh data/*/
   # Should show non-zero sizes for protocol directories after data insertion
   ```

## Next Steps

1. **Implement Hot/Warm/Cold Tier Disk Persistence**: Update `HybridStorageManager` to write to disk directories
2. **Implement Data Loading on Startup**: Complete `load_schemas_from_rocksdb()` and add row data loading
3. **Add WAL Configuration**: Configure RocksDB to use `data/wal` or remove the directory
4. **Add Migration Logic**: Implement background tasks to migrate data from hot→warm→cold tiers
5. **Add Tests**: Create integration tests to verify persistence across server restarts

