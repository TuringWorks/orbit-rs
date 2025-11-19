# Migration Guide: orbit/protocols → orbit/engine

This guide explains how to migrate existing protocol implementations from `orbit/protocols` to use the new unified `orbit/engine`.

## Table of Contents

- [Overview](#overview)
- [Migration Strategy](#migration-strategy)
- [PostgreSQL Migration](#postgresql-migration)
- [Redis (RESP) Migration](#redis-resp-migration)
- [Breaking Changes](#breaking-changes)
- [Testing Migration](#testing-migration)
- [Rollback Plan](#rollback-plan)

## Overview

### What's Changing?

**Before** (orbit/protocols):
```text
orbit/protocols/src/
├── postgres_wire/
│   ├── sql/
│   │   ├── execution/
│   │   │   ├── columnar.rs        # Columnar format
│   │   │   ├── hybrid.rs          # Storage manager
│   │   │   ├── iceberg_cold.rs    # Cold tier
│   │   │   ├── mvcc_executor.rs   # MVCC transactions
│   │   │   └── vectorized.rs      # Query execution
│   │   └── ...
│   └── storage/
│       └── memory.rs               # In-memory storage
└── ...
```

**After** (orbit/engine):
```text
orbit/engine/src/
├── storage/                # Core storage (hot/warm/cold tiers)
├── transaction/            # MVCC transactions
├── query/                  # Query execution
├── cluster/                # Clustering/Raft
└── adapters/               # Protocol adapters
    ├── postgres.rs         # PostgreSQL adapter
    ├── redis.rs            # Redis adapter
    └── rest.rs             # REST adapter
```

### Why Migrate?

**Benefits**:
- ✅ **Unified storage**: All protocols use the same storage engine
- ✅ **Tiered storage**: Automatic hot/warm/cold tier management
- ✅ **Better transactions**: Shared MVCC implementation
- ✅ **Clustering**: Built-in Raft consensus and replication
- ✅ **Multi-cloud**: S3, Azure Blob, MinIO support
- ✅ **Maintainability**: Single codebase for all protocols

## Migration Strategy

### Phase 1: Add orbit-engine Dependency

Update `orbit/protocols/Cargo.toml`:

```toml
[dependencies]
# Add orbit-engine
orbit-engine = { path = "../engine" }

# Keep existing dependencies for backward compatibility during migration
# ...
```

### Phase 2: Create Adapter Integration

Create a new module `orbit/protocols/src/adapters/mod.rs`:

```rust
//! Integration layer between wire protocols and orbit-engine adapters

use orbit_engine::adapters::{AdapterContext, PostgresAdapter, RedisAdapter};
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

/// Global storage engine instance
static STORAGE: once_cell::sync::Lazy<Arc<HybridStorageManager>> =
    once_cell::sync::Lazy::new(|| {
        Arc::new(HybridStorageManager::new_in_memory())
    });

/// Get the PostgreSQL adapter
pub fn get_postgres_adapter() -> PostgresAdapter {
    let context = AdapterContext::new(
        STORAGE.clone() as Arc<dyn orbit_engine::storage::TableStorage>
    );
    PostgresAdapter::new(context)
}

/// Get the Redis adapter
pub fn get_redis_adapter() -> RedisAdapter {
    let context = AdapterContext::new(
        STORAGE.clone() as Arc<dyn orbit_engine::storage::TableStorage>
    );
    RedisAdapter::new(context)
}
```

### Phase 3: Migrate Protocol-by-Protocol

Migrate each protocol individually to minimize risk.

## PostgreSQL Migration

### Step 1: Update Wire Protocol Handler

**Before** (orbit/protocols/src/postgres_wire/connection.rs):

```rust
use crate::postgres_wire::sql::execution::hybrid::HybridStorageManager;
use crate::postgres_wire::sql::execution::mvcc_executor::MvccExecutor;

pub struct PostgresConnection {
    storage: Arc<HybridStorageManager>,
    executor: MvccExecutor,
    // ...
}

impl PostgresConnection {
    pub async fn handle_query(&mut self, query: &str) -> Result<QueryResult> {
        // Old execution path
        self.executor.execute(query).await
    }
}
```

**After**:

```rust
use orbit_engine::adapters::PostgresAdapter;
use crate::adapters::get_postgres_adapter;

pub struct PostgresConnection {
    adapter: PostgresAdapter,
    // ...
}

impl PostgresConnection {
    pub fn new() -> Self {
        Self {
            adapter: get_postgres_adapter(),
        }
    }

    pub async fn handle_query(&mut self, query: &str) -> Result<QueryResult> {
        // Parse SQL query (reuse existing parser)
        let statement = self.parse_sql(query)?;

        // Dispatch to adapter
        match statement {
            Statement::CreateTable { name, columns, primary_key } => {
                let pg_columns = columns
                    .into_iter()
                    .map(|col| orbit_engine::adapters::postgres::PostgresColumnDef {
                        name: col.name,
                        data_type: self.map_data_type(col.data_type),
                        nullable: col.nullable,
                    })
                    .collect();

                self.adapter
                    .create_table(&name, pg_columns, primary_key)
                    .await?;

                Ok(QueryResult::Created)
            }
            Statement::Insert { table, rows } => {
                let sql_rows = rows
                    .into_iter()
                    .map(|row| self.row_to_sql_values(row))
                    .collect::<Result<Vec<_>>>()?;

                self.adapter.insert(&table, sql_rows).await?;

                Ok(QueryResult::Inserted(sql_rows.len()))
            }
            Statement::Select { table, columns, filter } => {
                let pg_filter = filter.map(|f| self.map_filter(f));

                match self.adapter.select(&table, columns, pg_filter).await? {
                    orbit_engine::adapters::CommandResult::Rows(rows) => {
                        Ok(QueryResult::Rows(rows))
                    }
                    _ => Err(Error::UnexpectedResult),
                }
            }
            // ... other statements
        }
    }

    fn map_data_type(&self, dt: DataType) -> orbit_engine::adapters::postgres::PostgresDataType {
        use orbit_engine::adapters::postgres::PostgresDataType;
        match dt {
            DataType::SmallInt => PostgresDataType::SmallInt,
            DataType::Integer => PostgresDataType::Integer,
            DataType::BigInt => PostgresDataType::BigInt,
            DataType::Text => PostgresDataType::Text,
            // ... more mappings
        }
    }
}
```

### Step 2: Update Type Mappings

Create `orbit/protocols/src/postgres_wire/type_mapping.rs`:

```rust
use orbit_engine::storage::SqlValue;
use crate::postgres_wire::types::PgValue;

/// Convert PostgreSQL wire protocol value to engine SqlValue
pub fn pg_value_to_sql_value(pg_value: PgValue) -> Result<SqlValue> {
    match pg_value {
        PgValue::Null => Ok(SqlValue::Null),
        PgValue::Bool(b) => Ok(SqlValue::Boolean(b)),
        PgValue::SmallInt(i) => Ok(SqlValue::Int16(i)),
        PgValue::Int(i) => Ok(SqlValue::Int32(i)),
        PgValue::BigInt(i) => Ok(SqlValue::Int64(i)),
        PgValue::Float4(f) => Ok(SqlValue::Float32(f)),
        PgValue::Float8(f) => Ok(SqlValue::Float64(f)),
        PgValue::Text(s) | PgValue::Varchar(s) => Ok(SqlValue::String(s)),
        PgValue::Bytea(b) => Ok(SqlValue::Binary(b)),
        PgValue::Timestamp(t) => Ok(SqlValue::Timestamp(t)),
        _ => Err(Error::UnsupportedType(format!("{:?}", pg_value))),
    }
}

/// Convert engine SqlValue to PostgreSQL wire protocol value
pub fn sql_value_to_pg_value(sql_value: SqlValue) -> Result<PgValue> {
    match sql_value {
        SqlValue::Null => Ok(PgValue::Null),
        SqlValue::Boolean(b) => Ok(PgValue::Bool(b)),
        SqlValue::Int16(i) => Ok(PgValue::SmallInt(i)),
        SqlValue::Int32(i) => Ok(PgValue::Int(i)),
        SqlValue::Int64(i) => Ok(PgValue::BigInt(i)),
        SqlValue::Float32(f) => Ok(PgValue::Float4(f)),
        SqlValue::Float64(f) => Ok(PgValue::Float8(f)),
        SqlValue::String(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
            Ok(PgValue::Text(s))
        }
        SqlValue::Binary(b) => Ok(PgValue::Bytea(b)),
        SqlValue::Timestamp(t) => Ok(PgValue::Timestamp(t)),
        _ => Err(Error::UnsupportedType(format!("{:?}", sql_value))),
    }
}
```

### Step 3: Update Transaction Handling

**Before**:

```rust
impl PostgresConnection {
    pub async fn handle_begin(&mut self) -> Result<()> {
        self.executor.begin_transaction().await?;
        Ok(())
    }

    pub async fn handle_commit(&mut self) -> Result<()> {
        self.executor.commit_transaction().await?;
        Ok(())
    }
}
```

**After**:

```rust
use orbit_engine::adapters::postgres::PostgresIsolationLevel;

impl PostgresConnection {
    pub async fn handle_begin(&mut self, isolation: Option<String>) -> Result<()> {
        let level = match isolation.as_deref() {
            Some("READ UNCOMMITTED") => PostgresIsolationLevel::ReadUncommitted,
            Some("READ COMMITTED") => PostgresIsolationLevel::ReadCommitted,
            Some("REPEATABLE READ") => PostgresIsolationLevel::RepeatableRead,
            Some("SERIALIZABLE") => PostgresIsolationLevel::Serializable,
            None => PostgresIsolationLevel::ReadCommitted,
            _ => return Err(Error::InvalidIsolationLevel),
        };

        self.current_tx_id = Some(
            self.adapter.begin_transaction(level).await?
        );

        Ok(())
    }

    pub async fn handle_commit(&mut self) -> Result<()> {
        if let Some(tx_id) = self.current_tx_id.take() {
            self.adapter.commit_transaction(&tx_id).await?;
        }
        Ok(())
    }

    pub async fn handle_rollback(&mut self) -> Result<()> {
        if let Some(tx_id) = self.current_tx_id.take() {
            self.adapter.rollback_transaction(&tx_id).await?;
        }
        Ok(())
    }
}
```

### Step 4: Remove Old Storage Code

After migration is complete and tested:

```bash
# Remove old storage implementations
rm -rf orbit/protocols/src/postgres_wire/sql/execution/hybrid.rs
rm -rf orbit/protocols/src/postgres_wire/sql/execution/mvcc_executor.rs
rm -rf orbit/protocols/src/postgres_wire/sql/execution/columnar.rs
rm -rf orbit/protocols/src/postgres_wire/sql/execution/iceberg_cold.rs
rm -rf orbit/protocols/src/postgres_wire/storage/memory.rs

# Update mod.rs to remove module declarations
```

## Redis (RESP) Migration

### Step 1: Update RESP Handler

**Before** (orbit/protocols/src/resp/handler.rs):

```rust
use std::collections::HashMap;

pub struct RespHandler {
    data: HashMap<String, String>,  // Simple in-memory storage
}

impl RespHandler {
    pub async fn handle_set(&mut self, key: String, value: String) -> Result<RespValue> {
        self.data.insert(key, value);
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    pub async fn handle_get(&self, key: &str) -> Result<RespValue> {
        match self.data.get(key) {
            Some(value) => Ok(RespValue::BulkString(value.clone())),
            None => Ok(RespValue::Null),
        }
    }
}
```

**After**:

```rust
use orbit_engine::adapters::RedisAdapter;
use crate::adapters::get_redis_adapter;

pub struct RespHandler {
    adapter: RedisAdapter,
}

impl RespHandler {
    pub async fn new() -> Result<Self> {
        let mut adapter = get_redis_adapter();
        adapter.initialize().await?;
        Ok(Self { adapter })
    }

    pub async fn handle_set(
        &mut self,
        key: String,
        value: String,
        ttl: Option<Duration>,
    ) -> Result<RespValue> {
        self.adapter.set(&key, &value, ttl).await?;
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    pub async fn handle_get(&self, key: &str) -> Result<RespValue> {
        match self.adapter.get(key).await? {
            Some(value) => Ok(RespValue::BulkString(value)),
            None => Ok(RespValue::Null),
        }
    }

    pub async fn handle_hset(
        &mut self,
        key: &str,
        field: &str,
        value: &str,
    ) -> Result<RespValue> {
        self.adapter.hset(key, field, value).await?;
        Ok(RespValue::Integer(1))
    }

    pub async fn handle_hgetall(&self, key: &str) -> Result<RespValue> {
        let fields = self.adapter.hgetall(key).await?;
        let values: Vec<RespValue> = fields
            .into_iter()
            .flat_map(|(k, v)| vec![
                RespValue::BulkString(k),
                RespValue::BulkString(v),
            ])
            .collect();
        Ok(RespValue::Array(values))
    }

    pub async fn handle_lpush(&mut self, key: &str, value: &str) -> Result<RespValue> {
        let len = self.adapter.lpush(key, value).await?;
        Ok(RespValue::Integer(len as i64))
    }

    // ... more RESP commands
}
```

### Step 2: Update Connection Handler

**Before**:

```rust
pub async fn handle_connection(stream: TcpStream) -> Result<()> {
    let mut handler = RespHandler::new();

    loop {
        let command = read_resp_command(&mut stream).await?;
        let response = match command {
            RespCommand::Set { key, value } => handler.handle_set(key, value).await?,
            RespCommand::Get { key } => handler.handle_get(&key).await?,
            // ...
        };
        write_resp_value(&mut stream, response).await?;
    }
}
```

**After**:

```rust
pub async fn handle_connection(stream: TcpStream) -> Result<()> {
    let mut handler = RespHandler::new().await?;

    loop {
        let command = read_resp_command(&mut stream).await?;
        let response = match command {
            RespCommand::Set { key, value, ex } => {
                let ttl = ex.map(|seconds| Duration::from_secs(seconds));
                handler.handle_set(key, value, ttl).await?
            }
            RespCommand::Get { key } => handler.handle_get(&key).await?,
            RespCommand::Hset { key, field, value } => {
                handler.handle_hset(&key, &field, &value).await?
            }
            RespCommand::Hgetall { key } => handler.handle_hgetall(&key).await?,
            RespCommand::Lpush { key, value } => {
                handler.handle_lpush(&key, &value).await?
            }
            // ... more commands
        };
        write_resp_value(&mut stream, response).await?;
    }
}
```

## Breaking Changes

### 1. Storage API Changes

**Old API** (orbit/protocols):
```rust
// Before
let storage = MemoryTableStorage::new();
storage.insert("users", row).await?;
```

**New API** (orbit/engine):
```rust
// After
let storage = HybridStorageManager::new_in_memory();
storage.insert_row("users", row).await?;
```

**Migration**: Update all `insert()` calls to `insert_row()`.

### 2. Error Types

**Old API**:
```rust
use crate::postgres_wire::error::ProtocolError;
```

**New API**:
```rust
use orbit_engine::error::EngineError;
```

**Migration**: Create error mapping functions:

```rust
pub fn engine_error_to_protocol_error(error: EngineError) -> ProtocolError {
    match error {
        EngineError::NotFound(msg) => ProtocolError::TableNotFound(msg),
        EngineError::AlreadyExists(msg) => ProtocolError::TableAlreadyExists(msg),
        EngineError::Transaction(msg) => ProtocolError::TransactionFailed(msg),
        _ => ProtocolError::InternalError(error.to_string()),
    }
}
```

### 3. Transaction IDs

**Old API**:
```rust
// Before: numeric transaction IDs
let tx_id: u64 = executor.begin_transaction().await?;
```

**New API**:
```rust
// After: UUID-based transaction IDs
let tx_id: String = adapter.begin_transaction(isolation_level).await?;
```

**Migration**: Change all transaction ID fields from `u64` to `String`.

### 4. Filter/Predicate API

**Old API**:
```rust
// Before
pub enum Filter {
    Eq(String, Value),
    Gt(String, Value),
    And(Box<Filter>, Box<Filter>),
}
```

**New API**:
```rust
// After
use orbit_engine::storage::FilterPredicate;
use orbit_engine::adapters::postgres::PostgresFilter;

// For PostgreSQL adapter
let filter = PostgresFilter::Equals("id".to_string(), SqlValue::Int32(123));
```

**Migration**: Update all filter construction code.

## Testing Migration

### Step 1: Add Integration Tests

Create `orbit/protocols/tests/engine_integration.rs`:

```rust
use orbit_engine::adapters::{AdapterContext, PostgresAdapter};
use orbit_engine::storage::HybridStorageManager;
use std::sync::Arc;

#[tokio::test]
async fn test_postgres_adapter_integration() {
    let storage = Arc::new(HybridStorageManager::new_in_memory());
    let context = AdapterContext::new(
        storage as Arc<dyn orbit_engine::storage::TableStorage>
    );

    let mut adapter = PostgresAdapter::new(context);

    // Test table creation
    adapter
        .create_table(
            "test_table",
            vec![
                PostgresColumnDef {
                    name: "id".to_string(),
                    data_type: PostgresDataType::Integer,
                    nullable: false,
                },
            ],
            vec!["id".to_string()],
        )
        .await
        .unwrap();

    // Test insert
    let mut row = HashMap::new();
    row.insert("id".to_string(), SqlValue::Int32(1));

    adapter.insert("test_table", vec![row]).await.unwrap();

    // Test select
    let result = adapter.select("test_table", None, None).await.unwrap();
    match result {
        CommandResult::Rows(rows) => assert_eq!(rows.len(), 1),
        _ => panic!("Expected Rows"),
    }
}
```

### Step 2: Run Existing Tests

```bash
# Run all protocol tests with new engine
cargo test --package orbit-protocols

# Run specific protocol tests
cargo test --package orbit-protocols postgres_wire
cargo test --package orbit-protocols resp
```

### Step 3: Performance Benchmarks

```bash
# Compare old vs new implementation
cargo bench --bench postgres_benchmark -- old
cargo bench --bench postgres_benchmark -- new

# Expected: New implementation should be similar or faster
```

## Rollback Plan

If migration encounters critical issues:

### Option 1: Feature Flag

Add feature flag to switch between old and new implementations:

```toml
[features]
default = ["use-engine"]
use-engine = ["orbit-engine"]
use-legacy = []
```

```rust
#[cfg(feature = "use-engine")]
use orbit_engine::adapters::PostgresAdapter;

#[cfg(feature = "use-legacy")]
use crate::postgres_wire::legacy::PostgresExecutor;
```

### Option 2: Git Revert

```bash
# If migration is in a single commit
git revert <migration-commit-hash>

# If migration spans multiple commits
git revert <first-commit>..<last-commit>
```

### Option 3: Keep Both Implementations

Keep old implementation alongside new one temporarily:

```text
orbit/protocols/src/
├── postgres_wire/
│   ├── adapter.rs       # New implementation using orbit-engine
│   └── legacy/          # Old implementation (deprecated)
│       ├── executor.rs
│       └── storage.rs
```

## Migration Checklist

### PostgreSQL Protocol
- [ ] Update Cargo.toml dependencies
- [ ] Create adapter integration module
- [ ] Update wire protocol handler to use PostgresAdapter
- [ ] Create type mapping functions
- [ ] Update transaction handling
- [ ] Update error handling
- [ ] Run integration tests
- [ ] Run benchmarks
- [ ] Remove old storage code (after verification)

### Redis Protocol
- [ ] Update Cargo.toml dependencies
- [ ] Create adapter integration module
- [ ] Update RESP handler to use RedisAdapter
- [ ] Update connection handler
- [ ] Add TTL support for SET commands
- [ ] Add hash operations (HSET, HGETALL)
- [ ] Add list operations (LPUSH, RPUSH, LRANGE)
- [ ] Run integration tests
- [ ] Remove old in-memory HashMap storage

### OrbitQL Protocol
- [ ] Create OrbitQLAdapter
- [ ] Update query parser
- [ ] Update execution engine
- [ ] Run integration tests

### Documentation
- [ ] Update protocol README files
- [ ] Update API documentation
- [ ] Add migration examples
- [ ] Update architecture diagrams

## Post-Migration

### Verify Functionality

```bash
# Test PostgreSQL wire protocol
psql -h localhost -p 5432 -U postgres
> CREATE TABLE test (id INT PRIMARY KEY, name TEXT);
> INSERT INTO test VALUES (1, 'Alice');
> SELECT * FROM test;

# Test Redis protocol
redis-cli -h localhost -p 6379
> SET mykey "myvalue"
> GET mykey
> HSET user:1 name "Alice"
> HGETALL user:1
```

### Monitor Metrics

```bash
# Check storage metrics
curl http://localhost:9090/metrics | grep orbit_storage

# Check transaction metrics
curl http://localhost:9090/metrics | grep orbit_transaction
```

### Update Dependencies

Remove old dependencies from `orbit/protocols/Cargo.toml`:

```toml
[dependencies]
# Remove these after migration:
# rocksdb = "0.22"  # Now in orbit-engine
# iceberg = "0.7"   # Now in orbit-engine
# arrow = "55.2"    # Now in orbit-engine
```

---

**Questions?** Open an issue at https://github.com/orbit-rs/orbit/issues
