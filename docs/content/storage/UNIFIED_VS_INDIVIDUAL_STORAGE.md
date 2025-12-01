# Unified vs Individual Protocol Storage

This guide explains how to configure and switch between unified cross-protocol storage and individual protocol storage in Orbit-RS, including the implications for data files.

## Overview

Orbit-RS supports two storage modes:

| Mode | Description | Use Case |
|------|-------------|----------|
| **Unified Storage** | All protocols share a single tiered storage backend | Cross-protocol data sharing, simplified management |
| **Individual Storage** | Each protocol uses isolated storage | Legacy compatibility, protocol-specific optimization |

## Storage Architecture

### Unified Storage Mode (Default)

```
┌─────────────────────────────────────────────────────────────────┐
│                      Protocol Layer                             │
├─────────┬────────-─┬─────────┬─────────┬─────────┬────────────-─┤
│  Redis  │PostgreSQL│  MySQL  │   CQL   │ Cypher  │  AQL/REST    │
│ Adapter │ Adapter  │ Adapter │ Adapter │ Adapter │  Adapter     │
├─────────┴────────-─┴─────────┴─────────┴─────────┴────────────-─┤
│                 Unified Query Engine                            │
├─────────────────────────────────────────────────────────────────┤
│                   Unified Storage                               │
│         (Single RocksDB + Tiered Storage Backend)               │
│                                                                 │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│   │  Hot Tier   │  │  Warm Tier  │  │  Cold Tier  │             │
│   │  (Memory)   │→→│  (RocksDB)  │→→│ (S3/Azure)  │             │
│   └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

### Individual Storage Mode

```
┌─────────────────────────────────────────────────────────────────┐
│                      Protocol Layer                             │
├─────────┬───────-──┬─────────┬─────────┬─────────┬───────-──────┤
│  Redis  │PostgreSQL│  MySQL  │   CQL   │ Cypher  │  AQL/REST    │
├─────────┼───────-──┼─────────┼─────────┼─────────┼──────────────┤
│ Redis   │Postgres  │ MySQL   │  CQL    │ Graph   │   AQL        │
│ Storage │ Storage  │ Storage │ Storage │ Storage │  Storage     │
│(RocksDB)│(RocksDB) │(RocksDB)│(RocksDB)│(RocksDB)│ (RocksDB)    │
└─────────┴───────--─┴─────────┴─────────┴─────────┴──────────────┘
```

## Configuration

### Enabling Unified Storage (Default)

In `config/orbit-server.toml`:

```toml
# ================================
# UNIFIED CROSS-PROTOCOL STORAGE
# ================================

[unified_storage]
# Master switch: true = unified, false = individual protocol storage
enabled = true

# Data directory for unified storage
data_dir = "./data/unified"

# Hot Tier: In-Memory Storage (fastest access, limited capacity)
[unified_storage.hot_tier]
enabled = true
max_memory_mb = 1024
eviction_policy = "lru"     # Options: "lru", "lfu", "fifo", "ttl"
eviction_threshold = 0.85   # Trigger eviction when 85% full
write_through = true        # Write to warm tier on every write
read_through = true         # Fetch from warm tier on cache miss

[unified_storage.hot_tier.prefetch]
enabled = true
batch_size = 100
trigger_threshold = 3       # Prefetch after 3 accesses to related data

# Warm Tier: RocksDB Storage (persistent, SSD-optimized)
[unified_storage.warm_tier]
enabled = true
data_dir = "./data/unified/rocksdb"
max_disk_gb = 100
enable_compression = true
compression_algorithm = "lz4"  # Options: "lz4", "snappy", "zstd", "none"
block_cache_mb = 256
write_buffer_mb = 64
max_write_buffers = 3
enable_bloom_filters = true
bloom_bits_per_key = 10
enable_wal = true
sync_wal = false

# Cold Tier: Cloud Object Storage (archival, cost-effective)
[unified_storage.cold_tier]
enabled = false              # Enable when cloud storage is configured
backend = "s3"              # Options: "s3", "azure", "gcs", "minio"
data_format = "parquet"     # Options: "parquet", "iceberg", "delta"
partition_strategy = "time" # Options: "time", "hash", "range"
compression = "snappy"

[unified_storage.cold_tier.async_upload]
enabled = true
batch_size_mb = 64
interval_secs = 300         # Upload every 5 minutes
max_concurrent = 4
retry_count = 3

# Cross-Protocol Data Sharing
[unified_storage.cross_protocol]
enabled = true
shared_protocols = ["redis", "postgresql", "mysql", "cql", "cypher", "aql", "rest"]
namespace_strategy = "auto"   # Options: "auto", "explicit"
default_namespace = "default"
enable_schema_inference = true
schema_cache_size = 1000

# Automatic Tier Migration
[unified_storage.tier_migration]
enabled = true
hot_to_warm_secs = 3600     # Move to warm tier after 1 hour of no access
warm_to_cold_secs = 604800  # Move to cold tier after 7 days
scan_interval_secs = 60     # Check for migrations every minute
batch_size = 1000           # Migrate up to 1000 records per batch
priority_strategy = "age"   # Options: "age", "size", "access_frequency"
enable_promotion = true     # Promote data back to hot tier on access
promotion_threshold = 5     # Promote after 5 accesses

# TTL and Data Expiration
[unified_storage.ttl]
enabled = true
check_interval_secs = 60
batch_size = 1000
default_ttl_secs = 0        # 0 = no default TTL
```

### Enabling Individual Protocol Storage

To disable unified storage and use individual protocol storage:

```toml
[unified_storage]
enabled = false  # Disable unified storage
```

Or remove the `[unified_storage]` section entirely.

### Selective Protocol Sharing

You can enable unified storage for only specific protocols:

```toml
[unified_storage]
enabled = true

[unified_storage.cross_protocol]
enabled = true
# Only Redis and PostgreSQL share data
shared_protocols = ["redis", "postgresql"]
```

Protocols not in `shared_protocols` will use their own isolated storage.

## Data File Locations

### Directory Structure

```
./data/
├── unified/                    # Unified storage (when enabled)
│   ├── rocksdb/               # Warm tier (persistent RocksDB)
│   │   ├── 000003.log         # RocksDB write-ahead log
│   │   ├── 000005.sst         # SST data files
│   │   ├── CURRENT            # Current manifest pointer
│   │   ├── MANIFEST-000004    # RocksDB manifest
│   │   ├── OPTIONS-000007     # RocksDB options
│   │   └── LOCK               # Lock file
│   ├── cold/                  # Cold tier staging area
│   │   └── pending/           # Files awaiting cloud upload
│   └── metadata/              # Schema registry, namespace mappings
│       ├── schemas.json
│       └── namespaces.json
│
├── postgresql/                # Individual PostgreSQL storage
│   └── rocksdb/
├── redis/                     # Individual Redis storage
│   └── rocksdb/
├── mysql/                     # Individual MySQL storage
│   └── rocksdb/
├── cql/                       # Individual CQL storage
│   └── rocksdb/
├── cypher/                    # Individual Cypher/Graph storage
│   └── rocksdb/
├── aql/                       # Individual AQL storage
│   └── rocksdb/
│
└── wal/                       # Write-ahead logs (shared)
    └── transactions.db
```

## Switching Between Storage Modes

### Important: No Automatic Data Migration

**Orbit-RS does NOT automatically migrate data when switching storage modes.**

When you switch modes:
- Old data files remain intact but become inaccessible
- New storage starts fresh/empty
- Manual export/import is required to preserve data

### Unified → Individual Storage

When switching from unified to individual storage:

| What Happens | Details |
|--------------|---------|
| Unified files | Remain in `./data/unified/` but are no longer used |
| Hot tier data | **Lost** - in-memory data is not persisted on mode switch |
| Warm tier data | **Preserved** on disk but inaccessible |
| Cold tier data | **Preserved** in cloud storage but inaccessible |
| Individual storage | Starts fresh/empty |

**Migration Steps:**

```bash
# 1. While unified storage is still enabled, export your data
# Via PostgreSQL:
psql -h localhost -p 5432 -c "COPY users TO '/tmp/users_backup.csv' CSV HEADER"
psql -h localhost -p 5432 -c "COPY orders TO '/tmp/orders_backup.csv' CSV HEADER"

# Via Redis:
redis-cli -h localhost -p 6379 --rdb /tmp/redis_backup.rdb

# 2. Stop the server
pkill orbit-server

# 3. Update configuration
# Edit config/orbit-server.toml:
# [unified_storage]
# enabled = false

# 4. Restart the server
orbit-server --config ./config/orbit-server.toml

# 5. Import data to individual protocol storage
# Via PostgreSQL:
psql -h localhost -p 5432 -c "CREATE TABLE users (...)"
psql -h localhost -p 5432 -c "COPY users FROM '/tmp/users_backup.csv' CSV HEADER"

# Via Redis:
redis-cli -h localhost -p 6379 --pipe < /tmp/redis_commands.txt
```

### Individual → Unified Storage

When switching from individual to unified storage:

| What Happens | Details |
|--------------|---------|
| Individual files | Remain in protocol-specific directories but are no longer used |
| Protocol data | **Preserved** on disk but inaccessible |
| Unified storage | Starts fresh/empty |

**Migration Steps:**

```bash
# 1. While individual storage is active, export data from each protocol
# PostgreSQL:
pg_dump -h localhost -p 5432 -F c -f /tmp/postgres_backup.dump

# Redis:
redis-cli -h localhost -p 6379 BGSAVE
cp ./data/redis/dump.rdb /tmp/redis_backup.rdb

# 2. Stop the server
pkill orbit-server

# 3. Update configuration
# Edit config/orbit-server.toml:
# [unified_storage]
# enabled = true

# 4. Restart the server
orbit-server --config ./config/orbit-server.toml

# 5. Import data via any protocol (it will be shared across all)
# Using PostgreSQL to import (data accessible via all protocols):
psql -h localhost -p 5432 -c "COPY users FROM '/tmp/users_backup.csv' CSV HEADER"

# Or using Redis:
redis-cli -h localhost -p 6379 --pipe < /tmp/redis_import.txt
```

## Preserving Both Storage Backends

To maintain both unified and individual storage files (allowing easy switching):

```toml
# Individual protocol storage path
[persistence]
data_dir = "./data/individual"

# Unified storage path (completely separate)
[unified_storage]
enabled = true
data_dir = "./data/unified"
```

This configuration:
- Keeps individual storage in `./data/individual/`
- Keeps unified storage in `./data/unified/`
- Switching `enabled` toggles which is active without affecting the other

## Cross-Protocol Data Sharing Examples

When unified storage is enabled, data written via one protocol is immediately accessible via all others:

### Write via Redis, Read via PostgreSQL

```bash
# Write data using Redis
redis-cli HSET user:alice name "Alice" email "alice@example.com" age 30

# Read the same data using PostgreSQL
psql -c "SELECT * FROM user WHERE id = 'alice'"
#  id    | name  |       email        | age
# -------+-------+--------------------+-----
#  alice | Alice | alice@example.com  | 30
```

### Write via MySQL, Read via Redis

```bash
# Write data using MySQL
mysql -e "INSERT INTO products (id, name, price) VALUES ('prod1', 'Widget', 29.99)"

# Read the same data using Redis
redis-cli HGETALL products:prod1
# 1) "id"
# 2) "prod1"
# 3) "name"
# 4) "Widget"
# 5) "price"
# 6) "29.99"
```

### Write via AQL, Read via CQL

```bash
# Write using ArangoDB AQL
arangosh --execute 'db.events.insert({_key: "evt1", type: "click", timestamp: 1234567890})'

# Read using Cassandra CQL
cqlsh -e "SELECT * FROM events WHERE id = 'evt1'"
#  id   | type  | timestamp
# ------+-------+------------
#  evt1 | click | 1234567890
```

## Data Type Mapping

Unified storage uses `UniversalValue` as the canonical type. Here's how types map across protocols:

| UniversalValue | PostgreSQL | MySQL | Redis | CQL | AQL |
|----------------|------------|-------|-------|-----|-----|
| `Null` | `NULL` | `NULL` | `(nil)` | `null` | `null` |
| `Bool` | `BOOLEAN` | `TINYINT(1)` | `1/0` | `boolean` | `Boolean` |
| `Int` | `BIGINT` | `BIGINT` | `Integer` | `bigint` | `Number` |
| `Float` | `DOUBLE PRECISION` | `DOUBLE` | `Float string` | `double` | `Number` |
| `String` | `TEXT` | `VARCHAR` | `String` | `text` | `String` |
| `Bytes` | `BYTEA` | `BLOB` | `Binary` | `blob` | `String (base64)` |
| `List` | `JSONB[]` | `JSON` | `List` | `list<>` | `Array` |
| `Map` | `JSONB` | `JSON` | `Hash` | `map<>` | `Object` |
| `Timestamp` | `TIMESTAMPTZ` | `DATETIME` | `String (ISO)` | `timestamp` | `String (ISO)` |

## Performance Considerations

### Unified Storage

**Advantages:**
- Single storage backend to manage
- Automatic cross-protocol data sharing
- Efficient tiered storage with automatic migration
- Reduced storage footprint (no data duplication)

**Trade-offs:**
- Additional translation overhead for type mapping
- All protocols share the same performance characteristics
- Schema inference may add latency for first access

### Individual Storage

**Advantages:**
- Protocol-specific optimizations possible
- No type translation overhead
- Independent scaling per protocol
- Isolation prevents cross-protocol issues

**Trade-offs:**
- Data duplication if same data needed by multiple protocols
- More complex management (multiple storage backends)
- No automatic cross-protocol data sharing

## Best Practices

1. **Choose the right mode upfront**: Switching modes requires manual data migration
2. **Use separate data directories**: Configure distinct paths for unified vs individual storage
3. **Regular backups**: Before any mode switch, back up all data
4. **Test in development**: Validate cross-protocol queries before production deployment
5. **Monitor tier migration**: Watch for unexpected data movement between tiers
6. **Set appropriate TTLs**: Prevent unbounded data growth in unified storage

## Troubleshooting

### Data not visible across protocols

1. Verify `unified_storage.enabled = true`
2. Check `unified_storage.cross_protocol.shared_protocols` includes both protocols
3. Ensure namespace matches (check `namespace_strategy` setting)

### Data loss after mode switch

1. Data is NOT automatically migrated - this is expected
2. Check original data directory - files should still exist
3. Re-enable previous mode to access old data
4. Perform manual export/import

### Performance degradation

1. Check hot tier memory usage (`max_memory_mb`)
2. Review tier migration settings - data may be moving to slower tiers
3. Consider adjusting `eviction_threshold` and `promotion_threshold`

## Related Documentation

- [Storage Architecture](./STORAGE_ARCHITECTURE_CURRENT.md)
- [RocksDB Configuration](./ROCKSDB_VS_RUST_NATIVE_ANALYSIS.md)
- [Persistence Guide](./PERSISTENCE_COMPLETE_DOCUMENTATION.md)
- [Protocol Adapters](../protocols/PROTOCOL_ADAPTERS_INTEGRATION.md)
