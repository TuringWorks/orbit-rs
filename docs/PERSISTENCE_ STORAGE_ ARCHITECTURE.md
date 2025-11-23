# Persistent Storage Architecture

This module provides a pluggable storage abstraction layer for the Persistent Storage implementation in Orbit-RS. It supports multiple storage backends to enable everything from development testing to production-ready persistent storage.

## Architecture Overview

The storage architecture consists of three main components:

1. **Storage Trait (`TableStorage`)**: Defines the interface for all storage operations including schema management, data operations, transactions, and maintenance.
2. **Storage Backends**: Concrete implementations of the storage trait for different storage engines.
3. **Factory Pattern**: `StorageBackendFactory` provides a unified way to create storage backend instances.

## Available Storage Backends

### Memory Backend (Ready)
- **Purpose**: Development, testing, and ephemeral workloads
- **Features**: Fast in-memory storage with full SQL support
- **Limitations**: Data is lost when the server stops
- **Use Cases**: Unit tests, development, prototyping

### LSM-Tree Backend (In Development)
- **Purpose**: Production workloads requiring persistence and high performance
- **Features**: 
  - Write-optimized storage with background compaction
  - Crash recovery with Write-Ahead Logging
  - Configurable bloom filters and memtable sizes
  - Integration with Orbit-RS's existing LSM implementation
- **Status**: Architecture complete, integration pending dependency resolution

### Cluster Backend (Planned)
- **Purpose**: Distributed deployments with high availability
- **Features**: 
  - Multi-node replication
  - Automatic failover
  - Horizontal scaling
  - Geographic distribution

## Configuration

Storage backends are configured using the `StorageBackendConfig` enum:

```rust
use orbit_protocols::postgres_wire::storage::StorageBackendConfig;

// In-memory storage (default for development)
let config = StorageBackendConfig::Memory;

// LSM-Tree storage for persistence
let config = StorageBackendConfig::Lsm {
    data_dir: "./postgres_data".to_string(),
    memtable_size_limit: 64 * 1024 * 1024, // 64MB
    max_memtables: 10,
    bloom_filter_fp_rate: 0.01,
    enable_compaction: true,
    compaction_threshold: 4,
};
```

## Usage

### Creating a SQL Executor with Storage

```rust
use orbit_protocols::postgres_wire::sql::executor::SqlExecutor;
use orbit_protocols::postgres_wire::storage::{StorageBackendConfig, StorageBackendFactory};

// Create with default LSM storage
let executor = SqlExecutor::new().await?;

// Create with custom storage configuration
let config = StorageBackendConfig::Memory;
let executor = SqlExecutor::new_with_storage_config(config).await?;

// Use a specific storage backend
let storage = StorageBackendFactory::create_backend(&config).await?;
let executor = SqlExecutor::with_storage(storage);
```

### Storage Operations

The storage layer provides comprehensive support for:

- **Schema Management**: Tables, views, indexes, schemas, extensions
- **Data Operations**: Insert, update, delete, select with complex conditions
- **Transaction Support**: ACID transactions with configurable isolation levels
- **Maintenance**: Checkpointing, compaction, storage metrics
- **Configuration**: Runtime settings and parameters

## Storage Features

### Transaction Support
- Configurable isolation levels (READ COMMITTED, REPEATABLE READ, SERIALIZABLE)
- Write buffering for performance
- Rollback and commit operations
- Savepoint support (planned)

### Schema Management
All PostgreSQL schema objects are supported:
- Tables with columns, constraints, and indexes
- Views (regular and materialized)
- Schemas for namespace organization
- Extensions for additional functionality

### Performance Monitoring
Built-in metrics tracking:
- Operation counts (read, write, delete)
- Average latencies
- Error rates
- Storage utilization (memory, disk)
- Cache hit rates

## Integration with Orbit-RS

The storage layer is designed to integrate seamlessly with Orbit-RS's existing infrastructure:

- **Actor Integration**: Tables can be mapped to actor state
- **Event Sourcing**: Transaction logs can feed into event streams
- **Distributed Storage**: Future cluster backend will use Orbit mesh networking
- **Metrics**: Storage metrics integrate with Orbit's monitoring system

## Development Status

- **Storage Abstraction**: Complete trait definition and factory pattern
- **Memory Backend**: Full implementation with all SQL operations
- **SQL Executor Integration**: Updated to use storage abstraction
- **Serialization**: Comprehensive support for all schema objects
- **LSM Backend**: Architecture complete, dependency resolution in progress
- **Transaction Log**: Basic support implemented
- **Cluster Backend**: Design phase
- **Migration Tools**: Planned for production deployments

## Future Enhancements

1. **Query Optimization**: Cost-based query planning and execution
2. **Indexing**: Advanced index types (GiST, GIN, Hash, Vector)
3. **Compression**: Column and row-level compression options
4. **Partitioning**: Horizontal and vertical partitioning support
5. **Replication**: Master-slave and multi-master replication
6. **Backup/Restore**: Point-in-time recovery and streaming backups

## Testing

The storage architecture includes comprehensive test coverage:
- Unit tests for each storage backend
- Integration tests with the SQL executor
- Performance benchmarks
- Compatibility tests with PostgreSQL clients

This architecture provides a solid foundation for scaling Orbit-RS's PostgreSQL compatibility from development prototypes to production-ready distributed databases.