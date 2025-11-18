# MVCC-Enabled SQL Engine for Orbit-RS

## Overview

The Orbit-RS SQL engine now features a comprehensive Multi-Version Concurrency Control (MVCC) system that provides deadlock prevention, high concurrency, and ACID compliance. The engine uses a strategy pattern that defaults to MVCC execution while maintaining backward compatibility.

## 🚀 Key Features

### MVCC Benefits
- **🛡️ Deadlock Prevention**: Row versioning and consistent lock ordering prevent common deadlock scenarios
- **🚀 Non-blocking Reads**: Readers don't block writers and vice versa using snapshot isolation  
- **⚡ High Concurrency**: Multiple transactions can operate simultaneously without contention
- **🔄 Transaction Management**: Full ACID compliance with begin/commit/rollback support
- **🧹 Automatic Cleanup**: Old transaction data and row versions are automatically cleaned up

### Architecture
- **Strategy Pattern**: Clean architecture allowing pluggable execution engines
- **Default MVCC**: All existing SQL code now uses MVCC by default
- **Backward Compatibility**: Traditional execution still available when needed
- **Comprehensive Testing**: Extensive test suite ensuring reliability

## 📋 Architecture Overview

```rust
SqlEngine (Type Alias)
    ↓
ConfigurableSqlEngine (Strategy-based engine)
    ↓
ExecutionStrategy (Trait)
    ├── MvccExecutionStrategy (Default) 
    └── TraditionalExecutionStrategy (Fallback)
```

## 🔧 Usage

### Basic Usage (MVCC by Default)

```rust
use orbit_protocols::postgres_wire::sql::SqlEngine;

// Create default MVCC engine
let mut engine = SqlEngine::new();

// Execute SQL with automatic transaction management
let result = engine.execute("SELECT * FROM users").await?;
```

### Explicit MVCC Configuration

```rust
use orbit_protocols::postgres_wire::sql::{
    ConfigurableSqlEngine, SqlEngineConfig, ExecutionStrategy
};

// Default configuration uses MVCC
let config = SqlEngineConfig::default();
let mut engine = ConfigurableSqlEngine::new(config);
```

### Traditional Execution (if needed)

```rust
use orbit_protocols::postgres_wire::sql::{
    ConfigurableSqlEngine, SqlEngineConfig, ExecutionStrategy
};

let config = SqlEngineConfig {
    strategy: ExecutionStrategy::Traditional,
    ..Default::default()
};
let mut engine = ConfigurableSqlEngine::new(config);
```

### Manual Transaction Management

```rust
// Begin a transaction
let tx_id = engine.begin_transaction(
    Some(IsolationLevel::ReadCommitted)
).await?;

// Execute statements within transaction
engine.execute_in_transaction(
    "INSERT INTO users (name) VALUES ('Alice')", 
    tx_id
).await?;

// Commit or rollback
engine.commit_transaction(tx_id).await?;
// or: engine.rollback_transaction(tx_id).await?;
```

## 📊 Configuration Options

```rust
pub struct SqlEngineConfig {
    /// The execution strategy to use
    pub strategy: ExecutionStrategy,
    /// Enable automatic transaction management
    pub auto_transaction: bool,
    /// Transaction isolation level for MVCC
    pub default_isolation_level: Option<IsolationLevel>,
    /// Enable automatic cleanup of old transactions/versions
    pub auto_cleanup: bool,
    /// Cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
    /// Enable query optimization
    pub enable_optimization: bool,
    /// Enable vector operations support
    pub enable_vectors: bool,
}
```

### Default Configuration

```rust
SqlEngineConfig {
    strategy: ExecutionStrategy::Mvcc, // MVCC by default
    auto_transaction: true,
    default_isolation_level: Some(IsolationLevel::ReadCommitted),
    auto_cleanup: true,
    cleanup_interval_seconds: 300, // 5 minutes
    enable_optimization: true,
    enable_vectors: true,
}
```

## 🔬 MVCC Implementation Details

### Deadlock Prevention Strategies

1. **Row Versioning**: Each row has multiple versions with transaction IDs
2. **Snapshot Isolation**: Transactions see consistent snapshots of data
3. **Consistent Lock Ordering**: Locks acquired in predictable order (alphabetical)
4. **Deadlock Detection**: Wait-for graph cycle detection with transaction abort
5. **Application Retry Logic**: Graceful handling of unavoidable deadlocks

### Transaction Isolation Levels

- **ReadUncommitted**: Dirty reads allowed
- **ReadCommitted**: Default, prevents dirty reads
- **RepeatableRead**: Prevents dirty and non-repeatable reads
- **Serializable**: Full ACID isolation

### Concurrency Control

```rust
// Non-blocking reads
let rows = executor.mvcc_read(&table_name, tx_id, predicate).await?;

// Versioned writes
executor.mvcc_insert(&table_name, tx_id, row_data).await?;
executor.mvcc_update(&table_name, tx_id, updates, predicate).await?;
executor.mvcc_delete(&table_name, tx_id, predicate).await?;
```

## 🧪 Testing

### Core Tests

```bash

# Run all MVCC tests
cargo test -p orbit-protocols --lib mvcc_executor

# Run strategy pattern tests
cargo test -p orbit-protocols --lib execution_strategy

# Run parser tests
cargo test -p orbit-protocols --lib postgres_wire::sql::tests
```

### Demonstrative Tests

The engine includes comprehensive demo tests showing:

- **Deadlock Prevention**: Consistent lock ordering prevents circular waits
- **Snapshot Isolation**: Transactions see consistent data snapshots
- **High Concurrency**: Multiple transactions without blocking
- **Transaction Cleanup**: Automatic cleanup of old versions

## 📈 Performance Characteristics

### MVCC Advantages

- **Read Performance**: Non-blocking reads don't wait for writers
- **Write Concurrency**: Multiple writers can operate simultaneously
- **Scalability**: Better performance under high concurrency
- **Reduced Contention**: Less lock contention compared to traditional approaches

### Memory Management

- **Version Cleanup**: Old row versions automatically cleaned up
- **Transaction Tracking**: Efficient transaction state management
- **Memory Bounds**: Configurable cleanup intervals prevent memory bloat

## 🔄 Migration Guide

### From Traditional to MVCC

The migration is **automatic** - existing code benefits from MVCC by default:

```rust
// Before (traditional)
let engine = SqlEngine::new(); // Old behavior

// After (MVCC-enabled)  
let engine = SqlEngine::new(); // Now uses MVCC automatically!
```

### Fallback to Traditional

If needed, you can explicitly use traditional execution:

```rust
let config = SqlEngineConfig {
    strategy: ExecutionStrategy::Traditional,
    ..Default::default()
};
let engine = ConfigurableSqlEngine::new(config);
```

## 🛠️ Advanced Features

### Vector Operations Support

The MVCC engine fully supports vector operations for AI/ML workloads:

```sql
-- Vector similarity search with MVCC
SELECT content, embedding <-> query_vector AS distance
FROM documents 
WHERE embedding <-> '[0.1, 0.2, 0.3]' < 0.5
ORDER BY distance;
```

### Query Optimization

Built-in query optimizer with:
- Cost-based optimization
- Statistics collection
- Index usage optimization
- Join order optimization

### Extensions

Support for PostgreSQL-compatible extensions:

```sql
CREATE EXTENSION vector;
CREATE EXTENSION postgis;  -- Future support
```

## 📚 API Reference

### Core Types

- `SqlEngine`: Type alias for `ConfigurableSqlEngine` with MVCC defaults
- `ConfigurableSqlEngine`: Strategy-based engine supporting multiple execution modes
- `MvccSqlExecutor`: Core MVCC implementation with deadlock prevention
- `TransactionId`: Unique identifier for transactions

### Result Types

```rust
pub enum UnifiedExecutionResult {
    Select { columns: Vec<String>, rows: Vec<Vec<Option<String>>>, row_count: usize, transaction_id: Option<TransactionId> },
    Insert { count: usize, transaction_id: Option<TransactionId> },
    Update { count: usize, transaction_id: Option<TransactionId> },
    Delete { count: usize, transaction_id: Option<TransactionId> },
    CreateTable { table_name: String, transaction_id: Option<TransactionId> },
    // ... other variants
}
```

## 🔍 Troubleshooting

### Common Issues

1. **Deadlock Detection**: Transactions may be aborted if deadlocks occur
   - **Solution**: Implement application-level retry logic

2. **Memory Usage**: Long-running transactions may accumulate versions
   - **Solution**: Configure appropriate cleanup intervals

3. **Performance**: Very high concurrency may impact performance
   - **Solution**: Tune configuration parameters

### Debugging

Enable detailed logging:

```rust
// Set environment variable
RUST_LOG=orbit_protocols::postgres_wire::sql=debug
```

## 🎯 Roadmap

### Future Enhancements

- [ ] **Advanced Isolation**: Serializable isolation with predicate locking
- [ ] **Distributed Transactions**: Two-phase commit for distributed setups
- [ ] **Parallel Execution**: Parallel query processing
- [ ] **Advanced Indexing**: B+ tree and hash indexes
- [ ] **Query Caching**: Result set caching for repeated queries

### Performance Optimizations

- [ ] **Lock-free Reads**: Even faster read operations
- [ ] **Batch Operations**: Bulk insert/update optimizations
- [ ] **Memory Pools**: Efficient memory management
- [ ] **Statistics**: Advanced query optimization statistics

## 🤝 Contributing

Contributions to the MVCC engine are welcome! Areas of interest:

- Performance optimizations
- Additional isolation levels
- Query optimization improvements  
- Test coverage expansion
- Documentation improvements

## 📄 License

This MVCC SQL engine is part of the Orbit-RS project and follows the same licensing terms.

---

*For more information about Orbit-RS, visit the main project documentation.*